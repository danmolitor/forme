import { Document, Page, View, Text } from '@formepdf/react';

export interface Props {
  name?: string;
  items?: string[];
  showFooter?: boolean;
}

/** TSX twin of hello-world.svelte for cross-adapter parity tests. */
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
