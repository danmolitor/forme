// E-invoice container gate: render a Factur-X invoice through the public
// JS API (visual PDF + caller-supplied EN 16931 CII XML embedded in one
// pass), then validate the result with BOTH validators:
//
//   - veraPDF: PDF/A-3b + PDF/UA-1 (the container conformance layer)
//   - Mustangproject CLI: the ZUGFeRD/Factur-X reference validator —
//     re-checks the PDF/A layer (embedded veraPDF), the XMP
//     identification, and runs the EN 16931 schematron over the XML
//
// The fixture XML is the official ZUGFeRD corpus EN 16931 sample
// (engine/tests/fixtures/einvoice/ — FeRD/ZUGFeRD corpus, Apache-2.0).
// Forme's claim is the CONTAINER: it does not generate or validate the
// XML's semantic content, so a failing XML here would mean the fixture
// itself regressed, not the engine.
//
// Env: VERAPDF (binary path), MUSTANG_JAR (jar path). REQUIRE_VALIDATORS=1
// turns missing tools into failures (CI); otherwise the script skips.
// PARITY_DIR: also emit the structured evidence section.

import { execFileSync } from 'node:child_process';
import { emitSection } from './parity/lib.mjs';
import { existsSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { homedir, tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import React from 'react';
import { Document, Page, View, Text, Font } from '@formepdf/react';
import { standardFonts } from '@formepdf/fonts-standard';
import { renderDocument } from '@formepdf/core';

const HERE = dirname(fileURLToPath(import.meta.url));
const XML_PATH = join(HERE, '..', 'engine', 'tests', 'fixtures', 'einvoice', 'EN16931_Einfach.cii.xml');

for (const f of standardFonts()) Font.register(f);
const h = React.createElement;

function invoiceDoc() {
  // Human-readable rendering of the same invoice the fixture XML
  // describes (ZUGFeRD corpus sample 471102).
  return h(Document, { pdfa: '3b', pdfUa: true, lang: 'de-DE', title: 'Rechnung 471102' },
    h(Page, { size: 'A4', margin: 50 },
      h(View, {},
        h(Text, { style: { fontSize: 20, fontWeight: 700, marginBottom: 12 } }, 'Rechnung Nr. 471102'),
        h(Text, {}, 'Lieferant GmbH, Lieferantenstraße 20, 80333 München'),
        h(Text, { style: { marginBottom: 12 } }, 'an Kunden AG Mitte, Kundenstraße 15, 69876 Frankfurt'),
        h(Text, {}, 'Rechnungsdatum: 14.02.2022'),
        h(Text, {}, 'Trennblätter A4 — 9,90 EUR netto (7% USt.)'),
        h(Text, { style: { fontWeight: 700, marginTop: 12 } }, 'Gesamtbetrag: 529,87 EUR'))));
}

function findTool(env, fallback) {
  const c = process.env[env] || fallback;
  return existsSync(c) ? c : null;
}

function veraCompliant(vera, pdfPath, flavour) {
  try {
    const out = execFileSync(vera, ['-f', flavour, pdfPath], { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
    return out.includes('isCompliant="true"');
  } catch (e) {
    // veraPDF exits non-zero on non-compliance but still prints the report
    return String(e.stdout || '').includes('isCompliant="true"');
  }
}

function mustangValid(jar, pdfPath) {
  try {
    const out = execFileSync(
      'java',
      ['-Xmx1G', '-Dfile.encoding=UTF-8', '-jar', jar, '--action', 'validate', '--source', pdfPath, '--no-notices'],
      { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 },
    );
    return { valid: /<summary status="valid"\/>/.test(out), report: out };
  } catch (e) {
    return { valid: false, report: String(e.stdout || e.message) };
  }
}

async function main() {
  const vera = findTool('VERAPDF', join(homedir(), 'verapdf', 'verapdf'));
  const jar = findTool('MUSTANG_JAR', join(homedir(), 'mustang', 'Mustang-CLI.jar'));
  const missing = [!vera && 'veraPDF (set VERAPDF)', !jar && 'Mustang CLI (set MUSTANG_JAR)'].filter(Boolean);
  if (missing.length > 0) {
    const msg = `missing: ${missing.join(', ')}`;
    if (process.env.REQUIRE_VALIDATORS) { console.error(`✗ ${msg}`); process.exit(1); }
    console.log(`⚠ ${msg} — skipping e-invoice validation.`); process.exit(0);
  }

  const xml = readFileSync(XML_PATH);
  const pdf = await renderDocument(invoiceDoc(), { facturX: { xml, profile: 'EN 16931' } });
  const outDir = mkdtempSync(join(tmpdir(), 'forme-einvoice-gate-'));
  const pdfPath = join(outDir, 'facturx-en16931.pdf');
  writeFileSync(pdfPath, pdf);

  const checks = [
    { id: 'verapdf-3b', label: 'veraPDF PDF/A-3b', pass: veraCompliant(vera, pdfPath, '3b') },
    { id: 'verapdf-ua1', label: 'veraPDF PDF/UA-1', pass: veraCompliant(vera, pdfPath, 'ua1') },
  ];
  const mustang = mustangValid(jar, pdfPath);
  checks.push({ id: 'mustang', label: 'Mustang (ZUGFeRD/Factur-X reference validator)', pass: mustang.valid });

  // Evidence first (the JSON section is the source; console is a render).
  const section = {
    fixture: 'facturx-en16931',
    render: 'pdfa:3b + pdfUa + facturX{EN 16931} via @formepdf/core renderDocument',
    xml: 'ZUGFeRD corpus EN16931_Einfach.cii.xml (FeRD, Apache-2.0)',
    checks: checks.map(({ id, label, pass }) => ({ id, label, pass })),
  };
  emitSection('einvoice', section);

  console.log('\nE-invoice container gate (Factur-X EN 16931 profile):');
  for (const c of checks) console.log(`  ${c.pass ? '✓' : '✗'}  ${c.label}`);
  if (!checks.every((c) => c.pass)) {
    if (!mustang.valid) console.error(`\nMustang report:\n${mustang.report}`);
    process.exit(1);
  }
  console.log('\n✓ The rendered PDF + embedded EN 16931 XML pass veraPDF (3b, ua1) and Mustang.');
}

main().catch((e) => { console.error(e); process.exit(1); });
