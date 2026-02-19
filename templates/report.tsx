import { Document, Page, View, Text, Svg, Table, Row, Cell, Fixed, PageBreak, StyleSheet } from '@formepdf/react';

const styles = StyleSheet.create({
  sectionTitle: { fontSize: 20, fontWeight: 700, color: '#0f172a', marginBottom: 12 },
  bodyText: { fontSize: 10, color: '#334155', lineHeight: 1.6, marginBottom: 12 },
  introText: { fontSize: 10, color: '#334155', lineHeight: 1.6, marginBottom: 16 },
  headerFooterText: { fontSize: 8, color: '#94a3b8' },
  headerBar: { flexDirection: 'row' as const, justifyContent: 'space-between' as const, paddingBottom: 8, borderBottomWidth: 1, borderColor: '#e2e8f0', marginBottom: 16 },
  footerBar: { flexDirection: 'row' as const, justifyContent: 'space-between' as const, paddingTop: 8, borderTopWidth: 1, borderColor: '#e2e8f0' },
  tableHeaderCell: { padding: 8 },
  tableHeaderText: { fontSize: 9, fontWeight: 700, color: '#ffffff', textAlign: 'right' as const },
  tableCell: { padding: 8 },
  tableCellText: { fontSize: 9, color: '#334155', textAlign: 'right' as const },
  chartTitle: { fontSize: 10, fontWeight: 700, color: '#334155', marginBottom: 6 },
  recCard: { flexDirection: 'row' as const, gap: 24, marginBottom: 16, padding: 16, backgroundColor: '#f8fafc', borderRadius: 4, borderLeftWidth: 3, borderColor: '#0f172a' },
  recBadge: { width: 24, height: 24, backgroundColor: '#0f172a', borderRadius: 12, justifyContent: 'center' as const, alignItems: 'center' as const },
  recBadgeText: { fontSize: 10, fontWeight: 700, color: '#ffffff', lineHeight: 1.2 },
  recTitle: { fontSize: 11, fontWeight: 700, color: '#0f172a', marginBottom: 4 },
  recBody: { fontSize: 9, color: '#475569', lineHeight: 1.5 },
  recLabel: { fontSize: 8, fontWeight: 700, color: '#64748b' },
  recValue: { fontSize: 8, color: '#334155' },
});

// ── Chart SVG generators ─────────────────────────────────────────────

const CHART_COLORS = ['#3b82f6', '#0f172a', '#64748b', '#94a3b8', '#cbd5e1'];

function renderBarChart(tableData: any[]): string {
  const totals = tableData.map((r: any) => ({
    label: r.region,
    value: parseFloat(r.q1.replace(/[$,]/g, '')) + parseFloat(r.q2.replace(/[$,]/g, ''))
         + parseFloat(r.q3.replace(/[$,]/g, '')) + parseFloat(r.q4.replace(/[$,]/g, '')),
  }));
  const maxVal = Math.max(...totals.map(t => t.value));

  const w = 230, h = 150;
  const barAreaTop = 8, barAreaBottom = h - 20;
  const barAreaH = barAreaBottom - barAreaTop;
  const barW = Math.min(32, (w - 20) / totals.length - 8);
  const gap = (w - barW * totals.length) / (totals.length + 1);

  let svg = '';
  // Grid lines
  for (let i = 0; i <= 4; i++) {
    const y = barAreaTop + (barAreaH / 4) * i;
    svg += `<line x1="0" y1="${y}" x2="${w}" y2="${y}" stroke="#e2e8f0" stroke-width="0.5"/>`;
  }
  // Bars and labels
  totals.forEach((t, i) => {
    const x = gap + i * (barW + gap);
    const barH = (t.value / maxVal) * barAreaH;
    const y = barAreaBottom - barH;
    svg += `<rect x="${x}" y="${y}" width="${barW}" height="${barH}" fill="${CHART_COLORS[i % CHART_COLORS.length]}" rx="2"/>`;
    // Region label below bar
    const labelX = x + barW / 2;
    svg += `<rect x="${labelX - 2}" y="${barAreaBottom + 4}" width="4" height="4" fill="${CHART_COLORS[i % CHART_COLORS.length]}" rx="1"/>`;
  });
  return svg;
}

