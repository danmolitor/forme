#!/usr/bin/env node
// forme-html — HTML + print-CSS to PDF from the command line, no browser.

import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { renderHtml } from '../index.js';

// Host-layer <link rel="stylesheet"> resolution. The library never fetches;
// the CLI reads local stylesheets and inlines them IN PLACE as <style> blocks
// so they cascade at the <link>'s source position (a <link> before a <style>
// stays earlier in source order — appending to options.css could not preserve
// that). Absolute http(s):// (and protocol-relative //) hrefs pass through so
// the library still warns. A missing local file throws a named error.
export function inlineStylesheetLinks(html, baseDir) {
  const attr = (tag, name) => {
    const m = tag.match(new RegExp(`${name}\\s*=\\s*("([^"]*)"|'([^']*)')`, 'i'));
    return m ? (m[2] ?? m[3]) : null;
  };
  const linkRe = /<link\b[^>]*>/gi;
  let result = '';
  let last = 0;
  let m;
  while ((m = linkRe.exec(html)) !== null) {
    const tag = m[0];
    const rel = attr(tag, 'rel');
    const href = attr(tag, 'href');
    const isStylesheet =
      rel && rel.split(/\s+/).some((t) => t.toLowerCase() === 'stylesheet');
    if (!isStylesheet || !href) continue;
    const lower = href.toLowerCase();
    if (lower.startsWith('http://') || lower.startsWith('https://') || href.startsWith('//')) {
      continue; // absolute — leave for the library to warn about
    }
    const resolved = path.join(baseDir, href);
    let css;
    try {
      css = fs.readFileSync(resolved, 'utf8');
    } catch (e) {
      throw new Error(
        `cannot read stylesheet '${resolved}' (from <link href="${href}">): ${e.message}`,
      );
    }
    result += html.slice(last, m.index) + '<style>\n' + css + '\n</style>';
    last = m.index + tag.length;
  }
  return result + html.slice(last);
}

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

function main() {
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
  try {
    html = inlineStylesheetLinks(html, path.dirname(input));
  } catch (e) {
    fail(e.message);
  }
  if (cssPath !== null) {
    try {
      options.css = fs.readFileSync(cssPath, 'utf8');
    } catch (e) {
      fail(`cannot read ${cssPath}: ${e.message}`);
    }
  }

  const outPath = output ?? input.replace(/\.html?$/i, '') + '.pdf';

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
}

// Run the CLI only when invoked directly; importing this file (tests) gets the
// helper export without executing the command.
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main();
}
