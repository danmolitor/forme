<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { Style } from '@formepdf/shared';
  import { encodeProps } from '../encode.js';

  interface Props {
    /** Marker style. Defaults to 'decimal'. */
    type?: 'decimal' | 'lower-alpha' | 'upper-alpha' | 'lower-roman' | 'upper-roman';
    /** Starting index. Defaults to 1. */
    start?: number;
    style?: Style;
    bookmark?: string;
    children?: Snippet;
  }

  let { children, ...rest }: Props = $props();
</script>

<forme-ordered-list props={encodeProps('OrderedList', rest)}>{@render children?.()}</forme-ordered-list>
