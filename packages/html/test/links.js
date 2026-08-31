// Tests for host-layer <link rel="stylesheet"> resolution in the CLI.
import assert from 'node:assert';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { inlineStylesheetLinks } from '../bin/forme-html.js';
import { renderHtml } from '../index.js';

function scratch(name) {
  const dir = path.join(os.tmpdir(), `forme-link-js-${name}-${process.pid}`);
  fs.rmSync(dir, { recursive: true, force: true });
  fs.mkdirSync(dir, { recursive: true });
  return dir;
}

// 1. Local link inlined in source order, before the document's own <style>.
{
  const dir = scratch('order');
  fs.writeFileSync(path.join(dir, 'brand.css'), 'h1{color:#ff0000}');
  const html = '<head><link rel="stylesheet" href="brand.css"><style>h1{color:#00ff00}</style></head>';
  const out = inlineStylesheetLinks(html, dir);
  assert.ok(out.includes('<style>\nh1{color:#ff0000}\n</style>'), out);
  assert.ok(!out.includes('<link'), out);
  assert.ok(out.indexOf('#ff0000') < out.indexOf('#00ff00'), 'linked css precedes inline <style>');
}
// 2. Missing file → named error.
{
  const dir = scratch('missing');
  assert.throws(() => inlineStylesheetLinks('<link rel="stylesheet" href="nope.css">', dir), /nope\.css/);
}
// 3. Absolute href untouched.
{
  const dir = scratch('abs');
  const html = '<link rel="stylesheet" href="https://cdn/app.css">';
  assert.strictEqual(inlineStylesheetLinks(html, dir), html);
}
// 4. Non-stylesheet link untouched.
{
  const dir = scratch('rel');
  const html = '<link rel="icon" href="favicon.ico">';
  assert.strictEqual(inlineStylesheetLinks(html, dir), html);
}
// 5. End-to-end: CLI path renders styled with zero flags; API path warns.
{
  const dir = scratch('e2e');
  fs.writeFileSync(path.join(dir, 'brand.css'), 'h1{color:#ff0000}');
  const raw = '<link rel="stylesheet" href="brand.css"><h1>Invoice</h1>';
  const cliHtml = inlineStylesheetLinks(raw, dir);
  const cli = renderHtml(cliHtml, {});
  assert.ok(cli.pdf.length > 100 && Buffer.from(cli.pdf.slice(0, 5)).toString() === '%PDF-');
  assert.ok(!cli.warnings.some((w) => w.includes('link') || w.includes('stylesheet')), 'CLI path: no link warning');
  const api = renderHtml(raw, {});
  assert.ok(api.warnings.some((w) => w.toLowerCase().includes('link') || w.includes('stylesheet') || w.includes('rel=')), `API path must warn: ${JSON.stringify(api.warnings)}`);
}
console.log('ok — link resolution: 5 checks passed');
