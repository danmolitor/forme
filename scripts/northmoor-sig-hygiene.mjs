#!/usr/bin/env node
// The Northmoor design rule: signature blocks never split from the text
// they execute — and never split internally. This gate asserts every
// signature CONTAINER class actually used in template markup resolves
// `break-inside: avoid` (from the shared spine or the template's own
// css). The rule was applied to the batch-2 contracts (`.keep`) and
// later to `.sig`, and missed on `.sigrow`, `.signoff`, and
// `.cert-sigs` — hygiene that greps, not memory that hopes.
// Run: node scripts/northmoor-sig-hygiene.mjs
import { readFileSync, readdirSync, existsSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const T = join(dirname(fileURLToPath(import.meta.url)), '..', 'templates');

// Signature container classes (the outermost block the design rule
// protects). Inner pieces (.sigbox, .sigline, .sigcell, .cert-sig) are
// carried by their container. `.keep` is the contracts' generic wrapper
// and is asserted too — it exists solely to express this rule.
const CONTAINERS = ['sig', 'sigrow', 'signoff', 'cert-sigs', 'keep'];

const shared = readFileSync(join(T, 'northmoor-shared.css'), 'utf8');

function hasAvoid(css, cls) {
  // A rule whose selector list contains .cls (exact class token) and whose
  // declaration block contains break-inside: avoid.
  const re = /([^{}]+)\{([^}]*)\}/g;
  for (const [, sel, body] of css.matchAll(re)) {
    if (
      new RegExp(`\\.${cls}(?![\\w-])`).test(sel) &&
      /break-inside\s*:\s*avoid/.test(body)
    ) {
      return true;
    }
  }
  return false;
}

let failures = 0;
for (const dir of readdirSync(T, { withFileTypes: true })) {
  if (!dir.isDirectory()) continue;
  const htmlPath = join(T, dir.name, 'index.html');
  if (!existsSync(htmlPath)) continue;
  const html = readFileSync(htmlPath, 'utf8');
  const own = existsSync(join(T, dir.name, 'style.css'))
    ? readFileSync(join(T, dir.name, 'style.css'), 'utf8')
    : '';
  for (const cls of CONTAINERS) {
    if (!new RegExp(`class="[^"]*\\b${cls}(?![\\w-])`).test(html)) continue;
    const ok = hasAvoid(shared, cls) || hasAvoid(own, cls);
    if (!ok) failures++;
    console.log(`${ok ? '  ok ' : 'FAIL'}  ${dir.name}: .${cls} carries break-inside: avoid`);
  }
}

console.log(
  failures
    ? `\n${failures} signature container(s) missing break-inside: avoid`
    : '\nAll signature containers carry break-inside: avoid.'
);
process.exit(failures ? 1 : 0);
