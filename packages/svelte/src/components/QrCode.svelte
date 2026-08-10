<script lang="ts">
  import type { Style } from '@formepdf/shared';
  import { encodeProps } from '../encode.js';

  interface Props {
    /** Data to encode (URL, text, etc.). */
    data: string;
    /** Display size in points (QR codes are square). Defaults to
     *  available width. */
    size?: number;
    /** QR code color. Default: black. */
    color?: string;
    style?: Style;
  }

  let rest: Props = $props();
</script>

<forme-qrcode props={encodeProps('QrCode', rest)}></forme-qrcode>
