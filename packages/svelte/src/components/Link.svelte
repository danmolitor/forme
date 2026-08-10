<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { Style } from '@formepdf/shared';
  import { encodeProps } from '../encode.js';

  interface Props {
    href: string;
    style?: Style;
    children?: Snippet;
  }

  let { children, ...rest }: Props = $props();
</script>

<forme-link props={encodeProps('Link', rest)}>{@render children?.()}</forme-link>
