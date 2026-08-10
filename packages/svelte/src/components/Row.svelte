<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { Style } from '@formepdf/shared';
  import { encodeProps } from '../encode.js';

  interface Props {
    /** Header rows repeat at the top of every continuation page when
     *  the table splits across pages. */
    header?: boolean;
    style?: Style;
    children?: Snippet;
  }

  let { children, ...rest }: Props = $props();
</script>

<forme-row props={encodeProps('Row', rest)}>{@render children?.()}</forme-row>
