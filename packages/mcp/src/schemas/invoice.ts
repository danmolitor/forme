import { z } from 'zod';

export const invoiceSchema = z.object({
  invoiceNumber: z.string().describe('Invoice identifier, e.g. "INV-2026-0142"'),
  date: z.string().describe('Invoice date, e.g. "February 10, 2026"'),
  dueDate: z.string().describe('Payment due date'),
  taxRate: z.number().describe('Tax rate as decimal, e.g. 0.08 for 8%'),
  company: z.object({
    name: z.string(),
    initials: z.string().describe('1-3 letter initials for logo badge'),
    address: z.string(),
    cityStateZip: z.string(),
    email: z.string(),
  }).describe('Your company details'),
  billTo: z.object({
    name: z.string(),
    company: z.string(),
    address: z.string(),
    cityStateZip: z.string(),
    email: z.string(),
  }).describe('Customer billing address'),
  shipTo: z.object({
    name: z.string(),
    address: z.string(),
    cityStateZip: z.string(),
  }).describe('Shipping address'),
  items: z.array(z.object({
    description: z.string(),
    quantity: z.number(),
    unitPrice: z.number(),
  })).describe('Line items'),
  paymentTerms: z.string().describe('Payment terms paragraph'),
  notes: z.string().optional().describe('Optional notes'),
});

export type InvoiceData = z.infer<typeof invoiceSchema>;

export const invoiceDescription = 'Professional invoice with company header, billing/shipping addresses, itemized line items table, tax calculation, and payment terms.';

export const invoiceFields: Record<string, string> = {
  invoiceNumber: 'string - invoice identifier',
  date: 'string - invoice date',
  dueDate: 'string - payment due date',
  taxRate: 'number - tax rate as decimal (e.g. 0.08)',
  company: 'object - your company name, initials, address, email',
  billTo: 'object - customer name, company, address, email',
  shipTo: 'object - shipping name, address',
  items: 'array - line items with description, quantity, unitPrice',
  paymentTerms: 'string - payment terms text',
  notes: 'string? - optional notes',
};

export const invoiceExample: InvoiceData = {
  invoiceNumber: 'INV-2026-0142',
  date: 'February 10, 2026',
  dueDate: 'March 12, 2026',
  taxRate: 0.08,
  company: {
    name: 'Northwind Design Co.',
    initials: 'ND',
    address: '1847 Lakewood Boulevard, Suite 300',
    cityStateZip: 'Portland, OR 97205',
    email: 'billing@northwinddesign.co',
  },
  billTo: {
    name: 'Sarah Chen',
    company: 'Meridian Architecture Group',
    address: '520 Market Street, Floor 14',
    cityStateZip: 'San Francisco, CA 94105',
    email: 'sarah.chen@meridianarch.com',
  },
  shipTo: {
    name: 'Meridian Architecture Group',
    address: '520 Market Street, Floor 14',
    cityStateZip: 'San Francisco, CA 94105',
  },
  items: [
    { description: 'Brand Identity Package - logo design, color palette, typography system', quantity: 1, unitPrice: 4500.00 },
    { description: 'Website Design - homepage, about, services, contact (desktop + mobile)', quantity: 1, unitPrice: 6200.00 },
    { description: 'Business Card Design - front and back, print-ready files', quantity: 1, unitPrice: 350.00 },
    { description: 'Social Media Templates - Instagram, LinkedIn, Twitter (12 templates)', quantity: 12, unitPrice: 125.00 },
    { description: 'Brand Guidelines Document - 24-page PDF with usage rules', quantity: 1, unitPrice: 1800.00 },
  ],
  paymentTerms: 'Net 30. Payment due within 30 days of invoice date. A late fee of 1.5% per month will be applied to overdue balances.',
  notes: 'Thank you for choosing Northwind Design.',
};
