<script setup lang="ts">
import { encodeProps } from '../encode.js';

// Array-form props (no runtime type → no Boolean coercion, so absent props
// stay undefined and are omitted, matching the React/Svelte serializers).
// `style` is declared here so Vue never treats it as the DOM style attribute.
defineOptions({ inheritAttrs: false });
const props = defineProps(['data', 'format', 'width', 'height', 'color', 'style']);
</script>

<template>
  <forme-barcode :props="encodeProps('Barcode', props)"></forme-barcode>
</template>
