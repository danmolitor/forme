import { Document, Page, View, Text, Image, Svg, Table, Row, Cell, Fixed, PageBreak, StyleSheet, H1, H2, H3 } from '@formepdf/react';
import { tw } from '@formepdf/tailwind';

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
          <View style={tw("p-8 bg-slate-900 rounded mb-8")}>
            <H1 style={{ ...tw("text-[32px] font-bold text-white"), marginTop: 0, marginBottom: 0 }}>{data.title}</H1>
            <Text style={tw("text-[14px] text-slate-400 mt-3")}>{data.subtitle}</Text>
          </View>
          <View>
          </View>
          <View style={tw("flex-row justify-between mt-6")}>
            <View>
              <Text style={tw("text-[10px] text-slate-500")}>Prepared by</Text>
              <Text style={tw("text-[12px] font-bold text-slate-800 mt-1")}>{data.author}</Text>
              <Text style={tw("text-[10px] text-slate-500 mt-0.5")}>{data.department}</Text>
            </View>
            <View style={tw("items-end")}>
              <Text style={tw("text-[10px] text-slate-500")}>Date</Text>
              <Text style={tw("text-[12px] font-bold text-slate-800 mt-1")}>{data.date}</Text>
              <Text style={tw("text-[10px] text-slate-500 mt-0.5")}>{data.classification}</Text>
            </View>
          </View>
        </View>
      </Page>

      {/* Content Pages */}
      <Page size="Letter" margin={54}>
        <Fixed position="header">
          <View style={tw("flex-row justify-between pb-2 border-b border-slate-200 mb-4")}>
            <Text style={tw("text-[8px] text-slate-400")}>{data.company}</Text>
            <Text style={tw("text-[8px] text-slate-400")}>{data.title}</Text>
          </View>
        </Fixed>

        <Fixed position="footer">
          <View style={tw("flex-row justify-between pt-2 border-t border-slate-200")}>
            <Text style={tw("text-[8px] text-slate-400")}>{data.classification}</Text>
            <Text style={tw("text-[8px] text-slate-400")}>Page {'{{pageNumber}}'} of {'{{totalPages}}'}</Text>
          </View>
        </Fixed>

        {/* Table of Contents */}
        <H2 style={{ ...tw("text-xl font-bold text-slate-900 mb-3"), marginTop: 0 }}>Table of Contents</H2>
        {data.sections.map((section: any, i: number) => (
          <View key={i} href={`#${section.title}`} style={{ ...tw("flex-row justify-between py-1.5"), borderBottomWidth: 1, borderColor: '#f1f5f9' }}>
            <Text style={tw("text-[10px] text-blue-600 underline")}>{i + 1}. {section.title}</Text>
          </View>
        ))}

        <PageBreak />

        {/* Executive Summary */}
        <H2 bookmark={data.sections[0].title} style={{ ...tw("text-xl font-bold text-slate-900 mb-3"), marginTop: 0 }}>1. {data.sections[0].title}</H2>
        {data.sections[0].paragraphs.map((p: string, i: number) => (
          <Text key={i} style={tw("text-[10px] text-slate-700 leading-[1.6] mb-3")}>{p}</Text>
        ))}

        {/* Key Metrics */}
        {data.keyMetrics && (
          <View style={tw("flex-row gap-3 mt-2 mb-6")}>
            {data.keyMetrics.map((metric: any, i: number) => (
              <View key={i} style={tw("flex-1 p-4 bg-slate-50 rounded border border-slate-200")}>
                <Text style={tw("text-xl font-bold text-slate-900")}>{metric.value}</Text>
                <Text style={tw("text-[9px] text-slate-500 mt-1")}>{metric.label}</Text>
              </View>
            ))}
          </View>
        )}

        <PageBreak />

        {/* Data Section */}
        <H2 bookmark={data.sections[1].title} style={{ ...tw("text-xl font-bold text-slate-900 mb-3"), marginTop: 0 }}>2. {data.sections[1].title}</H2>
        <Text style={tw("text-[10px] text-slate-700 leading-[1.6] mb-4")}>{data.sections[1].intro}</Text>

        <Table columns={[
          { width: { fraction: 0.28 } },
          { width: { fraction: 0.18 } },
          { width: { fraction: 0.18 } },
          { width: { fraction: 0.18 } },
          { width: { fraction: 0.18 } }
        ]}>
          <Row header style={tw("bg-slate-900")}>
            <Cell style={tw("p-2")}><Text style={tw("text-[9px] font-bold text-white")}>Region</Text></Cell>
            <Cell style={tw("p-2")}><Text style={tw("text-[9px] font-bold text-white text-right")}>Q1</Text></Cell>
            <Cell style={tw("p-2")}><Text style={tw("text-[9px] font-bold text-white text-right")}>Q2</Text></Cell>
            <Cell style={tw("p-2")}><Text style={tw("text-[9px] font-bold text-white text-right")}>Q3</Text></Cell>
            <Cell style={tw("p-2")}><Text style={tw("text-[9px] font-bold text-white text-right")}>Q4</Text></Cell>
          </Row>
          {tableData.map((row: any, i: number) => (
            <Row key={i} style={{ backgroundColor: i % 2 === 0 ? '#ffffff' : '#f8fafc' }}>
              <Cell style={tw("p-2")}><Text style={tw("text-[9px] text-slate-700 font-bold")}>{row.region}</Text></Cell>
              <Cell style={tw("p-2")}><Text style={tw("text-[9px] text-slate-700 text-right")}>{row.q1}</Text></Cell>
              <Cell style={tw("p-2")}><Text style={tw("text-[9px] text-slate-700 text-right")}>{row.q2}</Text></Cell>
              <Cell style={tw("p-2")}><Text style={tw("text-[9px] text-slate-700 text-right")}>{row.q3}</Text></Cell>
              <Cell style={tw("p-2")}><Text style={tw("text-[9px] text-slate-700 text-right")}>{row.q4}</Text></Cell>
            </Row>
          ))}
        </Table>

        <PageBreak />

        {/* Visual Analysis */}
        <H2 bookmark={data.sections[2].title} style={{ ...tw("text-xl font-bold text-slate-900 mb-3"), marginTop: 0 }}>3. {data.sections[2].title}</H2>
        <Text style={tw("text-[10px] text-slate-700 leading-[1.6] mb-4")}>{data.sections[2].intro}</Text>

        <View style={tw("flex-row gap-4 mb-6")}>
          <View style={tw("flex-1")}>
            <H3 style={{ ...tw("text-[10px] font-bold text-slate-700 mb-1.5"), marginTop: 0 }}>Revenue by Region</H3>
            <View style={tw("bg-slate-50 rounded border border-slate-200 p-2")}>
              <Svg width={230} height={150} viewBox="0 0 230 150" content={renderBarChart(tableData)} />
              <View style={tw("gap-[3] mt-2")}>
                {tableData.map((row: any, i: number) => (
                  <View key={i} style={tw("flex-row items-center gap-1")}>
                    <View style={{ width: 8, height: 8, borderRadius: 2, backgroundColor: CHART_COLORS[i % CHART_COLORS.length] }} />
                    <Text style={tw("text-[7px] text-slate-500")}>{row.region}</Text>
                  </View>
                ))}
              </View>
            </View>
          </View>
          <View style={tw("flex-1")}>
            <H3 style={{ ...tw("text-[10px] font-bold text-slate-700 mb-1.5"), marginTop: 0 }}>Market Share</H3>
            <View style={tw("bg-slate-50 rounded border border-slate-200 p-2")}>
              <View style={tw("items-center")}>
                <Svg width={110} height={150} viewBox="0 0 110 150" content={renderDonutChart(tableData)} />
              </View>
              <View style={tw("gap-[3] mt-2")}>
                {tableData.map((row: any, i: number) => {
                  const total = tableData.reduce((s: number, r: any) =>
                    s + parseFloat(r.q1.replace(/[$,]/g, '')) + parseFloat(r.q2.replace(/[$,]/g, ''))
                      + parseFloat(r.q3.replace(/[$,]/g, '')) + parseFloat(r.q4.replace(/[$,]/g, '')), 0);
                  const rowTotal = parseFloat(row.q1.replace(/[$,]/g, '')) + parseFloat(row.q2.replace(/[$,]/g, ''))
                                 + parseFloat(row.q3.replace(/[$,]/g, '')) + parseFloat(row.q4.replace(/[$,]/g, ''));
                  const pct = ((rowTotal / total) * 100).toFixed(0);
                  return (
                    <View key={i} style={tw("flex-row items-center gap-[3]")}>
                      <View style={{ width: 8, height: 8, borderRadius: 2, backgroundColor: CHART_COLORS[i % CHART_COLORS.length] }} />
                      <Text style={tw("text-[7px] text-slate-500")}>{row.region} {pct}%</Text>
                    </View>
                  );
                })}
              </View>
            </View>
          </View>
        </View>

        <H3 style={{ ...tw("text-[10px] font-bold text-slate-700 mb-1.5"), marginTop: 0 }}>Quarterly Growth Trend</H3>
        <View style={tw("bg-slate-50 rounded border border-slate-200 py-3 mb-6")}>
          <Svg width={484} height={140} viewBox="0 0 484 140" content={renderLineChart(tableData)} />
          <View style={{ position: 'relative', height: 14, marginTop: 4 }}>
            {['Q1', 'Q2', 'Q3', 'Q4'].map((q, i) => (
              <Text key={i} style={{ position: 'absolute', left: 16 + (452 / 3) * i - 12, width: 24, fontSize: 8, color: '#64748b', textAlign: 'center' }}>{q}</Text>
            ))}
          </View>
        </View>

        <PageBreak />

        {/* Recommendations */}
        <H2 bookmark={data.sections[3].title} style={{ ...tw("text-xl font-bold text-slate-900 mb-3"), marginTop: 0 }}>4. {data.sections[3].title}</H2>
        <Text style={tw("text-[10px] text-slate-700 leading-[1.6] mb-4")}>{data.sections[3].intro}</Text>

        {data.sections[3].items.map((item: any, i: number) => (
          <View key={i} style={{ ...tw("flex-row gap-6 mb-4 p-4 bg-slate-50 rounded"), borderLeftWidth: 3, borderColor: '#0f172a' }}>
            <View style={tw("w-6 h-6 bg-slate-900 rounded-[12] justify-center items-center")}>
              <Text style={tw("text-[10px] font-bold text-white leading-[1.2]")}>{i + 1}</Text>
            </View>
            <View style={tw("flex-1 flex-shrink")}>
              <H3 style={{ ...tw("text-[11px] font-bold text-slate-900 mb-1"), marginTop: 0 }}>{item.title}</H3>
              <Text style={tw("text-[9px] text-slate-600 leading-[1.5]")}>{item.description}</Text>
              <View style={tw("flex-row gap-4 mt-2")}>
                <View style={tw("flex-row gap-1")}>
                  <Text style={tw("text-[8px] font-bold text-slate-500")}>Priority:</Text>
                  <Text style={tw("text-[8px] text-slate-700")}>{item.priority}</Text>
                </View>
                <View style={tw("flex-row gap-1")}>
                  <Text style={tw("text-[8px] font-bold text-slate-500")}>Timeline:</Text>
                  <Text style={tw("text-[8px] text-slate-700")}>{item.timeline}</Text>
                </View>
              </View>
            </View>
          </View>
        ))}
      </Page>
    </Document>
  );
}
