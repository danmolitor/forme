<script lang="ts">
  import type { PieChartProps } from '@formepdf/shared';
  import { encodeProps } from '@formepdf/shared';

  let rest: PieChartProps = $props();
</script>

<forme-pie-chart props={encodeProps('PieChart', rest)}></forme-pie-chart>
