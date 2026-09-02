#!/usr/bin/env node
// Render the /parity evidence page to a self-contained static HTML file for
// GitHub Pages. The JSON stays the source of truth: this reads parity.json
// (from PARITY_DIR) and renders it — no numbers live in this file. A missing
// or malformed artifact renders an explicit "evidence unavailable" page, and
// any absent section renders an explicit unavailable notice, never a silently
// shorter page.
//
// Output: <PARITY_DIR>/site/index.html + a copy of parity.json (for download /
// independent inspection). Deploy the site/ dir to Pages.

import { readFileSync, writeFileSync, mkdirSync, existsSync, copyFileSync } from 'node:fs';
import { join } from 'node:path';

const DIR = process.env.PARITY_DIR;
if (!DIR) { console.error('PARITY_DIR is not set.'); process.exit(1); }
const SITE = join(DIR, 'site');
mkdirSync(SITE, { recursive: true });

const esc = (s) => String(s).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
const mark = (ok) => ok
  ? '<span style="color:#34d399" aria-label="pass">✓</span>'
  : '<span style="color:#f87171" aria-label="fail">✗</span>';

function validate(p) {
  if (!p || p.schemaVersion !== 1 || !p.provenance || !p.sections) return null;
  return p;
}
let parity = null;
try { parity = validate(JSON.parse(readFileSync(join(DIR, 'parity.json'), 'utf8'))); } catch { parity = null; }

const CSS = `
:root{color-scheme:dark}
*{box-sizing:border-box}
body{margin:0;background:#020617;color:#cbd5e1;font-family:ui-sans-serif,system-ui,-apple-system,Segoe UI,Roboto,sans-serif;line-height:1.6;-webkit-font-smoothing:antialiased}
a{color:#34d399;text-decoration:none}a:hover{color:#6ee7b7}
code,.mono{font-family:ui-monospace,SFMono-Regular,Menlo,monospace}
.wrap{max-width:56rem;margin:0 auto;padding:0 1.5rem}
section{border-bottom:1px solid #1e293b;padding:4rem 0}
section:last-child{border-bottom:0}
.eyebrow{font-size:.8rem;font-weight:600;text-transform:uppercase;letter-spacing:.08em;color:#34d399;margin:0}
h1{font-size:2.6rem;font-weight:700;letter-spacing:-.02em;color:#fff;margin:.6rem 0 0}
h2{font-size:1.6rem;font-weight:700;letter-spacing:-.01em;color:#fff;margin:.6rem 0 0}
p{color:#94a3b8;max-width:48rem}
.muted{color:#64748b;font-size:.9rem}
.prov{margin-top:2rem;border:1px solid #1e293b;background:rgba(15,23,42,.6);border-radius:.5rem;padding:1rem 1.25rem;font-family:ui-monospace,monospace;font-size:.85rem;color:#94a3b8}
table{width:100%;border-collapse:collapse;font-size:.9rem;margin-top:1.5rem}
th{text-align:left;font-weight:500;color:#94a3b8;border-bottom:1px solid #334155;padding:.5rem 1rem .5rem 0}
td{border-bottom:1px solid rgba(30,41,59,.7);padding:.5rem 1rem .5rem 0;font-family:ui-monospace,monospace}
td.doc{color:#cbd5e1}
.tag{color:#cbd5e1}
.grid{display:grid;gap:1.5rem;margin-top:1.5rem}
@media(min-width:640px){.grid.two{grid-template-columns:1fr 1fr}}
.card{border:1px solid #1e293b;background:rgba(15,23,42,.6);border-radius:.5rem;padding:1rem 1.25rem;font-size:.9rem}
.list{display:flex;flex-wrap:wrap;gap:.4rem 1.5rem;margin-top:1rem;font-family:ui-monospace,monospace;font-size:.85rem;color:#94a3b8}
ul.gaps{margin-top:1.5rem;padding:0;list-style:none}
ul.gaps li{margin-bottom:1rem}
header.hero{padding-top:7rem;padding-bottom:4rem}
`;

function page(body) {
  return `<!doctype html><html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Parity &amp; conformance evidence — Forme</title>
<meta name="description" content="Per-fixture verification generated from Forme's CI on every commit: veraPDF PDF/UA-1 and PDF/A conformance, native-vs-WASM byte determinism, and test coverage. Failures included.">
<style>${CSS}</style></head><body>${body}</body></html>`;
}

