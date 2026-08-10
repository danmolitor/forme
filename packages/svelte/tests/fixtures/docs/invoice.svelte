<script lang="ts">
  import { Document, Page, View, Text } from '@formepdf/svelte';

  interface Item {
    name: string;
    price: number;
  }

  interface Props {
    invoiceNo?: string;
    customer?: string;
    items?: Item[];
  }

  let {
    invoiceNo = '001',
    customer = 'Jane Smith',
    items = [
      { name: 'Website Redesign', price: 3500 },
      { name: 'Hosting (12 months)', price: 600 },
    ],
  }: Props = $props();

  const total = $derived(items.reduce((sum, item) => sum + item.price, 0));
</script>

<Document title="Invoice #{invoiceNo}">
  <Page size="Letter" margin={54}>
    <Text style={{ fontSize: 28, fontWeight: 700, color: '#1e293b' }}>Invoice</Text>

    <View style={{ flexDirection: 'row', justifyContent: 'space-between', marginTop: 24 }}>
      <View>
        <Text style={{ fontSize: 10, color: '#64748b' }}>Bill To</Text>
        <Text style={{ fontSize: 12, fontWeight: 700, marginTop: 4 }}>{customer}</Text>
      </View>
      <Text style={{ fontSize: 10, color: '#64748b' }}>Invoice #{invoiceNo}</Text>
    </View>

    <View style={{ marginTop: 32, padding: 12, backgroundColor: '#f8fafc', borderRadius: 4 }}>
      {#each items as item}
        <View style={{ flexDirection: 'row', justifyContent: 'space-between', marginTop: 8 }}>
          <Text style={{ fontSize: 10 }}>{item.name}</Text>
          <Text style={{ fontSize: 10, fontWeight: 700 }}>${item.price.toFixed(2)}</Text>
        </View>
      {/each}
    </View>

    <View style={{ flexDirection: 'row', justifyContent: 'flex-end', marginTop: 16 }}>
      <Text style={{ fontSize: 14, fontWeight: 700 }}>Total: ${total.toFixed(2)}</Text>
    </View>
  </Page>
</Document>
