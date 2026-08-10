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

<forme-code props={encodeProps('Code', rest)}>{@render children?.()}</forme-code>
