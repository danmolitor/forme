#!/usr/bin/env node
// Assemble the partial section files in PARITY_DIR into a single parity.json,
// stamped with provenance (commit, CI run, timestamp, version). This is the
// artifact the /parity page renders. Missing partials are recorded as absent
// rather than silently omitted, so the page can show honest coverage.

import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { execSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const HERE = dirname(fileURLToPath(import.meta.url));
const REPO = join(HERE, '..', '..');
const DIR = process.env.PARITY_DIR;
if (!DIR) {
  console.error('PARITY_DIR is not set — nothing to assemble.');
  process.exit(1);
}

const readJson = (name) => {
  const p = join(DIR, `${name}.json`);
  return existsSync(p) ? JSON.parse(readFileSync(p, 'utf8')) : null;
};
const git = (args) => {
  try { return execSync(`git ${args}`, { cwd: REPO, encoding: 'utf8' }).trim(); } catch { return null; }
};

const commit = process.env.GITHUB_SHA || git('rev-parse HEAD');
const runId = process.env.GITHUB_RUN_ID || null;
const server = process.env.GITHUB_SERVER_URL || 'https://github.com';
const repo = process.env.GITHUB_REPOSITORY || 'danmolitor/forme';
const version = JSON.parse(readFileSync(join(REPO, 'packages/core/package.json'), 'utf8')).version;

// Merge the two conformance partials into one section.
const ua = readJson('conformance-ua');
const a = readJson('conformance-a');
let conformance = null;
if (ua || a) {
  conformance = {
    tool: (a ?? ua).tool,
    configurations: [
      ...(ua ? [{ id: 'ua1', label: ua.label, render: ua.render, profiles: ['ua1'] }] : []),
      ...(a ? a.configurations : []),
    ],
    results: [
      ...(ua ? ua.results.map((r) => ({ ...r, configuration: 'ua1' })) : []),
      ...(a ? a.results : []),
    ],
  };
}

const determinism = readJson('determinism');
const tests = readJson('tests');

const missing = [];
if (!conformance) missing.push('conformance');
if (!determinism) missing.push('determinism');
if (!tests) missing.push('tests');

const artifact = {
  schemaVersion: 1,
  provenance: {
    commit,
    commitShort: commit ? commit.slice(0, 7) : null,
    version,
    runId,
    runUrl: runId ? `${server}/${repo}/actions/runs/${runId}` : null,
    commitUrl: commit ? `${server}/${repo}/commit/${commit}` : null,
    // Stamped by the caller if reproducibility matters; ISO 8601 UTC.
    generatedAt: process.env.PARITY_TIMESTAMP || new Date().toISOString(),
  },
  corpus: {
    templates: ['invoice', 'receipt', 'report', 'shipping-label', 'letter'],
    htmlFixtures: ['letterhead', 'dashed-borders', 'statement', 'zebra-invoice'],
  },
  sections: { conformance, determinism, tests },
  missingSections: missing,
};

const outPath = join(DIR, 'parity.json');
writeFileSync(outPath, JSON.stringify(artifact, null, 2) + '\n');
console.log(`Wrote ${outPath}`);
console.log(`  commit ${artifact.provenance.commitShort} · v${version} · sections: ${Object.entries(artifact.sections).filter(([, v]) => v).map(([k]) => k).join(', ') || 'none'}${missing.length ? ` · MISSING: ${missing.join(', ')}` : ''}`);
