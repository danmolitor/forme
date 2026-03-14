import { writeFile } from 'node:fs/promises';
import * as React from 'react';
import { renderDocument } from '@formepdf/core';
import { Watermark } from '@formepdf/react';
import { templates } from '../templates/index.js';
import { validateOutputPath } from '../utils/validate-output-path.js';
import { withTimeout } from '../utils/timeout.js';

export async function renderPdf(
  templateName: string,
  data: Record<string, unknown>,
  output?: string,
  watermark?: string,
): Promise<{ path: string; size: number }> {
  const entry = templates[templateName];
  if (!entry) {
    const available = Object.keys(templates).join(', ');
    throw new Error(`Template "${templateName}" not found. Available templates: ${available}`);
  }

  // Validate data against schema
  const parsed = entry.schema.parse(data);

  // Render template to React element
  let element = entry.fn(parsed);

  // Inject watermark into each Page child if requested
  if (watermark) {
    const elProps = (element as React.ReactElement<any>).props;
    element = React.cloneElement(element as React.ReactElement<any>, {},
      ...React.Children.map(elProps.children, (child: any) => {
        if (!React.isValidElement(child)) return child;
        const childEl = child as React.ReactElement<any>;
        return React.cloneElement(childEl, {},
          React.createElement(Watermark, { text: watermark }),
          ...(React.Children.toArray(childEl.props.children)),
        );
      }) || [],
    );
  }

  // Render to PDF with timeout
  let pdfBytes: Uint8Array;
  try {
    pdfBytes = await withTimeout(renderDocument(element, { embedData: parsed }), 30_000, 'PDF rendering');
  } catch (err: any) {
    throw new Error(`Rendering template '${templateName}' failed: ${err.message}`);
  }

  // Validate and write to disk
  const outputPath = validateOutputPath(output || `./${templateName}.pdf`);
  await writeFile(outputPath, pdfBytes);

  return { path: outputPath, size: pdfBytes.length };
}
