<script lang="ts">
  import type { LineChartProps } from '@formepdf/shared';
  import { encodeProps } from '../encode.js';

  let rest: LineChartProps = $props();
</script>

<forme-line-chart props={encodeProps('LineChart', rest)}></forme-line-chart>
