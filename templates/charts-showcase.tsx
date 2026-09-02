import {
  Document, Page, View, Text, H1, H2, BarChart, LineChart, PieChart, AreaChart, DotPlot, StyleSheet,
} from '@formepdf/react';

const s = StyleSheet.create({
  page: {
    padding: 40,
    fontFamily: 'Helvetica',
    fontSize: 10,
    color: '#1e293b',
  },
  title: {
    fontSize: 22,
    fontWeight: 700,
    marginTop: 0,
    marginBottom: 4,
    color: '#0f172a',
  },
  subtitle: {
    fontSize: 11,
    color: '#64748b',
    marginBottom: 24,
  },
  section: {
    marginBottom: 28,
  },
  sectionTitle: {
    fontSize: 13,
    fontWeight: 700,
    marginTop: 0,
    marginBottom: 8,
    color: '#334155',
    borderBottom: '1px solid #e2e8f0',
    paddingBottom: 4,
  },
  description: {
    fontSize: 9,
    color: '#64748b',
    marginBottom: 10,
  },
  row: {
    flexDirection: 'row' as const,
    gap: 20,
  },
  col: {
    flex: 1,
  },
  badge: {
    backgroundColor: '#f0f9ff',
    borderRadius: 4,
    padding: '4 8',
    fontSize: 8,
    color: '#0369a1',
    alignSelf: 'flex-start' as const,
    marginBottom: 8,
  },
  footer: {
    fontSize: 8,
    color: '#94a3b8',
    textAlign: 'center' as const,
    marginTop: 20,
    borderTop: '1px solid #e2e8f0',
    paddingTop: 8,
  },
});

// ── Sample Data ──────────────────────────────────────────────────────

const quarterlyRevenue = [
  { label: 'Q1', value: 142000 },
  { label: 'Q2', value: 186000 },
  { label: 'Q3', value: 164000 },
  { label: 'Q4', value: 228000 },
];

const monthlyUsers = {
  series: [
    { name: 'Active Users', data: [1200, 1800, 2400, 3100, 3800, 4200, 4900, 5600, 6100, 6800, 7400, 8200], color: '#3b82f6' },
    { name: 'New Signups', data: [400, 600, 800, 900, 1100, 1000, 1300, 1500, 1400, 1600, 1800, 2000], color: '#10b981' },
  ],
  labels: ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'],
};

const departmentBudget = [
  { label: 'Engineering', value: 420, color: '#3b82f6' },
  { label: 'Marketing', value: 180, color: '#f59e0b' },
  { label: 'Sales', value: 240, color: '#10b981' },
  { label: 'Operations', value: 110, color: '#8b5cf6' },
  { label: 'Support', value: 80, color: '#ef4444' },
];

const trafficSources = {
  series: [
    { name: 'Organic', data: [500, 800, 1200, 1800, 2600, 3200, 3800, 4100, 4500, 4800, 5200, 5800], color: '#3b82f6' },
    { name: 'Paid', data: [200, 400, 600, 900, 1100, 1400, 1600, 1800, 2000, 2200, 2400, 2600], color: '#f59e0b' },
    { name: 'Referral', data: [100, 150, 250, 400, 500, 600, 700, 800, 900, 1000, 1100, 1200], color: '#10b981' },
  ],
  labels: ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'],
};

const performanceData = [
  {
    name: 'API v1',
    color: '#3b82f6',
    data: [[10, 45], [50, 120], [100, 230], [200, 450], [500, 980], [1000, 1800]] as [number, number][],
  },
  {
    name: 'API v2',
    color: '#10b981',
    data: [[10, 25], [50, 60], [100, 110], [200, 210], [500, 480], [1000, 920]] as [number, number][],
  },
  {
    name: 'API v3',
    color: '#8b5cf6',
    data: [[10, 12], [50, 35], [100, 65], [200, 120], [500, 280], [1000, 540]] as [number, number][],
  },
];

const expenseCategories = [
  { label: 'Jan', value: 45000 },
  { label: 'Feb', value: 52000 },
  { label: 'Mar', value: 48000 },
  { label: 'Apr', value: 61000 },
  { label: 'May', value: 55000 },
  { label: 'Jun', value: 67000 },
  { label: 'Jul', value: 72000 },
  { label: 'Aug', value: 63000 },
];

