// The declared result types and the runtime objects must agree in BOTH
// directions, on every target. `Record<keyof T, true>` fails to COMPILE
// (npm run typecheck) whenever `index.d.ts` gains or loses a field this
// list doesn't mirror; the runtime tests then hold these exact key sets
// against `Object.keys()` of real results in each runtime. A type that
// declares a field no target returns is worse than a missing field,
// because it type-checks — which is exactly how `passes` shipped
// undefined on five of six constructions.
import type { RenderHtmlResult, RenderHtmlLayoutResult } from '../index.js';

const RESULT_KEY_RECORD: Record<keyof RenderHtmlResult, true> = {
  pdf: true,
  warnings: true,
  passes: true,
};
const LAYOUT_KEY_RECORD: Record<keyof RenderHtmlLayoutResult, true> = {
  pdf: true,
  warnings: true,
  passes: true,
  layout: true,
};

export const RESULT_KEYS = Object.keys(RESULT_KEY_RECORD).sort();
export const LAYOUT_KEYS = Object.keys(LAYOUT_KEY_RECORD).sort();

/** Assertion helper shared by the runtime shape tests (throws on mismatch). */
export function assertShape(actual: object, expected: string[], label: string): void {
  const keys = Object.keys(actual).sort();
  if (keys.join(',') !== expected.join(',')) {
    throw new Error(`${label}: runtime keys [${keys}] != declared keys [${expected}]`);
  }
}
