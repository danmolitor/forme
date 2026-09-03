// Rank the top allocation sites from a dhat-heap.json (produced by the
// `dhatprofile` example). Prints two tables: top sites by allocation COUNT
// (churn — what Fix 3 targets) and by BYTES (peak contribution). Each site is
// labelled by its first meaningful (non-allocator) stack frame.
//
// Usage: node benchmarks/harness/dhat-top.mjs [dhat-heap.json] [topN]
import { readFileSync } from 'node:fs';

const file = process.argv[2] || 'dhat-heap.json';
const topN = Number(process.argv[3] || 20);
const j = JSON.parse(readFileSync(file, 'utf8'));

// Frame strings are like "0x1234: forme::layout::foo (src/layout/mod.rs:12)".
// Strip the hex address; keep symbol + location.
const clean = (s) => s.replace(/^0x[0-9a-f]+:\s*/i, '').trim();

// Allocator/plumbing frames to skip when picking a site's representative frame.
const PLUMBING = /(?:^|:)(?:alloc|__rust|core::|std::|<alloc|RawVec|raw_vec|Vec<|Global|realloc|dhat::|_start|main\b|std::rt|std::sys|Allocator|reserve|grow)/;

const ftbl = j.ftbl || [];
const frameText = (idx) => clean(ftbl[idx] || '?');

// Representative frame: first frame in the backtrace that looks like project
// code (forme_pdf / forme / layout / font / text / pdf / html) rather than
// allocator or stdlib plumbing.
function site(fs) {
  for (const idx of fs) {
    const t = frameText(idx);
    if (/forme|layout|font\b|text::|pdf::|html|shape|glyph|paginat|inject/i.test(t) && !PLUMBING.test(t)) {
      return t;
    }
  }
  // fall back to first non-plumbing frame, else first frame
  for (const idx of fs) {
    const t = frameText(idx);
    if (!PLUMBING.test(t)) return t;
  }
  return frameText(fs[0] ?? 0);
}

const pps = (j.pps || []).map((p) => ({
  count: p.tbk,          // total blocks allocated at this point (churn)
  bytes: p.tb,           // total bytes allocated
  gmaxBytes: p.mb ?? 0,  // bytes live at global peak
  site: site(p.fs || []),
  top: frameText((p.fs || [])[0] ?? 0),
}));

const totalAllocs = pps.reduce((a, p) => a + p.count, 0);
const totalBytes = pps.reduce((a, p) => a + p.bytes, 0);
const kb = (b) => (b / 1024).toFixed(1);
const mb = (b) => (b / 1048576).toFixed(1);
const pct = (n, d) => ((100 * n) / d).toFixed(1);

function table(title, key, fmt) {
  const rows = [...pps].sort((a, b) => b[key] - a[key]).slice(0, topN);
  console.log(`\n=== TOP ${topN} SITES BY ${title} ===`);
  for (const r of rows) {
    const share = key === 'count' ? pct(r.count, totalAllocs) : pct(r.bytes, totalBytes);
    console.log(
      `${String(r.count).padStart(9)} allocs  ${kb(r.bytes).padStart(10)}KB  ` +
      `${share.padStart(5)}%  avg ${Math.round(r.bytes / Math.max(1, r.count))}B  ${r.site}`
    );
  }
}

console.log(`file: ${file}`);
console.log(`total: ${totalAllocs.toLocaleString()} allocs, ${mb(totalBytes)}MB allocated, ${mb(j.pps ? Math.max(...j.pps.map(p => p.mb || 0), 0) : 0)}MB largest single-site peak`);
table('ALLOCATION COUNT (churn)', 'count');
table('BYTES ALLOCATED', 'bytes');
