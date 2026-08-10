<script lang="ts">
  import { Document, Page, Text, Fixed, PAGE_NUMBER, TOTAL_PAGES } from '@formepdf/svelte';
</script>

<Document>
  <Page>
    <Fixed position="footer" style={{ paddingTop: 8 }}>
      <Text style={{ fontSize: 9, textAlign: 'center' }}>Page {PAGE_NUMBER} of {TOTAL_PAGES}</Text>
    </Fixed>
    <Text>Body content</Text>
  </Page>
</Document>
