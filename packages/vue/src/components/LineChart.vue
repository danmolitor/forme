<script setup lang="ts">
import { useAttrs } from 'vue';
import { encodeProps } from '../encode.js';

defineOptions({ inheritAttrs: false });
const attrs = useAttrs();
</script>

<template>
  <forme-line-chart :props="encodeProps('LineChart', attrs)"></forme-line-chart>
</template>
