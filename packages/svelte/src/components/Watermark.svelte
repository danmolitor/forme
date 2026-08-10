<script lang="ts">
  import type { Style } from '@formepdf/shared';
  import { encodeProps } from '../encode.js';

  interface Props {
    /** Watermark text (e.g. "DRAFT", "CONFIDENTIAL"). */
    text: string;
    /** Font size in points. Default: 60. */
    fontSize?: number;
    /** Text color. Use `rgba(0,0,0,0.1)` for subtle watermarks.
     *  Default: "rgba(0,0,0,0.1)". */
    color?: string;
    /** Rotation angle in degrees (negative = counterclockwise).
     *  Default: -45. */
    angle?: number;
    /** Additional style properties (fontFamily, etc.). */
    style?: Style;
  }

  let rest: Props = $props();
</script>

<forme-watermark props={encodeProps('Watermark', rest)}></forme-watermark>