// ── Template ─────────────────────────────────────────────────────────

export default function ChartsShowcase() {
  return (
    <Document title="Charts Showcase" author="Forme">
      {/* Page 1: Header + Bar/Pie row + Line chart */}
      <Page style={s.page}>
        <H1 style={s.title}>Annual Performance Report</H1>
        <Text style={s.subtitle}>
          FY 2025 — Generated with Forme engine-native chart components
        </Text>

        {/* Bar + Pie side by side */}
        <View style={s.section}>
          <H2 style={s.sectionTitle}>Revenue & Budget Overview</H2>
          <View style={s.row}>
            <View style={s.col}>
              <Text style={s.badge}>Quarterly Revenue</Text>
              <BarChart
                width={240}
                height={180}
                data={quarterlyRevenue}
                color="#3b82f6"
                showLabels
                showValues
                showGrid
                title="Revenue by Quarter"
              />
            </View>
            <View style={s.col}>
              <Text style={s.badge}>Department Budget (K)</Text>
              <PieChart
                width={240}
                height={180}
                data={departmentBudget}
                showLegend
                title="Budget Allocation"
              />
            </View>
          </View>
        </View>

        {/* Line chart full width */}
        <View style={s.section}>
          <H2 style={s.sectionTitle}>User Growth</H2>
          <Text style={s.description}>
            Monthly active users and new signups across the full fiscal year.
          </Text>
          <LineChart
            width={510}
            height={200}
            series={monthlyUsers.series}
            labels={monthlyUsers.labels}
            showPoints
            showGrid
            title="Monthly User Metrics"
          />
        </View>

        <Text style={s.footer}>
          Page 1 of 2 — Confidential
        </Text>
      </Page>

      {/* Page 2: Area chart + Dot plot + Expenses bar chart */}
      <Page style={s.page}>
        <View style={s.section}>
          <H2 style={s.sectionTitle}>Traffic Sources</H2>
          <Text style={s.description}>
            Cumulative visitor counts by acquisition channel, showing organic growth outpacing paid.
          </Text>
          <AreaChart
            width={510}
            height={200}
            series={trafficSources.series}
            labels={trafficSources.labels}
            showGrid
            title="Visitor Acquisition"
          />
        </View>

        <View style={s.row}>
          <View style={s.col}>
            <H2 style={s.sectionTitle}>API Latency</H2>
            <Text style={s.description}>
              Response time (ms) vs concurrent requests across API versions.
            </Text>
            <DotPlot
              width={240}
              height={200}
              groups={performanceData}
              xLabel="Concurrent Requests"
              yLabel="Latency (ms)"
              showLegend
              dotSize={3.5}
            />
          </View>
          <View style={s.col}>
            <H2 style={s.sectionTitle}>Monthly Expenses</H2>
            <Text style={s.description}>
              Operating expenses Jan–Aug with value labels.
            </Text>
            <BarChart
              width={240}
              height={200}
              data={expenseCategories}
              color="#ef4444"
              showLabels
              showValues
              showGrid
            />
          </View>
        </View>

        <View style={{ marginTop: 16 }}>
          <H2 style={s.sectionTitle}>Donut: Revenue Split</H2>
          <View style={s.row}>
            <View style={s.col}>
              <PieChart
                width={180}
                height={150}
                data={[
                  { label: 'Subscriptions', value: 62, color: '#3b82f6' },
                  { label: 'Enterprise', value: 28, color: '#0f172a' },
                  { label: 'Services', value: 10, color: '#94a3b8' },
                ]}
                donut
                showLegend
              />
            </View>
            <View style={{ ...s.col, justifyContent: 'center' as const }}>
              <H2 style={{ fontSize: 11, fontWeight: 700, marginTop: 0, marginBottom: 4 }}>Key Takeaways</H2>
              <Text style={{ fontSize: 9, color: '#475569', lineHeight: 1.6 }}>
                Subscription revenue accounts for 62% of total revenue, up from 54% last year. Enterprise deals grew 18% YoY. API v3 reduced p95 latency by 70% compared to v1. Organic traffic now exceeds paid for the first time.
              </Text>
            </View>
          </View>
        </View>

        <Text style={s.footer}>
          Page 2 of 2 — Confidential
        </Text>
      </Page>
    </Document>
  );
}
