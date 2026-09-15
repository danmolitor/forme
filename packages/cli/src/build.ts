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
    const { pdf, warnings } = await renderFromFile(absoluteInput, {
      dataPath: options.dataPath,
    });

    const outputPath = resolve(options.output);
    await writeFile(outputPath, pdf);
    console.log(`Written ${pdf.length} bytes to ${outputPath}`);

    // Warnings are the engine telling you what it did that you didn't ask
    // for — a flex row that serialized across a page break, a character with
    // no glyph, a table column clamped below its content. Silence here is
    // how a rendering compromise reaches a user as a mystery.
    for (const w of warnings ?? []) console.warn(`  warning: ${w}`);
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    console.error(`\n  ${message.split('\n').join('\n  ')}\n`);
    process.exit(1);
  }
}
