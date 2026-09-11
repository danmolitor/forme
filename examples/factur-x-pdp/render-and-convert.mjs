#!/usr/bin/env node
// Factur-X via a PDP / conversion service — the worked example behind
// docs.formepdf.com/guides/e-invoicing-with-a-pdp.
//
// The pipeline, end to end:
//   1. Forme renders the Northmoor invoice template as PDF/A-3b — the
//      conformance level Factur-X requires — with fonts embedded, and
//      gates it on veraPDF locally BEFORE it leaves the process.
//   2. The PDF and the EN 16931 semantic model (en16931-invoice.json,
//      mapped from the template's own data.json — the mapping table is
//      on the docs page) go to the conversion endpoint as multipart.
//   3. The returned Factur-X is verified by veraPDF (PDF/A-3b) and
//      Mustangproject (EN 16931 schematron) — the same two validators
//      this repo's CI runs. What they report is printed verbatim.
//
// The endpoint defaults to SuperPDP's public convert API (its OpenAPI
// spec documents the call as unauthenticated); any service with the same
// PDF+data-in, Factur-X-out shape fits — set SUPERPDP_URL, and
// SUPERPDP_TOKEN if yours needs a bearer token.
//
// Run from the repo root (packages must be built — see examples README):
//   node examples/factur-x-pdp/render-and-convert.mjs
//
// Env:
//   SUPERPDP_URL    conversion service base (default https://api.superpdp.tech)
//   SUPERPDP_TOKEN  optional bearer token
//   VERAPDF         veraPDF binary (default ~/verapdf/verapdf; skipped if absent)
//   MUSTANG_JAR     Mustang CLI jar (default ~/mustang/Mustang-CLI.jar; skipped if absent)

import { readFileSync, writeFileSync, existsSync, mkdirSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { homedir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = join(HERE, '..', '..');
const OUT = join(HERE, 'out');
mkdirSync(OUT, { recursive: true });

const { renderHtml } = await import('@formepdf/html');
const { standardFonts } = await import('@formepdf/fonts-standard');

// ── validators (optional locally, but the run reports what it skipped) ──
const VERAPDF = process.env.VERAPDF ?? join(homedir(), 'verapdf', 'verapdf');
const MUSTANG = process.env.MUSTANG_JAR ?? join(homedir(), 'mustang', 'Mustang-CLI.jar');

function veraPdf(flavour, path) {
  if (!existsSync(VERAPDF)) return { skipped: true };
  try {
    const xml = execFileSync(VERAPDF, ['-f', flavour, '--format', 'xml', path], {
      encoding: 'utf8', maxBuffer: 64 * 1024 * 1024,
    });
    return { pass: /isCompliant="true"/.test(xml) };
  } catch (e) {
    // veraPDF exits non-zero on non-compliance but still writes the report.
    const xml = String(e.stdout ?? '');
    return { pass: /isCompliant="true"/.test(xml) };
  }
}

function mustang(path) {
  if (!existsSync(MUSTANG)) return { skipped: true };
  try {
    const out = execFileSync('java', [
      '-Xmx1G', '-Dfile.encoding=UTF-8', '-jar', MUSTANG,
      '--action', 'validate', '--source', path, '--no-notices',
    ], { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
    return { pass: /status="valid"/.test(out), report: out };
  } catch (e) {
    const out = String(e.stdout ?? '') + String(e.stderr ?? '');
    return { pass: /status="valid"/.test(out), report: out };
  }
}

// ── 1. render the invoice as PDF/A-3b, fonts embedded ──────────────────
const T = join(ROOT, 'templates');
const sharedCss = readFileSync(join(T, 'northmoor-shared.css'), 'utf8');
const ownCss = readFileSync(join(T, 'invoice-standard', 'style.css'), 'utf8');
const html = readFileSync(join(HERE, 'invoice.html'), 'utf8')
  .replace('<link rel="stylesheet" href="../../templates/northmoor-shared.css">', `<style>${sharedCss}</style>`)
  .replace('<link rel="stylesheet" href="../../templates/invoice-standard/style.css">', `<style>${ownCss}</style>`);

const fonts = standardFonts().map((f) => ({
  family: f.family, data: f.src, weight: f.fontWeight, italic: f.fontStyle === 'italic',
}));

const { pdf, warnings } = renderHtml(html, { pdfA: '3b', fonts, lang: 'fr' });
for (const w of warnings ?? []) console.warn('render warning:', w);
const pdfPath = join(OUT, 'invoice-a3b.pdf');
writeFileSync(pdfPath, pdf);
console.log(`rendered ${pdfPath} (${pdf.length} bytes)`);

// The point of the exercise: the container is conformant BEFORE the
// service sees it. Refuse to post a PDF that fails its own claim.
const pre = veraPdf('3b', pdfPath);
if (pre.skipped) console.log('veraPDF not found — pre-flight PDF/A-3b check SKIPPED');
else if (pre.pass) console.log('pre-flight: veraPDF PDF/A-3b PASS');
else { console.error('pre-flight: veraPDF PDF/A-3b FAIL — not posting a non-conformant PDF'); process.exit(1); }

// ── 2. post PDF + EN 16931 model, get the Factur-X back ────────────────
const BASE = process.env.SUPERPDP_URL ?? 'https://api.superpdp.tech';
const url = `${BASE}/v1.beta/invoices/convert?from=en16931&to=factur-x`;
const invoiceJson = readFileSync(join(HERE, 'en16931-invoice.json'), 'utf8');

const form = new FormData();
form.append('invoice', new Blob([invoiceJson], { type: 'application/json' }), 'invoice.json');
form.append('pdf', new Blob([pdf], { type: 'application/pdf' }), 'invoice-a3b.pdf');

const headers = {};
if (process.env.SUPERPDP_TOKEN) headers.Authorization = `Bearer ${process.env.SUPERPDP_TOKEN}`;

console.log(`POST ${url}`);
const res = await fetch(url, { method: 'POST', body: form, headers });

if (!res.ok) {
  // The spec's error shape: { http_status_code, message } on 400/500.
  let detail = '';
  try {
    const body = await res.json();
    detail = body.message ?? JSON.stringify(body);
  } catch { detail = await res.text().catch(() => ''); }
  console.error(`conversion failed: HTTP ${res.status}${detail ? ` — ${detail}` : ''}`);
  process.exit(1);
}

const facturX = Buffer.from(await res.arrayBuffer());
const outPath = join(OUT, 'factur-x.pdf');
writeFileSync(outPath, facturX);
console.log(`received ${outPath} (${facturX.length} bytes, ${res.headers.get('content-type')})`);

// ── 3. verify the returned file — both validators, verdicts verbatim ───
let failed = false;

const post = veraPdf('3b', outPath);
if (post.skipped) console.log('veraPDF not found — returned-file PDF/A-3b check SKIPPED');
else { console.log(`returned file: veraPDF PDF/A-3b ${post.pass ? 'PASS' : 'FAIL'}`); failed ||= !post.pass; }

const m = mustang(outPath);
if (m.skipped) console.log('Mustang not found — EN 16931 schematron check SKIPPED');
else { console.log(`returned file: Mustang (Factur-X / EN 16931) ${m.pass ? 'PASS' : 'FAIL'}`); failed ||= !m.pass; }
if (m.report) writeFileSync(join(OUT, 'mustang-report.xml'), m.report);

process.exit(failed ? 1 : 0);
