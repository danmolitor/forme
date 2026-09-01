/**
 * Page-number placeholders. The engine substitutes `{{pageNumber}}` and
 * `{{totalPages}}` in text content at render time. In a Vue template
 * `{{pageNumber}}` is interpolation syntax, so the documented way to emit
 * the literal placeholders is interpolating these constants:
 *
 * ```vue
 * <Fixed position="footer">
 *   <Text>Page {{ PAGE_NUMBER }} of {{ TOTAL_PAGES }}</Text>
 * </Fixed>
 * ```
 */
export const PAGE_NUMBER = '{{pageNumber}}';
export const TOTAL_PAGES = '{{totalPages}}';
