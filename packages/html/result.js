// @formepdf/html — the ONE place a raw WASM result becomes the public
// result object.
//
// All three entries (node, browser, worker) previously built their result
// objects field by field, and when `passes` was added only the node
// `renderHtml` was updated — the declared `RenderHtmlResult` promised a
// field five of the six constructions never returned. Agreement by
// construction: every entry funnels through these two functions, so a new
// field is added exactly here (and in the shape tests that pin
// declared-vs-runtime agreement per target).

/**
 * @param {{ pdf: Uint8Array, warnings: string[], passes: number, free(): void }} raw
 * @returns {import('./index').RenderHtmlResult}
 */
export function toRenderResult(raw) {
  try {
    return { pdf: raw.pdf, warnings: raw.warnings, passes: raw.passes };
  } finally {
    raw.free();
  }
}

/**
 * @param {{ pdf: Uint8Array, layout_json: string, warnings: string[], passes: number, free(): void }} raw
 * @returns {import('./index').RenderHtmlLayoutResult}
 */
export function toLayoutResult(raw) {
  try {
    return {
      pdf: raw.pdf,
      // The crate's wasm feature returns LayoutInfo as a JSON string on
      // purpose (no serde-wasm-bindgen); parse it back to a native object.
      layout: JSON.parse(raw.layout_json),
      warnings: raw.warnings,
      passes: raw.passes,
    };
  } finally {
    raw.free();
  }
}
