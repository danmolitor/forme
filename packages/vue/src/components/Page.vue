<script setup lang="ts">
import { encodeProps } from '../encode.js';

// Array-form props (no runtime type → no Boolean coercion, so absent props
// stay undefined and are omitted, matching the React/Svelte serializers).
// `style` is declared here so Vue never treats it as the DOM style attribute.
defineOptions({ inheritAttrs: false });
const props = defineProps(['size', 'margin', 'style', 'backgroundImage', 'backgroundOpacity', 'backgroundSize', 'backgroundPosition']);
</script>

<template>
  <forme-page :props="encodeProps('Page', props)"><slot /></forme-page>
</template>
