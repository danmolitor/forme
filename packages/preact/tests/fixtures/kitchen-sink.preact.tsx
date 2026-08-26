/** @jsxImportSource preact */
import { Document, Page, View, Text, H1, H2, Strong, Em, OrderedList, ListItem } from '@formepdf/preact';

export interface Props {
  discount?: number;
}

export default function KitchenSink({ discount = 10 }: Props) {
  return (
    <Document title="Kitchen Sink" author="Forme">
      <Page size="Letter" margin={[36, 48, 36, 48]}>
        <H1 style={{ marginBottom: 16 }}>Report</H1>
        <View style={{ flexDirection: 'row', gap: 12, marginBottom: 12 }}>
          <View style={{ flex: 1, border: '1px solid #ccc', padding: '8 16' }}>
            <Text>
              Discount: <Strong>{discount}%</Strong> applied to <Em>eligible</Em> items.
            </Text>
          </View>
          <View style={{ flex: 2, backgroundColor: '#f5f5f5', padding: 8 }}>
            <H2>Notes</H2>
            <Text>{`Line one.\nLine two.\nLine three.`}</Text>
          </View>
        </View>
        <OrderedList>
          <ListItem>First</ListItem>
          <ListItem>Second</ListItem>
          <ListItem>Third</ListItem>
        </OrderedList>
      </Page>
    </Document>
  );
}
