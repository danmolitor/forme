<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { Style } from '@formepdf/shared';
  import { encodeProps } from '../encode.js';

  interface Props {
    colSpan?: number;
    rowSpan?: number;
    style?: Style;
    children?: Snippet;
  }

  let { children, ...rest }: Props = $props();
</script>

<forme-cell props={encodeProps('Cell', rest)}>{@render children?.()}</forme-cell>
