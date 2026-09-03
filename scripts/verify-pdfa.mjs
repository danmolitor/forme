#!/usr/bin/env node
// PDF/A conformance gate (Track 1 Part 3, Phase 4).
//
// Renders the 9-file corpus — five shipped @formepdf/templates + four HTML
// fixtures — as BOTH PDF/A and PDF/UA-1 at once (fonts-standard registered),
// and validates each output against veraPDF's PDF/A profile AND its PDF/UA-1
// profile. Runs the combination at PDF/A-2b and PDF/A-2a (2a ⊃ 2u ⊃ 2b, so
// passing 2a exercises the strictest path). Exits non-zero on any failure, so
// the "archival + accessible" claim is enforced, not merely reported.
//
// veraPDF via VERAPDF env or ~/verapdf/verapdf. REQUIRE_VERAPDF makes a missing
// binary a hard failure (CI); otherwise it renders the corpus and skips.

import { existsSync, mkdtempSync, writeFileSync } from 'node:fs';
import { readFile } from 'node:fs/promises';
import { tmpdir, homedir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { emitSection, veraValidate, veraVersion } from './parity/lib.mjs';

import { serialize } from '@formepdf/react';
import { getTemplate } from '@formepdf/templates';
import {
  invoiceExample, receiptExample, reportExample,
  shippingLabelExample, letterExample,
} from '@formepdf/templates/schemas';
import { standardFonts } from '@formepdf/fonts-standard';
import { renderPdfWithLayout } from '@formepdf/core';
import { renderHtml } from '@formepdf/html';

const HERE = dirname(fileURLToPath(import.meta.url));
const FIXTURES = join(HERE, '..', 'html', 'tests', 'fixtures');
const LANG = 'en-US';
const LEVELS = ['2b', '2a', '3b', '3a']; // 2a/3a additionally require tagging; 2u/3u sit between (same machinery, validated via 3b/3a).

const CORE_FONTS = standardFonts().map((f) => ({
  family: f.family, src: Buffer.from(f.src).toString('base64'),
  weight: f.fontWeight, italic: f.fontStyle === 'italic',
}));
const HTML_FONTS = standardFonts().map((f) => ({
  family: f.family, data: f.src, weight: f.fontWeight, italic: f.fontStyle === 'italic',
}));

const TEMPLATES = {
  invoice: invoiceExample, receipt: receiptExample, report: reportExample,
  'shipping-label': shippingLabelExample, letter: letterExample,
};
const HTML_FIXTURES = ['letterhead', 'dashed-borders', 'statement', 'zebra-invoice'];

async function renderTemplate(name, data, level) {
  const doc = serialize(getTemplate(name)(data));
  doc.pdfUa = true;
  doc.tagged = true;
  doc.pdfa = level;
  doc.metadata = { ...(doc.metadata ?? {}), lang: LANG };
  doc.fonts = CORE_FONTS;
  const { pdf } = await renderPdfWithLayout(JSON.stringify(doc));
  return pdf;
}
async function renderFixture(name, level) {
  const html = await readFile(join(FIXTURES, `${name}.html`), 'utf8');
  const { pdf } = renderHtml(html, { pdfUa: true, pdfA: level, lang: LANG, fonts: HTML_FONTS });
  return pdf;
}

function findVeraPdf() {
  const c = process.env.VERAPDF || join(homedir(), 'verapdf', 'verapdf');
  return existsSync(c) ? c : null;
}
async function main() {
  const vera = findVeraPdf();
  if (!vera) {
    const msg = 'veraPDF not found (set VERAPDF or install to ~/verapdf/verapdf).';
    if (process.env.REQUIRE_VERAPDF) { console.error(`✗ ${msg} REQUIRE_VERAPDF is set.`); process.exit(1); }
    console.log(`⚠ ${msg} Skipping validation.`); process.exit(0);
  }
  const outDir = mkdtempSync(join(tmpdir(), 'forme-pdfa-gate-'));

  // Build the evidence FIRST (source of truth): every fixture rendered as
  // pdfA+pdfUa at each level, validated against the PDF/A level AND ua1.
  const section = {
    tool: veraVersion(vera),
    configurations: LEVELS.map((level) => ({
      id: `a${level}`,
      level,
      label: `PDF/A-${level} + PDF/UA-1`,
      render: `pdfa:${level} + pdfUa + fonts-standard`,
      profiles: [level, 'ua1'],
    })),
    results: [],
  };
  for (const level of LEVELS) {
    const corpus = [];
    for (const [name, data] of Object.entries(TEMPLATES)) {
      const pdf = await renderTemplate(name, data, level);
      const p = join(outDir, `${level}-template-${name}.pdf`); writeFileSync(p, pdf);
      corpus.push({ label: `template/${name}`, path: p });
    }
    for (const name of HTML_FIXTURES) {
      const pdf = await renderFixture(name, level);
      const p = join(outDir, `${level}-html-${name}.pdf`); writeFileSync(p, pdf);
      corpus.push({ label: `html/${name}`, path: p });
    }
    for (const c of corpus) {
      for (const profile of [level, 'ua1']) {
        const { pass, failedClauses } = veraValidate(vera, profile, c.path);
        section.results.push({ fixture: c.label, configuration: `a${level}`, profile, pass, failedClauses });
      }
    }
  }
  emitSection('conformance-a', section);

  // Render the console FROM the section.
  const failures = [];
  for (const cfg of section.configurations) {
    console.log(`\n${cfg.label} (rendered as both):`);
    const rows = section.results.filter((r) => r.configuration === cfg.id);
    const byFixture = [...new Set(rows.map((r) => r.fixture))];
    for (const fixture of byFixture) {
      const parts = rows
        .filter((r) => r.fixture === fixture)
        .map((r) => `${r.profile === 'ua1' ? 'UA-1' : 'PDF/A-' + r.profile}:${r.pass ? '✓' : 'FAIL[' + r.failedClauses.map((c) => c.clause + '/t' + c.test).join(',') + ']'}`);
      const ok = rows.filter((r) => r.fixture === fixture).every((r) => r.pass);
      console.log(`  ${ok ? '✓' : '✗'}  ${fixture}  ${parts.join('  ')}`);
      if (!ok) failures.push(`${fixture} @ ${cfg.level}`);
    }
  }
  if (failures.length) {
    console.error(`\n✗ ${failures.length} corpus/level combination(s) failed: ${failures.join(', ')}`);
    process.exit(1);
  }
  console.log(`\n✓ All 9 corpus files pass ${LEVELS.map((l) => `PDF/A-${l}`).join(', ')}, and PDF/UA-1 together.`);
}

main().catch((e) => { console.error(e); process.exit(1); });
