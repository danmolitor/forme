// Warm-render measurement in an isolated process, so an OOM or a runaway on a
// large document kills THIS child (recorded by the parent as a gap) rather than
// taking down the emitter. Run: node warmrender.mjs <doc.html> <iters>.
// Prints one JSON line: { medianMs, p95Ms, iters, pdfBytes, passes, pages }.
import { readFileSync } from 'node:fs';
import os from 'node:os';
const { renderHtml } = await import('@formepdf/html');
const [, , path, itersStr] = process.argv;
const iters = Number(itersStr) || 5;
const html = readFileSync(path, 'utf8');
for (let w = 0; w < 3; w++) renderHtml(html, {}); // warm
const t = [];
let r;
for (let i = 0; i < iters; i++) { const s = performance.now(); r = renderHtml(html, {}); t.push(performance.now() - s); }
const sorted = [...t].sort((a, b) => a - b);
const median = sorted[Math.floor(sorted.length / 2)];
const p95 = sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * 0.95))];
const pages = (new TextDecoder('latin1').decode(r.pdf).match(/\/Type\s*\/Page(?!s)/g) || []).length;
// resourceUsage().maxRSS is kilobytes on Linux but bytes on macOS (a known
// Node quirk) — normalize to MB per platform.
const rawRss = process.resourceUsage().maxRSS;
const maxRssMB = Math.round(os.platform() === 'darwin' ? rawRss / 1048576 : rawRss / 1024);
process.stdout.write(JSON.stringify({ medianMs: median, p95Ms: p95, iters, pdfBytes: r.pdf.length, passes: r.passes, pages, maxRssMB }));
