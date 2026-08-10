<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { Style } from '@formepdf/shared';
  import { encodeProps } from '../encode.js';

  interface Props {
    /** Which page edge this element repeats on. Fixed elements reduce
     *  the available content area on every page. */
    position: 'header' | 'footer';
    style?: Style;
    bookmark?: string;
    children?: Snippet;
  }

  let { children, ...rest }: Props = $props();
</script>

<forme-fixed props={encodeProps('Fixed', rest)}>{@render children?.()}</forme-fixed>
