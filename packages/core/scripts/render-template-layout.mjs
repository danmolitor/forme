/**
 * Render one shipped template and write its raw `LayoutInfo` JSON to disk.
 *
 * Exists for CI's "Document Regression Demo" job, which needs the *current*
 * run as a file on disk to hand to @pdf-testkit/action. The vitest matcher
 * compares in memory and never writes one, so this is the missing producer —
 * deliberately the same two calls the regression suite makes (`getTemplate` +
 * `renderDocumentWithLayout`), so the demo can't drift from what that suite
 * asserts.
 *
 * `pdf-testkit snapshot` converts the LayoutInfo emitted here into the same
 * StructuralSnapshot shape as the committed baselines.
 *
 * Usage: node scripts/render-template-layout.mjs <template-name> <out.json>
 */
import { writeFile } from 'node:fs/promises';
import { getTemplate } from '@formepdf/templates';
import * as schemas from '@formepdf/templates/schemas';
import { renderDocumentWithLayout } from '../dist/index.js';

const [name, out] = process.argv.slice(2);
if (!name || !out) {
  console.error('usage: render-template-layout.mjs <template-name> <out.json>');
  process.exit(2);
}

const template = getTemplate(name);
if (!template) {
  console.error(`getTemplate(${JSON.stringify(name)}) returned null`);
  process.exit(1);
}

// Same `*Example` fixtures the regression suite uses. They're validated against
// their Zod schemas at import time, so they can't drift from the schemas.
const exampleKey = `${name.replace(/-([a-z])/g, (_, c) => c.toUpperCase())}Example`;
const data = schemas[exampleKey];
if (!data) {
  console.error(`No example fixture exported as "${exampleKey}" from @formepdf/templates/schemas`);
  process.exit(1);
}

const { layout } = await renderDocumentWithLayout(template(data));
await writeFile(out, JSON.stringify(layout, null, 2) + '\n', 'utf8');
console.error(`wrote layout: ${out} (${layout.pages.length} page(s))`);
