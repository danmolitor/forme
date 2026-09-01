<script setup lang="ts">
import { useAttrs } from 'vue';
import { encodeProps } from '../encode.js';

defineOptions({ inheritAttrs: false });
const attrs = useAttrs();
</script>

<template>
  <forme-dot-plot :props="encodeProps('DotPlot', attrs)"></forme-dot-plot>
</template>
