import { Document, Page, Text, View } from '@formepdf/react';
import { tw } from '@formepdf/tailwind';

export interface Props {
  total?: string;
}

/** TSX twin of tailwind.svelte for cross-adapter parity tests. */
export default function Tailwind({ total = '$1,280.00' }: Props) {
  return (
    <Document title="Tailwind">
      <Page size="A4" margin={40}>
        <View style={tw('flex-col gap-4 p-6 bg-gray-100 rounded-lg')}>
          <Text style={tw('text-2xl font-bold text-blue-600')}>Utility-class styling</Text>
          <View style={tw('flex-row justify-between items-center border-b pb-2')}>
            <Text style={tw('text-sm text-gray-500 uppercase')}>Total due</Text>
            <Text style={tw('text-lg font-semibold')}>{total}</Text>
          </View>
          <Text style={tw('text-sm text-justify')}>
            Every style on this page comes from tw() utility classes, proving the
            tailwind package needs no adapter-specific work.
          </Text>
        </View>
      </Page>
    </Document>
  );
}
