<script setup lang="ts">
import { useAttrs } from 'vue';
import { encodeProps } from '../encode.js';

defineOptions({ inheritAttrs: false });
const attrs = useAttrs();
</script>

<template>
  <forme-bar-chart :props="encodeProps('BarChart', attrs)"></forme-bar-chart>
</template>
