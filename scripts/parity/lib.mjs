// Shared helpers for the parity-evidence emission (formepdf.com/parity).
//
// The governing rule: the JSON is the SOURCE. Every verification script builds
// a plain data object, writes it as a partial section (when PARITY_DIR is set),
// and renders its human console output FROM that same object — never a parallel
// print path that could disagree with the emitted evidence.

import { writeFileSync, mkdirSync } from 'node:fs';
import { join } from 'node:path';
import { execFileSync } from 'node:child_process';

/** Write a partial section `<PARITY_DIR>/<name>.json` when PARITY_DIR is set. */
export function emitSection(name, data) {
  const dir = process.env.PARITY_DIR;
  if (!dir) return;
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, `${name}.json`), JSON.stringify(data, null, 2) + '\n');
}

/** The tool string veraPDF reports, e.g. "veraPDF 1.30.2" (best-effort). */
export function veraVersion(vera) {
  try {
    const out = execFileSync(vera, ['--version'], { encoding: 'utf8' });
    const m = out.match(/veraPDF\s+([0-9.]+)/i);
    return m ? `veraPDF ${m[1]}` : 'veraPDF';
  } catch {
    return 'veraPDF';
  }
}

/**
 * Validate `pdfPath` against a veraPDF flavour and return the structured
 * result — pass plus the FULL failed-clause list (clause, test number, failed
 * checks, description). Failures are data, not just a red exit.
 */
export function veraValidate(vera, flavour, pdfPath) {
  let xml;
  try {
    xml = execFileSync(vera, ['-f', flavour, pdfPath], {
      encoding: 'utf8',
      maxBuffer: 64 * 1024 * 1024,
    });
  } catch (err) {
    // A non-compliant file makes veraPDF exit non-zero; the report is on stdout.
    xml = (err.stdout ?? '').toString();
  }
  const pass = /isCompliant="true"/.test(xml);
  const failedClauses = [];
  const re =
    /<rule\b[^>]*\bclause="([^"]+)"[^>]*\btestNumber="([^"]+)"[^>]*\bstatus="failed"[^>]*\bfailedChecks="([^"]+)"[^>]*>\s*<description>([^<]*)<\/description>/g;
  let m;
  while ((m = re.exec(xml)) !== null) {
    failedClauses.push({
      clause: m[1],
      test: Number(m[2]),
      failedChecks: Number(m[3]),
      description: m[4],
    });
  }
  return { pass, failedClauses };
}
