// Vitest config that runs the worker smoke test inside Cloudflare's workerd
// via @cloudflare/vitest-pool-workers — the same harness @formepdf/core uses.
// The point is to catch regressions where the published @formepdf/html/worker
// entry (the web-target WASM glue) silently breaks under workerd's
// WASM-as-ESM semantics; Node-based tests can't see those.

import { defineWorkersConfig } from '@cloudflare/vitest-pool-workers/config';

export default defineWorkersConfig({
  test: {
    include: ['tests/worker.test.ts'],
    poolOptions: {
      workers: {
        wrangler: { configPath: './tests/worker-fixture/wrangler.toml' },
        miniflare: {
          // Treat .wasm imports as CompiledWasm modules (real Wrangler
          // behaviour): `import wasm from '…/forme_pdf_html_bg.wasm'` becomes a
          // WebAssembly.Module that `init(wasm)` can consume.
          modulesRules: [
            { type: 'CompiledWasm', include: ['**/*.wasm'], fallthrough: true },
          ],
        },
      },
    },
  },
});
