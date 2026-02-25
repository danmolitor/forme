import { renderDocument } from '@formepdf/core';
import { getTemplate, listTemplates } from './templates/index.js';
import type { MiddlewareHandler } from 'hono';
import type { ReactElement } from 'react';

interface PdfOptions {
  filename?: string;
  download?: boolean;
}

interface FormePdfOptions {
  defaultDownload?: boolean;
}

// --- Standalone pdfResponse (no middleware needed) ---

export async function pdfResponse(
  templateOrRenderFn: string | (() => ReactElement),
  dataOrOptions?: Record<string, any> | PdfOptions,
  maybeOptions?: PdfOptions
): Promise<Response> {
  let element: ReactElement;
  let options: PdfOptions;

  if (typeof templateOrRenderFn === 'string') {
    const template = templateOrRenderFn;
    const data = (dataOrOptions as Record<string, any>) || {};
    options = maybeOptions || {};

    const templateFn = getTemplate(template);
    if (!templateFn) {
      return Response.json({ error: `Unknown template: "${template}"` }, { status: 400 });
    }
    element = templateFn(data);
    options.filename = options.filename || `${template}.pdf`;
  } else {
    const renderFn = templateOrRenderFn;
    options = (dataOrOptions as PdfOptions) || {};
    element = renderFn();
    options.filename = options.filename || 'document.pdf';
  }

  const pdfBytes = await renderDocument(element);
  const disposition = (options.download ?? false) ? 'attachment' : 'inline';

  return new Response(pdfBytes as unknown as BodyInit, {
    headers: {
      'Content-Type': 'application/pdf',
      'Content-Disposition': `${disposition}; filename="${options.filename}"`,
      'Content-Length': String(pdfBytes.byteLength),
    },
  });
}

// --- Middleware (adds c.pdf()) ---

declare module 'hono' {
  interface Context {
    pdf: (
      templateOrRenderFn: string | (() => ReactElement),
      dataOrOptions?: Record<string, any> | PdfOptions,
      maybeOptions?: PdfOptions
    ) => Promise<Response>;
  }
}

export function formePdf(opts?: FormePdfOptions): MiddlewareHandler {
  const defaultDownload = opts?.defaultDownload ?? false;

  return async (c, next) => {
    (c as any).pdf = async function (
      templateOrRenderFn: string | (() => ReactElement),
      dataOrOptions?: Record<string, any> | PdfOptions,
      maybeOptions?: PdfOptions
    ): Promise<Response> {
      let options: PdfOptions;
      if (typeof templateOrRenderFn === 'string') {
        options = maybeOptions || {};
      } else {
        options = (dataOrOptions as PdfOptions) || {};
      }
      if (options.download === undefined) {
        options.download = defaultDownload;
      }

      return pdfResponse(
        templateOrRenderFn,
        dataOrOptions,
        typeof templateOrRenderFn === 'string' ? options : undefined
      );
    };

    await next();
  };
}

export { listTemplates };
export type { PdfOptions, FormePdfOptions };
