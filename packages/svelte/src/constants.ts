/**
 * Page-number placeholders.
 *
 * The engine substitutes `{{pageNumber}}` and `{{totalPages}}` in text
 * content when it serializes each page. In JSX the braces can be typed
 * as a string literal (`{'{{pageNumber}}'}`), but in a Svelte template
 * `{{pageNumber}}` parses as an object-literal expression - so the
 * documented way to emit the placeholders is interpolating these
 * constants:
 *
 * ```svelte
 * <Fixed position="footer">
 *   <Text>Page {PAGE_NUMBER} of {TOTAL_PAGES}</Text>
 * </Fixed>
 * ```
 */

/** Replaced with the current page number at render time. */
export const PAGE_NUMBER = '{{pageNumber}}';

/** Replaced with the document's total page count at render time. */
export const TOTAL_PAGES = '{{totalPages}}';
