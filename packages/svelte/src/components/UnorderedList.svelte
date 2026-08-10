<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { Style } from '@formepdf/shared';
  import { encodeProps } from '../encode.js';

  interface Props {
    /** Bullet glyph style. Defaults to 'disc'. */
    marker?: 'disc' | 'circle' | 'square' | 'none';
    style?: Style;
    bookmark?: string;
    children?: Snippet;
  }

  let { children, ...rest }: Props = $props();
</script>

<forme-unordered-list props={encodeProps('UnorderedList', rest)}>{@render children?.()}</forme-unordered-list>
