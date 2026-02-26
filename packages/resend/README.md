# @formepdf/resend

Render a PDF and email it in one call. Uses [Forme](https://github.com/formepdf/forme) for PDF generation and [Resend](https://resend.com) for delivery.

## Install

```bash
npm install @formepdf/resend resend @formepdf/react @formepdf/core
```

## Quick Start

```typescript
import { sendPdf } from '@formepdf/resend';

await sendPdf({
  resendApiKey: process.env.RESEND_API_KEY,
  from: 'Acme Corp <billing@acme.com>',
  to: 'customer@email.com',
  subject: 'Invoice #001',
  template: 'invoice',
  data: {
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
  },
});
```

One call. PDF rendered, email sent, invoice attached.

## Custom Templates

```typescript
import { sendPdf } from '@formepdf/resend';
import { MyTemplate } from './my-template';

await sendPdf({
  resendApiKey: process.env.RESEND_API_KEY,
  from: 'billing@acme.com',
  to: 'customer@email.com',
  subject: 'Your document',
  render: () => MyTemplate({ name: 'Jane' }),
});
```

## Add PDF to an Existing Resend Call

```typescript
import { renderAndAttach } from '@formepdf/resend';
import { Resend } from 'resend';

const resend = new Resend(process.env.RESEND_API_KEY);
const pdf = await renderAndAttach({ template: 'invoice', data: invoiceData });

await resend.emails.send({
  from: 'billing@acme.com',
  to: 'customer@email.com',
  subject: 'Your invoice',
  html: '<p>See attached.</p>',
  attachments: [pdf],
});
```

`renderAndAttach` returns `{ filename, content }` which is exactly what Resend's attachment API expects.

## API

### `sendPdf(options)`

Render a PDF and email it. Returns `Promise<{ id: string }>` (Resend's email ID).

| Option | Type | Required | Description |
|--------|------|----------|-------------|
| `resendApiKey` | `string` | Yes | Resend API key |
| `from` | `string` | Yes | Sender address |
| `to` | `string \| string[]` | Yes | Recipient(s) |
| `subject` | `string` | Yes | Email subject |
| `template` | `string` | One of template/render | Built-in template name |
| `data` | `object` | With template | Data for the template |
| `render` | `() => ReactElement` | One of template/render | Custom render function |
| `filename` | `string` | No | PDF filename (default: `{template}.pdf`) |
| `html` | `string` | No | Email body HTML |
| `text` | `string` | No | Email body plain text |
| `react` | `ReactElement` | No | React Email component for email body |
| `cc` | `string \| string[]` | No | CC recipients |
| `bcc` | `string \| string[]` | No | BCC recipients |
| `replyTo` | `string` | No | Reply-to address |

When `html`, `text`, and `react` are all omitted, a default email body is generated based on the template type.

### `renderAndAttach(options)`

Render a PDF and return a Resend-compatible attachment object. Does not send.

### `listTemplates()`

Returns available built-in templates with descriptions.

## Built-in Templates

| Template | Description |
|----------|-------------|
| `invoice` | Line items, tax, totals, company/customer info |
| `receipt` | Payment confirmation, items, total, payment method |
| `report` | Multi-section document with title, headings, body text |
| `letter` | Business letter with letterhead, date, recipient, body |
| `shipping-label` | From/to addresses, 4x6 label format |

## Links

- [Forme](https://github.com/formepdf/forme) -- PDF generation with JSX
- [Resend](https://resend.com) -- Email for developers
- [Docs](https://docs.formepdf.com)