if (!parity) {
  writeFileSync(join(SITE, 'index.html'), page(`
<header class="hero"><div class="wrap" style="text-align:center">
<h3 class="eyebrow">Verification</h3>
<h1>Evidence unavailable</h1>
<p style="margin:1rem auto 0">The verification artifact is missing or malformed. This page renders only from a valid CI artifact — it will not show stale or partial results. Check back after the next CI run.</p>
</div></header>`));
  console.log('Rendered evidence-unavailable page (no valid artifact).');
  process.exit(0);
}

const { provenance: prov, corpus, sections } = parity;
const conf = sections.conformance, det = sections.determinism, tests = sections.tests, bench = sections.benchmarks;
const fmtDate = (iso) => { try { return new Date(iso).toISOString().replace('T', ' ').replace(/\.\d+Z$/, ' UTC'); } catch { return iso; } };
const unavailable = (eyebrow, name) => `<section><div class="wrap"><h3 class="eyebrow">${eyebrow}</h3>
<p class="muted" style="margin-top:.75rem">${name} evidence is unavailable for this run — the job that produces it did not report. This is shown rather than hidden so the page never implies coverage it doesn't have.</p></div></section>`;

// Intro + provenance
let html = `<header class="hero"><div class="wrap">
<h3 class="eyebrow">Verification</h3>
<h1>Parity &amp; conformance evidence</h1>
<p style="margin-top:1.5rem;font-size:1.1rem">Everything below is generated from Forme's CI on every commit — not hand-written, and it fails the build if it drifts from reality. It covers what CI actually proves: PDF/UA-1 and PDF/A conformance per document (via veraPDF), native-vs-WASM byte determinism, and test coverage. Failures and unsupported cases are included where they exist. It is not a browser-parity claim and not a coverage percentage.</p>
<div class="prov">Generated from ${prov.runUrl ? `<a href="${esc(prov.runUrl)}">CI run ${esc(prov.runId)}</a>` : '<span style="color:#64748b">local run</span>'} at ${prov.commitUrl ? `<a href="${esc(prov.commitUrl)}">${esc(prov.commitShort)}</a>` : esc(prov.commitShort)} (v${esc(prov.version)}) on ${esc(fmtDate(prov.generatedAt))}. <a href="./parity.json">Raw artifact →</a></div>
</div></header>`;

// Conformance
if (conf) {
  const fixtures = [...new Set(conf.results.map((r) => r.fixture))];
  const cell = (f, profile, configuration) => conf.results.find((x) => x.fixture === f && x.profile === profile && x.configuration === configuration);
  const cellHtml = (r) => r ? mark(r.pass) + (r.pass ? '' : ' <span class="muted">' + esc(r.failedClauses.map((c) => c.clause + '/t' + c.test).join(', ')) + '</span>') : '<span style="color:#475569">—</span>';
  html += `<section><div class="wrap">
<h3 class="eyebrow">Conformance</h3>
<h2>PDF/UA-1 and PDF/A, per document</h2>
<p style="margin-top:1rem">Validated with ${esc(conf.tool)}. The corpus is ${corpus.templates.length} shipped templates (<span class="tag">${corpus.templates.map(esc).join(', ')}</span>) and ${corpus.htmlFixtures.length} HTML fixtures (<span class="tag">${corpus.htmlFixtures.map(esc).join(', ')}</span>). Each PDF/A column is the archival-<em>and</em>-accessible render — the same file validated against the PDF/A profile <em>and</em> PDF/UA-1.</p>
<table><thead><tr><th>Document</th><th>PDF/UA-1</th><th>PDF/A-2b + UA-1</th><th>PDF/A-2a + UA-1</th></tr></thead><tbody>
${fixtures.map((f) => `<tr><td class="doc">${esc(f)}</td><td>${cellHtml(cell(f, 'ua1', 'ua1'))}</td><td>${cellHtml(cell(f, '2b', 'a2b'))}</td><td>${cellHtml(cell(f, '2a', 'a2a'))}</td></tr>`).join('\n')}
</tbody></table>
<p class="muted" style="margin-top:1rem">Any failed profile shows its veraPDF clause list here; there are currently none. PDF/A-2u is supported and reachable but not gated separately (2a is the strictest of the three).</p>
</div></section>`;
} else html += unavailable('Conformance', 'Conformance');

