<script lang="ts">
  import type { Style } from '@formepdf/shared';
  import { encodeProps } from '../encode.js';

  interface Props {
    /** Image source: a `data:image/...;base64,...` URI, a path relative
     *  to the template file, or an absolute file path. Passed through
     *  unresolved - resolution happens at render time. */
    src: string;
    /** Display width in points. If only one dimension is given, the
     *  other is computed from the image's aspect ratio. */
    width?: number;
    /** Display height in points. */
    height?: number;
    style?: Style;
    /** Optional hyperlink URL - makes the image clickable. */
    href?: string;
    /** Alt text for accessibility. */
    alt?: string;
  }

  let rest: Props = $props();
</script>

<forme-image props={encodeProps('Image', rest)}></forme-image>
