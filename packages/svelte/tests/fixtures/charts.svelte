<script lang="ts">
  import type { ChartDataPoint, ChartSeries, DotPlotGroup } from '../../src/index.js';
  import {
    Document,
    Page,
    View,
    Text,
    BarChart,
    LineChart,
    PieChart,
    AreaChart,
    DotPlot,
  } from '../../src/index.js';

  interface Props {
    highlight?: string;
  }

  let { highlight = '#ef4444' }: Props = $props();

  const revenue: ChartDataPoint[] = $derived([
    { label: 'Q1', value: 120 },
    { label: 'Q2', value: 80, color: highlight },
    { label: 'Q3', value: 145 },
    { label: 'Q4', value: 90 },
  ]);
  const traffic: ChartDataPoint[] = $derived([
    { label: 'Direct', value: 55, color: '#1a365d' },
    { label: 'Referral', value: 30, color: '#f59e0b' },
    { label: 'Social', value: 15, color: highlight },
  ]);
  const actives: ChartSeries[] = [
    { name: '2025', data: [40, 55, 48, 70, 62, 80] },
    { name: '2026', data: [52, 60, 75, 84, 91, 108], color: '#10b981' },
  ];
  const load: ChartSeries[] = [
    { name: 'API', data: [12, 28, 19, 42, 31, 25] },
    { name: 'Web', data: [8, 14, 22, 18, 27, 33], color: '#8b5cf6' },
  ];
  const doseResponse: DotPlotGroup[] = $derived([
    { name: 'Control', data: [[1, 4], [3, 7], [5, 9], [8, 14]] },
    { name: 'Variant', color: highlight, data: [[2, 6], [4, 11], [6, 15]] },
  ]);
  const regions: ChartDataPoint[] = [
    { label: 'North', value: 34 },
    { label: 'South', value: 21 },
  ];
  const minimalGroup: DotPlotGroup[] = [{ name: 'G', data: [[1, 1], [2, 3]] }];
</script>

<Document title="Charts Parity">
  <Page size="A4" margin={36}>
    <Text style={{ fontSize: 20, marginBottom: 16 }}>Quarterly dashboard</Text>
    <View style={{ flexDirection: 'row', gap: 16, marginBottom: 16 }}>
      <BarChart
        width={240}
        height={160}
        data={revenue}
        color="#1a365d"
        showValues
        showGrid
        title="Revenue by quarter"
        style={{ marginBottom: 8 }}
      />
      <PieChart width={200} height={160} data={traffic} donut showLegend title="Traffic sources" />
    </View>
    <LineChart
      width={520}
      height={180}
      series={actives}
      labels={['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun']}
      showPoints
      showGrid
      title="Monthly actives"
      style={{ marginBottom: 16 }}
    />
    <AreaChart
      width={520}
      height={160}
      series={load}
      labels={['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat']}
      showGrid
      title="Server load"
      style={{ marginBottom: 16 }}
    />
    <DotPlot
      width={520}
      height={200}
      groups={doseResponse}
      xMin={0}
      xMax={12}
      yMin={0}
      yMax={20}
      xLabel="Dose"
      yLabel="Response"
      showLegend
      dotSize={5}
      style={{ marginBottom: 16 }}
    />
    <BarChart width={520} height={120} data={regions} />
    <View style={{ flexDirection: 'row', gap: 12 }}>
      <LineChart width={160} height={90} series={[{ name: 'S', data: [1, 3, 2] }]} labels={['a', 'b', 'c']} />
      <PieChart width={90} height={90} data={regions} />
      <AreaChart width={160} height={90} series={[{ name: 'S', data: [2, 1, 4] }]} labels={['a', 'b', 'c']} />
      <DotPlot width={90} height={90} groups={minimalGroup} />
    </View>
  </Page>
</Document>
