/**
 * Vue-layer prop encoding.
 *
 * Delegates to the shared `encodeProps` (the emitter/parser contract) but
 * first repairs one Vue-specific SSR artifact: a *bare* boolean attribute
 * (`<Row header>`) reaches the component as an empty string, where React and
 * Svelte both yield `true`. Left as-is it would serialize `header: ""`,
 * diverging from the other adapters. We coerce `""` → `true` for the known
 * boolean prop names only — no non-boolean prop in the Forme API carries any
 * of these names, so a legitimate empty-string value (an empty `title`, say)
 * is never touched. Absent props stay absent (the SFCs use array-form
 * `defineProps`, so an unset prop is `undefined`, not `false`), which is what
 * preserves the undefined-vs-false distinction the parser relies on for
 * `wrap`/`tagged`.
 */
import { encodeProps as sharedEncodeProps } from '@formepdf/shared';

/** Boolean prop names across the Forme component API. */
const BOOLEAN_PROPS = new Set([
  'header',
  'wrap',
  'tagged',
  'pdfUa',
  'multiline',
  'password',
  'readOnly',
  'checked',
]);

export function encodeProps(component: string, props: Record<string, unknown>): string {
  let normalized = props;
  for (const key of Object.keys(props)) {
    if (props[key] === '' && BOOLEAN_PROPS.has(key)) {
      if (normalized === props) normalized = { ...props };
      normalized[key] = true;
    }
  }
  return sharedEncodeProps(component, normalized);
}
