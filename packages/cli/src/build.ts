import { readFile, writeFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { isValidElement, type ReactElement } from 'react';
import { bundleFile, BUNDLE_DIR } from './bundle.js';
import { renderDocumentWithLayout } from '@formepdf/core';

export interface BuildOptions {
  output: string;
  dataPath?: string;
}

export async function buildPdf(inputPath: string, options: BuildOptions): Promise<void> {
  const absoluteInput = resolve(inputPath);
  console.log(`Building ${absoluteInput}...`);

  try {
    const code = await bundleFile(absoluteInput);

    // Write temp file inside CLI package dir so Node resolves @formepdf/* deps
    const tmpFile = join(BUNDLE_DIR, `.forme-build-${Date.now()}.mjs`);
    await writeFile(tmpFile, code);

    try {
      const mod = await import(pathToFileURL(tmpFile).href);
      const element = await resolveElement(mod, options.dataPath);

      const { pdf } = await renderDocumentWithLayout(element);

      const outputPath = resolve(options.output);
      await writeFile(outputPath, pdf);
      console.log(`Written ${pdf.length} bytes to ${outputPath}`);
    } finally {
      // Clean up temp file
      const { unlink } = await import('node:fs/promises');
      await unlink(tmpFile).catch(() => {});
    }
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    console.error(`\n  ${message.split('\n').join('\n  ')}\n`);
    process.exit(1);
  }
}

async function resolveElement(
  mod: Record<string, unknown>,
  dataPath?: string,
): Promise<ReactElement> {
  const exported = mod.default;

  if (exported === undefined) {
    throw new Error(
      `No default export found.\n\n` +
      `  Your file must export a Forme element or a function that returns one:\n\n` +
      `    export default (\n` +
      `      <Document>\n` +
      `        <Text>Hello</Text>\n` +
      `      </Document>\n` +
      `    );\n\n` +
      `  Or with data:\n\n` +
      `    export default function Report(data) {\n` +
      `      return <Document><Text>{data.title}</Text></Document>\n` +
      `    }`
    );
  }

  if (typeof exported === 'function') {
    const data = dataPath ? await loadJsonData(dataPath) : {};
    const result = await (exported as (data: unknown) => ReactElement | Promise<ReactElement>)(data);
    if (!isValidElement(result)) {
      throw new Error(
        `Default export function did not return a valid Forme element.\n` +
        `  Got: ${typeof result}\n` +
        `  Make sure your function returns a <Document> element.`
      );
    }
    return result;
  }

  if (isValidElement(exported)) {
    if (dataPath) {
      console.warn(
        `Warning: --data flag provided but default export is a static element, not a function.\n` +
        `  The data file will be ignored. Export a function to use --data.`
      );
    }
    return exported;
  }

  throw new Error(
    `Default export is not a valid Forme element.\n` +
    `  Got: ${typeof exported}\n` +
    `  Expected: a <Document> element or a function that returns one.`
  );
}

async function loadJsonData(dataPath: string): Promise<unknown> {
  const absolutePath = resolve(dataPath);
  const raw = await readFile(absolutePath, 'utf-8');
  try {
    return JSON.parse(raw);
  } catch {
    throw new Error(
      `Failed to parse data file as JSON: ${absolutePath}\n` +
      `  Make sure the file contains valid JSON.`
    );
  }
}
