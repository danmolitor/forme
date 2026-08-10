/**
 * Prop encoding for the emitting components.
 *
 * Each Forme Svelte component renders one placeholder tag whose
 * `props` attribute carries the component's props as JSON (part of
 * the internal emitter/parser contract - see `parser.ts`). Values
 * that cannot survive the JSON round-trip fail loudly here, naming
 * the component and prop, rather than silently producing a wrong PDF.
 * The one exception is `Uint8Array` (byte font sources on
 * `<Document fonts>`): it is tunneled through a marker object and
 * restored by the parser's reviver, so byte sources survive
 * serialization exactly as they do in the react adapter.
 */

/**
 * Marker key wrapping a base64-encoded `Uint8Array` inside the props
 * JSON. Part of the internal emitter/parser contract.
 */
const BYTES_MARKER = '__formeBytes';

/** Encode bytes to base64 without Node APIs (works in edge runtimes). */
function bytesToBase64(bytes: Uint8Array): string {
  let binary = '';
  const chunk = 0x8000;
  for (let i = 0; i < bytes.length; i += chunk) {
    binary += String.fromCharCode(...bytes.subarray(i, i + chunk));
  }
  return btoa(binary);
}

function base64ToBytes(b64: string): Uint8Array {
  const binary = atob(b64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

/**
 * `JSON.parse` reviver restoring `Uint8Array` values that
 * `encodeProps` tunneled through byte markers.
 */
export function reviveBytesMarker(_key: string, value: unknown): unknown {
  if (
    value !== null &&
    typeof value === 'object' &&
    typeof (value as Record<string, unknown>)[BYTES_MARKER] === 'string'
  ) {
    return base64ToBytes((value as Record<string, string>)[BYTES_MARKER]);
  }
  return value;
}

/**
 * JSON-encode a component's props for the placeholder `props`
 * attribute. `undefined` values are omitted (matching how absent
 * props behave in the react adapter). `Uint8Array` values become
 * byte markers (see `reviveBytesMarker`). Functions, symbols,
 * bigints, and circular structures throw an error naming the
 * component and the offending prop.
 */
export function encodeProps(component: string, props: object): string {
  const parts: string[] = [];
  for (const [key, value] of Object.entries(props)) {
    if (value === undefined) continue;
    let json: string | undefined;
    try {
      json = JSON.stringify(value, (_k, v) => {
        const t = typeof v;
        if (t === 'function' || t === 'symbol' || t === 'bigint') {
          throw new TypeError(`contains a ${t}, which cannot be serialized to JSON`);
        }
        if (v instanceof Uint8Array) {
          return { [BYTES_MARKER]: bytesToBase64(v) };
        }
        // A user object carrying the marker key would be revived as a
        // Uint8Array on the other side - fail loudly instead of
        // silently corrupting it. (Marker objects produced above are
        // never re-visited by the replacer, so this only fires on
        // user-supplied values.)
        if (v !== null && t === 'object' && BYTES_MARKER in v) {
          throw new TypeError(`contains a reserved "${BYTES_MARKER}" key`);
        }
        return v;
      });
    } catch (err) {
      const reason = err instanceof Error ? err.message : String(err);
      throw new Error(`[Forme] <${component}>: prop "${key}" is not serializable: ${reason}`);
    }
    // JSON.stringify returns undefined (without throwing) for values
    // with no JSON representation at the top level, e.g. functions -
    // drop such props rather than emitting invalid JSON.
    if (json === undefined) continue;
    parts.push(`${JSON.stringify(key)}:${json}`);
  }
  return `{${parts.join(',')}}`;
}
