import { inflateSync } from 'node:zlib';

/**
 * Concatenate every stream object in the PDF, inflating the
 * FlateDecode-compressed ones, so text-showing operators like
 * `(Page 1 of 2) Tj` become searchable.
 */
export function decompressedStreams(pdf: Uint8Array): string {
  const buf = Buffer.from(pdf);
  let out = '';
  let pos = 0;
  for (;;) {
    const start = buf.indexOf('stream', pos);
    if (start === -1) break;
    let dataStart = start + 'stream'.length;
    if (buf[dataStart] === 0x0d) dataStart++;
    if (buf[dataStart] === 0x0a) dataStart++;
    let end = buf.indexOf('endstream', dataStart);
    if (end === -1) break;
    while (end > dataStart && (buf[end - 1] === 0x0a || buf[end - 1] === 0x0d)) end--;
    const raw = buf.subarray(dataStart, end);
    try {
      out += inflateSync(raw).toString('latin1');
    } catch {
      out += raw.toString('latin1');
    }
    pos = end + 'endstream'.length;
  }
  return out;
}
