#!/usr/bin/env node
// Template-library docs generator: everything on docs.formepdf.com/templates
// is EMITTED from templates/ — names, descriptions, feature lists, page
// images, HTML, CSS, data — so the pages cannot drift from the library.
//
//   node scripts/docs-templates.mjs           # regenerate (render + rasterize + MDX + nav)
//   node scripts/docs-templates.mjs --check   # CI freshness gate: no rendering —
//                                             # recompute input hashes vs the manifest
//                                             # and re-emit the MDX for a diff
//
// Full generation needs pdftoppm (poppler) and the sharp devDependency;
// --check needs neither, so CI can gate freshness without cross-platform
// rasterizer differences (Linux/macOS poppler AA diverges 1.2–8.5% — the
// visual-regression suite's lesson; images are validated by input hash, not
// by re-rendering).
import { execFileSync } from 'node:child_process';
import {
  createHash,
} from 'node:crypto';
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO = join(dirname(fileURLToPath(import.meta.url)), '..');
const T = join(REPO, 'templates');
const IMG_DIR = join(REPO, 'docs', 'images', 'templates');
const MDX_DIR = join(REPO, 'docs', 'templates');
const MANIFEST = join(IMG_DIR, 'manifest.json');
const CHECK = process.argv.includes('--check');

// The library's own structure — the three groups exactly as the design
// delivered them (design_handoff_northmoor_document_system/README.md,
// ids 1a–3j). The assertion below fails the build when templates/ gains
// or loses a directory, forcing this list to be updated with it.
const GROUPS = [
  {
    title: 'Billing & Finance',
    slugs: [
      'invoice-standard', 'invoice-detailed', 'credit-note', 'receipt',
      'quote', 'statement', 'purchase-order', 'remittance-advice',
      'expense-report', 'payslip',
    ],
  },
  {
    title: 'Correspondence & Legal',
    slugs: [
      'letterhead', 'cover-letter', 'memo', 'employment-contract', 'nda',
      'service-agreement', 'offer-letter', 'termination-letter',
      'reference-letter', 'policy-acknowledgement',
    ],
  },
  {
    title: 'Reports, Logistics & Certificates',
    slugs: [
      'report-annual', 'report-monthly', 'lab-report', 'inspection-report',
      'shipping-label', 'packing-slip', 'delivery-note', 'certificate',
      'product-catalog', 'meeting-minutes',
    ],
  },
];
const ALL_SLUGS = GROUPS.flatMap((g) => g.slugs);

const dirSlugs = readdirSync(T, { withFileTypes: true })
  .filter((d) => d.isDirectory())
  .map((d) => d.name)
  .sort();
const listed = [...ALL_SLUGS].sort();
if (JSON.stringify(dirSlugs) !== JSON.stringify(listed)) {
  console.error('templates/ and the GROUPS list disagree:');
  console.error('  on disk but unlisted:', dirSlugs.filter((s) => !listed.includes(s)));
  console.error('  listed but missing:  ', listed.filter((s) => !dirSlugs.includes(s)));
  process.exit(1);
}

// Display names are mechanical (slug → Title Case) with a few explicit
// overrides where word order or casing differs from the slug.
const TITLE_OVERRIDES = {
  'invoice-standard': 'Standard Invoice',
  'invoice-detailed': 'Detailed Invoice',
  nda: 'Mutual NDA',
  'report-annual': 'Annual Report',
  'report-monthly': 'Monthly Report',
  'lab-report': 'Lab Report (Certificate of Analysis)',
};
const displayName = (slug) =>
  TITLE_OVERRIDES[slug] ??
  slug.split('-').map((w) => w[0].toUpperCase() + w.slice(1)).join(' ');

const sha = (buf) => createHash('sha256').update(buf).digest('hex');
const sharedCss = readFileSync(join(T, 'northmoor-shared.css'), 'utf8');
const selfHash = sha(readFileSync(fileURLToPath(import.meta.url)));

/** Parse a template README: h1 title, description paragraph,
 *  "Engine features:" line, "Render:" line. The format is the contract —
 *  fail loudly rather than emit a half-empty page. */
function parseReadme(slug) {
  const src = readFileSync(join(T, slug, 'README.md'), 'utf8');
  const title = displayName(slug);
  const lines = src.split('\n').filter((l) => l.trim());
  const desc = lines.find((l) => !l.startsWith('#') && !l.startsWith('Engine features:') && !l.startsWith('Render:'));
  // Two batch-fork dialects exist: "Engine features:" and "Engine
  // features exercised:"; twenty READMEs omit the Render line (the
  // command is uniform), so derive it.
  const features = src.match(/^Engine features(?: exercised)?: (.+)$/m)?.[1];
  const render =
    src.match(/^Render: `(.+)`$/m)?.[1] ??
    `npx @formepdf/html templates/${slug}/index.html -o ${slug}.pdf`;
  for (const [k, v] of Object.entries({ title, desc, features, render })) {
    if (!v) throw new Error(`${slug}/README.md: missing ${k}`);
  }
  return { title, desc, features: features.replace(/\.$/, ''), render };
}

