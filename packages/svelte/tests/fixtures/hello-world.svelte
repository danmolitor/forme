<script lang="ts">
  import { Document, Page, View, Text } from '../../src/index.js';

  interface Props {
    name?: string;
    items?: string[];
    showFooter?: boolean;
  }

  let { name = 'World', items = [], showFooter = false }: Props = $props();
</script>

<Document title="Hello">
  <Page size="A4" margin={40}>
    <View style={{ flexDirection: 'column', gap: 8 }}>
      <Text style={{ fontSize: 24 }}>Hello {name}!</Text>
      {#each items as item}
        <Text>Item: {item}</Text>
      {/each}
      {#if showFooter}
        <Text>The footer</Text>
      {/if}
    </View>
  </Page>
</Document>
