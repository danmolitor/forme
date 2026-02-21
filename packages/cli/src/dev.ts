import { createServer, type IncomingMessage, type ServerResponse } from 'node:http';
import { readFile, writeFile, unlink } from 'node:fs/promises';
import { resolve, basename, dirname, join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { watch } from 'chokidar';
import { WebSocketServer, type WebSocket } from 'ws';
import open from 'open';
import { isValidElement, type ReactElement } from 'react';
import { bundleFile, BUNDLE_DIR } from './bundle.js';
import { renderPdfWithLayout, type LayoutInfo } from '@formepdf/core';

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
  let currentLayout: LayoutInfo | null = null;
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
    const start = performance.now();

    try {
      const code = await bundleFile(absoluteInput);

      // Skip if a newer build started
      if (buildId !== buildCounter) return;

      // Write temp file inside CLI package dir so Node resolves @formepdf/* deps
      const tmpFile = join(BUNDLE_DIR, `.forme-dev-${Date.now()}.mjs`);
      await writeFile(tmpFile, code);

      let mod: Record<string, unknown>;
      try {
        mod = await import(pathToFileURL(tmpFile).href);
      } finally {
        await unlink(tmpFile).catch(() => {});
      }

      const overrideData = useInMemoryData ? inMemoryData : undefined;
      const element = await resolveElement(mod, dataPath, overrideData);

      // Serialize JSX to document JSON, apply overrides, then render
      const { serialize } = await import('@formepdf/react');
      const doc = serialize(element) as unknown as Record<string, unknown>;

      // Apply page size override
      if (pageSizeOverride) {
        const { width, height } = pageSizeOverride;
        const customSize = { Custom: { width, height } };

        // Override defaultPage size
        if (doc.defaultPage && typeof doc.defaultPage === 'object') {
          (doc.defaultPage as Record<string, unknown>).size = customSize;
        }

        // Override each Page node's config size
        if (Array.isArray(doc.children)) {
          for (const child of doc.children) {
            if (child && typeof child === 'object' && child.kind && typeof child.kind === 'object') {
              const kind = child.kind as Record<string, unknown>;
              if (kind.type === 'Page' && kind.config && typeof kind.config === 'object') {
                (kind.config as Record<string, unknown>).size = customSize;
              }
            }
          }
        }
      }

      // Resolve font file paths relative to the template directory
      await resolveFontPaths(doc, absoluteInput);

      const { pdf, layout } = await renderPdfWithLayout(JSON.stringify(doc));

      // Skip if a newer build started
      if (buildId !== buildCounter) return;

      currentPdf = pdf;
      currentLayout = layout;
      lastError = null;
      lastRenderTime = Math.round(performance.now() - start);

      const pageCount = layout?.pages?.length ?? 0;

      // Warn once if no source locations found (click-to-inspect won't work)
      if (firstRender && !hasAnySourceLocation(doc)) {
        console.warn(
          `\n  Warning: No source locations found — click-to-inspect is disabled.\n` +
          `  Ensure your tsconfig.json has "jsx": "react-jsx" (not "react").\n`
        );
      }

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

async function resolveElement(
  mod: Record<string, unknown>,
  dataPath?: string,
  overrideData?: unknown,
): Promise<ReactElement> {
  const exported = mod.default;

  if (exported === undefined) {
    throw new Error(
      `No default export found.\n\n` +
      `  Your file must export a Forme element or a function that returns one:\n\n` +
      `    export default (\n` +
      `      <Document>\n` +
      `        <Text>Hello</Text>\n` +
      `      </Document>\n` +
      `    );`
    );
  }

  if (typeof exported === 'function') {
    let data: unknown = {};
    if (overrideData !== undefined) {
      data = overrideData;
    } else if (dataPath) {
      const raw = await readFile(dataPath, 'utf-8');
      try {
        data = JSON.parse(raw);
      } catch {
        throw new Error(
          `Failed to parse data file as JSON: ${dataPath}\n` +
          `  Make sure the file contains valid JSON.`
        );
      }
    }
    const result = await (exported as (data: unknown) => ReactElement | Promise<ReactElement>)(data);
    if (!isValidElement(result)) {
      throw new Error(
        `Default export function did not return a valid Forme element.\n` +
        `  Got: ${typeof result}`
      );
    }
    return result;
  }

  if (isValidElement(exported)) {
    return exported;
  }

  throw new Error(
    `Default export is not a valid Forme element.\n` +
    `  Got: ${typeof exported}\n` +
    `  Expected: a <Document> element or a function that returns one.`
  );
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

function hasAnySourceLocation(doc: Record<string, unknown>): boolean {
  const children = doc.children as Array<Record<string, unknown>> | undefined;
  if (!children) return false;
  function check(node: Record<string, unknown>): boolean {
    if (node.sourceLocation) return true;
    const kids = node.children as Array<Record<string, unknown>> | undefined;
    return kids?.some(check) ?? false;
  }
  return children.some(check);
}

function uint8ArrayToBase64(bytes: Uint8Array): string {
  return Buffer.from(bytes).toString('base64');
}

async function resolveFontPaths(doc: Record<string, unknown>, templatePath: string): Promise<void> {
  const fonts = doc.fonts as Array<{ src: string | Uint8Array }> | undefined;
  if (!fonts?.length) return;

  const templateDir = dirname(templatePath);
  for (const font of fonts) {
    if (font.src instanceof Uint8Array) {
      font.src = uint8ArrayToBase64(font.src);
    } else if (typeof font.src === 'string' && !font.src.startsWith('data:')) {
      const fontPath = resolve(templateDir, font.src);
      const bytes = await readFile(fontPath);
      font.src = uint8ArrayToBase64(new Uint8Array(bytes));
    }
  }
}