/** Approximate a circular arc as cubic bezier segments. */
function arcPath(cx: number, cy: number, r: number, startAngle: number, endAngle: number): string {
  let path = '';
  const step = Math.PI / 2; // max 90 degrees per segment
  let a1 = startAngle;
  while (a1 < endAngle - 0.001) {
    const a2 = Math.min(a1 + step, endAngle);
    const alpha = a2 - a1;
    const k = (4 / 3) * Math.tan(alpha / 4);
    const x1 = cx + r * Math.cos(a1), y1 = cy + r * Math.sin(a1);
    const x2 = cx + r * Math.cos(a2), y2 = cy + r * Math.sin(a2);
    const cp1x = x1 - k * r * Math.sin(a1), cp1y = y1 + k * r * Math.cos(a1);
    const cp2x = x2 + k * r * Math.sin(a2), cp2y = y2 - k * r * Math.cos(a2);
    if (a1 === startAngle) {
      path += `M ${f(x1)} ${f(y1)} `;
    }
    path += `C ${f(cp1x)} ${f(cp1y)} ${f(cp2x)} ${f(cp2y)} ${f(x2)} ${f(y2)} `;
    a1 = a2;
  }
  return path;
}

function f(n: number): string { return n.toFixed(2); }

function renderDonutChart(tableData: any[]): string {
  const totals = tableData.map((r: any) => ({
    label: r.region,
    value: parseFloat(r.q1.replace(/[$,]/g, '')) + parseFloat(r.q2.replace(/[$,]/g, ''))
         + parseFloat(r.q3.replace(/[$,]/g, '')) + parseFloat(r.q4.replace(/[$,]/g, '')),
  }));
  const sum = totals.reduce((s, t) => s + t.value, 0);

  const cx = 55, cy = 75, r = 48, innerR = 28;
  let svg = '';
  let angle = -Math.PI / 2; // start at top

  totals.forEach((t, i) => {
    const sliceAngle = (t.value / sum) * Math.PI * 2;
    if (sliceAngle < 0.01) { angle += sliceAngle; return; }
    const endAngle = angle + sliceAngle;
    const ix2 = cx + innerR * Math.cos(endAngle), iy2 = cy + innerR * Math.sin(endAngle);
    const innerPath = arcPathReverse(cx, cy, innerR, angle, endAngle);
    const fullD = arcPath(cx, cy, r, angle, endAngle) + `L ${f(ix2)} ${f(iy2)} ` + innerPath + 'Z';
    svg += `<path d="${fullD}" fill="${CHART_COLORS[i % CHART_COLORS.length]}"/>`;
    angle = endAngle;
  });

  return svg;
}

/** Reverse arc: draws from endAngle back to startAngle using line-to after the first point. */
function arcPathReverse(cx: number, cy: number, r: number, startAngle: number, endAngle: number): string {
  const step = Math.PI / 2;
  const segments: { a1: number; a2: number }[] = [];
  let a = startAngle;
  while (a < endAngle - 0.001) {
    const a2 = Math.min(a + step, endAngle);
    segments.push({ a1: a, a2 });
    a = a2;
  }
  // Reverse and draw each segment backwards
  let path = '';
  for (let i = segments.length - 1; i >= 0; i--) {
    const { a1, a2 } = segments[i];
    const alpha = a2 - a1;
    const k = (4 / 3) * Math.tan(alpha / 4);
    const x1 = cx + r * Math.cos(a1), y1 = cy + r * Math.sin(a1);
    const x2 = cx + r * Math.cos(a2), y2 = cy + r * Math.sin(a2);
    const cp1x = x1 - k * r * Math.sin(a1), cp1y = y1 + k * r * Math.cos(a1);
    const cp2x = x2 + k * r * Math.sin(a2), cp2y = y2 - k * r * Math.cos(a2);
    // Draw from (x2,y2) to (x1,y1) with swapped control points
    path += `C ${f(cp2x)} ${f(cp2y)} ${f(cp1x)} ${f(cp1y)} ${f(x1)} ${f(y1)} `;
  }
  return path;
}

