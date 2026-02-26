# @formepdf/next

PDF generation for Next.js App Router. Route handlers, server actions, and custom templates.

## Install

```bash
npm install @formepdf/next @formepdf/react @formepdf/core
```

## Quick Start

```typescript
// app/api/invoice/[id]/route.ts
import { pdfHandler } from '@formepdf/next';

export const GET = pdfHandler('invoice', async (req, { params }) => {
  const invoice = await db.invoices.findById(params.id);

  return {
    invoiceNumber: invoice.number,
    date: invoice.date,
    dueDate: invoice.dueDate,
    company: { name: 'Acme Corp', initials: 'AC', address: '123 Main St', cityStateZip: 'San Francisco, CA 94105', email: 'billing@acme.com' },
    billTo: invoice.customer,
    shipTo: invoice.shipping,
    items: invoice.lineItems,
    taxRate: 0.08,
    paymentTerms: 'Net 30',
  };
});
```

Hit `GET /api/invoice/123`. Get a PDF.

## Three Levels of Control

### `pdfHandler` -- complete route handler

```typescript
export const GET = pdfHandler('invoice', fetchInvoiceData, {
  filename: 'invoice-001.pdf',
  download: true,
});
```

### `pdfResponse` -- inside an existing route handler

```typescript
export async function GET(req: NextRequest) {
  const type = new URL(req.url).searchParams.get('type');

  if (type === 'invoice') return pdfResponse('invoice', invoiceData);
  if (type === 'receipt') return pdfResponse('receipt', receiptData);

  return Response.json({ error: 'Unknown type' }, { status: 400 });
}
```

### `renderPdf` -- raw bytes for server actions

```typescript
'use server';

import { renderPdf } from '@formepdf/next';
import { put } from '@vercel/blob';

export async function generateInvoice(invoiceId: string) {
  const invoice = await db.invoices.findById(invoiceId);
  const pdfBytes = await renderPdf('invoice', {
    invoiceNumber: invoice.number,
    date: invoice.date,
    dueDate: invoice.dueDate,
    company: { name: 'Acme Corp', initials: 'AC', address: '123 Main St', cityStateZip: 'San Francisco, CA 94105', email: 'billing@acme.com' },
    billTo: invoice.customer,
    shipTo: invoice.shipping,
    items: invoice.lineItems,
    taxRate: 0.08,
    paymentTerms: 'Net 30',
  });

  const { url } = await put(`invoices/${invoice.number}.pdf`, pdfBytes, {
    access: 'public',
    contentType: 'application/pdf',
  });

  return url;
}
```

## Custom Templates

```typescript
import { MyReport } from '@/templates/quarterly-report';

export const GET = pdfHandler(async (req) => {
  const data = await fetchReportData();
  return () => MyReport(data);
}, { filename: 'Q1-report.pdf' });
```

## API

### `pdfHandler(template, dataFn, options?)`

Creates a Next.js route handler from a built-in template.

- `template` -- template name
- `dataFn` -- `async (req, context) => data`
- `options` -- `{ filename?, download? }`

### `pdfHandler(renderFnFactory, options?)`

Creates a route handler with a custom render function.

- `renderFnFactory` -- `async (req, context) => () => ReactElement`

### `pdfResponse(template, data, options?)`

Returns a `Response` with the PDF. Use inside existing route handlers.

### `pdfResponse(renderFn, options?)`

Same with a custom render function.

### `renderPdf(template, data)`

Returns raw `Uint8Array` bytes. Use in server actions or anywhere you need bytes without a Response.

### `renderPdf(renderFn)`

Same with a custom render function.

### Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `filename` | `string` | `{template}.pdf` | Filename in Content-Disposition |
| `download` | `boolean` | `false` | Force download vs browser preview |

## Built-in Templates

| Template | Description |
|----------|-------------|
| `invoice` | Line items, tax, totals, company/customer info |
| `receipt` | Payment confirmation, items, total, payment method |
| `report` | Multi-section document with title, headings, body text |
| `letter` | Business letter with letterhead, date, recipient, body |
| `shipping-label` | From/to addresses, 4x6 label format |

## No Puppeteer

This runs in a standard Vercel function. The PDF engine is a 3MB WASM binary, not a 200MB headless browser. A 4-page invoice renders in about 28ms.

## Links

- [Forme](https://github.com/formepdf/forme) -- PDF generation with JSX
- [Docs](https://docs.formepdf.com)
