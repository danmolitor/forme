import { Document, Page, View, Text, Svg, Fixed } from '@formepdf/react';

export default function GridDashboard(data: any) {
  const metrics = data.metrics || [];
  const regions = data.regions || [];
  const barColors = ['#3b82f6', '#0f172a', '#64748b', '#94a3b8'];

  // Bar chart SVG
  let barSvg = '';
  const maxPct = Math.max(...regions.map((r: any) => r.pct));
  regions.forEach((r: any, i: number) => {
    const barW = 36;
    const gap = (200 - barW * regions.length) / (regions.length + 1);
    const x = gap + i * (barW + gap);
    const barH = (r.pct / maxPct) * 100;
    const y = 110 - barH;
    barSvg += `<rect x="${x}" y="${y}" width="${barW}" height="${barH}" fill="${barColors[i]}" rx="2"/>`;
  });
  for (let i = 0; i <= 4; i++) {
    const y = 10 + (100 / 4) * i;
    barSvg += `<line x1="0" y1="${y}" x2="200" y2="${y}" stroke="#e2e8f0" stroke-width="0.5"/>`;
  }

  return (
    <Document title={data.title} author={data.author} lang="en">
      <Page size="Letter" margin={40}>
        <Fixed position="footer">
          <View style={{ flexDirection: 'row', justifyContent: 'space-between', paddingTop: 6, borderTopWidth: 1, borderColor: '#e2e8f0' }}>
            <Text style={{ fontSize: 7, color: '#94a3b8' }}>{data.author} — Confidential</Text>
            <Text style={{ fontSize: 7, color: '#94a3b8' }}>Page {'{{{pageNumber}}}'}</Text>
          </View>
        </Fixed>

        {/* Header */}
        <View style={{ flexDirection: 'row', justifyContent: 'space-between', alignItems: 'center', marginBottom: 20 }}>
          <View>
            <Text style={{ fontSize: 22, fontWeight: 700, color: '#0f172a' }}>{data.title}</Text>
            <Text style={{ fontSize: 10, color: '#64748b', marginTop: 4 }}>{data.subtitle}</Text>
          </View>
          <View style={{ padding: 8, backgroundColor: '#f0f9ff', borderRadius: 4 }}>
            <Text style={{ fontSize: 9, color: '#2563eb', fontWeight: 700 }}>{data.period}</Text>
          </View>
        </View>

        {/* KPI Cards: CSS Grid, 4 equal columns */}
        <View style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr 1fr', gap: 10, marginBottom: 20 }}>
          {metrics.map((m: any, i: number) => (
            <View key={i} style={{ padding: 14, backgroundColor: '#f8fafc', borderRadius: 6, borderWidth: 1, borderColor: '#e2e8f0' }}>
              <Text style={{ fontSize: 8, color: '#64748b', textTransform: 'uppercase', letterSpacing: 0.8 }}>{m.label}</Text>
              <Text style={{ fontSize: 22, fontWeight: 700, color: '#0f172a', marginTop: 4 }}>{m.value}</Text>
              <Text style={{ fontSize: 9, fontWeight: 700, color: m.up ? '#16a34a' : '#2563eb', marginTop: 4 }}>
                {m.change} vs Q3
              </Text>
            </View>
          ))}
        </View>

        {/* Two-column grid: chart + regional breakdown */}
        <View style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 16, marginBottom: 20 }}>
          {/* Left: Bar chart */}
          <View style={{ padding: 16, borderWidth: 1, borderColor: '#e2e8f0', borderRadius: 6 }}>
            <Text style={{ fontSize: 11, fontWeight: 700, color: '#0f172a', marginBottom: 10 }}>Revenue by Region</Text>
            <View style={{ alignItems: 'center' }}>
              <Svg width={200} height={120} viewBox="0 0 200 120" content={barSvg} />
            </View>
            <View style={{ flexDirection: 'row', justifyContent: 'center', gap: 12, marginTop: 8 }}>
              {regions.map((r: any, i: number) => (
                <View key={i} style={{ flexDirection: 'row', alignItems: 'center', gap: 4 }}>
                  <View style={{ width: 8, height: 8, borderRadius: 2, backgroundColor: barColors[i] }} />
                  <Text style={{ fontSize: 7, color: '#64748b' }}>{r.name}</Text>
                </View>
              ))}
            </View>
          </View>

          {/* Right: Regional details */}
          <View style={{ padding: 16, borderWidth: 1, borderColor: '#e2e8f0', borderRadius: 6 }}>
            <Text style={{ fontSize: 11, fontWeight: 700, color: '#0f172a', marginBottom: 10 }}>Regional Breakdown</Text>
            {regions.map((r: any, i: number) => (
              <View key={i} style={{ marginBottom: 10 }}>
                <View style={{ flexDirection: 'row', justifyContent: 'space-between', marginBottom: 4 }}>
                  <Text style={{ fontSize: 9, fontWeight: 700, color: '#334155' }}>{r.name}</Text>
                  <Text style={{ fontSize: 9, color: '#64748b' }}>{r.revenue}</Text>
                </View>
                <View style={{ height: 6, backgroundColor: '#e2e8f0', borderRadius: 3 }}>
                  <View style={{ width: (r.pct / 50 * 100) + '%', height: 6, backgroundColor: barColors[i], borderRadius: 3 }} />
                </View>
              </View>
            ))}
          </View>
        </View>

        {/* Three-column grid: text sections with justified + hyphenation */}
        <View style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: 14 }}>
          {data.insights.map((insight: any, i: number) => (
            <View key={i} style={{ padding: 14, backgroundColor: '#f8fafc', borderRadius: 6 }}>
              <Text style={{ fontSize: 10, fontWeight: 700, color: '#0f172a', marginBottom: 6 }}>{insight.heading}</Text>
              <Text style={{ fontSize: 8, lineHeight: 1.5, color: '#334155', textAlign: 'justify', hyphens: 'auto' }}>
                {insight.body}
              </Text>
            </View>
          ))}
        </View>

        {/* Bottom row: 2-col grid with German + French text */}
        <View style={{ marginTop: 20, display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 14 }}>
          {data.translations.map((t: any, i: number) => (
            <View key={i} style={{ padding: 14, backgroundColor: t.bgColor, borderRadius: 6, borderWidth: 1, borderColor: t.borderColor }}>
              <Text style={{ fontSize: 10, fontWeight: 700, color: t.headingColor, marginBottom: 6 }}>{t.heading}</Text>
              <Text style={{ fontSize: 8, lineHeight: 1.5, color: t.textColor, textAlign: 'justify', hyphens: 'auto', lang: t.lang }}>
                {t.body}
              </Text>
            </View>
          ))}
        </View>
      </Page>
    </Document>
  );
}