function renderLineChart(tableData: any[]): string {
  const quarters = ['q1', 'q2', 'q3', 'q4'];
  const qTotals = quarters.map(q =>
    tableData.reduce((sum: number, r: any) => sum + parseFloat(r[q].replace(/[$,]/g, '')), 0)
  );
  const minVal = Math.min(...qTotals) * 0.9;
  const maxVal = Math.max(...qTotals) * 1.05;

  const w = 484, h = 140;
  const padL = 16, padR = 16, padT = 12, padB = 24;
  const plotW = w - padL - padR, plotH = h - padT - padB;

  let svg = '';
  // Grid lines
  for (let i = 0; i <= 4; i++) {
    const y = padT + (plotH / 4) * i;
    svg += `<line x1="${padL}" y1="${y}" x2="${w - padR}" y2="${y}" stroke="#e2e8f0" stroke-width="0.5"/>`;
  }

  // Plot points
  const points = qTotals.map((v, i) => ({
    x: padL + (plotW / (quarters.length - 1)) * i,
    y: padT + plotH - ((v - minVal) / (maxVal - minVal)) * plotH,
  }));

  // Fill area under the line
  const areaPath = `M ${f(points[0].x)} ${f(padT + plotH)} L ${points.map(p => `${f(p.x)} ${f(p.y)}`).join(' L ')} L ${f(points[points.length - 1].x)} ${f(padT + plotH)} Z`;
  svg += `<path d="${areaPath}" fill="#3b82f6" fill-opacity="0.08"/>`;

  // Line
  const linePath = points.map((p, i) => `${i === 0 ? 'M' : 'L'} ${f(p.x)} ${f(p.y)}`).join(' ');
  svg += `<path d="${linePath}" fill="none" stroke="#3b82f6" stroke-width="2"/>`;

  // Dots
  points.forEach(p => {
    svg += `<circle cx="${f(p.x)}" cy="${f(p.y)}" r="3.5" fill="#ffffff" stroke="#3b82f6" stroke-width="2"/>`;
  });

  // Quarter markers on x-axis
  points.forEach((p, i) => {
    svg += `<line x1="${f(p.x)}" y1="${f(padT + plotH)}" x2="${f(p.x)}" y2="${f(padT + plotH + 4)}" stroke="#94a3b8" stroke-width="1"/>`;
  });

  return svg;
}

// ── Template ─────────────────────────────────────────────────────────

