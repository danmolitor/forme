<script setup lang="ts">
import { Document, Page, Text, View } from '../../src/index.js';
import type { FontRegistration } from '../../src/index.js';

withDefaults(
  defineProps<{
    fonts?: FontRegistration[];
    bodyFamily?: string;
  }>(),
  {
    fonts: () => [
      { family: 'Inter', src: 'fonts/Inter-Regular.ttf' },
      { family: 'Inter', src: 'fonts/Inter-Bold.ttf', fontWeight: 700 },
      { family: 'Custom', src: 'data:font/ttf;base64,AAAA', fontStyle: 'italic' },
    ],
    bodyFamily: 'Inter',
  },
);
</script>

<template>
  <Document title="Fonts" :fonts="fonts" :style="{ fontFamily: 'Inter', fontSize: 11 }">
    <Page size="A4" :margin="40">
      <View :style="{ flexDirection: 'column', gap: 6 }">
        <Text :style="{ fontSize: 20, fontWeight: 700 }">Custom fonts</Text>
        <Text :style="{ fontFamily: bodyFamily }">Body text in the registered family.</Text>
        <Text :style="{ fontFamily: 'Custom', fontStyle: 'italic' }">Italic custom face.</Text>
      </View>
    </Page>
  </Document>
</template>
