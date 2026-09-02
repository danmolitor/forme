<script setup lang="ts">
// Boolean-attribute coercion probe. Vue SSR renders a bare attribute
// (`<Row header>`) as the empty string where React/Svelte produce `true`;
// the Vue package's encode.ts normalizes "" back to true for known boolean
// props. This fixture exercises the bare form, the explicit :prop="false"
// form, and complete absence — the shared parser depends on
// undefined-vs-false being preserved for wrap/tagged.
import { Document, Page, View, Text, Table, Row, Cell, TextField } from '../../src/index.js';
</script>

<template>
  <Document tagged>
    <Page size="A4" :margin="40" wrap>
      <View :wrap="false"><Text>explicit false wrap</Text></View>
      <View><Text>no wrap prop at all</Text></View>
      <Table :columns="[{ width: { fraction: 1 } }]">
        <Row header>
          <Cell><Text>bare header attr</Text></Cell>
        </Row>
        <Row>
          <Cell><Text>no header attr</Text></Cell>
        </Row>
      </Table>
      <TextField name="notes" :width="180" multiline />
      <TextField name="plain" :width="180" />
    </Page>
  </Document>
</template>