export default function Report(data: any) {
  const tableData = data.sections[1].tableData;

  return (
    <Document title={data.title} author={data.author}>
      {/* Cover Page */}
      <Page size="Letter" margin={72}>
        <View style={{ flexGrow: 1, justifyContent: 'center' }}>
          <View style={{ backgroundColor: '#0f172a', padding: 32, borderRadius: 4, marginBottom: 32 }}>
            <Text style={{ fontSize: 32, fontWeight: 700, color: '#ffffff' }}>{data.title}</Text>
            <Text style={{ fontSize: 14, color: '#94a3b8', marginTop: 12 }}>{data.subtitle}</Text>
          </View>
          <View style={{ flexDirection: 'row', justifyContent: 'space-between', marginTop: 24 }}>
            <View>
              <Text style={{ fontSize: 10, color: '#64748b' }}>Prepared by</Text>
              <Text style={{ fontSize: 12, fontWeight: 700, color: '#1e293b', marginTop: 4 }}>{data.author}</Text>
              <Text style={{ fontSize: 10, color: '#64748b', marginTop: 2 }}>{data.department}</Text>
            </View>
            <View style={{ alignItems: 'flex-end' }}>
              <Text style={{ fontSize: 10, color: '#64748b' }}>Date</Text>
              <Text style={{ fontSize: 12, fontWeight: 700, color: '#1e293b', marginTop: 4 }}>{data.date}</Text>
              <Text style={{ fontSize: 10, color: '#64748b', marginTop: 2 }}>{data.classification}</Text>
            </View>
          </View>
        </View>
      </Page>

      {/* Content Pages */}
      <Page size="Letter" margin={54}>
        <Fixed position="header">
          <View style={styles.headerBar}>
            <Text style={styles.headerFooterText}>{data.company}</Text>
            <Text style={styles.headerFooterText}>{data.title}</Text>
          </View>
        </Fixed>

        <Fixed position="footer">
          <View style={styles.footerBar}>
            <Text style={styles.headerFooterText}>{data.classification}</Text>
            <Text style={styles.headerFooterText}>Page {'{{pageNumber}}'} of {'{{totalPages}}'}</Text>
          </View>
        </Fixed>

        {/* Table of Contents */}
        <Text style={styles.sectionTitle}>Table of Contents</Text>
        {data.sections.map((section: any, i: number) => (
          <View key={i} href={`#${section.title}`} style={{ flexDirection: 'row', justifyContent: 'space-between', paddingVertical: 6, borderBottomWidth: 1, borderColor: '#f1f5f9' }}>
            <Text style={{ fontSize: 10, color: '#2563eb', textDecoration: 'underline' }}>{i + 1}. {section.title}</Text>
          </View>
        ))}

        <PageBreak />

        {/* Executive Summary */}
        <Text bookmark={data.sections[0].title} style={styles.sectionTitle}>1. {data.sections[0].title}</Text>
        {data.sections[0].paragraphs.map((p: string, i: number) => (
          <Text key={i} style={styles.bodyText}>{p}</Text>
        ))}

        {/* Key Metrics */}
        {data.keyMetrics && (
          <View style={{ flexDirection: 'row', gap: 12, marginTop: 8, marginBottom: 24 }}>
            {data.keyMetrics.map((metric: any, i: number) => (
              <View key={i} style={{ flexGrow: 1, padding: 16, backgroundColor: '#f8fafc', borderRadius: 4, borderWidth: 1, borderColor: '#e2e8f0' }}>
                <Text style={{ fontSize: 20, fontWeight: 700, color: '#0f172a' }}>{metric.value}</Text>
                <Text style={{ fontSize: 9, color: '#64748b', marginTop: 4 }}>{metric.label}</Text>
              </View>
            ))}
          </View>
        )}

        <PageBreak />

        {/* Data Section */}
        <Text bookmark={data.sections[1].title} style={styles.sectionTitle}>2. {data.sections[1].title}</Text>
        <Text style={styles.introText}>{data.sections[1].intro}</Text>

        <Table columns={[
          { width: { fraction: 0.28 } },
          { width: { fraction: 0.18 } },
          { width: { fraction: 0.18 } },
          { width: { fraction: 0.18 } },
          { width: { fraction: 0.18 } }
        ]}>
          <Row header style={{ backgroundColor: '#0f172a' }}>
            <Cell style={styles.tableHeaderCell}><Text style={{ fontSize: 9, fontWeight: 700, color: '#ffffff' }}>Region</Text></Cell>
            <Cell style={styles.tableHeaderCell}><Text style={styles.tableHeaderText}>Q1</Text></Cell>
            <Cell style={styles.tableHeaderCell}><Text style={styles.tableHeaderText}>Q2</Text></Cell>
            <Cell style={styles.tableHeaderCell}><Text style={styles.tableHeaderText}>Q3</Text></Cell>
            <Cell style={styles.tableHeaderCell}><Text style={styles.tableHeaderText}>Q4</Text></Cell>
          </Row>
          {tableData.map((row: any, i: number) => (
            <Row key={i} style={{ backgroundColor: i % 2 === 0 ? '#ffffff' : '#f8fafc' }}>
              <Cell style={styles.tableCell}><Text style={{ fontSize: 9, color: '#334155', fontWeight: 700 }}>{row.region}</Text></Cell>
              <Cell style={styles.tableCell}><Text style={styles.tableCellText}>{row.q1}</Text></Cell>
              <Cell style={styles.tableCell}><Text style={styles.tableCellText}>{row.q2}</Text></Cell>
              <Cell style={styles.tableCell}><Text style={styles.tableCellText}>{row.q3}</Text></Cell>
              <Cell style={styles.tableCell}><Text style={styles.tableCellText}>{row.q4}</Text></Cell>
            </Row>
          ))}
        </Table>

        <PageBreak />

        {/* Visual Analysis */}
        <Text bookmark={data.sections[2].title} style={styles.sectionTitle}>3. {data.sections[2].title}</Text>
        <Text style={styles.introText}>{data.sections[2].intro}</Text>

        <View style={{ flexDirection: 'row', gap: 16, marginBottom: 24 }}>
          <View style={{ flexGrow: 1 }}>
            <Text style={styles.chartTitle}>Revenue by Region</Text>
            <View style={{ backgroundColor: '#f8fafc', borderRadius: 4, borderWidth: 1, borderColor: '#e2e8f0', padding: 8 }}>
              <Svg width={230} height={150} viewBox="0 0 230 150" content={renderBarChart(tableData)} />
              <View style={{ gap: 3, marginTop: 8 }}>
                {tableData.map((row: any, i: number) => (
                  <View key={i} style={{ flexDirection: 'row', alignItems: 'center', gap: 4 }}>
                    <View style={{ width: 8, height: 8, borderRadius: 2, backgroundColor: CHART_COLORS[i % CHART_COLORS.length] }} />
                    <Text style={{ fontSize: 7, color: '#64748b' }}>{row.region}</Text>
                  </View>
                ))}
              </View>
            </View>
          </View>
          <View style={{ flexGrow: 1 }}>
            <Text style={styles.chartTitle}>Market Share</Text>
            <View style={{ backgroundColor: '#f8fafc', borderRadius: 4, borderWidth: 1, borderColor: '#e2e8f0', padding: 8 }}>
              <View style={{ alignItems: 'center' }}>
                <Svg width={110} height={150} viewBox="0 0 110 150" content={renderDonutChart(tableData)} />
              </View>
              <View style={{ gap: 3, marginTop: 8 }}>
                {tableData.map((row: any, i: number) => {
                  const total = tableData.reduce((s: number, r: any) =>
                    s + parseFloat(r.q1.replace(/[$,]/g, '')) + parseFloat(r.q2.replace(/[$,]/g, ''))
                      + parseFloat(r.q3.replace(/[$,]/g, '')) + parseFloat(r.q4.replace(/[$,]/g, '')), 0);
                  const rowTotal = parseFloat(row.q1.replace(/[$,]/g, '')) + parseFloat(row.q2.replace(/[$,]/g, ''))
                                 + parseFloat(row.q3.replace(/[$,]/g, '')) + parseFloat(row.q4.replace(/[$,]/g, ''));
                  const pct = ((rowTotal / total) * 100).toFixed(0);
                  return (
                    <View key={i} style={{ flexDirection: 'row', alignItems: 'center', gap: 3 }}>
                      <View style={{ width: 8, height: 8, borderRadius: 2, backgroundColor: CHART_COLORS[i % CHART_COLORS.length] }} />
                      <Text style={{ fontSize: 7, color: '#64748b' }}>{row.region} {pct}%</Text>
                    </View>
                  );
                })}
              </View>
            </View>
          </View>
        </View>

        <Text style={styles.chartTitle}>Quarterly Growth Trend</Text>
        <View style={{ backgroundColor: '#f8fafc', borderRadius: 4, borderWidth: 1, borderColor: '#e2e8f0', paddingVertical: 12, marginBottom: 24 }}>
          <Svg width={484} height={140} viewBox="0 0 484 140" content={renderLineChart(tableData)} />
          <View style={{ position: 'relative', height: 14, marginTop: 4 }}>
            {['Q1', 'Q2', 'Q3', 'Q4'].map((q, i) => (
              <Text key={i} style={{ position: 'absolute', left: 16 + (452 / 3) * i - 12, width: 24, fontSize: 8, color: '#64748b', textAlign: 'center' }}>{q}</Text>
            ))}
          </View>
        </View>

        <PageBreak />

        {/* Recommendations */}
        <Text bookmark={data.sections[3].title} style={styles.sectionTitle}>4. {data.sections[3].title}</Text>
        <Text style={styles.introText}>{data.sections[3].intro}</Text>

        {data.sections[3].items.map((item: any, i: number) => (
          <View key={i} style={styles.recCard}>
            <View style={styles.recBadge}>
              <Text style={styles.recBadgeText}>{i + 1}</Text>
            </View>
            <View style={{ flexGrow: 1, flexShrink: 1 }}>
              <Text style={styles.recTitle}>{item.title}</Text>
              <Text style={styles.recBody}>{item.description}</Text>
              <View style={{ flexDirection: 'row', gap: 16, marginTop: 8 }}>
                <View style={{ flexDirection: 'row', gap: 4 }}>
                  <Text style={styles.recLabel}>Priority:</Text>
                  <Text style={styles.recValue}>{item.priority}</Text>
                </View>
                <View style={{ flexDirection: 'row', gap: 4 }}>
                  <Text style={styles.recLabel}>Timeline:</Text>
                  <Text style={styles.recValue}>{item.timeline}</Text>
                </View>
              </View>
            </View>
          </View>
        ))}
      </Page>
    </Document>
  );
}
