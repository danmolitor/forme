#!/usr/bin/env node
// forme-html — HTML + print-CSS to PDF from the command line, no browser.

const fs = require('node:fs');
const path = require('node:path');
const { renderHtml } = require('../index.js');

const USAGE = `forme-html — HTML + print-CSS to PDF, no browser

USAGE:
    forme-html <input.html> [OPTIONS]

OPTIONS:
    -o, --output <file>     Output path (default: input with .pdf extension)
        --css <file>        Extra stylesheet applied after the document's own
        --page-size <size>  A4, A3, A5, Letter, Legal, Tabloid
                            (overrides the document's @page rule; default A4)
        --margin <pt>       Uniform page margin in points
                            (overrides @page margins; default 54)
        --font <spec>       Register a TTF: 'Family=path.ttf'. Repeatable.
                            Variants: 'Family:700=..', 'Family:bold:italic=..
    -q, --quiet             Suppress unsupported-CSS warnings
    -h, --help              Show this help
`;

function fail(msg) {
  process.stderr.write(`error: ${msg}\n\n${USAGE}`);
  process.exit(1);
}

const args = process.argv.slice(2);
if (args.length === 0) fail('no input file');
if (args.includes('-h') || args.includes('--help')) {
  process.stdout.write(USAGE);
  process.exit(0);
}

let input = null;
let output = null;
let cssPath = null;
let quiet = false;
const options = {};

const SIZES = { a4: 'A4', a3: 'A3', a5: 'A5', letter: 'Letter', legal: 'Legal', tabloid: 'Tabloid' };

for (let i = 0; i < args.length; i++) {
  const arg = args[i];
  const value = () => {
    if (i + 1 >= args.length) fail(`${arg} requires a value`);
    return args[++i];
  };
  if (arg === '-o' || arg === '--output') output = value();
  else if (arg === '--css') cssPath = value();
  else if (arg === '--page-size') {
    const v = value().toLowerCase();
    if (!SIZES[v]) fail(`unknown page size '${v}'`);
    options.pageSize = SIZES[v];
  } else if (arg === '--margin') {
    const v = Number(value());
    if (!Number.isFinite(v)) fail('invalid margin');
    options.pageMargin = v;
  } else if (arg === '--font') {
    const spec = value();
    const eq = spec.indexOf('=');
    if (eq < 1) fail(`--font expects 'Family=path', got '${spec}'`);
    const head = spec.slice(0, eq);
    const fontPath = spec.slice(eq + 1);
    const [family, ...variants] = head.split(':');
    let weight = 400;
    let italic = false;
    for (const v of variants) {
      const lv = v.toLowerCase();
      if (lv === 'bold') weight = 700;
      else if (lv === 'italic') italic = true;
      else if (/^\d+$/.test(lv)) weight = Number(lv);
      else fail(`bad font variant '${v}'`);
    }
    let data;
    try {
      data = fs.readFileSync(fontPath);
    } catch (e) {
      fail(`cannot read font ${fontPath}: ${e.message}`);
    }
    (options.fonts ??= []).push({ family, data: new Uint8Array(data), weight, italic });
  } else if (arg === '-q' || arg === '--quiet') quiet = true;
  else if (arg.startsWith('-')) fail(`unknown option '${arg}'`);
  else if (input !== null) fail('multiple input files given');
  else input = arg;
}

if (input === null) fail('no input file');

let html;
try {
  html = fs.readFileSync(input, 'utf8');
} catch (e) {
  fail(`cannot read ${input}: ${e.message}`);
}
if (cssPath !== null) {
  try {
    options.css = fs.readFileSync(cssPath, 'utf8');
  } catch (e) {
    fail(`cannot read ${cssPath}: ${e.message}`);
  }
}

const outPath =
  output ?? input.replace(/\.html?$/i, '') + '.pdf';

let result;
try {
  result = renderHtml(html, options);
} catch (e) {
  process.stderr.write(`error: ${e.message ?? e}\n`);
  process.exit(1);
}

if (!quiet) {
  for (const w of result.warnings) process.stderr.write(`warning: ${w}\n`);
}
fs.writeFileSync(outPath, result.pdf);
process.stderr.write(`${outPath} (${result.pdf.length} bytes)\n`);
