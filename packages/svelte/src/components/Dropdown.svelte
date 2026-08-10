<script lang="ts">
  import type { Style } from '@formepdf/shared';
  import { encodeProps } from '../encode.js';

  interface Props {
    /** Unique field name. */
    name: string;
    /** List of selectable options. */
    options: string[];
    /** Pre-selected value. */
    value?: string;
    /** Field width in points. */
    width: number;
    /** Field height in points. Default: 24. */
    height?: number;
    /** Prevent editing. Default: false. */
    readOnly?: boolean;
    /** Font size in points. Default: 12. */
    fontSize?: number;
    style?: Style;
  }

  let rest: Props = $props();
</script>

<forme-dropdown props={encodeProps('Dropdown', rest)}></forme-dropdown>
