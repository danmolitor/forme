import { createServer, type IncomingMessage, type ServerResponse } from 'node:http';
import { readFile, writeFile } from 'node:fs/promises';
import { resolve, basename } from 'node:path';
import { watch } from 'chokidar';
import { WebSocketServer, type WebSocket } from 'ws';
import open from 'open';
import { renderFromFile, type RenderResult } from '@formepdf/renderer';

export interface DevOptions {
  port: number;
  dataPath?: string;
}

export function startDevServer(inputPath: string, options: DevOptions): void {
  const absoluteInput = resolve(inputPath);
  const fileName = basename(absoluteInput);
  const port = options.port;
  const dataPath = options.dataPath ? resolve(options.dataPath) : undefined;

  let currentPdf: Uint8Array | null = null;
  let currentLayout: RenderResult['layout'] | null = null;
  let lastRenderTime = 0;
  let lastError: string | null = null;
  let firstRender = true;

  // Override state
  let pageSizeOverride: { width: number; height: number } | null = null;
  let inMemoryData: unknown = null;
  let useInMemoryData = false;

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

  wss.on('connection', async (ws: WebSocket) => {
    clients.add(ws);
    ws.on('close', () => clients.delete(ws));

    // Send init message with data and page size state
    let dataContent: string | null = null;
    if (dataPath) {
      try {
        dataContent = await readFile(dataPath, 'utf-8');
      } catch { /* ignore */ }
    }
    const initMsg: Record<string, unknown> = {
      type: 'init',
      hasData: !!dataPath,
      dataContent,
      pageSizeOverride,
    };
    if (ws.readyState === ws.OPEN) {
      ws.send(JSON.stringify(initMsg));
    }

    ws.on('message', (raw: Buffer) => {
      let msg: Record<string, unknown>;
      try {
        msg = JSON.parse(raw.toString());
      } catch {
        return;
      }

      if (msg.type === 'setPageSize') {
        pageSizeOverride = { width: msg.width as number, height: msg.height as number };
        triggerRebuild();
      }

      if (msg.type === 'clearPageSize') {
        pageSizeOverride = null;
        triggerRebuild();
      }

      if (msg.type === 'updateData') {
        inMemoryData = msg.data;
        useInMemoryData = true;
        triggerRebuild();
      }

      if (msg.type === 'saveData' && dataPath) {
        const content = msg.content as string;
        writeFile(dataPath, content, 'utf-8').catch((err) => {
          console.error(`Failed to save data file: ${err}`);
        });
      }
    });
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

    try {
      const result = await renderFromFile(absoluteInput, {
        dataPath,
        data: useInMemoryData ? inMemoryData : undefined,
        pageSize: pageSizeOverride ?? undefined,
      });

      // Skip if a newer build started
      if (buildId !== buildCounter) return;

      currentPdf = result.pdf;
      currentLayout = result.layout;
      lastError = null;
      lastRenderTime = result.renderTimeMs;

      const pageCount = result.layout?.pages?.length ?? 0;

      if (firstRender) {
        firstRender = false;
        const url = `http://localhost:${port}`;
        console.log(`Forme dev server\n`);
        console.log(`  Watching:  ${fileName}${dataPath ? ` + ${basename(dataPath)}` : ''}`);
        console.log(`  Rendered:  ${pageCount} page${pageCount !== 1 ? 's' : ''} in ${lastRenderTime}ms`);
        console.log(`  Preview:   ${url}\n`);
        open(url);
      } else {
        console.log(`Rebuilt in ${lastRenderTime}ms (${pageCount} page${pageCount !== 1 ? 's' : ''})`);
      }

      broadcast({ type: 'reload', renderTime: lastRenderTime });
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      lastError = message;
      console.error(`Build error: ${message}`);
      broadcast({ type: 'error', message });
    }
  }

  // ── File Watcher ─────────────────────────────────────────────

  let debounceTimer: ReturnType<typeof setTimeout> | null = null;

  function triggerRebuild(): void {
    if (debounceTimer) clearTimeout(debounceTimer);
    debounceTimer = setTimeout(rebuild, 100);
  }

  const watchPaths = [absoluteInput, ...(dataPath ? [dataPath] : [])];
  const watcher = watch(watchPaths, { ignoreInitial: true });
  watcher.on('change', (changedPath: string) => {
    // If data file changed on disk, reset in-memory override and push new content
    if (dataPath && resolve(changedPath) === dataPath) {
      useInMemoryData = false;
      inMemoryData = null;
      readFile(dataPath, 'utf-8').then((content) => {
        broadcast({ type: 'dataUpdate', content });
      }).catch(() => {});
    }
    triggerRebuild();
  });

  // ── Graceful shutdown ───────────────────────────────────────

  process.on('SIGINT', () => {
    console.log('\nShutting down...');
    watcher.close();
    wss.close();
    server.close(() => process.exit(0));
    // Force exit after 2s if graceful shutdown stalls
    setTimeout(() => process.exit(0), 2000);
  });

  // ── Start ────────────────────────────────────────────────────

  server.listen(port, () => {
    rebuild();
  });
}

async function getPreviewHtml(): Promise<string> {
  // Try to load preview HTML from @formepdf/renderer
  try {
    const { createRequire } = await import('node:module');
    const require = createRequire(import.meta.url);
    const previewPath = require.resolve('@formepdf/renderer/preview');
    return await readFile(previewPath, 'utf-8');
  } catch {
    // Fallback: try local paths
  }

  const possiblePaths = [
    new URL('./preview/index.html', import.meta.url),
    new URL('../src/preview/index.html', import.meta.url),
  ];

  for (const p of possiblePaths) {
    try {
      return await readFile(p, 'utf-8');
    } catch {
      continue;
    }
  }

  return `<!DOCTYPE html><html><body><h1>Preview HTML not found</h1></body></html>`;
}
