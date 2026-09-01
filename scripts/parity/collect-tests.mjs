#!/usr/bin/env node
// Test-coverage + regression evidence for the parity page.
//
// Runs the suites and parses their real output — the counts on the page are a
// render of an actual run, not a hand-kept tally. Two parts:
//   - suites: per-suite passed/failed counts (facts, not a boast)
//   - regressions: the ironpress corpus tests (MIT third-party test material)
//     and the Chrome-reference STRUCTURAL assertions (spike.rs) — per test,
//     pass/fail. These are boolean structural checks, NOT a measured browser
//     diff; the schema and page label them exactly as that.

import { execSync } from 'node:child_process';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { emitSection } from './lib.mjs';

const HERE = dirname(fileURLToPath(import.meta.url));
const REPO = join(HERE, '..', '..');

/** Run a command, return stdout+stderr as text (never throws on test failures). */
function run(cmd, cwd) {
  try {
    return execSync(cmd, { cwd, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'], maxBuffer: 64 * 1024 * 1024 });
  } catch (e) {
    return (e.stdout ?? '') + (e.stderr ?? '');
  }
}

/** Sum cargo's "test result: ok. N passed; M failed" lines into {passed, failed}. */
function cargoCounts(out) {
  let passed = 0, failed = 0;
  for (const m of out.matchAll(/test result: \w+\. (\d+) passed; (\d+) failed/g)) {
    passed += Number(m[1]); failed += Number(m[2]);
  }
  return { passed, failed };
}

/** Parse `test NAME ... ok|FAILED` lines from a single cargo test binary. */
function cargoPerTest(out) {
  return [...out.matchAll(/^test (\S+) \.\.\. (ok|FAILED)/gm)].map((m) => ({
    test: m[1],
    pass: m[2] === 'ok',
  }));
}

/** Run a vitest package and return {passed, failed} from its JSON reporter. */
function vitestCounts(pkg) {
  const out = run(`npx vitest run --reporter=json 2>/dev/null`, join(REPO, 'packages', pkg));
  const start = out.indexOf('{');
  if (start < 0) return null;
  try {
    const j = JSON.parse(out.slice(start));
    return { passed: j.numPassedTests ?? 0, failed: j.numFailedTests ?? 0 };
  } catch {
    return null;
  }
}

function main() {
  const suites = [];
  const push = (suite, counts) => { if (counts) suites.push({ suite, ...counts }); };

  // Rust
  const engineOut = run('cargo test 2>&1', join(REPO, 'engine'));
  push('engine (lib + integration)', cargoCounts(engineOut));
  const htmlOut = run('cargo test 2>&1', join(REPO, 'html'));
  push('html crate', cargoCounts(htmlOut));

  // JS suites (vitest packages)
  for (const pkg of ['core', 'react', 'preact', 'svelte', 'vue', 'renderer', 'mcp', 'resend', 'tailwind', 'vscode']) {
    push(`@formepdf/${pkg}`, vitestCounts(pkg));
  }

  // Regression evidence — run the specific Rust test files for per-test detail.
  const ironOut = run('cargo test --test ironpress_regressions -- --test-threads=1 2>&1', join(REPO, 'html'));
  const spikeOut = run('cargo test --test spike -- --test-threads=1 2>&1', join(REPO, 'html'));

  const section = {
    suites,
    totals: suites.reduce((a, s) => ({ passed: a.passed + s.passed, failed: a.failed + s.failed }), { passed: 0, failed: 0 }),
    regressions: {
      ironpress: {
        note: 'Hand-reduced repros distilled from the ironpress parity corpus (third-party test material, MIT). Not upstream copies.',
        source: 'https://github.com/gastongouron/ironpress',
        license: 'MIT',
        tests: cargoPerTest(ironOut),
      },
      chromeStructural: {
        note: 'Boolean structural assertions (page count, break positions, table structure, valign) read from Chrome\'s rendering of these fixtures and asserted exactly. NOT a general browser-parity claim or a measured diff.',
        tests: cargoPerTest(spikeOut),
      },
    },
  };
  emitSection('tests', section);

  // Render console from the section.
  console.log('Suite counts:');
  for (const s of suites) console.log(`  ${s.failed ? '✗' : '✓'} ${s.suite}: ${s.passed} passed${s.failed ? `, ${s.failed} failed` : ''}`);
  console.log(`  total: ${section.totals.passed} passed, ${section.totals.failed} failed`);
  console.log(`ironpress regressions: ${section.regressions.ironpress.tests.filter((t) => t.pass).length}/${section.regressions.ironpress.tests.length} pass`);
  console.log(`Chrome-structural assertions: ${section.regressions.chromeStructural.tests.filter((t) => t.pass).length}/${section.regressions.chromeStructural.tests.length} pass`);
}

main();
