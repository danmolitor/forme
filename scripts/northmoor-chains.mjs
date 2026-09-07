#!/usr/bin/env node
// The Northmoor reconciling chains as a permanent gate. The design README
// says the cross-document arithmetic "is how the set is reviewed" — so it
// is asserted from the RENDERED text of each template (not from data.json,
// which could drift from the markup unnoticed). Run: node scripts/northmoor-chains.mjs
import { readFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { renderHtmlWithLayout } from '@formepdf/html';

const REPO = join(dirname(fileURLToPath(import.meta.url)), '..');
const T = join(REPO, 'templates');

function load(name) {
  const shared = readFileSync(join(T, 'northmoor-shared.css'), 'utf8');
  const own = readFileSync(join(T, name, 'style.css'), 'utf8');
  return readFileSync(join(T, name, 'index.html'), 'utf8')
    .replace('<link rel="stylesheet" href="../northmoor-shared.css">', `<style>${shared}</style>`)
    .replace('<link rel="stylesheet" href="style.css">', `<style>${own}</style>`);
}

const textCache = new Map();
function textOf(name) {
  if (!textCache.has(name)) {
    const { layout } = renderHtmlWithLayout(load(name), {});
    const parts = [];
    const walk = (els) => els.forEach((e) => { if (e.textContent) parts.push(e.textContent); walk(e.children ?? []); });
    layout.pages.forEach((p) => walk(p.elements));
    textCache.set(name, parts.join(' '));
  }
  return textCache.get(name);
}

let failures = 0;
function inDoc(name, needle, label) {
  const ok = textOf(name).includes(needle);
  if (!ok) failures++;
  console.log(`${ok ? '  ok ' : 'FAIL'}  ${label ?? `"${needle}" in ${name}`}`);
}
function calc(label, actual, expected) {
  const ok = Math.abs(actual - expected) < 0.005;
  if (!ok) failures++;
  console.log(`${ok ? '  ok ' : 'FAIL'}  ${label}: ${actual.toFixed(2)} vs ${expected.toFixed(2)}`);
}

// ── AR chain: INV-4417 → CN-0392 → RCT-9081 → statement ────────────────
inDoc('invoice-standard', '4,647.07');
inDoc('credit-note', '600.48');
inDoc('receipt', '4,046.59');
inDoc('receipt', 'RCT-9081');
// The statement's five OPEN ITEMS are invoices (the receipt shows as the
// payments-received aggregate, matching the prototype — verified against
// the design source, not assumed).
for (const n of ['INV-4417', 'CN-0392']) inDoc('statement', n, `${n} on statement`);
inDoc('statement', '35,731.24', 'statement total');
calc('aged buckets sum', 4046.59 + 9244.10 + 19290.55 + 0.0 + 3150.0, 35731.24);
for (const b of ['4,046.59', '9,244.10', '19,290.55', '3,150.00']) inDoc('statement', b, `bucket ${b} on statement`);

// ── INV-4421 progress billing ──────────────────────────────────────────
calc('INV-4421 sections', 16736.0 + 10422.3 + 18003.0 + 34832.0 + 5645.0, 85638.3);
calc('carried forward = taxable A+B+C', 16736.0 + 10422.3 + 18003.0, 45161.3);
for (const f of ['85,638.30', '45,161.30', '4,281.92', '3,612.90', '25,000.00', '59,969.28'])
  inDoc('invoice-detailed', f, `${f} on detailed invoice`);

// ── PO → remittance ────────────────────────────────────────────────────
inDoc('purchase-order', '10,940.00');
inDoc('remittance-advice', 'PO-77412', 'PO ref on remittance');
for (const f of ['8,742.60', '62.40', '215.00', '1,240.00'])
  inDoc('remittance-advice', f, `${f} on remittance`);

// ── employment chain ───────────────────────────────────────────────────
// The contract's prose writes "96,000 dollars"; the offer writes
// "96,000.00" — both the prototype's own copy.
inDoc('employment-contract', '96,000 dollars', 'salary in contract prose');
inDoc('offer-letter', '96,000.00', 'salary on offer');
for (const d of ['employment-contract', 'offer-letter'])
  inDoc(d, '5 October 2026', `start date on ${d}`);
inDoc('policy-acknowledgement', 'ACK-2026-20614');
inDoc('offer-letter', 'OFF-2026-0113');
inDoc('employment-contract', 'EMP-2026-0113');

// ── payslip ────────────────────────────────────────────────────────────
calc('payslip net', 3382.55 - 1150.34, 2232.21);
for (const f of ['3,382.55', '1,150.34', '2,232.21']) inDoc('payslip', f, `${f} on payslip`);

// ── annual report income statement ─────────────────────────────────────
calc('gross', 128412 - 96670, 31742);
calc('operating', 31742 - 20456, 11286);
calc('net', 11286 - 1204 - 2164, 7918);
for (const f of ['128,412', '31,742', '11,286', '7,918']) inDoc('report-annual', f, `${f} on annual report`);

// ── logistics chain ────────────────────────────────────────────────────
for (const [d, n] of [['packing-slip', 'PS-88214'], ['shipping-label', 'PS-88214'], ['delivery-note', 'PS-88214'],
                      ['packing-slip', 'R-07'], ['shipping-label', 'R-07'],
                      ['inspection-report', 'QA-2026-2087'], ['packing-slip', 'QA-2026-2087'],
                      ['shipping-label', '418 lb'], ['packing-slip', '418'], ['delivery-note', 'DN-40118']])
  inDoc(d, n, `${n} on ${d}`);

console.log(failures ? `\n${failures} chain check(s) FAILED` : '\nAll reconciling chains hold.');
process.exit(failures ? 1 : 0);
