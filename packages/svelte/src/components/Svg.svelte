<script lang="ts">
  import type { Style } from '@formepdf/shared';
  import { encodeProps } from '../encode.js';

  interface Props {
    /** Display width in points. */
    width: number;
    /** Display height in points. */
    height: number;
    /** SVG viewBox string (e.g., "0 0 100 100"). */
    viewBox?: string;
    /** SVG markup string - the inner content, not the outer `<svg>` tag.
     *  Supported shapes: rect, circle, ellipse, line, path, polygon,
     *  polyline. */
    content?: string;
    style?: Style;
    /** Optional hyperlink URL - makes the SVG clickable. */
    href?: string;
    /** Alt text for accessibility. */
    alt?: string;
  }

  let rest: Props = $props();
</script>

<forme-svg props={encodeProps('Svg', rest)}></forme-svg>
