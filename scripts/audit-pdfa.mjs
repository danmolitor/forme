#!/usr/bin/env node
// PDF/A baseline audit (Track 1 Part 3, Phase 1).
//
// Renders the SAME 9-file corpus the PDF/UA gate uses — five shipped templates
// and four HTML fixtures, tagged + pdfUa + fonts-standard — then validates each
// against veraPDF's PDF/A-2B, -2U and -2A profiles and prints the verbatim
// failed-clause list per file. This MEASURES the distance to each PDF/A level;
// it changes nothing. veraPDF path via VERAPDF env or ~/verapdf/verapdf.

import { execFileSync } from 'node:child_process';
import { existsSync, mkdtempSync, writeFileSync } from 'node:fs';
import { readFile } from 'node:fs/promises';
import { tmpdir, homedir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

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
const REPO = join(HERE, '..');
const FIXTURES = join(REPO, 'html', 'tests', 'fixtures');
const LANG = 'en-US';

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
const FLAVOURS = (process.argv[2] ? [process.argv[2]] : ['2b', '2u', '2a']);

async function renderTemplate(name, data) {
  const doc = serialize(getTemplate(name)(data));
  doc.pdfUa = true; doc.tagged = true;
  doc.metadata = { ...(doc.metadata ?? {}), lang: LANG };
  doc.fonts = CORE_FONTS;
  const { pdf } = await renderPdfWithLayout(JSON.stringify(doc));
  return pdf;
}
async function renderFixture(name) {
  const html = await readFile(join(FIXTURES, `${name}.html`), 'utf8');
  const { pdf } = renderHtml(html, { pdfUa: true, lang: LANG, fonts: HTML_FONTS });
  return pdf;
}

function findVeraPdf() {
  const c = process.env.VERAPDF || join(homedir(), 'verapdf', 'verapdf');
  return existsSync(c) ? c : null;
}

/** Run veraPDF for a flavour; return { pass, rules: [{clause,test,checks,desc}] }. */
function validate(vera, flavour, pdfPath) {
  let xml;
  try {
    xml = execFileSync(vera, ['-f', flavour, pdfPath], { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
  } catch (err) {
    xml = (err.stdout ?? '').toString();
  }
  const pass = /isCompliant="true"/.test(xml) || /compliant="true"/.test(xml);
  const rules = [];
  const re = /<rule\b[^>]*\bclause="([^"]+)"[^>]*\btestNumber="([^"]+)"[^>]*\bstatus="failed"[^>]*\bfailedChecks="([^"]+)"[^>]*>\s*<description>([^<]*)<\/description>/g;
  let m;
  while ((m = re.exec(xml)) !== null) {
    rules.push({ clause: m[1], test: m[2], checks: Number(m[3]), desc: m[4] });
  }
  return { pass, rules };
}

async function main() {
  const vera = findVeraPdf();
  if (!vera) { console.error('veraPDF not found (set VERAPDF).'); process.exit(2); }
  const outDir = mkdtempSync(join(tmpdir(), 'forme-pdfa-'));

  console.log('Rendering the 9-file corpus (tagged + pdfUa + fonts-standard)…\n');
  const corpus = [];
  for (const [name, data] of Object.entries(TEMPLATES)) {
    const pdf = await renderTemplate(name, data);
    const p = join(outDir, `template-${name}.pdf`); writeFileSync(p, pdf);
    corpus.push({ label: `template/${name}`, path: p });
  }
  for (const name of HTML_FIXTURES) {
    const pdf = await renderFixture(name);
    const p = join(outDir, `html-${name}.pdf`); writeFileSync(p, pdf);
    corpus.push({ label: `html/${name}`, path: p });
  }

  for (const flavour of FLAVOURS) {
    console.log(`\n${'='.repeat(72)}\nPDF/A-${flavour.toUpperCase()}  (ISO 19005-2)\n${'='.repeat(72)}`);
    const clauseAgg = new Map(); // clause|test -> { desc, files:Set, checks }
    let passCount = 0;
    for (const c of corpus) {
      const { pass, rules } = validate(vera, flavour, c.path);
      if (pass) { passCount++; console.log(`\n  ✓ PASS  ${c.label}`); continue; }
      console.log(`\n  ✗ FAIL  ${c.label}  (${rules.length} failed clause(s))`);
      for (const r of rules.sort((a, b) => a.clause.localeCompare(b.clause))) {
        console.log(`      ${r.clause} (t${r.test}, ${r.checks}×)  ${r.desc}`);
        const key = `${r.clause}|${r.test}`;
        const agg = clauseAgg.get(key) ?? { desc: r.desc, files: new Set(), checks: 0 };
        agg.files.add(c.label); agg.checks += r.checks; clauseAgg.set(key, agg);
      }
    }
    console.log(`\n  ── PDF/A-${flavour.toUpperCase()} summary: ${passCount}/${corpus.length} pass ──`);
    if (clauseAgg.size) {
      console.log(`  Distinct failing clauses (clause t# — files — total checks):`);
      for (const [key, a] of [...clauseAgg.entries()].sort()) {
        console.log(`    ${key.replace('|', ' t')}  — ${a.files.size}/${corpus.length} files, ${a.checks} checks — ${a.desc}`);
      }
    }
  }
  console.log(`\nPDFs kept at ${outDir}`);
}

main().catch((e) => { console.error(e); process.exit(1); });
