import { writeFile } from 'node:fs/promises';
import { resolve, join, basename } from 'node:path';
import { pathToFileURL } from 'node:url';
import { isValidElement, type ReactElement } from 'react';
import { bundleFile, BUNDLE_DIR } from './bundle.js';

export interface TemplateBuildOptions {
  output?: string;
}

export async function buildTemplate(inputPath: string, options: TemplateBuildOptions): Promise<void> {
  const absoluteInput = resolve(inputPath);
  console.log(`Building template from ${absoluteInput}...`);

  try {
    const code = await bundleFile(absoluteInput);

    const tmpFile = join(BUNDLE_DIR, `.forme-template-${Date.now()}.mjs`);
    await writeFile(tmpFile, code);

    try {
      const mod = await import(pathToFileURL(tmpFile).href);
      const exported = mod.default;

      if (exported === undefined) {
        throw new Error(
          `No default export found.\n\n` +
          `  Your template file must export a function that takes data and returns JSX:\n\n` +
          `    export default function Invoice(data) {\n` +
          `      return <Document><Text>{data.title}</Text></Document>\n` +
          `    }`
        );
      }

      if (typeof exported !== 'function') {
        throw new Error(
          `Default export must be a function for template compilation.\n` +
          `  Got: ${typeof exported}\n\n` +
          `  Export a function that takes data and returns a <Document>:\n\n` +
          `    export default function Template(data) {\n` +
          `      return <Document>...</Document>\n` +
          `    }`
        );
      }

      // Create a recording proxy and call the template function
      const { createDataProxy, serializeTemplate } = await import('@formepdf/react');
      const dataProxy = createDataProxy();
      const element = exported(dataProxy);

      if (!isValidElement(element)) {
        throw new Error(
          `Template function did not return a valid Forme element.\n` +
          `  Got: ${typeof element}\n` +
          `  Make sure your function returns a <Document> element.`
        );
      }

      const templateJson = serializeTemplate(element as ReactElement);

      // Resolve fonts to base64 for portability
      await resolveFontPaths(templateJson, absoluteInput);

      const outputPath = resolve(
        options.output ?? defaultTemplateName(inputPath)
      );
      const json = JSON.stringify(templateJson, null, 2);
      await writeFile(outputPath, json);
      console.log(`Written template (${json.length} bytes) to ${outputPath}`);
    } finally {
      const { unlink } = await import('node:fs/promises');
      await unlink(tmpFile).catch(() => {});
    }
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    console.error(`\n  ${message.split('\n').join('\n  ')}\n`);
    process.exit(1);
  }
}

function defaultTemplateName(inputPath: string): string {
  const base = basename(inputPath).replace(/\.(tsx|jsx|ts|js)$/, '');
  return `${base}.template.json`;
}

async function resolveFontPaths(
  doc: Record<string, unknown>,
  templatePath: string,
): Promise<void> {
  const { dirname } = await import('node:path');
  const { readFile } = await import('node:fs/promises');
  const { resolve: resolvePath } = await import('node:path');

  const templateDir = dirname(templatePath);
  const fonts = doc.fonts as Array<{
    family: string;
    src: string | Uint8Array;
    weight: number;
    italic: boolean;
  }> | undefined;

  if (!fonts?.length) return;

  for (const font of fonts) {
    if (font.src instanceof Uint8Array) {
      font.src = Buffer.from(font.src).toString('base64');
    } else if (typeof font.src === 'string' && !font.src.startsWith('data:')) {
      const fontPath = resolvePath(templateDir, font.src);
      const bytes = await readFile(fontPath);
      font.src = Buffer.from(bytes).toString('base64');
    }
  }
}
