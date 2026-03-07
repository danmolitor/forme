import { writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { renderFromFile } from '@formepdf/renderer';

export interface BuildOptions {
  output: string;
  dataPath?: string;
}

export async function buildPdf(inputPath: string, options: BuildOptions): Promise<void> {
  const absoluteInput = resolve(inputPath);
  console.log(`Building ${absoluteInput}...`);

  try {
    const { pdf } = await renderFromFile(absoluteInput, { dataPath: options.dataPath });

    const outputPath = resolve(options.output);
    await writeFile(outputPath, pdf);
    console.log(`Written ${pdf.length} bytes to ${outputPath}`);
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    console.error(`\n  ${message.split('\n').join('\n  ')}\n`);
    process.exit(1);
  }
}
