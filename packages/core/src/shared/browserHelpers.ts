/**
 * Helpers shared between the browser entry (`src/browser.ts`, backed by
 * the wasm-pack `--target bundler` build) and the worker entry
 * (`src/worker.ts`, backed by `--target web`). The two entries differ
 * only in how the WASM module gets instantiated; everything from
 * font/image URL resolution through embedded-JSON extraction is the
 * same and lives here.
 *
 * Strictly browser/edge primitives: `fetch`, `btoa`, `TextDecoder`,
 * `DecompressionStream`. No Node-only APIs.
 */

export function uint8ArrayToBase64(bytes: Uint8Array): string {
  if ('toBase64' in bytes && typeof bytes.toBase64 === 'function') {
    return bytes.toBase64();
  }

  // Keep older browsers and edge runtimes working without a polyfill.
  let binary = '';
  for (let i = 0; i < bytes.length; i++) {
    binary += String.fromCharCode(bytes[i]);
  }
  return btoa(binary);
}

export async function resolveFonts(doc: Record<string, unknown>): Promise<void> {
  const fonts = doc.fonts as
    | Array<{ family: string; src: string | Uint8Array; weight: number; italic: boolean }>
    | undefined;
  if (!fonts?.length) return;

  for (const font of fonts) {
    if (font.src instanceof Uint8Array) {
      font.src = uint8ArrayToBase64(font.src);
    } else if (typeof font.src === 'string' && !font.src.startsWith('data:')) {
      // In the browser, non-data-URI strings are treated as URLs
      const res = await fetch(font.src);
      if (!res.ok) throw new Error(`Failed to fetch font: ${font.src} (${res.status})`);
      const buf = new Uint8Array(await res.arrayBuffer());
      font.src = uint8ArrayToBase64(buf);
    }
    // data URIs pass through as-is (engine extracts base64 portion)
  }
}

export async function resolveImages(doc: Record<string, unknown>): Promise<void> {
  const children = doc.children as Array<Record<string, unknown>> | undefined;
  if (!children?.length) return;
  for (const child of children) {
    await resolveImagesInNode(child);
  }
}

async function resolveImagesInNode(node: Record<string, unknown>): Promise<void> {
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
    for (const child of children) {
      await resolveImagesInNode(child);
    }
  }
}

/**
 * Extract embedded JSON data from a Forme-generated PDF using browser-
 * native DecompressionStream (no node:zlib needed).
 */
export async function extractDataFromPdf(pdfBytes: Uint8Array): Promise<unknown | null> {
  const text = new TextDecoder('latin1').decode(pdfBytes);

  const fsMatch = text.match(/\/F\s*\(forme-data\.json\)/);
  if (!fsMatch) return null;

  const fsStart = fsMatch.index!;
  const fsRegion = text.slice(Math.max(0, fsStart - 200), fsStart + 200);
  const efMatch = fsRegion.match(/\/EF\s*<<\s*\/F\s+(\d+)\s+0\s+R\s*>>/);
  if (!efMatch) return null;

  const streamObjId = efMatch[1];
  const objPattern = new RegExp(streamObjId + '\\s+0\\s+obj\\b');
  const objMatch = text.match(objPattern);
  if (!objMatch) return null;

  const objStart = objMatch.index!;
  const streamKeyword = text.indexOf('stream', objStart);
  if (streamKeyword === -1) return null;

  let streamDataStart = streamKeyword + 6;
  if (pdfBytes[streamDataStart] === 0x0d) streamDataStart++;
  if (pdfBytes[streamDataStart] === 0x0a) streamDataStart++;

  const endstreamPos = text.indexOf('\nendstream', streamDataStart);
  if (endstreamPos === -1) return null;

  const compressedBytes = pdfBytes.slice(streamDataStart, endstreamPos);
  const objRegion = text.slice(objStart, streamKeyword);
  const isCompressed = objRegion.includes('/FlateDecode');

  let jsonBytes: Uint8Array;
  if (isCompressed) {
    const ds = new DecompressionStream('deflate');
    const writer = ds.writable.getWriter();
    writer.write(compressedBytes);
    writer.close();
    const reader = ds.readable.getReader();
    const chunks: Uint8Array[] = [];
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      chunks.push(value);
    }
    const totalLength = chunks.reduce((sum, c) => sum + c.length, 0);
    jsonBytes = new Uint8Array(totalLength);
    let offset = 0;
    for (const chunk of chunks) {
      jsonBytes.set(chunk, offset);
      offset += chunk.length;
    }
  } else {
    jsonBytes = compressedBytes;
  }

  const jsonString = new TextDecoder('utf-8').decode(jsonBytes);
  return JSON.parse(jsonString);
}