function inputHash(slug) {
  const files = ['index.html', 'style.css', 'README.md', 'data.json'];
  const h = createHash('sha256');
  h.update(sharedCss);
  h.update(selfHash); // generator changes invalidate everything
  for (const f of files) {
    const p = join(T, slug, f);
    if (existsSync(p)) h.update(readFileSync(p));
  }
  return h.digest('hex');
}

function inlined(slug) {
  const own = readFileSync(join(T, slug, 'style.css'), 'utf8');
  return readFileSync(join(T, slug, 'index.html'), 'utf8')
    .replace('<link rel="stylesheet" href="../northmoor-shared.css">', `<style>${sharedCss}</style>`)
    .replace('<link rel="stylesheet" href="style.css">', `<style>${own}</style>`);
}

// ── MDX emission (deterministic text; runs in --check too) ───────────────

const mdxEscape = (s) => s.replace(/</g, '&lt;').replace(/\{/g, '&#123;');
const firstSentence = (s) => {
  const m = s.match(/^(.+?\.)( |$)/);
  return m ? m[1] : s;
};

function galleryMdx(meta) {
  const out = [];
  out.push(`---
title: Template Library
sidebarTitle: Overview
description: Thirty MIT-licensed document templates for the HTML path — invoices, contracts, reports, labels — every one gated on PDF/UA-1 conformance in CI.
---

{/* GENERATED by scripts/docs-templates.mjs from templates/ — do not edit by hand.
    Regenerate: node scripts/docs-templates.mjs */}

Thirty ready-to-copy documents, built as one system: a shared type scale, one accent, reconciling numbers across the set. They are **starting points to copy and edit**, not a package to install — every template is MIT-licensed, and the point of them is that you take the files and make them yours.

Every template renders warning-free and passes [PDF/UA-1 validation (veraPDF)](/accessibility) in CI on every commit. Passing a validator is a floor, not a certification — accessibility of your final document still depends on your content.

To use one: open its page below, copy \`index.html\` and \`style.css\` (plus the [shared stylesheet](https://github.com/formepdf/forme/blob/main/templates/northmoor-shared.css)), and render with the CLI or any \`@formepdf/html\` entry point:

\`\`\`bash
npx @formepdf/html templates/invoice-standard/index.html -o invoice.pdf
\`\`\`
`);
  for (const g of GROUPS) {
    out.push(`\n## ${g.title}\n`);
    out.push('<Columns cols={3}>');
    for (const slug of g.slugs) {
      const m = meta[slug];
      out.push(`  <Card title="${m.title}" href="/templates/${slug}" img="/images/templates/${slug}/card.webp">
    ${mdxEscape(firstSentence(m.desc))}
  </Card>`);
    }
    out.push('</Columns>');
  }
  return out.join('\n') + '\n';
}

function detailMdx(slug, m, pageCount) {
  const out = [];
  out.push(`---
title: ${JSON.stringify(m.title)}
description: ${JSON.stringify(firstSentence(m.desc))}
---

{/* GENERATED by scripts/docs-templates.mjs from templates/${slug}/ — do not edit by hand. */}
`);
  for (let p = 1; p <= pageCount; p++) {
    out.push(`<Frame caption="${pageCount > 1 ? `Page ${p} of ${pageCount}` : `${m.title} — rendered output`}">
  <img src="/images/templates/${slug}/page-${p}.webp" alt="${m.title}, page ${p}" />
</Frame>
`);
  }
  out.push(mdxEscape(m.desc) + '\n');
  out.push(`**Engine features exercised:** ${mdxEscape(m.features)}.\n`);
  out.push(`Render it:

\`\`\`bash
${m.render}
\`\`\`
`);
  const html = readFileSync(join(T, slug, 'index.html'), 'utf8');
  const css = readFileSync(join(T, slug, 'style.css'), 'utf8');
  out.push(`<CodeGroup>

\`\`\`html index.html
${html.trimEnd()}
\`\`\`

\`\`\`css style.css
${css.trimEnd()}
\`\`\`

</CodeGroup>

Both files assume [\`northmoor-shared.css\`](https://github.com/formepdf/forme/blob/main/templates/northmoor-shared.css) alongside them — the set's shared spine (type scale, tables, signature blocks, the accent).
`);
  const dataPath = join(T, slug, 'data.json');
  if (existsSync(dataPath)) {
    out.push(`<Accordion title="Sample data (data.json)">

\`\`\`json data.json
${readFileSync(dataPath, 'utf8').trimEnd()}
\`\`\`

</Accordion>
`);
  }
  out.push(`---

MIT-licensed, free to copy — [source on GitHub](https://github.com/formepdf/forme/tree/main/templates/${slug}). Passes PDF/UA-1 validation (veraPDF) in CI; validation is a floor, not a certification.
`);
  return out.join('\n');
}

function navBlock() {
  return {
    group: 'Template Library',
    pages: [
      'templates',
      ...GROUPS.map((g) => ({ group: g.title, pages: g.slugs.map((s) => `templates/${s}`) })),
    ],
  };
}

function updateNav() {
  const p = join(REPO, 'docs', 'docs.json');
  const cfg = JSON.parse(readFileSync(p, 'utf8'));
  const groups = cfg.navigation.groups;
  const i = groups.findIndex((g) => g.group === 'Template Library');
  if (i >= 0) groups[i] = navBlock();
  else groups.splice(groups.findIndex((g) => g.group === 'Reference'), 0, navBlock());
  writeFileSync(p, JSON.stringify(cfg, null, 2) + '\n');
}

// ── main ─────────────────────────────────────────────────────────────────

const meta = Object.fromEntries(ALL_SLUGS.map((s) => [s, parseReadme(s)]));

if (CHECK) {
  const manifest = JSON.parse(readFileSync(MANIFEST, 'utf8'));
  let stale = 0;
  for (const slug of ALL_SLUGS) {
    const rec = manifest[slug];
    if (!rec) { console.error(`FAIL ${slug}: not in manifest`); stale++; continue; }
    if (rec.hash !== inputHash(slug)) {
      console.error(`FAIL ${slug}: inputs changed since images were generated`);
      stale++;
      continue;
    }
    const want = detailMdx(slug, meta[slug], rec.pages);
    const have = readFileSync(join(MDX_DIR, `${slug}.mdx`), 'utf8');
    if (want !== have) { console.error(`FAIL ${slug}: templates/${slug}.mdx is stale`); stale++; }
  }
  const wantGallery = galleryMdx(meta);
  if (wantGallery !== readFileSync(join(REPO, 'docs', 'templates.mdx'), 'utf8')) {
    console.error('FAIL gallery: docs/templates.mdx is stale'); stale++;
  }
  if (stale) {
    console.error(`\n${stale} stale artifact(s) — run: node scripts/docs-templates.mjs`);
    process.exit(1);
  }
  console.log('docs template gallery is current (30 templates, manifest + MDX verified)');
  process.exit(0);
}

// Full generation: render each template, rasterize, emit images + MDX.
const { renderHtmlWithLayout } = await import('@formepdf/html');
const sharp = (await import('sharp')).default;

mkdirSync(IMG_DIR, { recursive: true });
mkdirSync(MDX_DIR, { recursive: true });
const manifest = {};
let totalBytes = 0;

for (const slug of ALL_SLUGS) {
  const { pdf, warnings } = renderHtmlWithLayout(inlined(slug), {});
  if (warnings.length) {
    throw new Error(`${slug}: must render warning-free, got: ${warnings.join(' | ')}`);
  }
  const work = mkdtempSync(join(tmpdir(), `forme-docs-${slug}-`));
  const pdfPath = join(work, 'doc.pdf');
  writeFileSync(pdfPath, pdf);
  // 150 dpi masters; sharp downscales from there.
  execFileSync('pdftoppm', ['-png', '-r', '150', pdfPath, join(work, 'page')]);
  const pages = readdirSync(work).filter((f) => f.endsWith('.png')).sort();
  const dir = join(IMG_DIR, slug);
  mkdirSync(dir, { recursive: true });
  for (let i = 0; i < pages.length; i++) {
    const src = join(work, pages[i]);
    const out = join(dir, `page-${i + 1}.webp`);
    await sharp(src).resize({ width: 1100 }).webp({ quality: 82 }).toFile(out);
    totalBytes += readFileSync(out).length;
    if (i === 0) {
      const card = join(dir, 'card.webp');
      await sharp(src).resize({ width: 640 }).webp({ quality: 80 }).toFile(card);
      totalBytes += readFileSync(card).length;
    }
  }
  manifest[slug] = { hash: inputHash(slug), pages: pages.length };
  writeFileSync(join(MDX_DIR, `${slug}.mdx`), detailMdx(slug, meta[slug], pages.length));
  rmSync(work, { recursive: true, force: true });
  console.log(`ok ${slug}: ${pages.length} page(s)`);
}

writeFileSync(join(REPO, 'docs', 'templates.mdx'), galleryMdx(meta));
writeFileSync(MANIFEST, JSON.stringify(manifest, null, 2) + '\n');
updateNav();
console.log(`\n30 templates emitted; images total ${(totalBytes / 1024 / 1024).toFixed(1)} MB`);
