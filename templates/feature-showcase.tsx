import { Document, Page, View, Text, Canvas, Fixed, Watermark, BarChart, LineChart, PieChart } from '@formepdf/react';

export default function FeatureShowcase() {
  // Chart data
  const revenueData = [
    { label: 'Jan', value: 42 },
    { label: 'Feb', value: 58 },
    { label: 'Mar', value: 45 },
    { label: 'Apr', value: 72 },
    { label: 'May', value: 65 },
    { label: 'Jun', value: 88 },
  ];

  const trendData = [
    { label: 'W1', value: 120 },
    { label: 'W2', value: 190 },
    { label: 'W3', value: 160 },
    { label: 'W4', value: 240 },
    { label: 'W5', value: 210 },
    { label: 'W6', value: 310 },
    { label: 'W7', value: 280 },
    { label: 'W8', value: 350 },
  ];

  const categoryData = [
    { label: 'Product', value: 42, color: '#3b82f6' },
    { label: 'Services', value: 28, color: '#0f172a' },
    { label: 'Support', value: 18, color: '#64748b' },
    { label: 'Other', value: 12, color: '#cbd5e1' },
  ];

  // Gauge config
  const gaugeValue = 73;
  const gaugeSize = 90;

  // Overflow demo text
  const overflowText =
    'This text demonstrates overflow: hidden clipping. ' +
    'The container has a fixed height of 48pt, and any content that extends beyond ' +
    'that boundary is cleanly clipped by the PDF clip path. ' +
    'This line and everything after it should be invisible. ' +
    'You cannot see this sentence because it overflows the box.';

  return (
    <Document title="Feature Showcase" author="Forme" lang="en">
      <Page size="Letter" margin={40}>
        <Watermark text="PREVIEW" fontSize={72} color="rgba(0,0,0,0.05)" angle={-45} />

        <Fixed position="footer">
          <View style={{ flexDirection: 'row', justifyContent: 'space-between', paddingTop: 6, borderTopWidth: 1, borderColor: '#e2e8f0' }}>
            <Text style={{ fontSize: 7, color: '#94a3b8' }}>Forme — Feature Showcase</Text>
            <Text style={{ fontSize: 7, color: '#94a3b8' }}>Page {'{{{pageNumber}}}'}</Text>
          </View>
        </Fixed>

        {/* Header */}
        <View style={{ marginBottom: 20 }}>
          <Text style={{ fontSize: 24, fontWeight: 700, color: '#0f172a' }}>Feature Showcase</Text>
          <Text style={{ fontSize: 10, color: '#64748b', marginTop: 4 }}>
            Five new Forme capabilities in one document
          </Text>
        </View>

        {/* Top row: Canvas gauge + Overflow demo */}
        <View style={{ flexDirection: 'row', gap: 14, marginBottom: 16 }}>
          {/* Canvas Gauge */}
          <View style={{ flex: 1, padding: 14, borderWidth: 1, borderColor: '#e2e8f0', borderRadius: 6 }}>
            <Text style={{ fontSize: 9, fontWeight: 700, color: '#0f172a', textTransform: 'uppercase', letterSpacing: 0.8, marginBottom: 8 }}>
              Canvas Drawing
            </Text>
            <View style={{ flex: 1, alignItems: 'center', justifyContent: 'center' }}>
              <Canvas
                width={gaugeSize}
                height={gaugeSize * 0.4 + 14}
                draw={(ctx) => {
                  const r = gaugeSize * 0.4;
                  const canvasH = r + 14;
                  const cx = gaugeSize / 2;
                  const cy = canvasH - 6;

                  // Background arc (gray)
                  ctx.setStrokeColor(226, 232, 240);
                  ctx.setLineWidth(8);
                  ctx.setLineCap(1); // round
                  ctx.arc(cx, cy, r, Math.PI, 0);
                  ctx.stroke();

                  // Value arc (blue)
                  const pct = gaugeValue / 100;
                  const endAngle = Math.PI + pct * Math.PI;
                  ctx.setStrokeColor(59, 130, 246);
                  ctx.setLineWidth(8);
                  ctx.setLineCap(1);
                  ctx.arc(cx, cy, r, Math.PI, endAngle);
                  ctx.stroke();

                  // Tick marks
                  ctx.setStrokeColor(148, 163, 184);
                  ctx.setLineWidth(1);
                  for (let i = 0; i <= 10; i++) {
                    const angle = Math.PI + (i / 10) * Math.PI;
                    const inner = r - 12;
                    const outer = r - 6;
                    ctx.moveTo(cx + inner * Math.cos(angle), cy + inner * Math.sin(angle));
                    ctx.lineTo(cx + outer * Math.cos(angle), cy + outer * Math.sin(angle));
                    ctx.stroke();
                  }

                  // Needle
                  const needleAngle = Math.PI + pct * Math.PI;
                  const needleLen = r - 16;
                  ctx.setStrokeColor(15, 23, 42);
                  ctx.setLineWidth(2);
                  ctx.setLineCap(1);
                  ctx.moveTo(cx, cy);
                  ctx.lineTo(cx + needleLen * Math.cos(needleAngle), cy + needleLen * Math.sin(needleAngle));
                  ctx.stroke();

                  // Center dot
                  ctx.setFillColor(15, 23, 42);
                  ctx.circle(cx, cy, 3);
                  ctx.fill();
                }}
              />
              <Text style={{ fontSize: 20, fontWeight: 700, color: '#0f172a', marginTop: 4, textAlign: 'center' }}>{gaugeValue}%</Text>
              <Text style={{ fontSize: 8, color: '#64748b', textAlign: 'center' }}>System Health</Text>
            </View>
          </View>

          {/* Overflow hidden demo */}
          <View style={{ flex: 1, padding: 14, borderWidth: 1, borderColor: '#e2e8f0', borderRadius: 6 }}>
            <Text style={{ fontSize: 9, fontWeight: 700, color: '#0f172a', textTransform: 'uppercase', letterSpacing: 0.8, marginBottom: 8 }}>
              Overflow: Hidden
            </Text>

            {/* Clipped box */}
            <View style={{ marginBottom: 8 }}>
              <Text style={{ fontSize: 7, color: '#3b82f6', fontWeight: 700, marginBottom: 3 }}>Clipped (overflow: hidden)</Text>
              <View style={{ height: 60, overflow: 'hidden', backgroundColor: '#f8fafc', borderWidth: 1, borderColor: '#e2e8f0', borderRadius: 4, padding: 6 }}>
                <Text style={{ fontSize: 8, lineHeight: 1.4, color: '#334155' }}>{overflowText}</Text>
              </View>
            </View>

            {/* Visible box (default) */}
            <View>
              <Text style={{ fontSize: 7, color: '#64748b', fontWeight: 700, marginBottom: 3 }}>Default (overflow: visible)</Text>
              <View style={{ height: 60, backgroundColor: '#f8fafc', borderWidth: 1, borderColor: '#e2e8f0', borderRadius: 4, padding: 6 }}>
                <Text style={{ fontSize: 8, lineHeight: 1.4, color: '#334155' }}>
                  This box has the same fixed height but no clipping. Content flows naturally beyond the boundary.
                </Text>
              </View>
            </View>
          </View>
        </View>

        {/* Charts row */}
        <View style={{ marginBottom: 16 }}>
          <Text style={{ fontSize: 9, fontWeight: 700, color: '#0f172a', textTransform: 'uppercase', letterSpacing: 0.8, marginBottom: 10 }}>
            Chart Components
          </Text>
          <View style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: 14 }}>
            {/* Bar Chart */}
            <View style={{ padding: 12, borderWidth: 1, borderColor: '#e2e8f0', borderRadius: 6 }}>
              <Text style={{ fontSize: 9, fontWeight: 700, color: '#334155', marginBottom: 6 }}>Monthly Revenue</Text>
              <BarChart
                width={152}
                height={120}
                data={revenueData}
                color="#3b82f6"
                showGrid
                showValues
              />
            </View>

            {/* Line Chart */}
            <View style={{ padding: 12, borderWidth: 1, borderColor: '#e2e8f0', borderRadius: 6 }}>
              <Text style={{ fontSize: 9, fontWeight: 700, color: '#334155', marginBottom: 6 }}>Weekly Trend</Text>
              <LineChart
                width={152}
                height={120}
                data={trendData}
                color="#0f172a"
                showGrid
                showDots
                showArea
              />
            </View>

            {/* Pie Chart */}
            <View style={{ padding: 12, borderWidth: 1, borderColor: '#e2e8f0', borderRadius: 6 }}>
              <Text style={{ fontSize: 9, fontWeight: 700, color: '#334155', marginBottom: 6 }}>Categories</Text>
              <PieChart
                width={152}
                height={120}
                data={categoryData}
                innerRadius={25}
                showLabels
              />
            </View>
          </View>
        </View>

        {/* Font fallback section */}
        <View style={{ padding: 14, borderWidth: 1, borderColor: '#e2e8f0', borderRadius: 6, marginBottom: 16 }}>
          <Text style={{ fontSize: 9, fontWeight: 700, color: '#0f172a', textTransform: 'uppercase', letterSpacing: 0.8, marginBottom: 8 }}>
            Font Fallback Chains
          </Text>
          <View style={{ flexDirection: 'row', gap: 16 }}>
            <View style={{ flex: 1 }}>
              <Text style={{ fontSize: 8, color: '#64748b', marginBottom: 4 }}>fontFamily: "Helvetica, Courier"</Text>
              <Text style={{ fontSize: 12, fontFamily: 'Helvetica, Courier', color: '#0f172a' }}>
                Hello · Bonjour · Hola · Grüße
              </Text>
            </View>
            <View style={{ flex: 1 }}>
              <Text style={{ fontSize: 8, color: '#64748b', marginBottom: 4 }}>fontFamily: "Courier, Helvetica"</Text>
              <Text style={{ fontSize: 12, fontFamily: 'Courier, Helvetica', color: '#0f172a' }}>
                Hello · Bonjour · Hola · Grüße
              </Text>
            </View>
          </View>
          <Text style={{ fontSize: 7, color: '#94a3b8', marginTop: 8, lineHeight: 1.4 }}>
            Comma-separated font families enable per-character fallback. With custom fonts (e.g. "Inter, NotoSansArabic, NotoSansSC"), each character resolves to the first font in the chain that contains its glyph — enabling mixed-script text in a single line.
          </Text>
        </View>

        {/* Feature summary */}
        <View style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr 1fr 1fr', gap: 8 }}>
          {[
            { label: 'Canvas', desc: 'Arbitrary vector drawing via draw callback' },
            { label: 'overflow: hidden', desc: 'PDF clip path clips children to bounds' },
            { label: 'Charts', desc: 'BarChart, LineChart, PieChart components' },
            { label: 'Font Fallback', desc: 'Per-character resolution across font chains' },
            { label: 'Watermarks', desc: 'Rotated text behind page content' },
          ].map((f, i) => (
            <View key={i} style={{ padding: 8, backgroundColor: '#f0f9ff', borderRadius: 4 }}>
              <Text style={{ fontSize: 7, fontWeight: 700, color: '#2563eb' }}>{f.label}</Text>
              <Text style={{ fontSize: 6, color: '#64748b', marginTop: 2, lineHeight: 1.3 }}>{f.desc}</Text>
            </View>
          ))}
        </View>
      </Page>
    </Document>
  );
}
