<script setup lang="ts">
import {
  Document, Page, View, Text,
  H1, H2, H3, H4, H5, H6,
  OrderedList, UnorderedList, ListItem,
  Table, Row, Cell,
  Image, Svg, QrCode, Barcode,
  BarChart, LineChart, PieChart, AreaChart, DotPlot,
  Canvas, Watermark, PageBreak, Fixed, Link,
  Strong, Em, Code,
  TextField, Checkbox, Dropdown, RadioButton,
} from '../../src/index.js';

const draw = () => {};
</script>

<template>
  <Document title="Kitchen sink">
    <Page size="Letter" :margin="36">
      <Fixed position="header"><Text>Running head</Text></Fixed>
      <H1>Heading one</H1>
      <H2>Heading two</H2>
      <H3>Heading three</H3>
      <H4>Heading four</H4>
      <H5>Heading five</H5>
      <H6>Heading six</H6>
      <Text>Body with <Strong>bold</Strong> and <Em>italic</Em> and <Code>mono</Code> runs.</Text>
      <Text><Link href="https://example.com">A link</Link></Text>
      <OrderedList>
        <ListItem><Text>First</Text></ListItem>
        <ListItem><Text>Second</Text></ListItem>
      </OrderedList>
      <UnorderedList>
        <ListItem><Text>Bullet</Text></ListItem>
      </UnorderedList>
      <Table :columns="[{ width: { fraction: 0.5 } }, { width: { fraction: 0.5 } }]">
        <Row header>
          <Cell><Text>Col A</Text></Cell>
          <Cell><Text>Col B</Text></Cell>
        </Row>
        <Row>
          <Cell><Text>a1</Text></Cell>
          <Cell><Text>b1</Text></Cell>
        </Row>
      </Table>
      <Image src="logo.png" :width="40" :height="40" />
      <Svg :width="50" :height="20" content="<rect width='50' height='20' fill='#ccc'/>" />
      <QrCode data="https://formepdf.com" :size="60" />
      <Barcode data="ABC-123" format="Code128" :width="120" :height="40" />
      <BarChart :width="200" :height="100" :data="[{ label: 'A', value: 3 }, { label: 'B', value: 5 }]" />
      <LineChart :width="200" :height="100" :labels="['Q1', 'Q2']" :series="[{ name: 'S', data: [1, 2] }]" />
      <PieChart :width="120" :height="120" :data="[{ label: 'X', value: 1 }, { label: 'Y', value: 2 }]" />
      <AreaChart :width="200" :height="100" :labels="['Q1', 'Q2']" :series="[{ name: 'S', data: [1, 3] }]" />
      <DotPlot :width="200" :height="100" :groups="[{ name: 'G', data: [[1, 1]] }]" />
      <Canvas :width="80" :height="40" :draw="draw" />
      <Watermark text="DRAFT" :fontSize="48" />
      <PageBreak />
      <Text>Page two content</Text>
      <TextField name="fullName" :width="180" />
      <Checkbox name="agree" />
      <Dropdown name="color" :width="120" :options="['red', 'green']" />
      <RadioButton name="size" value="L" />
    </Page>
  </Document>
</template>
