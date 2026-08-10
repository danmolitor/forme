import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  plugins: [svelte()],
  resolve: {
    // Self-name aliases so docs-sample fixtures import '@formepdf/svelte'
    // exactly as the docs show, while still testing the source tree.
    alias: [
      {
        find: '@formepdf/svelte/preview',
        replacement: fileURLToPath(new URL('./src/preview/index.ts', import.meta.url)),
      },
      {
        find: '@formepdf/svelte',
        replacement: fileURLToPath(new URL('./src/index.ts', import.meta.url)),
      },
    ],
  },
  test: {
    include: ['tests/**/*.test.{ts,tsx}'],
    environment: 'node',
  },
  esbuild: {
    jsx: 'automatic',
  },
});
