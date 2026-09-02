<script setup lang="ts">
import { Document, Page, View, Text } from '../../src/index.js';

withDefaults(
  defineProps<{
    name?: string;
    items?: string[];
    showFooter?: boolean;
  }>(),
  { name: 'World', items: () => [], showFooter: false },
);
</script>

<template>
  <Document title="Hello">
    <Page size="A4" :margin="40">
      <View :style="{ flexDirection: 'column', gap: 8 }">
        <Text :style="{ fontSize: 24 }">Hello {{ name }}!</Text>
        <Text v-for="item in items" :key="item">Item: {{ item }}</Text>
        <Text v-if="showFooter">The footer</Text>
      </View>
    </Page>
  </Document>
</template>
