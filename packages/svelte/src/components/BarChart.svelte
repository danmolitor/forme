<script lang="ts">
  import type { BarChartProps } from '@formepdf/shared';
  import { encodeProps } from '@formepdf/shared';

  let rest: BarChartProps = $props();
</script>

<forme-bar-chart props={encodeProps('BarChart', rest)}></forme-bar-chart>
