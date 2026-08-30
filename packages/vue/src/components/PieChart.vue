<script setup lang="ts">
import { useAttrs } from 'vue';
import { encodeProps } from '../encode.js';

defineOptions({ inheritAttrs: false });
const attrs = useAttrs();
</script>

<template>
  <forme-pie-chart :props="encodeProps('PieChart', attrs)"></forme-pie-chart>
</template>
