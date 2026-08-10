<script lang="ts">
  import type { BarcodeFormat, Style } from '@formepdf/shared';
  import { encodeProps } from '../encode.js';

  interface Props {
    /** The data to encode. */
    data: string;
    /** Barcode format: "Code128" (default), "Code39", "EAN13", "EAN8",
     *  "Codabar". */
    format?: BarcodeFormat;
    /** Width in points. Defaults to available width. */
    width?: number;
    /** Height in points. Default: 60. */
    height?: number;
    /** Bar color. Default: black. */
    color?: string;
    style?: Style;
  }

  let rest: Props = $props();
</script>

<forme-barcode props={encodeProps('Barcode', rest)}></forme-barcode>
