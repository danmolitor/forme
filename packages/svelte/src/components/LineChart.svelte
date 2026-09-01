<script lang="ts">
  import type { LineChartProps } from '@formepdf/shared';
  import { encodeProps } from '@formepdf/shared';

  let rest: LineChartProps = $props();
</script>

<forme-line-chart props={encodeProps('LineChart', rest)}></forme-line-chart>
