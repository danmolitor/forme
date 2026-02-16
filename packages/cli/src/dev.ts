import { createServer, type IncomingMessage, type ServerResponse } from 'node:http';
import { readFile, writeFile, unlink } from 'node:fs/promises';
import { resolve, basename, join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { watch } from 'chokidar';
import { WebSocketServer, type WebSocket } from 'ws';
import open from 'open';
import { bundleFile, BUNDLE_DIR } from './bundle.js';
import { renderDocumentWithLayout, type LayoutInfo } from '@forme/core';
import type { ReactElement } from 'react';

export interface DevOptions {
  port: number;
}

export function startDevServer(inputPath: string, options: DevOptions): void {
  const absoluteInput = resolve(inputPath);
  const fileName = basename(absoluteInput);
  const port = options.port;

  let currentPdf: Uint8Array | null = null;
  let currentLayout: LayoutInfo | null = null;
  let lastRenderTime = 0;
  let lastError: string | null = null;
  let firstRender = true;

  // ── HTTP Server ──────────────────────────────────────────────

  const server = createServer(async (req: IncomingMessage, res: ServerResponse) => {
    const url = req.url ?? '/';

    if (url === '/' || url === '/index.html') {
      const html = await getPreviewHtml();
      res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' });
      res.end(html);
      return;
    }

    if (url === '/pdf') {
      if (currentPdf) {
        res.writeHead(200, {
          'Content-Type': 'application/pdf',
          'Content-Length': String(currentPdf.length),
        });
        res.end(currentPdf);
      } else {
        res.writeHead(503, { 'Content-Type': 'text/plain' });
        res.end(lastError ?? 'PDF not yet rendered');
      }
      return;
    }

    if (url === '/layout') {
      if (currentLayout) {
        const json = JSON.stringify(currentLayout);
        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(json);
      } else {
        res.writeHead(503, { 'Content-Type': 'text/plain' });
        res.end('Layout not yet available');
      }
      return;
    }

    res.writeHead(404, { 'Content-Type': 'text/plain' });
    res.end('Not found');
  });

  // ── WebSocket ────────────────────────────────────────────────

  const wss = new WebSocketServer({ server });
  const clients = new Set<WebSocket>();

  wss.on('connection', (ws: WebSocket) => {
    clients.add(ws);
    ws.on('close', () => clients.delete(ws));
  });

  function broadcast(message: object): void {
    const data = JSON.stringify(message);
    for (const ws of clients) {
      if (ws.readyState === ws.OPEN) {
        ws.send(data);
      }
    }
  }

  // ── Build + Render ───────────────────────────────────────────

  let buildCounter = 0;

  async function rebuild(): Promise<void> {
    const buildId = ++buildCounter;
    const start = performance.now();

    try {
      const code = await bundleFile(absoluteInput);

      // Skip if a newer build started
      if (buildId !== buildCounter) return;

      // Write temp file inside CLI package dir so Node resolves @forme/* deps
      const tmpFile = join(BUNDLE_DIR, `.forme-dev-${Date.now()}.mjs`);
      await writeFile(tmpFile, code);

      let mod: Record<string, unknown>;
      try {
        mod = await import(pathToFileURL(tmpFile).href);
      } finally {
        await unlink(tmpFile).catch(() => {});
      }
      let element: ReactElement = mod.default as ReactElement;

      if (typeof element === 'function') {
        element = await (element as () => ReactElement | Promise<ReactElement>)();
      }

      const { pdf, layout } = await renderDocumentWithLayout(element);

      // Skip if a newer build started
      if (buildId !== buildCounter) return;

      currentPdf = pdf;
      currentLayout = layout;
      lastError = null;
      lastRenderTime = Math.round(performance.now() - start);

      console.log(`Rendered ${pdf.length} bytes in ${lastRenderTime}ms`);
      broadcast({ type: 'reload', renderTime: lastRenderTime });

      if (firstRender) {
        firstRender = false;
        const url = `http://localhost:${port}`;
        console.log(`Opening ${url}`);
        open(url);
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      lastError = message;
      console.error(`Build error: ${message}`);
      broadcast({ type: 'error', message });
    }
  }

  // ── File Watcher ─────────────────────────────────────────────

  let debounceTimer: ReturnType<typeof setTimeout> | null = null;

  const watcher = watch(absoluteInput, { ignoreInitial: true });
  watcher.on('change', () => {
    if (debounceTimer) clearTimeout(debounceTimer);
    debounceTimer = setTimeout(rebuild, 100);
  });

  // ── Start ────────────────────────────────────────────────────

  server.listen(port, () => {
    console.log(`Forme dev server watching ${fileName}`);
    console.log(`  http://localhost:${port}`);
    console.log('');
    rebuild();
  });
}

async function getPreviewHtml(): Promise<string> {
  // Try to load the preview HTML from the package's own file tree
  const possiblePaths = [
    // When running from dist/ (built)
    new URL('./preview/index.html', import.meta.url),
    // When running from src/ (dev with ts-node)
    new URL('../src/preview/index.html', import.meta.url),
  ];

  for (const p of possiblePaths) {
    try {
      return await readFile(p, 'utf-8');
    } catch {
      continue;
    }
  }

  // Inline fallback
  return `<!DOCTYPE html><html><body><h1>Preview HTML not found</h1></body></html>`;
}
