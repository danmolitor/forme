import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vitest/config';
import vue from '@vitejs/plugin-vue';
import { resolve } from 'node:path';

export default defineConfig({
  plugins: [
    vue({
      // `<forme-*>` placeholder tags are native custom elements, not Vue
      // components — otherwise Vue tries to resolve them as components.
      template: {
        compilerOptions: {
          isCustomElement: (tag) => tag.startsWith('forme-'),
        },
      },
    }),
  ],
  resolve: {
    alias: [
      {
        find: '@formepdf/vue',
        replacement: fileURLToPath(new URL('./src/index.ts', import.meta.url)),
      },
    ],
  },
  build: {
    lib: {
      entry: resolve(fileURLToPath(new URL('.', import.meta.url)), 'src/index.ts'),
      formats: ['es'],
      fileName: () => 'index.js',
    },
    rollupOptions: {
      external: ['vue', 'vue/server-renderer', '@formepdf/shared', '@formepdf/core'],
    },
  },
  test: {
    include: ['tests/**/*.test.{ts,tsx}'],
    environment: 'node',
  },
  // React fixtures for the cross-framework equivalence gate use the
  // automatic JSX runtime (mirrors @formepdf/svelte's parity test).
  esbuild: {
    jsx: 'automatic',
  },
});
