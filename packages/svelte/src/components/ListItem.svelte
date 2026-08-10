<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { Style } from '@formepdf/shared';
  import { encodeProps } from '../encode.js';

  interface Props {
    style?: Style;
    children?: Snippet;
  }

  let { children, ...rest }: Props = $props();
</script>

<forme-list-item props={encodeProps('ListItem', rest)}>{@render children?.()}</forme-list-item>
