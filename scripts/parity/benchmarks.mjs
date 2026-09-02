#!/usr/bin/env node
// Benchmark evidence for the parity page. Same rule as the determinism emitter:
// the JSON object is the SOURCE; the console output is rendered from it, never a
// parallel formatting path. Emits `$PARITY_DIR/benchmarks.json`.
//
// Measured live per run (reproducible, stamped): cold start + warm render +
// peak memory + output size across native / node / web / workerd, plus a
// Puppeteer baseline on the identical HTML. NOT a CI gate — emit and publish.
//
// TWO RUNS. environment.runner is 'dev' (clean hardware) or 'ci' (conservative
// shared runner). A dev baseline is committed at benchmarks/results/dev.json;
// in CI the emitter measures the 'ci' run and merges the committed dev run so
// the artifact carries `runs: [dev, ci]`. If a target/document fails, times
// out, or the runner can't fit it, that is a recorded status — never a crash
// or a silent omission; the parent isolates heavy renders in child processes so
// an OOM kills the child, not the emitter.
//
// Recorded analysis (machine-independent, tagged with `method`): the sentinel
// before/after A/B and the whereWeLose profile — they need a pre-fix rebuild or
// a profiling binary and are not re-measured each run.

import { readFileSync, existsSync, writeFileSync, statSync } from 'node:fs';
import { execFileSync, spawnSync } from 'node:child_process';
import { createServer } from 'node:http';
import { extname, join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import os from 'node:os';
import { emitSection } from './lib.mjs';

const HERE = dirname(fileURLToPath(import.meta.url));
const REPO = join(HERE, '..', '..');
const CORPUS = join(REPO, 'benchmarks', 'corpus');
const HARNESS = join(REPO, 'benchmarks', 'harness');
const NATIVE = join(REPO, 'html', 'target', 'release', 'forme-html');
const BASELINE = join(REPO, 'benchmarks', 'results', 'dev.json');
const CHROME = process.env.CHROME_PATH || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const CI = !!process.env.CI;

// Per-document plan. Iteration counts scale down on CI (published per cell, so a
// smaller count is honest, not hidden). Timeouts are explicit per document — a
// slow large-document render on a shared runner records "did not complete"
// rather than hanging the job.
const DOCS = [
  { file: 'receipt', pages: 1, devIters: 30, ciIters: 12, timeoutMs: 60_000 },
  { file: 'report-6p', pages: 6, devIters: 30, ciIters: 12, timeoutMs: 60_000 },
  { file: 'letterhead-paged', pages: 5, devIters: 30, ciIters: 12, timeoutMs: 60_000 },
  { file: 'compliance', pages: 2, devIters: 30, ciIters: 12, timeoutMs: 60_000 },
  { file: 'invoice-50p', pages: 50, devIters: 8, ciIters: 3, timeoutMs: 120_000 },
  { file: 'ledger-500p', pages: 500, devIters: 3, ciIters: 1, timeoutMs: 300_000 },
];
const itersFor = (d) => (CI ? d.ciIters : d.devIters);

// Prefer full `puppeteer` (bundled Chromium — what CI installs) and fall back to
// `puppeteer-core` + a system Chrome path (the dev machine). Returns null if
// neither is available, so the browser-based targets record "unavailable".
let _pptr;
async function getPuppeteer() {
  if (_pptr !== undefined) return _pptr;
  try { _pptr = { mod: (await import('puppeteer')).default, opts: {} }; return _pptr; } catch { /* try core */ }
  try { _pptr = { mod: (await import('puppeteer-core')).default, opts: { executablePath: CHROME } }; return _pptr; } catch { /* none */ }
  _pptr = null; return _pptr;
}
const readDoc = (f) => readFileSync(join(CORPUS, `${f}.html`), 'utf8');
const round = (x, d = 0) => (x == null || Number.isNaN(x) ? null : Number(x.toFixed(d)));
const median = (a) => { const s = [...a].sort((x, y) => x - y); return s[Math.floor(s.length / 2)]; };

const TIMEOUT = Symbol('timeout');
function withTimeout(promise, ms) {
  let t;
  const timer = new Promise((res) => { t = setTimeout(() => res(TIMEOUT), ms); });
  return Promise.race([Promise.resolve(promise).then((v) => { clearTimeout(t); return v; }, (e) => { clearTimeout(t); throw e; }), timer]);
}
// Run a child that prints one JSON line; classify timeout / OOM-kill / error.
function spawnJson(cmd, args, timeoutMs) {
  const s = performance.now();
  const r = spawnSync(cmd, args, { encoding: 'utf8', timeout: timeoutMs, maxBuffer: 256 * 1024 * 1024, killSignal: 'SIGKILL' });
  const wall = performance.now() - s;
  if (r.signal || (r.error && r.error.code === 'ETIMEDOUT')) {
    return wall >= timeoutMs * 0.9
      ? { status: 'timeout', limitMs: timeoutMs }
      : { status: 'killed', note: 'process killed before timeout — likely out of memory on this runner' };
  }
  if (r.status !== 0) return { status: 'error', detail: (r.stderr || '').trim().split('\n').pop()?.slice(0, 140) || `exit ${r.status}` };
  try { return { status: 'ok', ...JSON.parse((r.stdout || '').trim()) }; }
  catch { return { status: 'error', detail: 'unparseable child output' }; }
}

// ── environment ──────────────────────────────────────────────────────────────
function environment() {
  const cargo = (() => { try { return execFileSync('cargo', ['--version'], { encoding: 'utf8' }).split(' ')[1]; } catch { return null; } })();
  const chrome = (() => { try { return execFileSync(CHROME, ['--version'], { encoding: 'utf8' }).trim(); } catch { return null; } })();
  // Stamp the commit + time this run was actually measured. The dev baseline is
  // committed and refreshed periodically, so it can lag the page's current
  // commit — recording measuredAtCommit lets the page label it honestly rather
  // than implying it was measured against the code being served.
  const commit = process.env.GITHUB_SHA || (() => { try { return execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8', cwd: REPO }).trim(); } catch { return null; } })();
  return {
    runner: CI ? 'ci' : 'dev',
    measuredAtCommit: commit,
    measuredAtCommitShort: commit ? commit.slice(0, 7) : null,
    measuredAt: new Date().toISOString(),
    label: CI ? 'CI ubuntu-latest (conservative shared runner)' : 'dev machine (clean hardware, single run)',
    machine: os.cpus()[0]?.model ?? 'unknown',
    arch: os.arch(),
    cpus: os.cpus().length,
    memoryGB: round(os.totalmem() / 1024 ** 3),
    os: `${os.platform()} ${os.release()}`,
    node: process.version,
    cargo,
    chrome,
  };
}

// ── node: warm (isolated child) + cold (fresh process) ───────────────────────
function nodeMetrics() {
  const bootMs = (() => { const t = []; for (let i = 0; i < 8; i++) { const s = performance.now(); spawnSync('node', ['-e', '']); t.push(performance.now() - s); } return median(t); })();
  const docs = {};
  for (const d of DOCS) {
    const docPath = join(CORPUS, `${d.file}.html`);
    const warm = spawnJson('node', [join(HARNESS, 'warmrender.mjs'), docPath, String(itersFor(d))], d.timeoutMs);
    const cr = spawnJson('node', [join(HARNESS, 'coldrender.mjs'), docPath], d.timeoutMs);
    const s = performance.now();
    const cw = spawnSync('node', [join(HARNESS, 'coldrender.mjs'), docPath], { encoding: 'utf8', timeout: d.timeoutMs });
    const coldWall = cw.status === 0 ? performance.now() - s : null;
    docs[d.file] = {
      pages: warm.status === 'ok' ? warm.pages : d.pages,
      passes: warm.status === 'ok' ? warm.passes : null,
      warm: warm.status === 'ok'
        ? { status: 'ok', medianMs: round(warm.medianMs, 1), p95Ms: round(warm.p95Ms, 1), iters: warm.iters, pdfBytes: warm.pdfBytes }
        : warm,
      cold: cr.status === 'ok'
        ? { status: 'ok', totalWallMs: round(coldWall), instantiateMs: cr.instantiate ?? null, firstRenderMs: cr.render ?? null, nodeBootMs: round(bootMs) }
        : cr,
      peakRssMB: warm.status === 'ok' ? warm.maxRssMB ?? null : null,
    };
  }
  return { bootMs: round(bootMs), docs };
}

// ── native: cold (fresh exec) + peak RSS via /usr/bin/time when available ────
function nativeMetrics() {
  if (!existsSync(NATIVE)) return { unavailable: 'native binary not built (html/target/release/forme-html)' };
  const docs = {};
  for (const d of DOCS) {
    const path = join(CORPUS, `${d.file}.html`);
    const t = []; let ok = true;
    for (let i = 0; i < Math.min(12, itersFor(d) + 6); i++) {
      const s = performance.now();
      const r = spawnSync(NATIVE, [path, '-o', '/tmp/_b.pdf', '-q'], { timeout: d.timeoutMs, killSignal: 'SIGKILL' });
      if (r.status !== 0) { ok = false; break; }
      t.push(performance.now() - s);
    }
    let rssMB = null;
    try {
      const tv = spawnSync('/usr/bin/time', ['-l', NATIVE, path, '-o', '/tmp/_b.pdf', '-q'], { encoding: 'utf8', timeout: d.timeoutMs });
      const mac = /(\d+)\s+maximum resident set size/.exec(tv.stderr || '');
      const lin = /Maximum resident set size \(kbytes\): (\d+)/.exec(tv.stderr || '');
      rssMB = mac ? round(Number(mac[1]) / 1024 ** 2) : lin ? round(Number(lin[1]) / 1024) : null;
    } catch { /* /usr/bin/time absent on some runners — recorded as null */ }
    docs[d.file] = ok
      ? { pages: d.pages, coldMs: round(median(t)), peakRssMB: rssMB }
      : { pages: d.pages, status: 'error', detail: 'native render failed or timed out' };
  }
  return { docs };
}

// ── puppeteer baseline: cold + warm (browser reused), per-doc timeout ────────
async function puppeteerMetrics() {
  const P = await getPuppeteer();
  if (!P) return { unavailable: 'no puppeteer / puppeteer-core available' };
  const tL0 = performance.now();
  const browser = await P.mod.launch({ headless: true, args: ['--no-sandbox'], ...P.opts });
  const launchMs = performance.now() - tL0;
  const page = await browser.newPage();
  const tR0 = performance.now();
  await page.setContent(readDoc('receipt'), { waitUntil: 'load' });
  await page.pdf({ preferCSSPageSize: true });
  const firstRenderMs = performance.now() - tR0;
  const docs = {};
  for (const d of DOCS) {
    const html = readDoc(d.file);
    const it = Math.max(1, Math.min(itersFor(d), d.file === 'ledger-500p' ? 1 : 5));
    try {
      const res = await withTimeout((async () => {
        const t = []; let size = 0;
        for (let i = 0; i < it; i++) { const s = performance.now(); await page.setContent(html, { waitUntil: 'load' }); const pdf = await page.pdf({ preferCSSPageSize: true }); t.push(performance.now() - s); size = pdf.length; }
        return { medianMs: round(median(t)), iters: it, pdfBytes: size };
      })(), d.timeoutMs);
      docs[d.file] = res === TIMEOUT ? { pages: d.pages, warm: { status: 'timeout', limitMs: d.timeoutMs } } : { pages: d.pages, warm: { status: 'ok', ...res } };
    } catch (e) {
      docs[d.file] = { pages: d.pages, warm: { status: 'error', detail: String(e.message).split('\n')[0].slice(0, 100) } };
    }
  }
  await browser.close();
  return { coldStart: { launchMs: round(launchMs), firstRenderMs: round(firstRenderMs), totalMs: round(launchMs + firstRenderMs), coldServerlessNote: 'On real serverless, cold Chrome is 3-10s, and on many consumption tiers a browser cannot boot at all.' }, docs };
}

// ── web WASM: cold decomposition + peak WASM linear memory (fresh isolate) ───
async function webMetrics() {
  const P = await getPuppeteer();
  if (!P) return { unavailable: 'no puppeteer / puppeteer-core available' };
  const MIME = { '.js': 'text/javascript', '.wasm': 'application/wasm', '.html': 'text/html' };
  const server = createServer((req, res) => {
    if (req.url === '/blank') { res.setHeader('content-type', 'text/html'); return res.end('<!doctype html><meta charset=utf-8>'); }
    try { const p = join(REPO, decodeURIComponent(req.url.split('?')[0])); statSync(p); res.setHeader('content-type', MIME[extname(p)] || 'application/octet-stream'); res.end(readFileSync(p)); }
    catch { res.statusCode = 404; res.end('nf'); }
  });
  await new Promise((r) => server.listen(0, r));
  const base = `http://localhost:${server.address().port}`;
  const modUrl = `${base}/packages/html/pkg-web/forme_pdf_html.js`;
  const wasmUrl = `${base}/packages/html/pkg-web/forme_pdf_html_bg.wasm`;
  const browser = await P.mod.launch({ headless: true, args: ['--no-sandbox', '--js-flags=--max-old-space-size=4096'], ...P.opts });
  const docs = {};
  for (const d of DOCS) {
    const page = await browser.newPage();
    try {
      await page.goto(`${base}/blank`);
      const res = await withTimeout(page.evaluate(async (modUrl, wasmUrl, docUrl) => {
        const t0 = performance.now();
        const bytes = await (await fetch(wasmUrl)).arrayBuffer();
        const tFetch = performance.now() - t0;
        const mod = await import(modUrl);
        const t1 = performance.now();
        const wexports = await mod.default({ module_or_path: bytes });
        const tInst = performance.now() - t1;
        const html = await (await fetch(docUrl)).text();
        const t2 = performance.now();
        const r = mod.render_html_wasm(html, '{}');
        const tRender = performance.now() - t2;
        const out = { tFetch, tInst, tRender, passes: r.passes, memMB: wexports.memory.buffer.byteLength / 1048576, pdfLen: r.pdf.length };
        r.free();
        return out;
      }, modUrl, wasmUrl, `${base}/benchmarks/corpus/${d.file}.html`), d.timeoutMs);
      docs[d.file] = res === TIMEOUT
        ? { pages: d.pages, status: 'timeout', limitMs: d.timeoutMs }
        : { pages: d.pages, status: 'ok', cold: { fetchMs: round(res.tFetch), compileInstantiateMs: round(res.tInst), firstRenderMs: round(res.tRender) }, wasmLinearMemMB: round(res.memMB), passes: res.passes, pdfBytes: res.pdfLen };
    } catch (e) {
      docs[d.file] = { pages: d.pages, status: 'error', detail: String(e.message).split('\n')[0].slice(0, 100) };
    }
    await page.close().catch(() => {});
  }
  await browser.close(); server.close();
  return { docs };
}

// ── workerd via miniflare: cold (fresh isolate first request) + warm ─────────
async function workerdMetrics() {
  let Miniflare;
  try { ({ Miniflare } = await import('miniflare')); } catch { return { unavailable: 'miniflare not installed' }; }
  const { mkdirSync, copyFileSync } = await import('node:fs');
  const WK = join(HARNESS, '_wk');
  mkdirSync(WK, { recursive: true });
  for (const f of ['forme_pdf_html.js', 'forme_pdf_html_bg.wasm']) copyFileSync(join(REPO, 'packages/html/pkg-web', f), join(WK, f));
  writeFileSync(join(WK, 'entry.mjs'), `import initWasm, { render_html_wasm } from './forme_pdf_html.js';\nimport wasm from './forme_pdf_html_bg.wasm';\nlet ready=false;\nexport default { async fetch(req){ const t0=Date.now(); if(!ready){await initWasm({module_or_path:wasm});ready=true;} const tInit=Date.now()-t0; const html=await req.text(); const t1=Date.now(); const r=render_html_wasm(html,'{}'); const tRender=Date.now()-t1; const passes=r.passes; r.free(); return new Response(JSON.stringify({tInit,tRender,passes}),{headers:{'content-type':'application/json'}}); } };\n`);
  const mkMf = () => new Miniflare({ scriptPath: join(WK, 'entry.mjs'), modules: true, modulesRules: [{ type: 'ESModule', include: ['**/*.js', '**/*.mjs'] }, { type: 'CompiledWasm', include: ['**/*.wasm'] }], compatibilityDate: '2024-09-01' });
  let coldStart;
  try {
    const mf = mkMf(); await mf.ready;
    const t0 = performance.now();
    const b = await withTimeout(mf.dispatchFetch('http://x/', { method: 'POST', body: readDoc('receipt') }).then((r) => r.json()), 60_000);
    coldStart = b === TIMEOUT ? { status: 'timeout' } : { wallMs: round(performance.now() - t0), instantiateMs: b.tInit, firstRenderMs: b.tRender, edgeEstimateMs: b.tInit + b.tRender + 5, caveat: 'miniflare runs a workerd subprocess whose routing overhead the real edge does not pay; wall time OVERSTATES production cold start (true edge isolate spin-up ~5ms). Instantiate/render split is accurate. Real production cold start needs a deployment (out of scope).' };
    await mf.dispose();
  } catch (e) { coldStart = { status: 'error', detail: String(e.message).split('\n')[0].slice(0, 100) }; }
  const mf = mkMf(); await mf.ready;
  await mf.dispatchFetch('http://x/', { method: 'POST', body: readDoc('receipt') }).catch(() => {});
  const docs = {};
  for (const d of DOCS) {
    const it = d.file === 'ledger-500p' ? 1 : Math.min(itersFor(d), 10);
    try {
      const res = await withTimeout((async () => {
        const rt = []; let passes = 0;
        for (let i = 0; i < it; i++) { const r = await mf.dispatchFetch('http://x/', { method: 'POST', body: readDoc(d.file) }); const b = await r.json(); rt.push(b.tRender); passes = b.passes; }
        return { medianMs: round(median(rt)), iters: it, passes };
      })(), d.timeoutMs);
      docs[d.file] = res === TIMEOUT ? { pages: d.pages, warm: { status: 'timeout', limitMs: d.timeoutMs } } : { pages: d.pages, warm: { status: 'ok', medianMs: res.medianMs, iters: res.iters }, passes: res.passes };
    } catch (e) {
      docs[d.file] = { pages: d.pages, warm: { status: 'error', detail: String(e.message).split('\n')[0].slice(0, 100) } };
    }
  }
  await mf.dispose();
  return { coldStart, docs };
}

// ── shared (machine-independent) ─────────────────────────────────────────────
function corpus() {
  const m = JSON.parse(readFileSync(join(CORPUS, 'manifest.json'), 'utf8'));
  return { fixed: true, hashChecked: true, generator: 'benchmarks/corpus/generate.mjs', documents: m.documents };
}
const METHOD = {
  coldStart: 'Process/isolate start through first PDF byte, including module instantiation.',
  warmRender: 'Steady state after warm-up; median and p95 over the published iteration count. Puppeteer reuses one browser across iterations (setContent + page.pdf each).',
  comparisonSurface: 'All targets and Puppeteer render byte-identical HTML from the fixed corpus, so every column is the same document.',
  twoRuns: 'Two environments are published side by side and neither is cherry-picked: a dev machine (clean, quiet, single run) shows the engine on good hardware; CI ubuntu-latest (a shared, noisy runner) is the conservative number and the one that is regenerated on every push. Where a document ran on one but not the other, the gap is shown rather than the row hidden.',
};
const SENTINEL_FIX = {
  method: 'A/B: pre-fix WASM rebuilt from HEAD~1 vs the fix, same machine/harness, node target, warm median.',
  finding: 'The published ~26ms figure was measured on a document doing two layout passes; the honest single-pass number is ~21ms.',
  rows: [
    { file: 'receipt', pages: 1, beforeMs: 19.0, afterMs: 17.0, beforePasses: 2, afterPasses: 1 },
    { file: 'report-6p', pages: 6, beforeMs: 26.7, afterMs: 21.3, beforePasses: 2, afterPasses: 1 },
    { file: 'compliance', pages: 2, beforeMs: 23.9, afterMs: 19.8, beforePasses: 2, afterPasses: 1 },
    { file: 'no-counter-100p', pages: 100, beforeMs: 1979, afterMs: 1033, beforePasses: 2, afterPasses: 1 },
    { file: 'no-counter-300p', pages: 300, beforeMs: 5887, afterMs: 3087, beforePasses: 2, afterPasses: 1 },
  ],
};
const WHERE_WE_LOSE = {
  method: 'FORME_PROFILE phase timing (native) + examples/allocprofile.rs (counting allocator), invoice-50p and ledger-500p.',
  phaseBreakdown: [
    { file: 'invoice-50p', pages: 50, passes: 1, parseMs: 14.3, layoutMs: 368.3, serializeMs: 24.9, layoutMsPerPagePerPass: 7.36 },
    { file: 'ledger-500p', pages: 500, passes: 2, parseMs: 116.9, layoutMs: 7227.9, serializeMs: 232.9, layoutMsPerPagePerPass: 7.23 },
  ],
  allocations: [
    { file: 'invoice-50p', passes: 1, totalAllocs: 9235766, peakLiveMB: 64, finalLiveMB: 0, allocsPerRowPerPass: 11545 },
    { file: 'ledger-500p', passes: 2, totalAllocs: 182824737, peakLiveMB: 984, finalLiveMB: 3, allocsPerRowPerPass: 11425 },
  ],
  decomposition: 'The 500-page gap vs Puppeteer (~3.8×) is two linear factors multiplying: ~2× from the page-number second layout pass, and ~1.9× from per-node allocation churn (~1,100 allocations per layout node; Chrome arena-allocates its layout). Per-pass layout is flat at ~7.3 ms/page from 50 to 500 pages — there is no super-linear scaling.',
  memoryShape: 'The full layout tree is retained until serialize (~2 MB/page), so peak memory is linear in document size and freed at exit — not a leak.',
  trackedFixes: [
    { name: 'Streaming (per-page/chunked) serialize-and-release', target: 'memory ceiling', note: 'Would cap peak at ~O(one page) and let large docs render on Workers. Structural; not yet built.' },
    { name: 'Reduce per-node allocation (arena / SmallVec / drop style clones)', target: 'throughput', note: 'Addresses the ~1.9× constant-factor gap. Hot-path; needs site-level allocation profiling first.' },
    { name: 'Sentinel re-pass re-measures only the running element', target: 'page-numbered large docs', note: 'Investigated 2026-09 and set aside: not scopable as written. The sentinel width is consumed during injection (footer/margin-box layout), not the flow pass, and HTML @page margin boxes are Fixed nodes — so a "reuse flow + re-inject" split has no guard signal to read and silently skips the correction (byte-identity caught it). Would need injection-time or model-scan detection.' },
  ],
};

// ── measure one run ──────────────────────────────────────────────────────────
async function safe(label, fn) { try { return await fn(); } catch (e) { console.error(`  (${label} failed: ${e.message})`); return { error: String(e.message).split('\n')[0].slice(0, 140) }; } }
async function measureRun() {
  const run = { environment: environment() };
  console.error('measuring node…'); run.node = await safe('node', async () => nodeMetrics());
  console.error('measuring native…'); run.native = await safe('native', async () => nativeMetrics());
  console.error('measuring web…'); run.web = await safe('web', webMetrics);
  console.error('measuring workerd…'); run.workerd = await safe('workerd', workerdMetrics);
  console.error('measuring puppeteer…'); run.puppeteer = await safe('puppeteer', puppeteerMetrics);
  return run;
}

// ── assemble two-run artifact + emit ─────────────────────────────────────────
const run = await measureRun();
const shared = { method: METHOD, corpus: corpus(), sentinelFix: SENTINEL_FIX, whereWeLose: WHERE_WE_LOSE };
let runs;
if (run.environment.runner === 'dev') {
  runs = [run];
} else {
  const baseline = existsSync(BASELINE) ? JSON.parse(readFileSync(BASELINE, 'utf8')) : null;
  const devRun = baseline?.runs?.find((r) => r.environment?.runner === 'dev') ?? baseline?.runs?.[0] ?? null;
  runs = [devRun, run].filter(Boolean);
}
const section = { ...shared, runs };
emitSection('benchmarks', section);

if (process.env.BENCH_SAVE_BASELINE && run.environment.runner === 'dev') {
  writeFileSync(BASELINE, JSON.stringify(section, null, 2) + '\n');
  console.error(`saved dev baseline → ${BASELINE}`);
}

// ── console render (FROM the object) ─────────────────────────────────────────
console.log(`\nBenchmarks — runs: ${runs.map((r) => `${r.environment.runner} (${r.environment.machine})`).join(', ')}`);
for (const r of runs) {
  console.log(`\n[${r.environment.runner}] cold start: native ${r.native?.docs?.receipt?.coldMs ?? '—'}ms · workerd ~${r.workerd?.coldStart?.edgeEstimateMs ?? '—'}ms · node ${r.node?.docs?.receipt?.cold?.totalWallMs ?? '—'}ms · puppeteer ${r.puppeteer?.coldStart?.totalMs ?? '—'}ms`);
  console.log(`[${r.environment.runner}] warm (forme node / pptr):`);
  for (const d of DOCS) {
    const n = r.node?.docs?.[d.file]?.warm, p = r.puppeteer?.docs?.[d.file]?.warm, w = r.web?.docs?.[d.file];
    const fm = n?.status === 'ok' ? n.medianMs + 'ms' : n?.status ?? '—';
    const pm = p?.status === 'ok' ? p.medianMs + 'ms' : p?.status ?? '—';
    const mem = w?.status === 'ok' ? w.wasmLinearMemMB + 'MB' : w?.status ?? '—';
    console.log(`  ${d.file.padEnd(16)} forme ${String(fm).padStart(9)}  pptr ${String(pm).padStart(9)}  wasmMem ${mem}`);
  }
}
console.log('\nemitted benchmarks.json' + (process.env.PARITY_DIR ? ` → ${process.env.PARITY_DIR}` : ' (PARITY_DIR unset — not written)'));