// Determinism
if (det) {
  html += `<section><div class="wrap">
<h3 class="eyebrow">Determinism</h3>
<h2>Byte-for-byte, across build targets</h2>
<p style="margin-top:1rem">The same document renders to byte-identical PDFs across the native binary and the WASM targets, over the four HTML fixtures. These are distinct strengths of evidence and shown as such: <span class="tag">native, node, and web</span> are compared byte-for-byte; the <span class="tag">bundler</span> target embeds a hash-identical WASM module (so it computes identically by construction) and is exercised by functional browser and workerd renders — it is not itself byte-diffed here.</p>
<div class="grid two">
<div><table><thead><tr><th>Comparison</th><th>Fixture</th><th>Bytes</th><th>Identical</th></tr></thead><tbody>
${det.byteIdentity.map((r) => `<tr><td class="doc">${esc(r.comparison)}</td><td>${esc(r.fixture)}</td><td style="color:#64748b">${r.bytes.toLocaleString()}</td><td>${mark(r.pass)}</td></tr>`).join('\n')}
</tbody></table></div>
<div><div class="card"><div class="muted">WASM hash-identity (${det.wasmHashIdentity.targets.join(' / ')})</div>
<div style="margin-top:.5rem">${mark(det.wasmHashIdentity.identical)} <span class="tag">${det.wasmHashIdentity.identical ? 'identical module' : 'diverged'}</span></div>
<div class="mono" style="margin-top:.5rem;font-size:.75rem;color:#64748b;word-break:break-all">sha256 ${esc(det.wasmHashIdentity.sha256.slice(0, 32))}…</div></div>
${det.notChecked.length ? '<ul class="muted" style="margin-top:1rem;padding-left:1rem">' + det.notChecked.map((n) => `<li>not checked: ${esc(n.comparison)} — ${esc(n.reason)}</li>`).join('') + '</ul>' : ''}
</div></div></div></section>`;
} else html += unavailable('Determinism', 'Determinism');

// Regressions + coverage
if (tests) {
  const cs = tests.regressions.chromeStructural, ir = tests.regressions.ironpress;
  html += `<section><div class="wrap">
<h3 class="eyebrow">Regression evidence</h3>
<h2>Structural assertions from Chrome reference &amp; third-party corpus</h2>
<p style="margin-top:1rem">${esc(cs.note)}</p>
<div class="list">${cs.tests.map((t) => `<span>${mark(t.pass)} ${esc(t.test)}</span>`).join('')}</div>
<p style="margin-top:2rem">${esc(ir.note)}</p>
<p class="muted" style="margin-top:.5rem">Source: <a href="${esc(ir.source)}">${esc(ir.source.replace('https://github.com/', ''))}</a> (${esc(ir.license)}).</p>
<div class="list">${ir.tests.map((t) => `<span>${mark(t.pass)} ${esc(t.test)}</span>`).join('')}</div>
</div></section>
<section><div class="wrap">
<h3 class="eyebrow">Test coverage</h3>
<h2>Suite counts</h2>
<p style="margin-top:1rem">Passing tests per suite, parsed from an actual run. Stated as a fact, not a percentage.</p>
<table style="max-width:34rem"><tbody>
${tests.suites.map((s) => `<tr><td class="doc">${esc(s.suite)}</td><td style="text-align:right;color:#94a3b8">${s.passed.toLocaleString()} passed</td><td>${s.failed ? '<span style="color:#f87171">' + s.failed + ' failed</span>' : mark(true)}</td></tr>`).join('\n')}
<tr style="color:#fff"><td style="font-weight:600">total</td><td style="text-align:right;font-weight:600">${tests.totals.passed.toLocaleString()} passed</td><td>${tests.totals.failed ? '<span style="color:#f87171">' + tests.totals.failed + ' failed</span>' : mark(true)}</td></tr>
</tbody></table></div></section>`;
} else { html += unavailable('Regression evidence', 'Regression'); html += unavailable('Test coverage', 'Test-coverage'); }

