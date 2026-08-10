<script lang="ts">
  import type { DotPlotProps } from '@formepdf/shared';
  import { encodeProps } from '../encode.js';

  let rest: DotPlotProps = $props();
</script>

<forme-dot-plot props={encodeProps('DotPlot', rest)}></forme-dot-plot>
