<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { ColumnDef, Style } from '@formepdf/shared';
  import { encodeProps } from '../encode.js';

  interface Props {
    /** Column width definitions: `{ fraction }`, `{ fixed }`, or `'auto'`.
     *  Rows split across pages automatically; header rows repeat on
     *  continuation pages. */
    columns?: ColumnDef[];
    style?: Style;
    children?: Snippet;
  }

  let { children, ...rest }: Props = $props();
</script>

<forme-table props={encodeProps('Table', rest)}>{@render children?.()}</forme-table>
