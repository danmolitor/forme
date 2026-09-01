#!/usr/bin/env node
// Determinism evidence for the parity page.
//
// Emits, as structured data (source of truth; console is a render of it), the
// byte-determinism results the html-parity job proves — as SEPARATE evidence
// classes of different strength, exactly as they are:
//
//   - byte-identity: native ⇆ node ⇆ web, byte-for-byte, over the 4 HTML
//     fixtures. (native side only when the release binary is present.)
//   - wasm hash-identity: the three published wasm targets embed the same
//     compiled module (so the *bundler* target — which can't be loaded outside
//     a bundler to byte-diff — computes identically by construction).
//
// The bundler target is deliberately NOT claimed as byte-diffed; it is covered
// by hash-identity plus the functional browser/workerd renders elsewhere.

import { readFileSync, existsSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { emitSection } from './lib.mjs';

const HERE = dirname(fileURLToPath(import.meta.url));
const REPO = join(HERE, '..', '..');
const FIXTURES = join(REPO, 'html', 'tests', 'fixtures');
const PKG = join(REPO, 'packages', 'html');
const CORPUS = ['letterhead', 'report', 'zebra-invoice', 'dashed-borders'];
const NATIVE = join(REPO, 'html', 'target', 'release', 'forme-html');

const sha = (b) => createHash('sha256').update(b).digest('hex');

async function main() {
  const { renderHtml: renderNode } = await import('@formepdf/html');
  const { init: initWeb, renderHtml: renderWeb } = await import('@formepdf/html/worker');
  await initWeb(readFileSync(join(PKG, 'pkg-web', 'forme_pdf_html_bg.wasm')));

  const section = {
    corpus: CORPUS,
    byteIdentity: [],
    wasmHashIdentity: null,
    notChecked: [],
  };

  // wasm hash-identity across the 3 targets.
  const hashes = ['pkg', 'pkg-web', 'pkg-node'].map((d) =>
    sha(readFileSync(join(PKG, d, 'forme_pdf_html_bg.wasm'))),
  );
  section.wasmHashIdentity = {
    targets: ['bundler', 'web', 'node'],
    identical: new Set(hashes).size === 1,
    sha256: hashes[0],
  };

  const nativeAvailable = existsSync(NATIVE);
  if (!nativeAvailable) {
    section.notChecked.push({
      comparison: 'native↔node',
      reason: 'release binary html/target/release/forme-html not built in this environment',
    });
  }

  for (const name of CORPUS) {
    const html = readFileSync(join(FIXTURES, `${name}.html`), 'utf8');
    const node = renderNode(html, {}).pdf;
    const web = renderWeb(html, {}).pdf;
    section.byteIdentity.push({
      comparison: 'node↔web',
      fixture: name,
      pass: sha(node) === sha(web),
      bytes: node.length,
    });
    if (nativeAvailable) {
      execFileSync(NATIVE, [join(FIXTURES, `${name}.html`), '-o', '/tmp/pd-native.pdf', '-q']);
      const native = readFileSync('/tmp/pd-native.pdf');
      section.byteIdentity.push({
        comparison: 'native↔node',
        fixture: name,
        pass: sha(native) === sha(node),
        bytes: native.length,
      });
    }
  }

  emitSection('determinism', section);

  // Render console from the section.
  console.log(`wasm hash-identity (bundler/web/node): ${section.wasmHashIdentity.identical ? '✓ identical' : '✗ DIVERGE'} (${section.wasmHashIdentity.sha256.slice(0, 12)}…)`);
  for (const r of section.byteIdentity) {
    console.log(`  ${r.pass ? '✓' : '✗'} ${r.comparison}  ${r.fixture}  (${r.bytes} bytes)`);
  }
  for (const n of section.notChecked) console.log(`  · not checked: ${n.comparison} — ${n.reason}`);

  const failed = section.byteIdentity.filter((r) => !r.pass);
  if (failed.length || !section.wasmHashIdentity.identical) {
    console.error(`\n✗ determinism failures: ${failed.map((f) => f.comparison + '/' + f.fixture).join(', ')}`);
    process.exit(1);
  }
  console.log('\n✓ determinism: all byte-identity checks pass; wasm identical across targets.');
}

main().catch((e) => { console.error(e); process.exit(1); });
