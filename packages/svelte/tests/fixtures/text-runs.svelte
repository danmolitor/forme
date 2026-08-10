<script lang="ts">
  import { Document, Page, Text } from '../../src/index.js';

  interface Props {
    price?: number;
  }

  let { price = 42 }: Props = $props();
</script>

<Document>
  <Page>
    <Text style={{ fontSize: 12 }}>Was <Text style={{ textDecoration: 'line-through', color: '#999999' }}>$56.00</Text> <Text style={{ fontWeight: 700 }}>${price}.00</Text> due now</Text>
    <Text>Visit <Text href="https://forme.dev" style={{ color: '#0000ff' }}>our site</Text> for details</Text>
  </Page>
</Document>
