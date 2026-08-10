import { Document, Page, Text, View } from '@formepdf/react';
import type { FontRegistration } from '@formepdf/react';

export interface Props {
  fonts?: FontRegistration[];
  bodyFamily?: string;
}

/** TSX twin of fonts.svelte for cross-adapter parity tests. */
export default function Fonts({
  fonts = [
    { family: 'Inter', src: 'fonts/Inter-Regular.ttf' },
    { family: 'Inter', src: 'fonts/Inter-Bold.ttf', fontWeight: 700 },
    { family: 'Custom', src: 'data:font/ttf;base64,AAAA', fontStyle: 'italic' },
    { family: 'Bytes', src: new Uint8Array([0x00, 0x01, 0x02, 0x80, 0xfe, 0xff]) },
  ],
  bodyFamily = 'Inter',
}: Props) {
  return (
    <Document title="Fonts" fonts={fonts} style={{ fontFamily: 'Inter', fontSize: 11 }}>
      <Page size="A4" margin={40}>
        <View style={{ flexDirection: 'column', gap: 6 }}>
          <Text style={{ fontSize: 20, fontWeight: 700 }}>Custom fonts</Text>
          <Text style={{ fontFamily: bodyFamily }}>Body text in the registered family.</Text>
          <Text style={{ fontFamily: 'Custom', fontStyle: 'italic' }}>Italic custom face.</Text>
        </View>
      </Page>
    </Document>
  );
}
