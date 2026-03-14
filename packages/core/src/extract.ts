import { inflateSync } from 'node:zlib';

/**
 * Extract embedded JSON data from a Forme-generated PDF.
 *
 * Scans the PDF bytes for a `forme-data.json` EmbeddedFile attachment,
 * decompresses the stream, and parses the JSON. Returns `null` if the
 * PDF doesn't contain embedded data (e.g. non-Forme PDF).
 */
export function extractData(pdfBytes: Uint8Array): unknown | null {
  const text = new TextDecoder('latin1').decode(pdfBytes);

  // Find the FileSpec referencing forme-data.json
  const fsMatch = text.match(/\/F\s*\(forme-data\.json\)/);
  if (!fsMatch) return null;

  // Extract the EmbeddedFile stream object number from /EF << /F N 0 R >>
  // Search around the FileSpec location for the /EF reference
  const fsStart = fsMatch.index!;
  const fsRegion = text.slice(Math.max(0, fsStart - 200), fsStart + 200);
  const efMatch = fsRegion.match(/\/EF\s*<<\s*\/F\s+(\d+)\s+0\s+R\s*>>/);
  if (!efMatch) return null;

  const streamObjId = efMatch[1];

  // Find the stream object: "N 0 obj ... stream\n...\nendstream"
  const objPattern = new RegExp(streamObjId + '\\s+0\\s+obj\\b');
  const objMatch = text.match(objPattern);
  if (!objMatch) return null;

  const objStart = objMatch.index!;

  // Find stream start (after "stream\r\n" or "stream\n")
  const streamKeyword = text.indexOf('stream', objStart);
  if (streamKeyword === -1) return null;

  let streamDataStart = streamKeyword + 6; // "stream".length
  // Skip \r\n or \n after "stream"
  if (pdfBytes[streamDataStart] === 0x0d) streamDataStart++; // \r
  if (pdfBytes[streamDataStart] === 0x0a) streamDataStart++; // \n

  // Find endstream
  const endstreamPos = text.indexOf('\nendstream', streamDataStart);
  if (endstreamPos === -1) return null;

  const compressedBytes = pdfBytes.slice(streamDataStart, endstreamPos);

  // Check if FlateDecode is used
  const objRegion = text.slice(objStart, streamKeyword);
  const isCompressed = objRegion.includes('/FlateDecode');

  let jsonBytes: Uint8Array;
  if (isCompressed) {
    jsonBytes = inflateSync(Buffer.from(compressedBytes));
  } else {
    jsonBytes = compressedBytes;
  }

  const jsonString = new TextDecoder('utf-8').decode(jsonBytes);
  return JSON.parse(jsonString);
}
