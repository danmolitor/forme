<script setup lang="ts">
import { encodeProps } from '../encode.js';

// Array-form props (no runtime type → no Boolean coercion, so absent props
// stay undefined and are omitted, matching the React/Svelte serializers).
// `style` is declared here so Vue never treats it as the DOM style attribute.
defineOptions({ inheritAttrs: false });
const props = defineProps(['text', 'fontSize', 'color', 'angle', 'style']);
</script>

<template>
  <forme-watermark :props="encodeProps('Watermark', props)"></forme-watermark>
</template>
