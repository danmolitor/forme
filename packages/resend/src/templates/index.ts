import type { ReactElement } from 'react';
import Invoice from './invoice.js';
import Receipt from './receipt.js';
import Report from './report.js';
import ShippingLabel from './shipping-label.js';
import Letter from './letter.js';

const templates: Record<string, (data: any) => ReactElement> = {
  invoice: Invoice,
  receipt: Receipt,
  report: Report,
  'shipping-label': ShippingLabel,
  letter: Letter,
};

export function getTemplate(name: string): ((data: any) => ReactElement) | null {
  return templates[name] || null;
}

export function listTemplates(): { name: string }[] {
  return Object.keys(templates).map(name => ({ name }));
}
