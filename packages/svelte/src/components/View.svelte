<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { Style } from '@formepdf/shared';
  import { encodeProps } from '../encode.js';

  interface Props {
    style?: Style;
    wrap?: boolean;
    bookmark?: string;
    href?: string;
    children?: Snippet;
  }

  let { children, ...rest }: Props = $props();
</script>

<forme-view props={encodeProps('View', rest)}>{@render children?.()}</forme-view>
