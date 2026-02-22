/// Recording proxy that traces property access for template compilation.
///
/// When JSX templates use `data.user.name`, the proxy records the access path
/// and produces `$ref` markers in the serialized output. Array `.map()` calls
/// produce `$each` markers.

/** Sentinel prefix/suffix for ref markers embedded in template strings. */
const REF_SENTINEL = '\0FORME_REF:';
const REF_SENTINEL_END = '\0';

/** Symbol to identify each markers produced by .map() */
const EACH_MARKER = Symbol.for('forme:each');

/** Symbol to identify expression marker objects */
const EXPR_MARKER = Symbol.for('forme:expr');

/** Create a recording proxy for template data. */
export function createDataProxy(rootPath: string[] = []): unknown {
  const handler: ProxyHandler<object> = {
    get(_target, prop) {
      // String coercion hooks — produce sentinel string for JSX interpolation
      // Must be checked before the generic symbol guard below
      if (prop === Symbol.toPrimitive || prop === 'toString' || prop === 'valueOf') {
        return () => `${REF_SENTINEL}${rootPath.join('.')}${REF_SENTINEL_END}`;
      }

      // .map() on array proxies → produce $each marker
      if (prop === 'map') {
        return (fn: (item: unknown, index: unknown) => unknown) => {
          const itemProxy = createDataProxy(['$item']);
          const indexProxy = createDataProxy(['$index']);
          const template = fn(itemProxy, indexProxy);
          return createEachMarker(rootPath.join('.'), template);
        };
      }

      // Other symbols — not supported
      if (typeof prop === 'symbol') {
        return undefined;
      }

      // Property access → extend path
      return createDataProxy([...rootPath, prop as string]);
    },

    // Support `Symbol.toPrimitive in proxy` checks
    has(_target, prop) {
      if (prop === Symbol.toPrimitive) return true;
      return prop in _target;
    },
  };

  return new Proxy(Object.create(null), handler);
}

// ─── Marker detection ────────────────────────────────────────────────

export function isRefMarker(value: unknown): boolean {
  if (typeof value !== 'string') return false;
  return value.startsWith(REF_SENTINEL) && value.endsWith(REF_SENTINEL_END);
}

export function getRefPath(value: string): string {
  return value.slice(REF_SENTINEL.length, -REF_SENTINEL_END.length);
}

export function isEachMarker(value: unknown): value is { [EACH_MARKER]: true; path: string; template: unknown } {
  return (
    typeof value === 'object' &&
    value !== null &&
    (value as Record<symbol, unknown>)[EACH_MARKER] === true
  );
}

export function getEachPath(marker: { path: string }): string {
  return marker.path;
}

export function getEachTemplate(marker: { template: unknown }): unknown {
  return marker.template;
}

export function isExprMarker(value: unknown): value is { [EXPR_MARKER]: true; expr: Record<string, unknown> } {
  return (
    typeof value === 'object' &&
    value !== null &&
    (value as Record<symbol, unknown>)[EXPR_MARKER] === true
  );
}

export function getExpr(marker: { expr: Record<string, unknown> }): Record<string, unknown> {
  return marker.expr;
}

// ─── Internal helpers ────────────────────────────────────────────────

function createEachMarker(path: string, template: unknown) {
  return Object.defineProperty(
    { path, template, [EACH_MARKER]: true as const },
    EACH_MARKER,
    { enumerable: false, value: true },
  );
}

/** Opaque marker type returned by expression helpers. */
export interface ExprMarkerObject {
  expr: Record<string, unknown>;
}

/** Create an expression marker wrapping a template expression object. */
export function createExprMarker(expr: Record<string, unknown>): ExprMarkerObject {
  return Object.defineProperty(
    { expr, [EXPR_MARKER]: true as const },
    EXPR_MARKER,
    { enumerable: false, value: true },
  ) as ExprMarkerObject;
}

/** Convert a value that may be a proxy/marker to its expression form. */
export function toExprValue(v: unknown): unknown {
  if (typeof v === 'string' && isRefMarker(v)) {
    return { $ref: getRefPath(v) };
  }
  if (isExprMarker(v)) {
    return v.expr;
  }
  // Proxy objects will coerce to string via toPrimitive when used in expressions
  if (typeof v === 'object' && v !== null && Symbol.toPrimitive in (v as object)) {
    const str = String(v);
    if (isRefMarker(str)) {
      return { $ref: getRefPath(str) };
    }
  }
  return v;
}

export { REF_SENTINEL, REF_SENTINEL_END, EACH_MARKER, EXPR_MARKER };
