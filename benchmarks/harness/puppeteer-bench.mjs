// Puppeteer baseline on the identical corpus HTML. Cold start (browser launch
// -> first page.pdf) is reported separately from warm (browser reused across
// iterations, each doing setContent + page.pdf — the full parse+layout+render,
// comparable to Forme's renderHtml). 500-page doc is capped at 5 minutes.
import puppeteer from 'puppeteer-core';
import { readFileSync } from 'node:fs';
const CHROME = '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const median = a => { const s=[...a].sort((x,y)=>x-y); return s[Math.floor(s.length/2)]; };
const read = d => readFileSync(`benchmarks/corpus/${d}.html`, 'utf8');
const withTimeout = (p, ms) => Promise.race([p, new Promise((_, r) => setTimeout(() => r(new Error('TIMEOUT')), ms))]);

// COLD: launch -> first render
const tL0 = performance.now();
const browser = await puppeteer.launch({ executablePath: CHROME, headless: true, args: ['--no-sandbox'] });
const launchMs = performance.now() - tL0;
const page = await browser.newPage();
const tR0 = performance.now();
await page.setContent(read('receipt'), { waitUntil: 'load' });
await page.pdf({ preferCSSPageSize: true });
const firstRenderMs = performance.now() - tR0;
console.log(`COLD START: browser launch ${launchMs.toFixed(0)}ms + first render ${firstRenderMs.toFixed(0)}ms = ${(launchMs+firstRenderMs).toFixed(0)}ms total`);

// WARM: browser reused, setContent + pdf per iteration
console.log('\nWARM (browser reused):');
console.log('doc            median   pdfKB   iters');
const plan = [['receipt',20],['report-6p',20],['letterhead-paged',20],['compliance',20],['invoice-50p',5],['ledger-500p',1]];
for (const [d, iters] of plan) {
  const html = read(d);
  try {
    await withTimeout((async () => {
      const t = []; let size = 0;
      for (let i = 0; i < iters; i++) {
        const t0 = performance.now();
        await page.setContent(html, { waitUntil: 'load' });
        const pdf = await page.pdf({ preferCSSPageSize: true });
        t.push(performance.now() - t0); size = pdf.length;
      }
      console.log(`${d.padEnd(14)} ${median(t).toFixed(0).padStart(7)}ms ${String(Math.round(size/1024)).padStart(6)}   ${iters}`);
    })(), 300000);
  } catch (e) {
    console.log(`${d.padEnd(14)}  did not complete in 5 min (${e.message})`);
  }
}
await browser.close();
