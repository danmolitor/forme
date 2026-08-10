<script lang="ts">
  import { Document, Page, Text } from '@formepdf/svelte';

  let { name = 'World' }: { name?: string } = $props();
</script>

<Document>
  <Page size="Letter" margin={54}>
    <Text style={{ fontSize: 24, fontWeight: 700, marginBottom: 12 }}>Hello {name}</Text>
    <Text style={{ fontSize: 10, lineHeight: 1.6 }}>Page breaks that actually work.</Text>
  </Page>
</Document>
