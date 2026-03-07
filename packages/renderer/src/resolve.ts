import { readFile } from 'node:fs/promises';
import { resolve, dirname, extname } from 'node:path';

export function uint8ArrayToBase64(bytes: Uint8Array): string {
  return Buffer.from(bytes).toString('base64');
}

/// Resolve font sources to base64 strings for the WASM engine.
/// File paths are resolved relative to `basePath` (defaults to cwd).
/// Uint8Array values are base64-encoded. Data URIs pass through as-is.
export async function resolveFontSources(
  doc: Record<string, unknown>,
  basePath?: string,
): Promise<void> {
  const fonts = doc.fonts as Array<{ src: string | Uint8Array }> | undefined;
  if (!fonts?.length) return;

  const baseDir = basePath ?? process.cwd();
  for (const font of fonts) {
    if (font.src instanceof Uint8Array) {
      font.src = uint8ArrayToBase64(font.src);
    } else if (typeof font.src === 'string' && !font.src.startsWith('data:')) {
      const fontPath = resolve(baseDir, font.src);
      const bytes = await readFile(fontPath);
      font.src = uint8ArrayToBase64(new Uint8Array(bytes));
    }
  }
}

const MIME_BY_EXT: Record<string, string> = {
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.jpeg': 'image/jpeg',
  '.gif': 'image/gif',
  '.webp': 'image/webp',
  '.svg': 'image/svg+xml',
  '.bmp': 'image/bmp',
  '.ico': 'image/x-icon',
  '.avif': 'image/avif',
};

/// Resolve image sources — converts HTTP/HTTPS URLs and local file paths to base64 data URIs.
/// Walks the document tree recursively. File paths are resolved relative to `basePath`.
export async function resolveImageSources(
  doc: Record<string, unknown>,
  basePath?: string,
): Promise<void> {
  const children = doc.children as Array<Record<string, unknown>> | undefined;
  if (!children?.length) return;
  await Promise.all(children.map((n) => resolveImageSourcesInNode(n, basePath)));
}

async function resolveImageSourcesInNode(
  node: Record<string, unknown>,
  basePath?: string,
): Promise<void> {
  const kind = node.kind as Record<string, unknown> | undefined;
  if (kind?.type === 'Image' && typeof kind.src === 'string') {
    const src = kind.src as string;
    if (src.startsWith('data:')) {
      // Already a data URI — pass through
    } else if (src.startsWith('http://') || src.startsWith('https://')) {
      const res = await fetch(src);
      if (!res.ok) throw new Error(`Failed to fetch image: ${src} (${res.status})`);
      const contentType = res.headers.get('content-type') || 'image/png';
      const buf = new Uint8Array(await res.arrayBuffer());
      kind.src = `data:${contentType};base64,${uint8ArrayToBase64(buf)}`;
    } else if (basePath) {
      const filePath = resolve(basePath, src);
      const ext = extname(filePath).toLowerCase();
      const mime = MIME_BY_EXT[ext] || 'application/octet-stream';
      const bytes = await readFile(filePath);
      kind.src = `data:${mime};base64,${uint8ArrayToBase64(new Uint8Array(bytes))}`;
    }
  }
  const children = node.children as Array<Record<string, unknown>> | undefined;
  if (children?.length) {
    await Promise.all(children.map((n) => resolveImageSourcesInNode(n, basePath)));
  }
}

/// Resolve all asset sources (fonts + images) in parallel.
export async function resolveAllSources(
  doc: Record<string, unknown>,
  basePath?: string,
): Promise<void> {
  await Promise.all([
    resolveFontSources(doc, basePath),
    resolveImageSources(doc, basePath),
  ]);
}
