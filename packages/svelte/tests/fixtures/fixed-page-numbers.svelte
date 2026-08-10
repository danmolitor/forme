<script lang="ts">
  import { Document, Page, Text, Fixed, PAGE_NUMBER, TOTAL_PAGES } from '../../src/index.js';

  interface Props {
    paragraphs?: number;
  }

  let { paragraphs = 5 }: Props = $props();
</script>

<Document>
  <Page>
    <Fixed position="header" style={{ paddingBottom: 8 }}>
      <Text style={{ fontSize: 9, color: '#666666' }}>Quarterly Report</Text>
    </Fixed>
    <Fixed position="footer" style={{ paddingTop: 8 }}>
      <Text style={{ fontSize: 9, textAlign: 'center' }}>Page {PAGE_NUMBER} of {TOTAL_PAGES}</Text>
    </Fixed>
    {#each Array.from({ length: paragraphs }) as _unused, i (i)}
      <Text style={{ marginBottom: 12 }}>Paragraph {i + 1}: repeated body copy that fills the page so a long document breaks across multiple pages and the footer repeats.</Text>
    {/each}
  </Page>
</Document>
