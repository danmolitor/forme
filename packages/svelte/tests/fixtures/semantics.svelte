<script lang="ts">
  import { Document, Page, H1, H2, H3, H6, OrderedList, UnorderedList, ListItem, Strong, Em, Code, Link, Text, View } from '../../src/index.js';

  interface Props {
    productName?: string;
  }

  let { productName = 'Forme' }: Props = $props();
</script>

<Document>
  <Page>
    <H1 bookmark="Overview">Getting started with {productName}</H1>
    <H2 style={{ color: '#334155', fontSize: 22 }}>Install the <Code>@formepdf/svelte</Code> package</H2>
    <H3 href="https://forme.dev/docs">Read the <Strong>full</Strong> docs</H3>
    <H6>Fine print</H6>
    <Text>Mix <Strong>bold</Strong>, <Em>italic</Em>, <Strong><Em>both</Em></Strong>, <Code style={{ backgroundColor: '#e0e7ff' }}>styled code</Code>, and a <Strong><Link href="https://forme.dev">bold link</Link></Strong> in one paragraph.</Text>
    <OrderedList type="lower-roman" start={3} style={{ marginTop: 8 }}>
      <ListItem>Third step</ListItem>
      <ListItem style={{ color: '#dc2626' }}><Text>Fourth step with nested <Strong>runs</Strong></Text></ListItem>
      <ListItem>
        <Text>Step with a sublist</Text>
        <UnorderedList marker="square">
          <ListItem>Nested bullet</ListItem>
          <ListItem>Another bullet</ListItem>
        </UnorderedList>
      </ListItem>
    </OrderedList>
    <UnorderedList>
      <ListItem>Default disc bullet</ListItem>
    </UnorderedList>
    <View style={{ transform: 'rotate(-15deg) scale(1.2)', transformOrigin: '0% 100%', padding: 8 }}>
      <Text style={{ transform: 'translate(10, -4)' }}>Transformed stamp</Text>
    </View>
  </Page>
</Document>
