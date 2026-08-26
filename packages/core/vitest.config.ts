import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    include: ['tests/**/*.test.{ts,tsx}'],
    // worker.test.ts only runs under workerd via `npm run test:workers`
    // (vitest.config.workers.ts). It does `import wasm from '.wasm'`,
    // which Node's loader can't resolve.
    //
    // The two templates.*regression suites run under `npm run
    // test:templates` instead, which CI surfaces as its own step. They're
    // excluded here so that step is the only place they run — a CI step
    // named "Test Templates (regression)" that re-runs what "Test Core"
    // already covered would be a green check reporting something other
    // than what its name claims, which is the exact failure shape those
    // suites exist to catch in documents.
    exclude: [
      'tests/worker.test.ts',
      'tests/templates.regression.test.ts',
      'tests/templates-demo.regression.test.ts',
      'node_modules/**',
    ],
  },
  esbuild: {
    jsx: 'automatic',
  },
});
