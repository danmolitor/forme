// Shared option wire-encoding for every @formepdf/html entry point.
//
// The engine's font/pageSize wire format is identical across the Node,
// browser, and worker targets, so it lives in one place. `btoa` is the only
// primitive used for base64 (available in Node 18+, browsers, and Workers) —
// no `Buffer`, so this is safe in every runtime.

function uint8ArrayToBase64(bytes) {
  let binary = '';
  for (let i = 0; i < bytes.length; i++) {
    binary += String.fromCharCode(bytes[i]);
  }
  return btoa(binary);
}

/** Encode RenderHtmlOptions to the JSON the WASM boundary expects. */
export function toWireOptions(options) {
  const wireOptions = { ...options };
  if (options.fonts) {
    wireOptions.fonts = options.fonts.map((f) => ({
      ...f,
      data: f.data instanceof Uint8Array ? uint8ArrayToBase64(f.data) : f.data,
    }));
  }
  return wireOptions;
}
