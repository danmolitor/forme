<script setup lang="ts">
import { encodeProps } from '../encode.js';

// Array-form props (no runtime type → no Boolean coercion, so absent props
// stay undefined and are omitted, matching the React/Svelte serializers).
// `style` is declared here so Vue never treats it as the DOM style attribute.
defineOptions({ inheritAttrs: false });
const props = defineProps(['header', 'style']);
</script>

<template>
  <forme-row :props="encodeProps('Row', props)"><slot /></forme-row>
</template>
