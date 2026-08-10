<script module lang="ts">
  import { Font } from '@formepdf/svelte';

  Font.register({ family: 'Inter', src: './fonts/Inter-Regular.ttf' });
  Font.register({ family: 'Inter', src: './fonts/Inter-Bold.ttf', fontWeight: 700 });
</script>

<script lang="ts">
  import { Document, Page, Text } from '@formepdf/svelte';
</script>

<Document style={{ fontFamily: 'Inter' }}>
  <Page size="A4" margin={40}>
    <Text style={{ fontSize: 20, fontWeight: 700 }}>Custom fonts</Text>
    <Text>Registered once, used anywhere.</Text>
  </Page>
</Document>
