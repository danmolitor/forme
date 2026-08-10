<script lang="ts">
  import { Document, Page, View, Text } from '@formepdf/svelte';
  import { tw } from '@formepdf/tailwind';
</script>

<Document>
  <Page size="A4" margin={40}>
    <View style={tw('flex-row items-center justify-between p-6 bg-slate-100 rounded-lg')}>
      <Text style={tw('text-2xl font-bold text-slate-900')}>Invoice #001</Text>
      <Text style={tw('text-sm text-slate-500')}>March 2026</Text>
    </View>
  </Page>
</Document>
