// Shared helpers for the parity-evidence emission (formepdf.com/parity).
//
// The governing rule: the JSON is the SOURCE. Every verification script builds
// a plain data object, writes it as a partial section (when PARITY_DIR is set),
// and renders its human console output FROM that same object — never a parallel
// print path that could disagree with the emitted evidence.

import { createHash } from 'node:crypto';
import { writeFileSync, mkdirSync, readFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
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
  return { pass, failedClauses, xml };
}

/**
 * Validate MANY files with one veraPDF invocation (one JVM). The CI corpus
 * is ~39 documents; per-file invocation spent ~15s of JVM startup each —
 * most of the conformance job's wall clock. veraPDF's multi-file report
 * carries one <job> per input naming it by <item ...><name>; this splits
 * the combined XML on job boundaries and reuses the single-file clause
 * parsing per slice. Returns a Map from ABSOLUTE input path to
 * { pass, failedClauses, xml } with the same shape as veraValidate.
 */
export function veraValidateBatch(vera, flavour, pdfPaths) {
  if (pdfPaths.length === 0) return new Map();
  let xml;
  try {
    xml = execFileSync(vera, ['-f', flavour, ...pdfPaths], {
      encoding: 'utf8',
      maxBuffer: 256 * 1024 * 1024,
    });
  } catch (err) {
    xml = (err.stdout ?? '').toString();
  }
  const out = new Map();
  const jobs = xml.split(/<job>/).slice(1);
  for (const slice of jobs) {
    const nameM = slice.match(/<name>([^<]+)<\/name>/);
    if (!nameM) continue;
    const jobXml = '<job>' + slice;
    const pass = /isCompliant="true"/.test(jobXml);
    const failedClauses = [];
    const re =
      /<rule\b[^>]*\bclause="([^"]+)"[^>]*\btestNumber="([^"]+)"[^>]*\bstatus="failed"[^>]*\bfailedChecks="([^"]+)"[^>]*>\s*<description>([^<]*)<\/description>/g;
    let m;
    while ((m = re.exec(jobXml)) !== null) {
      failedClauses.push({
        clause: m[1],
        test: Number(m[2]),
        failedChecks: Number(m[3]),
        description: m[4],
      });
    }
    out.set(nameM[1], { pass, failedClauses, xml: jobXml });
  }
  return out;
}

/**
 * Keep a validator's raw report where Forme Review's upload step can attach
 * it (`--conformance`). No-op unless OUT_DIR is set. The report names the PDF
 * by its path under OUT_DIR, which is also the path the upload names it by.
 */
/**
 * Write the layout Forme returned with a PDF beside it as
 * `<file>.pdf.layout.json`, the sidecar pdf-testkit's upload reads to take
 * structure from the layout (confidence 1) instead of inferring it from the
 * PDF with pdfjs. Carries the PDF's sha256 so a stale sidecar is refused.
 * No-op unless OUT_DIR is set, like `keepReport`.
 */
export function keepLayout(pdfPath, pdf, layout, producer) {
  if (!process.env.OUT_DIR) return null;
  const p = `${pdfPath}.layout.json`;
  writeFileSync(p, JSON.stringify({ format: 'forme-layout/1', pdf_sha256: createHash('sha256').update(pdf).digest('hex'), producer: { name: producer, version: formeVersion(producer) }, layout }));
  return p;
}

const REPO_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..', '..');
/** The version of a workspace package, for the sidecar's producer stamp. */
export function formeVersion(pkg) {
  try {
    return JSON.parse(readFileSync(resolve(REPO_ROOT, 'packages', pkg.replace(/^@formepdf\//, ''), 'package.json'), 'utf8')).version;
  } catch {
    return 'unknown';
  }
}

export function keepReport(name, text) {
  const dir = process.env.OUT_DIR;
  if (!dir) return null;
  const reports = join(dir, 'reports');
  mkdirSync(reports, { recursive: true });
  const p = join(reports, name);
  writeFileSync(p, text);
  return p;
}

/**
 * Mustang's validation report → the documented `forme-review-conformance/1`
 * shape. Forme Review parses veraPDF and nothing else by policy; every other
 * validator, including this one that we run ourselves, goes through this JSON.
 * The verdict is Mustang's summary; each typed error or exception becomes a
 * failure with the type as its clause. Attributed to Mustang by version.
 */
export function mustangToConformance(reportXml, { documentPath, jarPath, file = null }) {
  const status = (reportXml.match(/<summary status="([a-z]+)"\/>\s*<\/validation>/) ?? reportXml.match(/<summary status="([a-z]+)"\/>/))?.[1] ?? null;
  const verdict = status === 'valid' ? 'pass' : status === 'invalid' ? 'fail' : 'error';
  const failures = [];
  for (const m of reportXml.matchAll(/<(error|exception|criterion)\b([^>]*)>([^<]*)<\/\1>/g)) {
    const type = m[2].match(/type="([^"]+)"/)?.[1] ?? m[1];
    failures.push({ clause: `${m[1]}/${type}`, test: null, count: 1, description: m[3].trim().slice(0, 300) });
  }
  const ranAt = reportXml.match(/datetime="([^"]+)"/)?.[1];
  const result = {
    profile: 'Factur-X EN 16931',
    verdict,
    tool: { name: 'Mustang', version: mustangVersion(jarPath) },
    ran_at: ranAt ? new Date(ranAt.replace(' ', 'T') + 'Z').toISOString() : null,
    source: { format: 'mustang-report-xml', file },
    failure_count: failures.length,
    failures: failures.slice(0, 200),
  };
  return { format: 'forme-review-conformance/1', documents: { [documentPath]: [result] } };
}

export function mustangVersion(jarPath) {
  if (process.env.MUSTANG_VERSION) return process.env.MUSTANG_VERSION;
  try {
    const manifest = execFileSync('unzip', ['-p', jarPath, 'META-INF/MANIFEST.MF'], { encoding: 'utf8' });
    return manifest.match(/Implementation-Version:\s*([^\s]+)/)?.[1] ?? 'unknown';
  } catch {
    return 'unknown';
  }
}
