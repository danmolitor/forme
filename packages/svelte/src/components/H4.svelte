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

<forme-h4 props={encodeProps('H4', rest)}>{@render children?.()}</forme-h4>
