import { renderDocument } from '@formepdf/core';
import { getTemplate, listTemplates } from './templates/index.js';
import type { ReactElement } from 'react';

interface PdfOptions {
  filename?: string;
  download?: boolean;
}

// --- renderPdf: returns raw bytes ---

export async function renderPdf(
  templateOrRenderFn: string | (() => ReactElement),
  data?: Record<string, any>
): Promise<Uint8Array> {
  let element: ReactElement;

  if (typeof templateOrRenderFn === 'string') {
    const templateFn = getTemplate(templateOrRenderFn);
    if (!templateFn) {
      throw new Error(`Unknown template: "${templateOrRenderFn}"`);
    }
    element = templateFn(data || {});
  } else {
    element = templateOrRenderFn();
  }

  return renderDocument(element);
}

// --- pdfResponse: returns a Response object ---

export async function pdfResponse(
  templateOrRenderFn: string | (() => ReactElement),
  dataOrOptions?: Record<string, any> | PdfOptions,
  maybeOptions?: PdfOptions
): Promise<Response> {
  let pdfBytes: Uint8Array;
  let options: PdfOptions;

  if (typeof templateOrRenderFn === 'string') {
    const data = (dataOrOptions as Record<string, any>) || {};
    options = maybeOptions || {};
    pdfBytes = await renderPdf(templateOrRenderFn, data);
    options.filename = options.filename || `${templateOrRenderFn}.pdf`;
  } else {
    options = (dataOrOptions as PdfOptions) || {};
    pdfBytes = await renderPdf(templateOrRenderFn);
    options.filename = options.filename || 'document.pdf';
  }

  const disposition = options.download ? 'attachment' : 'inline';

  return new Response(pdfBytes as unknown as BodyInit, {
    headers: {
      'Content-Type': 'application/pdf',
      'Content-Disposition': `${disposition}; filename="${options.filename}"`,
      'Content-Length': String(pdfBytes.byteLength),
    },
  });
}

// --- pdfHandler: creates a route handler ---

export function pdfHandler(
  templateOrRenderFn: string | ((req: Request, context: any) => Promise<() => ReactElement>),
  dataFnOrOptions?: ((req: Request, context: any) => Promise<Record<string, any>>) | PdfOptions,
  maybeOptions?: PdfOptions
) {
  if (typeof templateOrRenderFn === 'string') {
    const template = templateOrRenderFn;
    const dataFn = dataFnOrOptions as (req: Request, context: any) => Promise<Record<string, any>>;
    const options = maybeOptions || {};

    return async (req: Request, context: any): Promise<Response> => {
      try {
        const data = await dataFn(req, context);
        return pdfResponse(template, data, options);
      } catch (err) {
        return Response.json(
          { error: 'PDF render failed', message: (err as Error).message },
          { status: 500 }
        );
      }
    };
  } else {
    const renderFnFactory = templateOrRenderFn;
    const options = (dataFnOrOptions as PdfOptions) || {};

    return async (req: Request, context: any): Promise<Response> => {
      try {
        const renderFn = await renderFnFactory(req, context);
        return pdfResponse(renderFn, options);
      } catch (err) {
        return Response.json(
          { error: 'PDF render failed', message: (err as Error).message },
          { status: 500 }
        );
      }
    };
  }
}

export { listTemplates };
export type { PdfOptions };
