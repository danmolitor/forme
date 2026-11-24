import { getTemplate, listTemplates } from '@formepdf/templates';
import type { MiddlewareHandler } from 'hono';
import type { ReactElement } from 'react';

interface PdfOptions {
  filename?: string;
  download?: boolean;
}

interface FormePdfOptions {
  defaultDownload?: boolean;
}

// --- Engine loading ---
// Each entry point (index.ts for Node and bundlers, worker.ts for Cloudflare
// Workers) sets how the engine is loaded; the rest of the API is shared.

export type RenderDocumentFn = (element: ReactElement) => Promise<Uint8Array>;

let loadRenderDocument: (() => Promise<RenderDocumentFn>) | null = null;
let _renderDocument: RenderDocumentFn | null = null;

export function setEngineLoader(loader: () => Promise<RenderDocumentFn>): void {
  loadRenderDocument = loader;
}

async function getRenderDocument(): Promise<RenderDocumentFn> {
  if (_renderDocument) return _renderDocument;
  if (!loadRenderDocument) throw new Error('@formepdf/hono: no engine loader set');
  _renderDocument = await loadRenderDocument();
  return _renderDocument;
}

// --- Render callback invocation (with React Compiler diagnostic) ---
// User-supplied render callbacks are called outside React's render cycle.
// If the callback was compiled by React Compiler, it will inject useMemoCache
// and throw a cryptic "Invalid hook call" error. Catch and rethrow with guidance.

function callRenderFn(fn: () => ReactElement, entryPoint: string): ReactElement {
  try {
    return fn();
  } catch (err) {
    if (err instanceof Error && /hook|useMemoCache|Invalid hook call/i.test(err.message)) {
      const name = (fn as any).displayName || fn.name || 'render callback';
      throw new Error(
        `The function passed to ${entryPoint}() ("${name}") appears to be compiled by ` +
        `React Compiler, which injects hooks that cannot run outside of React's render cycle.\n\n` +
        `Fix: Add 'use no memo' at the top of the function to opt it out:\n\n` +
        `  function ${name}() {\n` +
        `    'use no memo';\n` +
        `    return <Document>...</Document>;\n` +
        `  }\n\n` +
        `Alternatively, wrap it in an inline arrow:\n\n` +
        `  ${entryPoint}(() => <${name} />)`
      );
    }
    throw err;
  }
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
    element = callRenderFn(renderFn, 'pdfResponse');
    options.filename = options.filename || 'document.pdf';
  }

  const renderDocument = await getRenderDocument();
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
