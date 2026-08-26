/** @jsxImportSource preact */
import { Document, Page, View, Text } from '@formepdf/preact';

export interface Props {
  name?: string;
  items?: string[];
  showFooter?: boolean;
}

/** Preact fixture — twin of hello-world.react.tsx for cross-adapter parity. */
export default function HelloWorld({ name = 'World', items = [], showFooter = false }: Props) {
  return (
    <Document title="Hello">
      <Page size="A4" margin={40}>
        <View style={{ flexDirection: 'column', gap: 8 }}>
          <Text style={{ fontSize: 24 }}>Hello {name}!</Text>
          {items.map(item => (
            <Text key={item}>Item: {item}</Text>
          ))}
          {showFooter && <Text>The footer</Text>}
        </View>
      </Page>
    </Document>
  );
}
