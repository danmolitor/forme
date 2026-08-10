<script lang="ts">
  import type { CanvasContext, Style } from '@formepdf/shared';
  import { recordCanvasOperations } from '@formepdf/shared';
  import { encodeProps } from '../encode.js';

  interface Props {
    /** Canvas width in points. */
    width: number;
    /** Canvas height in points. */
    height: number;
    /**
     * Drawing callback that receives a recording `CanvasContext`.
     *
     * The callback runs once, synchronously, during server-side
     * serialization - not at PDF render time. It must be pure:
     * synchronous, no side effects, and no reliance on browser or
     * runtime APIs. The recorded operations are serialized to JSON
     * and rendered as native PDF vector commands.
     */
    draw: (ctx: CanvasContext) => void;
    style?: Style;
  }

  let { draw, ...rest }: Props = $props();
</script>

<forme-canvas props={encodeProps('Canvas', { ...rest, operations: recordCanvasOperations(draw) })}
></forme-canvas>
