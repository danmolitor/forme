<script lang="ts">
  import { Document, Page, Text } from '@formepdf/svelte';

  let { price = 42 }: { price?: number } = $props();
</script>

<Document>
  <Page>
    <Text style={{ fontSize: 12 }}>Was <Text style={{ textDecoration: 'line-through', color: '#999999' }}>$56.00</Text> <Text style={{ fontWeight: 700 }}>${price}.00</Text> due now</Text>
  </Page>
</Document>
