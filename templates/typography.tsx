import { Document, Page, View, Text } from '@formepdf/react';

export default function Typography(data: any) {
  return (
    <Document title={data.title} lang="en">
      <Page size="Letter" margin={{ top: 48, right: 48, bottom: 54, left: 48 }}>

        {/* Full-width header */}
        <View style={{ marginBottom: 20 }}>
          <Text style={{ fontSize: 26, fontWeight: 700, color: '#1a1a1a', letterSpacing: -0.5 }}>
            {data.title}
          </Text>
          <Text style={{ fontSize: 10, color: '#6b7280', marginTop: 4, letterSpacing: 0.3 }}>
            {data.subtitle}
          </Text>
          <View style={{ height: 0.75, backgroundColor: '#d1d5db', marginTop: 14 }} />
        </View>

        {/* Two-column body */}
        <View style={{ flexDirection: 'row', gap: 24 }}>

          {/* Left column */}
          <View style={{ flex: 1 }}>
            {data.leftColumn.map((paragraph: string, i: number) => (
              <Text key={i} style={{ fontSize: 9, lineHeight: 1.5, color: '#1a1a1a', textAlign: 'justify', hyphens: 'auto', marginTop: i > 0 ? 9 : 0 }}>
                {paragraph}
              </Text>
            ))}
          </View>

          {/* Right column */}
          <View style={{ flex: 1 }}>
            <Text style={{ fontSize: 9, lineHeight: 1.5, color: '#1a1a1a', textAlign: 'justify', hyphens: 'auto' }}>
              {data.rightColumnIntro}
            </Text>

            {/* Pull quote */}
            <View style={{ borderLeftWidth: 2.5, borderLeftColor: '#9ca3af', paddingLeft: 14, paddingVertical: 10, marginVertical: 14 }}>
              <Text style={{ fontSize: 11, fontStyle: 'italic', lineHeight: 1.55, color: '#374151' }}>
                {data.pullQuote.text}
              </Text>
              <Text style={{ fontSize: 8, color: '#6b7280', marginTop: 5, letterSpacing: 0.2 }}>
                — {data.pullQuote.attribution}
              </Text>
            </View>

            {/* Multilingual sections */}
            {data.sections.map((section: any, i: number) => (
              <View key={i} style={{ marginTop: i === 0 ? 4 : 12 }}>
                <Text style={{ fontSize: 10, fontWeight: 700, color: '#1a1a1a', marginBottom: 5 }}>
                  {section.heading}
                </Text>
                <Text style={{ fontSize: 9, lineHeight: 1.5, color: '#1a1a1a', textAlign: 'justify', hyphens: 'auto', lang: section.lang }}>
                  {section.body}
                </Text>
              </View>
            ))}
          </View>
        </View>

      </Page>
    </Document>
  );
}
