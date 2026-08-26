/// Expression helpers for template operations that Proxy can't capture.
///
/// These produce expression marker objects that `serializeTemplate()` detects
/// and converts to the corresponding `$op` nodes in the template JSON.

import { createExprMarker, toExprValue } from './template-proxy.js';

/** Comparison and arithmetic expression helpers. */
export const expr = {
  // Comparison
  eq: (a: unknown, b: unknown) =>
    createExprMarker({ $eq: [toExprValue(a), toExprValue(b)] }),
  ne: (a: unknown, b: unknown) =>
    createExprMarker({ $ne: [toExprValue(a), toExprValue(b)] }),
  gt: (a: unknown, b: unknown) =>
    createExprMarker({ $gt: [toExprValue(a), toExprValue(b)] }),
  lt: (a: unknown, b: unknown) =>
    createExprMarker({ $lt: [toExprValue(a), toExprValue(b)] }),
  gte: (a: unknown, b: unknown) =>
    createExprMarker({ $gte: [toExprValue(a), toExprValue(b)] }),
  lte: (a: unknown, b: unknown) =>
    createExprMarker({ $lte: [toExprValue(a), toExprValue(b)] }),

  // Arithmetic
  add: (a: unknown, b: unknown) =>
    createExprMarker({ $add: [toExprValue(a), toExprValue(b)] }),
  sub: (a: unknown, b: unknown) =>
    createExprMarker({ $sub: [toExprValue(a), toExprValue(b)] }),
  mul: (a: unknown, b: unknown) =>
    createExprMarker({ $mul: [toExprValue(a), toExprValue(b)] }),
  div: (a: unknown, b: unknown) =>
    createExprMarker({ $div: [toExprValue(a), toExprValue(b)] }),

  // String transforms
  upper: (v: unknown) =>
    createExprMarker({ $upper: toExprValue(v) }),
  lower: (v: unknown) =>
    createExprMarker({ $lower: toExprValue(v) }),
  concat: (...args: unknown[]) =>
    createExprMarker({ $concat: args.map(toExprValue) }),
  format: (v: unknown, fmt: string) =>
    createExprMarker({ $format: [toExprValue(v), fmt] }),

  // Conditional
  cond: (condition: unknown, ifTrue: unknown, ifFalse: unknown) =>
    createExprMarker({ $cond: [toExprValue(condition), toExprValue(ifTrue), toExprValue(ifFalse)] }),

  if: (condition: unknown, then: unknown, elseVal?: unknown) => {
    const obj: Record<string, unknown> = {
      $if: toExprValue(condition),
      then: toExprValue(then),
    };
    if (elseVal !== undefined) {
      obj.else = toExprValue(elseVal);
    }
    return createExprMarker(obj);
  },

  // Array
  count: (v: unknown) =>
    createExprMarker({ $count: toExprValue(v) }),
};
