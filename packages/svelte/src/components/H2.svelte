<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { Style } from '@formepdf/shared';
  import { encodeProps } from '../encode.js';

  interface Props {
    style?: Style;
    href?: string;
    bookmark?: string;
    children?: Snippet;
  }

  let { children, ...rest }: Props = $props();
</script>

<forme-h2 props={encodeProps('H2', rest)}>{@render children?.()}</forme-h2>