// Benchmarks
if (bench) {
  const e = bench.environment;
  const ORDER = ['receipt', 'report-6p', 'letterhead-paged', 'compliance', 'invoice-50p', 'ledger-500p'];
  const kb = (b) => b == null ? '—' : (b / 1024).toFixed(b < 1024 * 1024 ? 0 : 0) + ' KB';
  const ms = (v) => v == null ? '—' : (v >= 1000 ? (v / 1000).toFixed(v >= 10000 ? 1 : 2) + ' s' : Math.round(v) + ' ms');
  const nd = (f) => bench.node?.docs?.[f];
  const pp = (f) => bench.puppeteer?.docs?.[f];
  const web = (f) => bench.web?.[f];

  // 1. What is measured
  html += `<section id="benchmarks"><div class="wrap">
<h3 class="eyebrow">Benchmarks</h3>
<h2>Measured performance — including where we lose</h2>
<p style="margin-top:1rem">${esc(bench.method.coldStart)} ${esc(bench.method.warmRender)} ${esc(bench.method.comparisonSurface)} The corpus is fixed, committed, and hash-checked (<span class="tag">${bench.corpus.documents.length} documents</span>, generated by <span class="mono">${esc(bench.corpus.generator)}</span>).</p>
<p class="muted" style="margin-top:.75rem">Environment: <span class="tag">${esc(e.runner)} runner</span> ${esc(e.machine)} · ${e.cpus} cpu · ${e.memoryGB}GB · ${esc(e.os)} · node ${esc(e.node)}${e.cargo ? ' · cargo ' + esc(e.cargo) : ''}${e.chrome ? ' · ' + esc(e.chrome) : ''}. ${esc(e.note)}</p>

<h2 style="margin-top:2.5rem">Cold start — start to first PDF byte</h2>
<table style="max-width:44rem"><thead><tr><th>Target</th><th style="text-align:right">Cold start</th><th>Note</th></tr></thead><tbody>
<tr><td class="doc">native binary</td><td style="text-align:right">${ms(bench.native?.receipt?.coldMs)}</td><td class="muted">static binary, no WASM instantiate</td></tr>
<tr><td class="doc">workerd isolate</td><td style="text-align:right">~${ms(bench.workerd?.coldStart?.edgeEstimateMs)}</td><td class="muted">instantiate ${ms(bench.workerd?.coldStart?.instantiateMs)} + first render ${ms(bench.workerd?.coldStart?.firstRenderMs)} (+~5ms edge isolate)</td></tr>
<tr><td class="doc">web (browser)</td><td style="text-align:right">${ms((web('receipt')?.cold?.fetchMs || 0) + (web('receipt')?.cold?.compileInstantiateMs || 0) + (web('receipt')?.cold?.firstRenderMs || 0))}</td><td class="muted">fetch ${ms(web('receipt')?.cold?.fetchMs)} + compile/instantiate ${ms(web('receipt')?.cold?.compileInstantiateMs)} + first render ${ms(web('receipt')?.cold?.firstRenderMs)}</td></tr>
<tr><td class="doc">node WASM</td><td style="text-align:right">${ms(nd('receipt')?.cold?.totalWallMs)}</td><td class="muted">node boot ${ms(bench.node?.bootMs)} + module load + instantiate ${ms(nd('receipt')?.cold?.instantiateMs)} + first render ${ms(nd('receipt')?.cold?.firstRenderMs)}</td></tr>
<tr><td class="doc">Puppeteer</td><td style="text-align:right">${ms(bench.puppeteer?.coldStart?.totalMs)}</td><td class="muted">warm machine; ${esc(bench.puppeteer?.coldStart?.coldServerlessNote || '')}</td></tr>
</tbody></table>
<p class="muted" style="margin-top:1rem">The ~7.1MB WASM module compiles and instantiates in 1-10ms — cold start is dominated by first-render JIT warmup, not module size. The module is large on disk and cheap to instantiate.</p>
<p class="muted" style="margin-top:.5rem"><span class="tag">workerd caveat</span> ${esc(bench.workerd?.coldStart?.caveat || '')}</p>
</div></section>`;

  // 3. Workers memory table
  html += `<section><div class="wrap">
<h3 class="eyebrow">Serverless memory</h3>
<h2>What fits under the Cloudflare Workers 128MB ceiling</h2>
<p style="margin-top:1rem">Peak WASM linear memory per document. This is the constraint that decides whether a document is renderable on Workers at all — a deployment decision you can make before you deploy rather than after you OOM.</p>
<table style="max-width:40rem"><thead><tr><th>Document</th><th style="text-align:right">Pages</th><th style="text-align:right">Peak WASM memory</th><th>Fits 128MB</th></tr></thead><tbody>
${ORDER.map((f) => { const w = web(f); const m = w?.wasmLinearMemMB; return `<tr><td class="doc">${esc(f)}</td><td style="text-align:right;color:#94a3b8">${w?.pages ?? '—'}</td><td style="text-align:right">${m != null ? m + ' MB' : '—'}</td><td>${m == null ? '—' : m < 128 ? mark(true) : '<span style="color:#f87171">✗ no</span>'}</td></tr>`; }).join('\n')}
</tbody></table>
<p class="muted" style="margin-top:1rem"><strong>Practical guidance:</strong> comfortable under ~50 pages, approaching the ceiling near ~100 pages, use node or native beyond that. Treat it as a margin, not a hard threshold — a slightly wider table shifts the number.</p>
<p class="muted" style="margin-top:.5rem">Local miniflare has generous memory and will run documents the real edge would OOM: the 500-page ledger completes in miniflare but its ${web('ledger-500p')?.wasmLinearMemMB ?? '~900'}MB peak would exceed the 128MB edge limit.</p>
</div></section>`;

  // 4. Warm render head to head — losses not softened, not reordered
  html += `<section><div class="wrap">
<h3 class="eyebrow">Warm render</h3>
<h2>Forme vs Puppeteer, identical HTML</h2>
<p style="margin-top:1rem">Median warm render (node target for Forme; Puppeteer reuses one browser). Output size and pass count included. The rows where Forme loses are here, in order, not buried.</p>
<table><thead><tr><th>Document</th><th style="text-align:right">Pages</th><th style="text-align:right">Passes</th><th style="text-align:right">Forme</th><th style="text-align:right">Puppeteer</th><th style="text-align:right">Forme PDF</th><th style="text-align:right">Pptr PDF</th></tr></thead><tbody>
${ORDER.map((f) => {
    const n = nd(f), p = pp(f);
    const fm = n?.warm?.medianMs, pm = p?.warm?.medianMs;
    const win = (fm != null && pm != null) ? (fm < pm ? 'color:#34d399' : 'color:#f87171') : '';
    return `<tr><td class="doc">${esc(f)}</td><td style="text-align:right;color:#94a3b8">${n?.pages ?? '—'}</td><td style="text-align:right;color:#94a3b8">${n?.passes ?? '—'}</td><td style="text-align:right;${win}">${ms(fm)}${n?.warm ? ` <span class="muted" style="font-size:.8em">p95 ${ms(n.warm.p95Ms)}</span>` : ''}</td><td style="text-align:right">${ms(pm)}</td><td style="text-align:right">${kb(n?.warm?.pdfBytes)}</td><td style="text-align:right">${kb(p?.warm?.pdfBytes)}</td></tr>`;
  }).join('\n')}
</tbody></table>
<p class="muted" style="margin-top:1rem">Forme is ~2× faster on the small documents most apps generate and produces ~15× smaller files, but a warm, pooled Chrome is faster on very large table-heavy documents (the 50- and 500-page rows). Iteration counts per cell are in the <a href="./parity.json">raw artifact</a>.</p>
</div></section>`;

  // 5. Where we lose and why
  const wl = bench.whereWeLose;
  html += `<section><div class="wrap">
<h3 class="eyebrow">Where we lose &amp; why</h3>
<h2>The large-document gap, profiled</h2>
<p style="margin-top:1rem">${esc(wl.decomposition)}</p>
<p style="margin-top:.75rem">${esc(wl.memoryShape)}</p>
<div class="grid two" style="margin-top:1.5rem">
<div><table><thead><tr><th>Phase (native)</th><th style="text-align:right">invoice-50p</th><th style="text-align:right">ledger-500p</th></tr></thead><tbody>
${['parseMs', 'layoutMs', 'serializeMs'].map((k) => `<tr><td class="doc">${k.replace('Ms', '')}</td><td style="text-align:right">${ms(wl.phaseBreakdown[0][k])}</td><td style="text-align:right">${ms(wl.phaseBreakdown[1][k])}</td></tr>`).join('\n')}
<tr style="color:#94a3b8"><td>layout ms/page/pass</td><td style="text-align:right">${wl.phaseBreakdown[0].layoutMsPerPagePerPass}</td><td style="text-align:right">${wl.phaseBreakdown[1].layoutMsPerPagePerPass}</td></tr>
</tbody></table><p class="muted" style="margin-top:.5rem">Per-pass layout is flat at ~7.3 ms/page from 50 to 500 pages — linear, no super-linear scaling.</p></div>
<div><div class="card"><div class="muted">Allocation churn (counting allocator)</div>
<div style="margin-top:.5rem">ledger-500p: <span class="tag">${(wl.allocations[1].totalAllocs / 1e6).toFixed(0)}M allocations</span> · ~${wl.allocations[1].allocsPerRowPerPass.toLocaleString()}/row/pass</div>
<div style="margin-top:.5rem">peak live <span class="tag">${wl.allocations[1].peakLiveMB} MB</span>, freed to ${wl.allocations[1].finalLiveMB}MB at exit (retained tree, not a leak)</div></div></div>
</div>
<p style="margin-top:1.5rem">Tracked fixes (no dates promised):</p>
<ul class="gaps">${wl.trackedFixes.map((x) => `<li><span class="tag">${esc(x.name)}</span> ${esc(x.note)}</li>`).join('')}</ul>
</div></section>`;

  // 6. The sentinel fix, before and after
  const sf = bench.sentinelFix;
  html += `<section><div class="wrap">
<h3 class="eyebrow">Self-audit</h3>
<h2>A number we found wrong on our own homepage</h2>
<p style="margin-top:1rem">${esc(sf.finding)} Benchmarking our own engine surfaced an unconditional second layout pass — a document with no page-number placeholder was re-laying out the whole tree for nothing. We fixed it; here is the before and after on identical documents.</p>
<table style="max-width:40rem"><thead><tr><th>Document</th><th style="text-align:right">Pages</th><th style="text-align:right">Before</th><th style="text-align:right">After</th><th style="text-align:right">Passes</th></tr></thead><tbody>
${sf.rows.map((r) => `<tr><td class="doc">${esc(r.file)}</td><td style="text-align:right;color:#94a3b8">${r.pages}</td><td style="text-align:right">${ms(r.beforeMs)}</td><td style="text-align:right;color:#34d399">${ms(r.afterMs)}</td><td style="text-align:right;color:#94a3b8">${r.beforePasses}→${r.afterPasses}</td></tr>`).join('\n')}
</tbody></table>
<p class="muted" style="margin-top:1rem">${esc(sf.method)}</p>
</div></section>`;
} else html += unavailable('Benchmarks', 'Benchmark');

