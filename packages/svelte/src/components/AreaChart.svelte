<script lang="ts">
  import type { AreaChartProps } from '@formepdf/shared';
  import { encodeProps } from '@formepdf/shared';

  let rest: AreaChartProps = $props();
</script>

<forme-area-chart props={encodeProps('AreaChart', rest)}></forme-area-chart>
