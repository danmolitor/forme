<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { Edges, Style } from '@formepdf/shared';
  import { encodeProps } from '../encode.js';

  interface Props {
    size?: 'A4' | 'A3' | 'A5' | 'Letter' | 'Legal' | 'Tabloid' | { width: number; height: number };
    margin?: number | string | number[] | Edges;
    /** Style applied to the page's content area (background, padding, default text styles). */
    style?: Style;
    /** Optional background image painted behind the page's content. URL,
     *  file path, or `data:image/...;base64,` URI. */
    backgroundImage?: string;
    /** Background image opacity 0–1. Defaults to 1.0. */
    backgroundOpacity?: number;
    /** How the background image is sized within the page. */
    backgroundSize?: 'fill' | 'cover' | 'contain';
    /** Where the background image is positioned within the page. */
    backgroundPosition?: 'center' | 'top-left' | 'top-right' | 'bottom-left' | 'bottom-right';
    children?: Snippet;
  }

  let { children, ...rest }: Props = $props();
</script>

<forme-page props={encodeProps('Page', rest)}>{@render children?.()}</forme-page>
