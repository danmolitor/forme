import { Document, Page, View, Text, Table, Row, Cell, Fixed, PageBreak } from '@forme/react';

export default function Report(data: any) {
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
          <View style={{ flexDirection: 'row', justifyContent: 'space-between', paddingBottom: 8, borderWidth: { top: 0, right: 0, bottom: 1, left: 0 }, borderColor: '#e2e8f0', marginBottom: 16 }}>
            <Text style={{ fontSize: 8, color: '#94a3b8' }}>{data.company}</Text>
            <Text style={{ fontSize: 8, color: '#94a3b8' }}>{data.title}</Text>
          </View>
        </Fixed>

        <Fixed position="footer">
          <View style={{ flexDirection: 'row', justifyContent: 'space-between', paddingTop: 8, borderWidth: { top: 1, right: 0, bottom: 0, left: 0 }, borderColor: '#e2e8f0' }}>
            <Text style={{ fontSize: 8, color: '#94a3b8' }}>{data.classification}</Text>
            <Text style={{ fontSize: 8, color: '#94a3b8' }}>Page {'{{pageNumber}}'} of {'{{totalPages}}'}</Text>
          </View>
        </Fixed>

        {/* Table of Contents */}
        <Text style={{ fontSize: 20, fontWeight: 700, color: '#0f172a', marginBottom: 16 }}>Table of Contents</Text>
        {data.sections.map((section: any, i: number) => (
          <View key={i} style={{ flexDirection: 'row', justifyContent: 'space-between', paddingTop: 6, paddingBottom: 6, borderWidth: { top: 0, right: 0, bottom: 1, left: 0 }, borderColor: '#f1f5f9' }}>
            <Text style={{ fontSize: 10, color: '#334155' }}>{i + 1}. {section.title}</Text>
          </View>
        ))}

        <PageBreak />

        {/* Executive Summary */}
        <Text style={{ fontSize: 20, fontWeight: 700, color: '#0f172a', marginBottom: 12 }}>1. {data.sections[0].title}</Text>
        {data.sections[0].paragraphs.map((p: string, i: number) => (
          <Text key={i} style={{ fontSize: 10, color: '#334155', lineHeight: 1.6, marginBottom: 12 }}>{p}</Text>
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
        <Text style={{ fontSize: 20, fontWeight: 700, color: '#0f172a', marginBottom: 12 }}>2. {data.sections[1].title}</Text>
        <Text style={{ fontSize: 10, color: '#334155', lineHeight: 1.6, marginBottom: 16 }}>{data.sections[1].intro}</Text>

        <Table columns={[
          { width: { fraction: 0.28 } },
          { width: { fraction: 0.18 } },
          { width: { fraction: 0.18 } },
          { width: { fraction: 0.18 } },
          { width: { fraction: 0.18 } }
        ]}>
          <Row header style={{ backgroundColor: '#0f172a' }}>
            <Cell style={{ padding: 8 }}><Text style={{ fontSize: 9, fontWeight: 700, color: '#ffffff' }}>Region</Text></Cell>
            <Cell style={{ padding: 8 }}><Text style={{ fontSize: 9, fontWeight: 700, color: '#ffffff', textAlign: 'right' }}>Q1</Text></Cell>
            <Cell style={{ padding: 8 }}><Text style={{ fontSize: 9, fontWeight: 700, color: '#ffffff', textAlign: 'right' }}>Q2</Text></Cell>
            <Cell style={{ padding: 8 }}><Text style={{ fontSize: 9, fontWeight: 700, color: '#ffffff', textAlign: 'right' }}>Q3</Text></Cell>
            <Cell style={{ padding: 8 }}><Text style={{ fontSize: 9, fontWeight: 700, color: '#ffffff', textAlign: 'right' }}>Q4</Text></Cell>
          </Row>
          {data.sections[1].tableData.map((row: any, i: number) => (
            <Row key={i} style={{ backgroundColor: i % 2 === 0 ? '#ffffff' : '#f8fafc' }}>
              <Cell style={{ padding: 8 }}><Text style={{ fontSize: 9, color: '#334155', fontWeight: 700 }}>{row.region}</Text></Cell>
              <Cell style={{ padding: 8 }}><Text style={{ fontSize: 9, color: '#334155', textAlign: 'right' }}>{row.q1}</Text></Cell>
              <Cell style={{ padding: 8 }}><Text style={{ fontSize: 9, color: '#334155', textAlign: 'right' }}>{row.q2}</Text></Cell>
              <Cell style={{ padding: 8 }}><Text style={{ fontSize: 9, color: '#334155', textAlign: 'right' }}>{row.q3}</Text></Cell>
              <Cell style={{ padding: 8 }}><Text style={{ fontSize: 9, color: '#334155', textAlign: 'right' }}>{row.q4}</Text></Cell>
            </Row>
          ))}
        </Table>

        <PageBreak />

        {/* Visual Analysis (Chart Placeholders) */}
        <Text style={{ fontSize: 20, fontWeight: 700, color: '#0f172a', marginBottom: 12 }}>3. {data.sections[2].title}</Text>
        <Text style={{ fontSize: 10, color: '#334155', lineHeight: 1.6, marginBottom: 16 }}>{data.sections[2].intro}</Text>

        <View style={{ flexDirection: 'row', gap: 16, marginBottom: 24 }}>
          <View style={{ flexGrow: 1 }}>
            <View style={{ height: 160, backgroundColor: '#dbeafe', borderRadius: 4, justifyContent: 'center', alignItems: 'center', borderWidth: 1, borderColor: '#93c5fd' }}>
              <Text style={{ fontSize: 10, color: '#2563eb' }}>Revenue by Region</Text>
              <Text style={{ fontSize: 8, color: '#60a5fa', marginTop: 4 }}>(Bar Chart)</Text>
            </View>
          </View>
          <View style={{ flexGrow: 1 }}>
            <View style={{ height: 160, backgroundColor: '#fce7f3', borderRadius: 4, justifyContent: 'center', alignItems: 'center', borderWidth: 1, borderColor: '#f9a8d4' }}>
              <Text style={{ fontSize: 10, color: '#be185d' }}>Market Share</Text>
              <Text style={{ fontSize: 8, color: '#ec4899', marginTop: 4 }}>(Pie Chart)</Text>
            </View>
          </View>
        </View>

        <View style={{ height: 160, backgroundColor: '#ecfdf5', borderRadius: 4, justifyContent: 'center', alignItems: 'center', borderWidth: 1, borderColor: '#6ee7b7', marginBottom: 24 }}>
          <Text style={{ fontSize: 10, color: '#047857' }}>Quarterly Growth Trend</Text>
          <Text style={{ fontSize: 8, color: '#34d399', marginTop: 4 }}>(Line Chart)</Text>
        </View>

        <PageBreak />

        {/* Recommendations */}
        <Text style={{ fontSize: 20, fontWeight: 700, color: '#0f172a', marginBottom: 12 }}>4. {data.sections[3].title}</Text>
        <Text style={{ fontSize: 10, color: '#334155', lineHeight: 1.6, marginBottom: 16 }}>{data.sections[3].intro}</Text>

        {data.sections[3].items.map((item: any, i: number) => (
          <View key={i} style={{ flexDirection: 'row', gap: 24, marginBottom: 16, padding: 16, backgroundColor: '#f8fafc', borderRadius: 4, borderWidth: { top: 0, right: 0, bottom: 0, left: 3 }, borderColor: '#0f172a' }}>
            <View style={{ width: 24, height: 24, backgroundColor: '#0f172a', borderRadius: 12, justifyContent: 'center', alignItems: 'center' }}>
              <Text style={{ fontSize: 10, fontWeight: 700, color: '#ffffff', lineHeight: 1.2 }}>{i + 1}</Text>
            </View>
            <View style={{ flexGrow: 1, flexShrink: 1 }}>
              <Text style={{ fontSize: 11, fontWeight: 700, color: '#0f172a', marginBottom: 4 }}>{item.title}</Text>
              <Text style={{ fontSize: 9, color: '#475569', lineHeight: 1.5 }}>{item.description}</Text>
              <View style={{ flexDirection: 'row', gap: 16, marginTop: 8 }}>
                <View style={{ flexDirection: 'row', gap: 4 }}>
                  <Text style={{ fontSize: 8, fontWeight: 700, color: '#64748b' }}>Priority:</Text>
                  <Text style={{ fontSize: 8, color: '#334155' }}>{item.priority}</Text>
                </View>
                <View style={{ flexDirection: 'row', gap: 4 }}>
                  <Text style={{ fontSize: 8, fontWeight: 700, color: '#64748b' }}>Timeline:</Text>
                  <Text style={{ fontSize: 8, color: '#334155' }}>{item.timeline}</Text>
                </View>
              </View>
            </View>
          </View>
        ))}
      </Page>
    </Document>
  );
}
