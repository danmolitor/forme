<script lang="ts">
  import type { Style } from '@formepdf/shared';
  import { encodeProps } from '../encode.js';

  interface Props {
    /** Unique field name (used as the PDF field identifier). */
    name: string;
    /** Pre-filled value. */
    value?: string;
    /** Placeholder text shown when empty. */
    placeholder?: string;
    /** Field width in points. */
    width: number;
    /** Field height in points. Default: 24. */
    height?: number;
    /** Allow multi-line input. Default: false. */
    multiline?: boolean;
    /** Mask input as password. Default: false. */
    password?: boolean;
    /** Prevent editing. Default: false. */
    readOnly?: boolean;
    /** Maximum number of characters. */
    maxLength?: number;
    /** Font size in points. Default: 12. */
    fontSize?: number;
    style?: Style;
  }

  let rest: Props = $props();
</script>

<forme-text-field props={encodeProps('TextField', rest)}></forme-text-field>
