<script lang="ts">
  import type { Style } from '@formepdf/shared';
  import { encodeProps } from '../encode.js';

  interface Props {
    /** Field name - multiple RadioButtons with the same name form a group. */
    name: string;
    /** The value this button represents within the group. */
    value: string;
    /** Whether this button is selected. Default: false. */
    checked?: boolean;
    /** Display width in points. Default: 14. */
    width?: number;
    /** Display height in points. Default: 14. */
    height?: number;
    /** Prevent editing. Default: false. */
    readOnly?: boolean;
    style?: Style;
  }

  let rest: Props = $props();
</script>

<forme-radio-button props={encodeProps('RadioButton', rest)}></forme-radio-button>
