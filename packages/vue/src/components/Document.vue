<script setup lang="ts">
import { encodeProps } from '../encode.js';
import type { FormeDocumentClaimProps } from '@formepdf/shared';

// Array-form props (no runtime type → no Boolean coercion, so absent props
// stay undefined and are omitted, matching the React/Svelte serializers).
// `style` is declared here so Vue never treats it as the DOM style attribute.
defineOptions({ inheritAttrs: false });
const props = defineProps(['title', 'author', 'subject', 'creator', 'lang', 'style', 'tagged', 'pdfa', 'pdfVersion', 'pdfUa', 'pdfUa2', 'certification', 'signature', 'fonts']);

// Compile-time claim parity (vue-tsc, at build): every conformance claim
// the serializer accepts (FormeDocumentClaimProps) must appear in the
// props array above — a missing name makes `keyof typeof props` lose the
// key and this alias fails to compile. Array-form props carry no types,
// so this checks presence; the shared parser's guard checks the types.
type AssertTrue<T extends true> = T;
type _DocumentClaimParity = AssertTrue<
  keyof FormeDocumentClaimProps extends keyof typeof props ? true : false
>;
</script>

<template>
  <forme-document :props="encodeProps('Document', props)"><slot /></forme-document>
</template>
