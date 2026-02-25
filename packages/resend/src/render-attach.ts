import { renderDocument } from '@formepdf/core';
import { getTemplate } from './templates/index.js';
import type { RenderAttachOptions } from './types.js';

export async function renderAndAttach(options: RenderAttachOptions) {
  const { template, data, render, filename } = options;

  let element;
  if (render) {
    element = render();
  } else if (template) {
    const templateFn = getTemplate(template);
    if (!templateFn) {
      throw new Error(`Unknown template: "${template}".`);
    }
    element = templateFn(data || {});
  } else {
    throw new Error('Either "template" or "render" must be provided.');
  }

  const pdfBytes = await renderDocument(element);

  return {
    filename: filename || `${template || 'document'}.pdf`,
    content: Buffer.from(pdfBytes),
  };
}
