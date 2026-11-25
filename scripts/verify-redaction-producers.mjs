#!/usr/bin/env node
// Redaction gate on real producer PDFs, checked by independent parsers.
//
// For every PDF in engine/tests/fixtures/redaction-producers/ (written by
// headless Chrome, ReportLab and macOS Quartz; see the README there), this
// redacts with the engine BUILT FROM THIS REPO (@formepdf/core over
// pkg-node) and reads the result back with two parsers that share no code
// with the engine:
//
//   - Poppler   `pdftotext` (poppler-utils)
//   - pdf.js    `pdfjs-dist` legacy Node build, every page's text content
//
// Two redaction modes per fixture:
//
//   text     redactText() with Regex + Literal patterns
//   regions  redactPdf() with rectangles taken from pdf.js's own text
//            positions in the ORIGINAL file (not from the engine's search)
//
// Assertions, per fixture and mode:
//   - control: in the unredacted fixture, both parsers find every SENSITIVE
//     and every KEEP value (otherwise a broken extractor would pass);
//   - the redacted output opens in both parsers;
//   - no SENSITIVE value (or fragment of one) is in either parser's text;
//   - every KEEP value is still in both parsers' text.
//
// Exits non-zero on any failure. pdftotext is required: a missing binary
// is a failure, never a skip. OUT_DIR=<dir> keeps the redacted PDFs.

import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { getDocument } from 'pdfjs-dist/legacy/build/pdf.mjs';
import { redactPdf, redactText } from '@formepdf/core';

const HERE = dirname(fileURLToPath(import.meta.url));
const FIXTURES = join(HERE, '..', 'engine', 'tests', 'fixtures', 'redaction-producers');

// Values that must be gone after redaction. Fragments catch a partial
// removal (e.g. one glyph run of a split value left behind).
const SENSITIVE = ['Jane Doe', '123-45-6789', 'jane.doe@example.com'];
const SENSITIVE_FRAGMENTS = ['Jane', '6789', '123-45', 'example.com'];
// Values on the same page that must survive: they show the check reads text.
const KEEP = ['Claim Summary', 'Policy 7781', 'Claimant', 'SSN', 'Email', 'Status Approved'];

const PATTERNS = [
  { pattern: '\\d{3}-\\d{2}-\\d{4}', pattern_type: 'Regex' },
  { pattern: 'Jane Doe', pattern_type: 'Literal' },
  { pattern: '[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\\.[A-Za-z]{2,}', pattern_type: 'Regex' },
];

// Fixtures the engine is expected to refuse, with the reason. A refusal not
// listed here is a failure; so is a listed fixture that is not refused.
const EXPECTED_REFUSALS = {};

// Compare without whitespace and case: a parser may split or space a value.
const norm = (s) => s.replace(/\s+/g, '').toLowerCase();
const has = (text, value) => norm(text).includes(norm(value));

function popplerText(bytes, dir, name) {
  const path = join(dir, name);
  writeFileSync(path, bytes);
  const r = spawnSync('pdftotext', ['-layout', path, '-'], { encoding: 'utf8' });
  if (r.error) throw new Error(`pdftotext did not run: ${r.error.message}`);
  if (r.status !== 0) throw new Error(`pdftotext exit ${r.status}: ${r.stderr.trim()}`);
  if (r.stderr.trim()) throw new Error(`pdftotext reported errors: ${r.stderr.trim()}`);
  return r.stdout;
}

async function withPdfjs(bytes, fn) {
  const task = getDocument({ data: new Uint8Array(bytes), verbosity: 0, isEvalSupported: false });
  try {
    const doc = await task.promise;
    if (doc.numPages < 1) throw new Error('pdf.js: no pages');
    return await fn(doc);
  } finally {
    await task.destroy();
  }
}

async function pdfjsText(bytes) {
  return withPdfjs(bytes, async (doc) => {
    const pages = [];
    for (let i = 1; i <= doc.numPages; i++) {
      const content = await (await doc.getPage(i)).getTextContent();
      pages.push(content.items.map((it) => ('str' in it ? it.str + (it.hasEOL ? '\n' : '') : '')).join(''));
    }
    return pages.join('\n');
  });
}

