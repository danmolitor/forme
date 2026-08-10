<script lang="ts">
  import { Document, Page, View, Text, Table, Row, Cell } from '../../src/index.js';

  interface Props {
    rowCount?: number;
  }

  let { rowCount = 50 }: Props = $props();

  const rows = $derived(
    Array.from({ length: rowCount }, (_, i) => ({
      sku: `SKU-${String(i + 1).padStart(3, '0')}`,
      name: `Item ${i + 1}`,
      qty: (i % 7) + 1,
      price: ((i % 7) + 1) * 4.25,
    }))
  );
  const total = $derived(rows.reduce((sum, row) => sum + row.price, 0));
</script>

<Document title="Table Parity">
  <Page size="A4" margin={40}>
    <Table
      columns={[
        { width: { fixed: 70 } },
        { width: { fraction: 0.6 } },
        { width: 'auto' },
        { width: { fraction: 0.4 } },
      ]}
      style={{ borderWidth: 1, borderColor: '#ddd' }}
    >
      <Row header style={{ backgroundColor: '#333' }}>
        <Cell><Text style={{ color: '#fff' }}>SKU</Text></Cell>
        <Cell><Text style={{ color: '#fff' }}>Product</Text></Cell>
        <Cell><Text style={{ color: '#fff' }}>Qty</Text></Cell>
        <Cell><Text style={{ color: '#fff' }}>Price</Text></Cell>
      </Row>
      {#each rows as row}
        <Row>
          <Cell><Text>{row.sku}</Text></Cell>
          <Cell>
            <View style={{ flexDirection: 'row', gap: 4 }}>
              <Text>{row.name}</Text>
              {#if row.qty > 5}
                <Text style={{ color: '#c00' }}>(bulk)</Text>
              {/if}
            </View>
          </Cell>
          <Cell><Text>{row.qty}</Text></Cell>
          <Cell><Text>${row.price.toFixed(2)}</Text></Cell>
        </Row>
      {/each}
      <Row style={{ backgroundColor: '#f5f5f5' }}>
        <Cell colSpan={3}><Text style={{ fontWeight: 'bold' }}>Total</Text></Cell>
        <Cell><Text style={{ fontWeight: 'bold' }}>${total.toFixed(2)}</Text></Cell>
      </Row>
    </Table>
  </Page>
</Document>
