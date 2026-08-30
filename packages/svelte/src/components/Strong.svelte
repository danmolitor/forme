<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { Style } from '@formepdf/shared';
  import { encodeProps } from '@formepdf/shared';

  interface Props {
    style?: Style;
    children?: Snippet;
  }

  let { children, ...rest }: Props = $props();
</script>

<forme-strong props={encodeProps('Strong', rest)}>{@render children?.()}</forme-strong>
