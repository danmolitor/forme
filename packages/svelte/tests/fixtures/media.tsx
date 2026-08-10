import { Document, Page, View, Text, Image, Svg, QrCode, Barcode } from '@formepdf/react';

export interface Props {
  ticketId?: string;
}

const logo =
  'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==';

/** TSX twin of media.svelte for cross-adapter parity tests. */
export default function MediaFixture({ ticketId = 'TKT-0042' }: Props) {
  return (
    <Document title="Media Parity">
      <Page size="A4" margin={40}>
        <View style={{ flexDirection: 'row', gap: 12, marginBottom: 16 }}>
          <Image
            src={logo}
            width={64}
            height={64}
            alt="Company logo"
            href="https://formepdf.com"
            style={{ borderWidth: 1, borderColor: '#ddd' }}
          />
          <Image src="./assets/photo.jpg" width={120} />
          <Image src="/absolute/banner.png" height={48} />
        </View>

        <Svg
          width={100}
          height={100}
          viewBox="0 0 100 100"
          content={'<rect x="5" y="5" width="90" height="90" fill="#eef" stroke="#00c"/><path d="M10 10 L90 90" stroke="#c00" stroke-width="2"/>'}
          alt="Diagonal line over a square"
          href="https://formepdf.com/svg"
          style={{ marginBottom: 16 }}
        />

        <View style={{ flexDirection: 'row', gap: 24 }}>
          <QrCode data={`https://tickets.example.com/${ticketId}`} size={96} color="#1a365d" />
          <QrCode data="plain data, no size" />
        </View>

        <Barcode data={ticketId} format="Code39" width={220} height={50} color="#333333" style={{ marginTop: 16 }} />
        <Barcode data="ABC-123" />

        <Text>Ticket {ticketId}</Text>
      </Page>
    </Document>
  );
}
