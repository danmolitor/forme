import { readFile } from 'node:fs/promises';
import { resolve, dirname } from 'node:path';

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

/// Resolve image sources — converts HTTP/HTTPS URLs to base64 data URIs.
/// Walks the document tree recursively.
export async function resolveImageSources(
  doc: Record<string, unknown>,
): Promise<void> {
  const children = doc.children as Array<Record<string, unknown>> | undefined;
  if (!children?.length) return;
  await Promise.all(children.map(resolveImageSourcesInNode));
}

async function resolveImageSourcesInNode(node: Record<string, unknown>): Promise<void> {
  const kind = node.kind as Record<string, unknown> | undefined;
  if (kind?.type === 'Image' && typeof kind.src === 'string') {
    const src = kind.src as string;
    if (src.startsWith('http://') || src.startsWith('https://')) {
      const res = await fetch(src);
      if (!res.ok) throw new Error(`Failed to fetch image: ${src} (${res.status})`);
      const contentType = res.headers.get('content-type') || 'image/png';
      const buf = new Uint8Array(await res.arrayBuffer());
      kind.src = `data:${contentType};base64,${uint8ArrayToBase64(buf)}`;
    }
  }
  const children = node.children as Array<Record<string, unknown>> | undefined;
  if (children?.length) {
    await Promise.all(children.map(resolveImageSourcesInNode));
  }
}

/// Resolve all asset sources (fonts + images) in parallel.
export async function resolveAllSources(
  doc: Record<string, unknown>,
  basePath?: string,
): Promise<void> {
  await Promise.all([
    resolveFontSources(doc, basePath),
    resolveImageSources(doc),
  ]);
}
