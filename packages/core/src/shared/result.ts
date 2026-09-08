// @formepdf/core — the ONE place a raw WASM layout result becomes the
// public RenderWithLayoutResult.
//
// The node, browser, and worker entries (plus the template path) each
// built this object by hand with the same `warnings ?? []` fixup — the
// exact shape that produced the html package's `passes` divergence,
// where a field added to the declared type reached one construction out
// of six. Agreement by construction: every entry funnels through this
// function, so a new field is added exactly here.
import type { LayoutInfo } from '../index.js';

export interface RawLayoutResult {
  pdf: Uint8Array;
  layout: LayoutInfo;
  warnings?: string[];
}

export interface RenderWithLayoutResultShape {
  pdf: Uint8Array;
  layout: LayoutInfo;
  warnings: string[];
}

export function toRenderWithLayoutResult(raw: RawLayoutResult): RenderWithLayoutResultShape {
  return { pdf: raw.pdf, layout: raw.layout, warnings: raw.warnings ?? [] };
}

/**
 * Serialize the per-render engine options (`RenderOptions` on the Rust
 * side) for the WASM boundary. Returns `undefined` when every option is
 * off, so the default render takes the exact historical code path —
 * "costs nothing when off" is enforced by construction, not by hoping
 * the engine ignores an empty object cheaply.
 */
export function encodeRenderOptions(options?: { auditContent?: boolean }): string | undefined {
  if (!options?.auditContent) return undefined;
  return JSON.stringify({ auditContent: true });
}