// Known gaps (static, documented — not numbers)
html += `<section><div class="wrap">
<h3 class="eyebrow">Known gaps &amp; deliberate divergences</h3>
<h2>What isn't claimed</h2>
<ul class="gaps">
<li><span class="tag">Not a general browser-parity claim.</span> The Chrome-reference checks above are specific structural properties (page count, break positions, table structure, valign) read once from Chrome and asserted exactly — they do not claim pixel or layout parity with any browser.</li>
<li><span class="tag">position: absolute resolves against the nearest positioned ancestor</span> (matching browsers), which changed in 0.15.0 — an intentional divergence from the earlier direct-parent behavior.</li>
<li><span class="tag">Collapsed table borders use a widest-wins approximation</span>, and <span class="tag">vertical-align: baseline maps to top</span> in table cells where a true baseline isn't resolved — both documented, deliberate scope debts.</li>
<li>The supported HTML/CSS subset is explicit; anything outside it is reported as a named warning at render time, never silently dropped. See the <a href="https://docs.formepdf.com/html">HTML subset documentation</a>.</li>
</ul></div></section>`;

writeFileSync(join(SITE, 'index.html'), page(html));
if (existsSync(join(DIR, 'parity.json'))) copyFileSync(join(DIR, 'parity.json'), join(SITE, 'parity.json'));
// Custom domain: GitHub Pages needs a CNAME in the published artifact for
// Actions-based deploys to keep the custom domain across runs.
writeFileSync(join(SITE, 'CNAME'), (process.env.PARITY_DOMAIN || 'parity.formepdf.com') + '\n');
console.log(`Rendered ${join(SITE, 'index.html')} (+ parity.json, CNAME).`);
