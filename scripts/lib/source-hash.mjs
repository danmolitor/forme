// The ONE definition of "which engine source produced this render".
//
// The docs gallery gate hashes the render-affecting SOURCE trees rather than
// the built wasm, because wasm bytes differ between local and CI toolchains
// while source bytes do not. That is still true and is why this hashes .rs
// files.
//
// It lives here, rather than inside the gate, because the gate is no longer
// the only consumer: `packages/html/build.sh` bakes this same value into the
// wasm at compile time so the gate can ask a renderer which source it was
// built from. Two implementations of "the source hash" would defeat the
// entire point of asking, so there is one.
import { createHash } from 'node:crypto';
import { readdirSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO = join(dirname(fileURLToPath(import.meta.url)), '..', '..');

function treeHash(dir) {
  const h = createHash('sha256');
  const walkDir = (d) => {
    for (const ent of readdirSync(d, { withFileTypes: true }).sort((a, b) =>
      a.name.localeCompare(b.name),
    )) {
      const p = join(d, ent.name);
      if (ent.isDirectory()) walkDir(p);
      else if (ent.name.endsWith('.rs')) {
        h.update(p.slice(REPO.length));
        h.update(readFileSync(p));
      }
    }
  };
  walkDir(dir);
  return h.digest('hex');
}

/** Hash of every .rs file under engine/src and html/src. */
export function sourceHash() {
  return createHash('sha256')
    .update(treeHash(join(REPO, 'engine', 'src')) + treeHash(join(REPO, 'html', 'src')))
    .digest('hex');
}

// `node scripts/lib/source-hash.mjs` prints it, for build.sh.
if (process.argv[1] && process.argv[1].endsWith('source-hash.mjs')) {
  process.stdout.write(sourceHash());
}
