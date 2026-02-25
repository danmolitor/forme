import { writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { renderDocument } from '@formepdf/core';
import { templates } from '../templates/index.js';

export async function renderPdf(
  templateName: string,
  data: Record<string, unknown>,
  output?: string,
): Promise<{ path: string; size: number }> {
  const entry = templates[templateName];
  if (!entry) {
    const available = Object.keys(templates).join(', ');
    throw new Error(`Template "${templateName}" not found. Available templates: ${available}`);
  }

  // Validate data against schema
  const parsed = entry.schema.parse(data);

  // Render template to React element, then to PDF
  const element = entry.fn(parsed);
  const pdfBytes = await renderDocument(element);

  // Write to disk
  const outputPath = resolve(output || `./${templateName}.pdf`);
  await writeFile(outputPath, pdfBytes);

  return { path: outputPath, size: pdfBytes.length };
}
