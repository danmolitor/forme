<script setup lang="ts">
import { Document, Text } from '../../src/index.js';

const oops = () => {};
</script>

<template>
  <Document>
    <Text :style="oops">oops</Text>
  </Document>
</template>
