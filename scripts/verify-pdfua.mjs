#!/usr/bin/env node
// PDF/UA-1 conformance gate.
//
// Renders the full conformance corpus — the five shipped @formepdf/templates
// plus the four HTML fixtures — in pdfUa mode with a metric-compatible font
// (@formepdf/fonts-standard) registered, then validates every output with
// veraPDF against the PDF/UA-1 profile. Exits non-zero if any file fails.
//
// veraPDF is an external Java tool. Point at it with the VERAPDF env var, or
// this falls back to ~/verapdf/verapdf. When the binary is absent the script
// still RENDERS the whole corpus (surfacing any render/crash regression) and
// exits 0 with a skip notice, so it is safe to run anywhere; CI installs
// veraPDF and thus runs the full validation.

import { existsSync, mkdirSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir, homedir } from 'node:os';
import { basename, join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { readFile } from 'node:fs/promises';

import { emitSection, keepReport, veraValidate, veraVersion } from './parity/lib.mjs';

import { serialize } from '@formepdf/react';
import { getTemplate } from '@formepdf/templates';
import {
  invoiceExample,
  receiptExample,
  reportExample,
  shippingLabelExample,
  letterExample,
} from '@formepdf/templates/schemas';
import { standardFonts } from '@formepdf/fonts-standard';
import { renderPdfWithLayout } from '@formepdf/core';
import { renderHtml } from '@formepdf/html';

const HERE = dirname(fileURLToPath(import.meta.url));
const REPO = join(HERE, '..');
const FIXTURES = join(REPO, 'html', 'tests', 'fixtures');
const LANG = 'en-US';

// Fonts for the core/JSX path: base64 src, engine's FontEntry shape.
const CORE_FONTS = standardFonts().map((f) => ({
  family: f.family,
  src: Buffer.from(f.src).toString('base64'),
  weight: f.fontWeight,
  italic: f.fontStyle === 'italic',
}));

// Fonts for the HTML path: raw bytes under RenderHtmlOptions.fonts.
const HTML_FONTS = standardFonts().map((f) => ({
  family: f.family,
  data: f.src,
  weight: f.fontWeight,
  italic: f.fontStyle === 'italic',
}));

const TEMPLATES = {
  invoice: invoiceExample,
  receipt: receiptExample,
  report: reportExample,
  'shipping-label': shippingLabelExample,
  letter: letterExample,
};

const HTML_FIXTURES = ['letterhead', 'dashed-borders', 'statement', 'zebra-invoice'];

// The Northmoor document system: thirty production templates, every one
// gated for PDF/UA-1 permanently. Their stylesheets are <link>ed (the CLI
// inlines local links; the library does not), so inline them here.
const NORTHMOOR = [
  'invoice-standard',
  'invoice-detailed',
  'credit-note',
  'receipt',
  'quote',
  'statement',
  'purchase-order',
  'remittance-advice',
  'expense-report',
  'payslip',
  'letterhead',
  'cover-letter',
  'memo',
  'employment-contract',
  'nda',
  'service-agreement',
  'offer-letter',
  'termination-letter',
  'reference-letter',
  'policy-acknowledgement',
  'report-annual',
  'report-monthly',
  'lab-report',
  'inspection-report',
  'shipping-label',
  'packing-slip',
  'delivery-note',
  'certificate',
  'product-catalog',
  'meeting-minutes',
];

async function renderNorthmoor(name) {
  const dir = join(REPO, 'templates', name);
  const shared = await readFile(join(REPO, 'templates', 'northmoor-shared.css'), 'utf8');
  const own = await readFile(join(dir, 'style.css'), 'utf8');
  const html = (await readFile(join(dir, 'index.html'), 'utf8'))
    .replace('<link rel="stylesheet" href="../northmoor-shared.css">', `<style>${shared}</style>`)
    .replace('<link rel="stylesheet" href="style.css">', `<style>${own}</style>`);
  const { pdf, warnings } = renderHtml(html, { pdfUa: true, lang: LANG, fonts: HTML_FONTS });
  return { pdf, warnings };
}

async function renderTemplate(name, data) {
  const doc = serialize(getTemplate(name)(data));
  doc.pdfUa = true;
  doc.tagged = true;
  doc.metadata = { ...(doc.metadata ?? {}), lang: LANG };
  doc.fonts = CORE_FONTS;
  const { pdf, warnings } = await renderPdfWithLayout(JSON.stringify(doc));
  return { pdf, warnings };
}

async function renderFixture(name) {
  const html = await readFile(join(FIXTURES, `${name}.html`), 'utf8');
  const { pdf, warnings } = renderHtml(html, { pdfUa: true, lang: LANG, fonts: HTML_FONTS });
  return { pdf, warnings };
}

function findVeraPdf() {
  const candidate = process.env.VERAPDF || join(homedir(), 'verapdf', 'verapdf');
  return existsSync(candidate) ? candidate : null;
}

async function main() {
  // OUT_DIR keeps the rendered corpus where a later step can read it (CI uploads
  // it to Forme Review); otherwise a temp dir as before.
  const outDir = process.env.OUT_DIR ? (mkdirSync(process.env.OUT_DIR, { recursive: true }), process.env.OUT_DIR) : mkdtempSync(join(tmpdir(), 'forme-pdfua-'));
  const vera = findVeraPdf();
  const corpus = [];

  console.log('Rendering PDF/UA-1 corpus…');
  for (const [name, data] of Object.entries(TEMPLATES)) {
    const { pdf, warnings } = await renderTemplate(name, data);
    const p = join(outDir, `template-${name}.pdf`);
    writeFileSync(p, pdf);
    corpus.push({ label: `template/${name}`, path: p, warnings });
  }
  for (const name of HTML_FIXTURES) {
    const { pdf, warnings } = await renderFixture(name);
    const p = join(outDir, `html-${name}.pdf`);
    writeFileSync(p, pdf);
    corpus.push({ label: `html/${name}`, path: p, warnings });
  }
  for (const name of NORTHMOOR) {
    const { pdf, warnings } = await renderNorthmoor(name);
    const p = join(outDir, `northmoor-${name}.pdf`);
    writeFileSync(p, pdf);
    corpus.push({ label: `northmoor/${name}`, path: p, warnings });
  }

  // No font warnings are expected — fonts-standard is registered everywhere.
  const noisy = corpus.filter((c) =>
    (c.warnings ?? []).some((w) => w.startsWith('pdfUa:') && /not embedded/.test(w)),
  );
  if (noisy.length) {
    console.error('\n✗ Unexpected font warnings (fonts-standard not applied):');
    for (const c of noisy) console.error(`  ${c.label}: ${c.warnings.join(' | ')}`);
    process.exit(1);
  }

  if (!vera) {
    if (process.env.REQUIRE_VERAPDF) {
      console.error(
        `\n✗ veraPDF not found but REQUIRE_VERAPDF is set` +
          ` (checked VERAPDF and ~/verapdf/verapdf). This is a hard gate in CI.`,
      );
      process.exit(1);
    }
    console.log(
      `\n⚠ veraPDF not found (set VERAPDF or install to ~/verapdf/verapdf).` +
        `\n  Rendered ${corpus.length}/9 corpus files without error; skipping validation.`,
    );
    process.exit(0);
  }

  // Build the evidence object FIRST — it is the source of truth. The console
  // output below is a render of it, and the gate is derived from it.
  const section = {
    tool: veraVersion(vera),
    configuration: 'ua1',
    label: 'PDF/UA-1',
    render: 'pdfUa + tagged + fonts-standard',
    corpus: corpus.map((c) => c.label),
    results: corpus.map((c) => {
      const { pass, failedClauses, xml } = veraValidate(vera, 'ua1', c.path);
      keepReport(`${basename(c.path, '.pdf')}.ua1.xml`, xml); // for Forme Review's --conformance
      return { fixture: c.label, profile: 'ua1', pass, failedClauses };
    }),
  };
  emitSection('conformance-ua', section);

  // Render the console FROM the section.
  console.log(`\nValidating ${corpus.length} files with ${section.tool}…\n`);
  for (const r of section.results) {
    console.log(`  ${r.pass ? '✓ PASS' : '✗ FAIL'}  ${r.fixture}`);
  }
  const failures = section.results.filter((r) => !r.pass);
  if (failures.length) {
    console.error(`\n✗ ${failures.length}/${section.results.length} failed PDF/UA-1:`);
    for (const f of failures) {
      const clauses = f.failedClauses.map((c) => `${c.clause}/t${c.test}`).join(', ');
      console.error(`  ${f.fixture}: ${clauses}`);
    }
    process.exit(1);
  }
  console.log(`\n✓ ${section.results.length}/${section.results.length} pass PDF/UA-1.`);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
