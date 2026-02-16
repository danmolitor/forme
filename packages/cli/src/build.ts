import { writeFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { bundleFile, BUNDLE_DIR } from './bundle.js';
import { renderDocumentWithLayout } from '@forme/core';
import type { ReactElement } from 'react';

export interface BuildOptions {
  output: string;
}

export async function buildPdf(inputPath: string, options: BuildOptions): Promise<void> {
  const absoluteInput = resolve(inputPath);
  console.log(`Building ${absoluteInput}...`);

  try {
    const code = await bundleFile(absoluteInput);

    // Write temp file inside CLI package dir so Node resolves @forme/* deps
    const tmpFile = join(BUNDLE_DIR, `.forme-build-${Date.now()}.mjs`);
    await writeFile(tmpFile, code);

    try {
      const mod = await import(pathToFileURL(tmpFile).href);
      let element: ReactElement = mod.default;

      // If the export is a function, call it (supports async factory functions)
      if (typeof element === 'function') {
        element = await (element as () => ReactElement | Promise<ReactElement>)();
      }

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
    console.error('Build failed:', err instanceof Error ? err.message : err);
    process.exit(1);
  }
}