// Rectangles (top-left origin, as displayed) around every text line that
// holds a SENSITIVE value, measured by pdf.js on the original file.
async function pdfjsRegions(bytes) {
  return withPdfjs(bytes, async (doc) => {
    const regions = [];
    for (let i = 1; i <= doc.numPages; i++) {
      const page = await doc.getPage(i);
      const viewport = page.getViewport({ scale: 1 });
      const items = (await page.getTextContent()).items.filter((it) => 'str' in it && it.str.trim());
      const lines = [];
      for (const it of items) {
        const y = it.transform[5];
        const line = lines.find((l) => Math.abs(l.y - y) < 1);
        if (line) line.items.push(it);
        else lines.push({ y, items: [it] });
      }
      for (const line of lines) {
        const text = line.items.map((it) => it.str).join('');
        if (!SENSITIVE.some((v) => has(text, v))) continue;
        let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
        for (const it of line.items) {
          const [, , c, d, e, f] = it.transform;
          const size = Math.hypot(c, d) || it.height;
          // Descent below the baseline to a full em above it.
          for (const [px, py] of [[e, f - 0.25 * size], [e + it.width, f + size]]) {
            const [vx, vy] = viewport.convertToViewportPoint(px, py);
            x0 = Math.min(x0, vx); x1 = Math.max(x1, vx);
            y0 = Math.min(y0, vy); y1 = Math.max(y1, vy);
          }
        }
        regions.push({ page: i - 1, x: x0 - 2, y: y0, width: x1 - x0 + 4, height: y1 - y0 });
      }
    }
    return regions;
  });
}

let failures = 0;
const fail = (msg) => { failures++; console.log(`  FAIL ${msg}`); };

function checkText(label, text, { sensitivePresent }) {
  for (const v of KEEP) if (!has(text, v)) fail(`${label}: KEEP value "${v}" missing`);
  if (sensitivePresent) {
    for (const v of SENSITIVE) if (!has(text, v)) fail(`${label}: control cannot see SENSITIVE "${v}"`);
  } else {
    for (const v of [...SENSITIVE, ...SENSITIVE_FRAGMENTS]) {
      if (has(text, v)) fail(`${label}: SENSITIVE "${v}" still extractable`);
    }
  }
}

if (spawnSync('pdftotext', ['-v']).error) {
  console.error('pdftotext (poppler-utils) is required and was not found.');
  process.exit(1);
}

const work = mkdtempSync(join(tmpdir(), 'forme-redaction-producers-'));
const outDir = process.env.OUT_DIR;
if (outDir) mkdirSync(outDir, { recursive: true });
const fixtures = readdirSync(FIXTURES).filter((f) => f.endsWith('.pdf')).sort();
if (fixtures.length === 0) {
  console.error(`no fixtures in ${FIXTURES}`);
  process.exit(1);
}

try {
  for (const name of fixtures) {
    const original = readFileSync(join(FIXTURES, name));
    const producer = (/\/Producer\s*\(((?:\\.|[^\\)])*)\)/.exec(original.toString('latin1'))?.[1] ?? '?')
      .replace(/\\(.)/g, '$1');
    console.log(`${name}  (${original.length} bytes, /Producer ${producer})`);

    const failsBefore = failures;
    checkText(`${name} control/poppler`, popplerText(original, work, 'control.pdf'), { sensitivePresent: true });
    checkText(`${name} control/pdf.js`, await pdfjsText(original), { sensitivePresent: true });
    if (failures === failsBefore) console.log('  ok   control: both parsers see every SENSITIVE and KEEP value');

    const modes = {
      text: () => redactText(original, PATTERNS),
      regions: async () => {
        const regions = await pdfjsRegions(original);
        if (regions.length !== SENSITIVE.length) {
          throw new Error(`pdf.js located ${regions.length} sensitive lines, expected ${SENSITIVE.length}`);
        }
        return redactPdf(original, regions);
      },
    };

    for (const [mode, run] of Object.entries(modes)) {
      const label = `${name} ${mode}`;
      let out;
      try {
        out = await run();
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        if (EXPECTED_REFUSALS[name]) console.log(`  ok   ${mode}: refused as expected (${msg})`);
        else fail(`${label}: engine error: ${msg}`);
        continue;
      }
      if (EXPECTED_REFUSALS[name]) {
        fail(`${label}: expected a refusal (${EXPECTED_REFUSALS[name]}) but got a PDF`);
        continue;
      }
      if (outDir) writeFileSync(join(outDir, `${name.replace(/\.pdf$/, '')}.${mode}.pdf`), out);
      const before = failures;
      let poppler, pdfjs;
      try {
        poppler = popplerText(out, work, `${mode}.pdf`);
      } catch (e) {
        fail(`${label}: Poppler cannot read the output: ${e.message}`);
      }
      try {
        pdfjs = await pdfjsText(out);
      } catch (e) {
        fail(`${label}: pdf.js cannot read the output: ${e.message}`);
      }
      if (poppler !== undefined) checkText(`${label}/poppler`, poppler, { sensitivePresent: false });
      if (pdfjs !== undefined) checkText(`${label}/pdf.js`, pdfjs, { sensitivePresent: false });
      if (failures === before) {
        console.log(`  ok   ${mode}: opens in both; ${SENSITIVE.length} SENSITIVE gone, ${KEEP.length} KEEP present (Poppler + pdf.js)`);
      }
    }
  }
} finally {
  rmSync(work, { recursive: true, force: true });
}

if (failures) {
  console.log(`\n${failures} failure(s) across ${fixtures.length} producer fixtures.`);
  process.exit(1);
}
console.log(`\nAll ${fixtures.length} producer fixtures pass in both modes (Poppler + pdf.js).`);
