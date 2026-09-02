#!/usr/bin/env node
// Deterministic benchmark corpus generator.
//
// Emits the fixed six-document corpus the benchmark suite measures. Every
// document is a pure function of constants below — no Date.now(), no random,
// no locale — so regenerating produces byte-identical HTML and the published
// numbers stay comparable across runs and machines. The generated .html files
// AND a manifest.json (with a sha256 of each) are committed; `--check`
// regenerates in memory and fails if anything drifts from what's on disk.
//
// The corpus is intentionally HTML/CSS: it is the one input surface shared by
// every Forme target (native `forme-html`, `@formepdf/html` on node/web/
// workerd) AND by Puppeteer — so Forme-vs-Forme and Forme-vs-Puppeteer both
// run on byte-identical documents. See benchmarks/README.md.
//
// Usage:
//   node benchmarks/corpus/generate.mjs          # write files + manifest
//   node benchmarks/corpus/generate.mjs --check   # verify on-disk == generated

import { readFileSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const HERE = dirname(fileURLToPath(import.meta.url));

// ── deterministic pseudo-data (index arithmetic only) ────────────────────────

const DESCRIPTIONS = [
  'Widget Pro Max', 'Enterprise License (Annual)', 'Cloud Hosting — Standard',
  'Consulting Hours', 'Support Package Premium', 'API Access (10k req/mo)',
  'Data Migration Service', 'Custom Integration Setup', 'Onboarding Session',
  'Priority SLA Add-on', 'Storage Overage (100GB)', 'Seat License (Team)',
];
const STATUSES = ['Paid', 'Pending', 'Overdue', 'Shipped', 'Cancelled'];

// A fixed reference date advanced by index — never the wall clock.
export function isoDate(i) {
  const day = 1 + (i % 28);
  const month = 1 + (Math.floor(i / 28) % 12);
  return `2025-${String(month).padStart(2, '0')}-${String(day).padStart(2, '0')}`;
}
function money(i) {
  const cents = ((i * 3773) % 900000) + 1099; // 10.99 .. ~9010.99, deterministic
  return (cents / 100).toFixed(2);
}
export function row(i) {
  const desc = DESCRIPTIONS[i % DESCRIPTIONS.length];
  const status = STATUSES[i % STATUSES.length];
  const qty = 1 + (i % 9);
  const unit = money(i);
  const amount = (qty * Number(unit)).toFixed(2);
  return { n: i + 1, date: isoDate(i), desc, status, qty, unit, amount };
}

// ── shared skeleton ──────────────────────────────────────────────────────────

function page(title, style, body) {
  return `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>${title}</title>
<style>
${style}
</style>
</head>
<body>
${body}
</body>
</html>
`;
}

const PROSE = [
  'The fourth quarter closed with revenue growth across every region, led by sustained demand in the enterprise segment and a healthy expansion of the mid-market book of business.',
  'Retention held at ninety-four percent, reflecting continued investment in product quality and customer success, and churn among annual contracts fell to its lowest level in eight quarters.',
  'Operating margin improved by three points as infrastructure costs declined on a per-render basis, a direct result of the migration away from headless-browser rendering in the document pipeline.',
  'Headcount grew modestly, concentrated in engineering and support, and the company enters the new fiscal year with eighteen months of runway and no external financing requirement.',
];

// ── 1. receipt — one page, simple ────────────────────────────────────────────

function receipt() {
  const items = Array.from({ length: 9 }, (_, i) => row(i));
  const subtotal = items.reduce((s, r) => s + Number(r.amount), 0);
  const tax = subtotal * 0.0825;
  const total = subtotal + tax;
  const lines = items
    .map(
      (r) =>
        `      <tr><td>${r.desc}</td><td class="q">${r.qty}</td><td class="a">$${r.amount}</td></tr>`,
    )
    .join('\n');
  const style = `
    @page { size: Letter; margin: 48pt; }
    body { font-family: Helvetica, Arial, sans-serif; color: #111; font-size: 11pt; }
    .store { text-align: center; margin-bottom: 18pt; }
    .store h1 { font-size: 18pt; margin: 0 0 2pt; }
    .store .meta { color: #555; font-size: 9pt; }
    table { width: 100%; border-collapse: collapse; margin-top: 8pt; }
    td { padding: 4pt 0; border-bottom: 1px solid #eee; }
    td.q { text-align: center; width: 40pt; color: #555; }
    td.a { text-align: right; width: 90pt; }
    .totals { margin-top: 12pt; width: 100%; }
    .totals td { border: none; padding: 2pt 0; }
    .totals .label { text-align: right; color: #555; }
    .totals .val { text-align: right; width: 90pt; }
    .totals .grand { font-weight: bold; font-size: 13pt; border-top: 2px solid #111; }
    .foot { text-align: center; margin-top: 24pt; color: #777; font-size: 9pt; }`;
  const body = `<div class="store">
  <h1>Corner Hardware Co.</h1>
  <div class="meta">1420 Market Street · Receipt #A-10427 · ${isoDate(0)}</div>
</div>
<table>
  <tbody>
${lines}
  </tbody>
</table>
<table class="totals">
  <tr><td class="label">Subtotal</td><td class="val">$${subtotal.toFixed(2)}</td></tr>
  <tr><td class="label">Tax (8.25%)</td><td class="val">$${tax.toFixed(2)}</td></tr>
  <tr><td class="label grand">Total</td><td class="val grand">$${total.toFixed(2)}</td></tr>
</table>
<div class="foot">Thank you for your business.</div>`;
  return page('Receipt #A-10427', style, body);
}

// ── 2. report — six pages, prose + one table (continuity doc) ─────────────────

function report6p() {
  const sections = [
    'Executive Summary', 'Revenue & Growth', 'Operating Efficiency',
    'Customer Retention', 'Regional Performance', 'Outlook',
  ];
  const tableRows = Array.from({ length: 10 }, (_, i) => {
    const r = row(i);
    return `    <tr><td>${r.date}</td><td>${r.desc}</td><td class="r">$${r.amount}</td><td>${r.status}</td></tr>`;
  }).join('\n');
  const secHtml = sections
    .map((title, i) => {
      const first = i === 0 ? ' class="first"' : '';
      const table =
        i === 4
          ? `\n  <table>\n    <thead><tr><th>Date</th><th>Item</th><th class="r">Amount</th><th>Status</th></tr></thead>\n    <tbody>\n${tableRows}\n    </tbody>\n  </table>`
          : '';
      const paras = (i === 4 ? PROSE.slice(0, 2) : PROSE)
        .map((p) => `  <p>${p}</p>`)
        .join('\n');
      return `<section${first}>\n  <h2>${i + 1}. ${title}</h2>\n${paras}${table}\n</section>`;
    })
    .join('\n');
  const style = `
    @page { size: Letter; margin: 72pt 60pt; }
    body { font-family: Helvetica, Arial, sans-serif; color: #1f2430; font-size: 11pt; line-height: 1.5; }
    h1 { font-size: 24pt; margin: 0 0 24pt; }
    section { break-before: page; }
    section.first { break-before: auto; }
    h2 { font-size: 16pt; color: #1a365d; margin: 0 0 8pt; }
    p { margin: 0 0 10pt; text-align: justify; }
    table { width: 100%; border-collapse: collapse; margin-top: 12pt; font-size: 10pt; }
    th, td { padding: 5pt 6pt; border-bottom: 1px solid #d8dee9; text-align: left; }
    th { background: #f1f5f9; }
    td.r, th.r { text-align: right; }`;
  const body = `<h1>Quarterly Report — FY2025 Q4</h1>\n${secHtml}`;
  return page('Quarterly Report — FY2025 Q4', style, body);
}

// ── 3 & 4. table-heavy statement (parameterized rows) ────────────────────────

export function statement(title, heading, rowCount) {
  const rows = Array.from({ length: rowCount }, (_, i) => {
    const r = row(i);
    return `    <tr><td class="n">${r.n}</td><td>${r.date}</td><td>${r.desc}</td><td class="c">${r.qty}</td><td class="r">$${r.unit}</td><td class="r">$${r.amount}</td><td>${r.status}</td></tr>`;
  }).join('\n');
  const style = `
    @page { size: Letter; margin: 54pt 48pt; @bottom-center { content: "Page " counter(page) " of " counter(pages); } }
    body { font-family: Helvetica, Arial, sans-serif; color: #1a1a1a; font-size: 9pt; }
    .head { margin-bottom: 12pt; }
    .head h1 { font-size: 18pt; margin: 0 0 2pt; }
    .head .meta { color: #555; font-size: 9pt; }
    table { width: 100%; border-collapse: collapse; }
    thead th { background: #1a365d; color: #fff; padding: 5pt 6pt; text-align: left; font-size: 9pt; }
    tbody td { padding: 4pt 6pt; border-bottom: 1px solid #e6e9ee; }
    tbody tr:nth-child(even) { background: #f6f8fa; }
    td.n { color: #888; width: 34pt; }
    td.c { text-align: center; }
    td.r, th.r { text-align: right; }`;
  const body = `<div class="head">
  <h1>${heading}</h1>
  <div class="meta">Account #4471-0092 · Statement period ${isoDate(0)} – ${isoDate(rowCount)}</div>
</div>
<table>
  <thead>
    <tr><th class="n">#</th><th>Date</th><th>Description</th><th class="c">Qty</th><th class="r">Unit</th><th class="r">Amount</th><th>Status</th></tr>
  </thead>
  <tbody>
${rows}
  </tbody>
</table>`;
  return page(title, style, body);
}

// ── 5. letterhead — paged-media: margin boxes + running header + counters ────

function letterhead() {
  const paras = [];
  for (let i = 0; i < 52; i++) paras.push(`  <p>${PROSE[i % PROSE.length]}</p>`);
  const style = `
    @page {
      size: Letter;
      margin: 108pt 72pt 84pt;
      @top-center { content: "MERIDIAN & COLE LLP — Attorneys at Law"; }
      @top-right { content: "Confidential"; }
      @bottom-center { content: "Page " counter(page) " of " counter(pages); }
      @bottom-left { content: "1420 Market Street, Suite 900"; }
    }
    body { font-family: Times, 'Times New Roman', serif; color: #1a1a1a; font-size: 11pt; line-height: 1.6; }
    h1 { font-size: 16pt; margin: 0 0 4pt; }
    .date { color: #555; margin-bottom: 18pt; }
    p { margin: 0 0 12pt; text-align: justify; }
    .sig { margin-top: 28pt; }`;
  const body = `<h1>Re: Matter No. 2025-0417</h1>
<div class="date">${isoDate(0)}</div>
${paras.join('\n')}
<div class="sig">Sincerely,<br><br>Meridian &amp; Cole LLP</div>`;
  return page('Letterhead — Matter No. 2025-0417', style, body);
}

// ── 6. compliance — rendered plain AND as pdfUa+pdfA (see harness) ───────────

function compliance() {
  const tableRows = Array.from({ length: 12 }, (_, i) => {
    const r = row(i);
    return `      <tr><td>${r.date}</td><td>${r.desc}</td><td class="r">$${r.amount}</td><td>${r.status}</td></tr>`;
  }).join('\n');
  const style = `
    @page { size: Letter; margin: 72pt; }
    body { font-family: Helvetica, Arial, sans-serif; color: #1a1a1a; font-size: 11pt; line-height: 1.5; }
    h1 { font-size: 22pt; margin: 0 0 12pt; }
    h2 { font-size: 15pt; color: #1a365d; margin: 18pt 0 6pt; }
    p { margin: 0 0 10pt; text-align: justify; }
    ul { margin: 0 0 10pt 18pt; }
    li { margin: 0 0 4pt; }
    table { width: 100%; border-collapse: collapse; margin-top: 8pt; font-size: 10pt; }
    th, td { padding: 5pt 6pt; border-bottom: 1px solid #d8dee9; text-align: left; }
    th { background: #f1f5f9; }
    td.r, th.r { text-align: right; }`;
  const body = `<h1>Annual Benefits Statement</h1>
<p>${PROSE[0]}</p>
<h2>Coverage Summary</h2>
<p>${PROSE[1]}</p>
<ul>
  <li>Medical, dental, and vision enrollment confirmed for the plan year.</li>
  <li>Employer contribution to the retirement plan matched to six percent.</li>
  <li>Health savings account funded per the elected schedule.</li>
</ul>
<h2>Transaction Detail</h2>
<table>
  <thead>
    <tr><th>Date</th><th>Description</th><th class="r">Amount</th><th>Status</th></tr>
  </thead>
  <tbody>
${tableRows}
  </tbody>
</table>
<h2>Notice</h2>
<p>${PROSE[2]}</p>
<p>${PROSE[3]}</p>`;
  return page('Annual Benefits Statement', style, body);
}

// ── emit / check ─────────────────────────────────────────────────────────────

// rowCounts calibrated to page targets after rendering (see manifest.pages).
export const DOCS = {
  'receipt.html': { build: receipt, target: '1 page', desc: 'Point-of-sale receipt — the simple single-page shape.' },
  'report-6p.html': { build: report6p, target: '6 pages', desc: 'Quarterly report — prose + one table. Continuity with the legacy ~26ms figure.' },
  'invoice-50p.html': { build: () => statement('Account Statement', 'Account Statement', 800), target: '~50 pages', desc: 'Table-heavy statement: repeating <thead>, zebra rows, many page breaks.' },
  'ledger-500p.html': { build: () => statement('Transaction Ledger', 'Transaction Ledger', 8000), target: '~500 pages', desc: 'The 500-page stress shape (the QuestPDF-thread document).' },
  'letterhead-paged.html': { build: letterhead, target: '~5 pages', desc: 'Paged-media overhead: @page margin boxes, running header/footer, page counters.' },
  'compliance.html': { build: compliance, target: '2 pages', desc: 'Rendered plain AND as PDF/UA-1 + PDF/A-2b — the delta is the cost of conformance.' },
};

// Only run the emit/check when invoked directly — importing this module for
// its builders (the harness, scaling probes) must have no side effects.
const isMain = import.meta.url === pathToFileURL(process.argv[1] || '').href;
if (isMain) {
const sha = (s) => createHash('sha256').update(s).digest('hex');
const check = process.argv.includes('--check');

const manifest = { generatedBy: 'benchmarks/corpus/generate.mjs', documents: [] };
let drift = false;
for (const [file, { build, target, desc }] of Object.entries(DOCS)) {
  const html = build();
  const path = join(HERE, file);
  const digest = sha(html);
  manifest.documents.push({ file, description: desc, target, bytes: Buffer.byteLength(html, 'utf8'), sha256: digest });
  if (check) {
    let onDisk = '';
    try { onDisk = readFileSync(path, 'utf8'); } catch { /* missing */ }
    if (onDisk !== html) { console.error(`DRIFT: ${file} on disk differs from generator output`); drift = true; }
  } else {
    writeFileSync(path, html);
    console.log(`wrote ${file}  (${Buffer.byteLength(html, 'utf8')} B, ${target})`);
  }
}

const manifestJson = JSON.stringify(manifest, null, 2) + '\n';
const manifestPath = join(HERE, 'manifest.json');
if (check) {
  let onDisk = '';
  try { onDisk = readFileSync(manifestPath, 'utf8'); } catch { /* missing */ }
  if (onDisk !== manifestJson) { console.error('DRIFT: manifest.json differs'); drift = true; }
  if (drift) { console.error('\nCorpus drift detected — run `node benchmarks/corpus/generate.mjs` and commit.'); process.exit(1); }
  console.log('corpus OK — on-disk matches generator');
} else {
  writeFileSync(manifestPath, manifestJson);
  console.log('wrote manifest.json');
}
}
