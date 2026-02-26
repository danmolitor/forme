# @formepdf/hono

PDF generation for Hono. Middleware or standalone helpers. Runs on Cloudflare Workers, Deno, Bun, and Node.

## Install

```bash
npm install @formepdf/hono @formepdf/react @formepdf/core hono
```

## Quick Start

```typescript
import { Hono } from 'hono';
import { formePdf } from '@formepdf/hono';

const app = new Hono();
app.use(formePdf());

app.get('/invoice/:id', async (c) => {
  const invoice = await db.invoices.findById(c.req.param('id'));

  return c.pdf('invoice', {
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
});

export default app;
```

## Without Middleware

```typescript
import { pdfResponse } from '@formepdf/hono';

app.get('/invoice/:id', async (c) => {
  const data = await fetchInvoiceData(c.req.param('id'));
  return pdfResponse('invoice', data);
});
```

## Custom Templates

```typescript
import { MyReport } from './templates/report';

app.get('/report', async (c) => {
  const data = await fetchReportData();

  return c.pdf(() => MyReport(data), {
    filename: 'quarterly-report.pdf',
  });
});
```

## Download vs Preview

```typescript
// Opens in browser PDF viewer (default)
return c.pdf('invoice', data);

// Forces download dialog
return c.pdf('invoice', data, { download: true });

// Custom filename + download
return c.pdf('invoice', data, { filename: 'invoice-001.pdf', download: true });
```

## Cloudflare Workers

```typescript
// src/index.ts
import { Hono } from 'hono';
import { formePdf } from '@formepdf/hono';

const app = new Hono();
app.use(formePdf());

app.get('/invoice', async (c) => {
  return c.pdf('invoice', {
    invoiceNumber: 'INV-001',
    date: 'February 25, 2026',
    dueDate: 'March 27, 2026',
    company: { name: 'Acme Corp', initials: 'AC', address: '123 Main St', cityStateZip: 'San Francisco, CA 94105', email: 'billing@acme.com' },
    billTo: { name: 'Jane Smith', company: 'Smith Co', address: '456 Oak Ave', cityStateZip: 'Portland, OR 97201', email: 'jane@smith.co' },
    shipTo: { name: 'Jane Smith', address: '456 Oak Ave', cityStateZip: 'Portland, OR 97201' },
    items: [
      { description: 'Consulting', quantity: 10, unitPrice: 150 },
    ],
    taxRate: 0.08,
    paymentTerms: 'Net 30',
  });
});

export default app;
```

```toml
# wrangler.toml
name = "pdf-api"
compatibility_date = "2024-01-01"
```

```bash
wrangler deploy
```

PDF API running at the edge. No servers, no Puppeteer, no headless Chrome.

## API

### `formePdf(options?)`

Returns Hono middleware that adds `c.pdf()` to the context.

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `defaultDownload` | `boolean` | `false` | Default disposition for all responses |

### `c.pdf(template, data, options?)`

Render a built-in template and return as Response.

### `c.pdf(renderFn, options?)`

Render a custom template and return as Response.

### `pdfResponse(template, data, options?)`

Standalone function. Returns a Response with the PDF. No middleware required.

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

## Why Hono + Forme

Forme's engine is Rust compiled to WASM. It runs anywhere JavaScript runs, including edge runtimes where Puppeteer can't. If you're building an API on Cloudflare Workers and need PDF generation, Forme is one of the only options.

## Links

- [Forme](https://github.com/formepdf/forme) -- PDF generation with JSX
- [Docs](https://docs.formepdf.com)
