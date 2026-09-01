<script setup lang="ts">
import { computed } from 'vue';
import { recordCanvasOperations } from '@formepdf/shared';
import { encodeProps } from '../encode.js';

defineOptions({ inheritAttrs: false });
const props = defineProps(['width', 'height', 'draw', 'style']);
const encoded = computed(() => {
  const { draw, ...rest } = props as Record<string, any>;
  return encodeProps('Canvas', { ...rest, operations: recordCanvasOperations(draw) });
});
</script>

<template>
  <forme-canvas :props="encoded"></forme-canvas>
</template>
