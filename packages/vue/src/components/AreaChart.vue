<script setup lang="ts">
import { useAttrs } from 'vue';
import { encodeProps } from '../encode.js';

defineOptions({ inheritAttrs: false });
const attrs = useAttrs();
</script>

<template>
  <forme-area-chart :props="encodeProps('AreaChart', attrs)"></forme-area-chart>
</template>
