// Vitest config for the structural-regression suites covering the templates
// we ship on npm (`@formepdf/templates`). Used by the `test:templates`
// script + the corresponding CI step.
//
// These suites live in packages/core because rendering needs the WASM engine
// only core builds, but they assert on a different surface than the rest of
// core's tests: the documents users get by name from `getTemplate()`. Pulling
// them into their own config — and excluding them from `vitest.config.ts` —
// keeps `Test Templates (regression)` the sole place they run, so that step's
// green check means what its name says rather than re-reporting work `Test
// Core` already did.
//
// A separate config rather than passing the paths to the default one: a CLI
// path filter is applied *on top of* the config's `exclude`, not instead of
// it, so `vitest run tests/templates.regression.test.ts` against the default
// config matches nothing at all.

import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    include: ['tests/templates.regression.test.ts', 'tests/templates-demo.regression.test.ts'],
    exclude: ['node_modules/**'],
  },
  esbuild: {
    jsx: 'automatic',
  },
});
