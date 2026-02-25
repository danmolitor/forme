import type { ReactElement } from 'react';

import Invoice from './invoice.js';
import Receipt from './receipt.js';
import Report from './report.js';
import ShippingLabel from './shipping-label.js';
import Letter from './letter.js';

import { invoiceSchema, invoiceDescription, invoiceFields, invoiceExample } from '../schemas/invoice.js';
import { receiptSchema, receiptDescription, receiptFields, receiptExample } from '../schemas/receipt.js';
import { reportSchema, reportDescription, reportFields, reportExample } from '../schemas/report.js';
import { shippingLabelSchema, shippingLabelDescription, shippingLabelFields, shippingLabelExample } from '../schemas/shipping-label.js';
import { letterSchema, letterDescription, letterFields, letterExample } from '../schemas/letter.js';

import type { z } from 'zod';

export interface TemplateEntry {
  fn: (data: any) => ReactElement;
  description: string;
  fields: Record<string, string>;
  schema: z.ZodType;
  example: Record<string, unknown>;
}

export const templates: Record<string, TemplateEntry> = {
  invoice: {
    fn: Invoice,
    description: invoiceDescription,
    fields: invoiceFields,
    schema: invoiceSchema,
    example: invoiceExample as unknown as Record<string, unknown>,
  },
  receipt: {
    fn: Receipt,
    description: receiptDescription,
    fields: receiptFields,
    schema: receiptSchema,
    example: receiptExample as unknown as Record<string, unknown>,
  },
  report: {
    fn: Report,
    description: reportDescription,
    fields: reportFields,
    schema: reportSchema,
    example: reportExample as unknown as Record<string, unknown>,
  },
  'shipping-label': {
    fn: ShippingLabel,
    description: shippingLabelDescription,
    fields: shippingLabelFields,
    schema: shippingLabelSchema,
    example: shippingLabelExample as unknown as Record<string, unknown>,
  },
  letter: {
    fn: Letter,
    description: letterDescription,
    fields: letterFields,
    schema: letterSchema,
    example: letterExample as unknown as Record<string, unknown>,
  },
};
