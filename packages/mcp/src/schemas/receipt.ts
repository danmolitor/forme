import { z } from 'zod';

export const receiptSchema = z.object({
  receiptNumber: z.string().describe('Receipt identifier, e.g. "R-84291"'),
  date: z.string().describe('Purchase date'),
  taxRate: z.number().describe('Tax rate as decimal, e.g. 0.0875 for 8.75%'),
  store: z.object({
    name: z.string(),
    address: z.string(),
    cityStateZip: z.string(),
    phone: z.string(),
    website: z.string(),
  }).describe('Store details'),
  items: z.array(z.object({
    name: z.string(),
    price: z.number(),
    quantity: z.number().optional().describe('Defaults to 1'),
  })).describe('Purchased items'),
  paymentMethod: z.string().describe('Payment method, e.g. "Visa"'),
  cardLastFour: z.string().optional().describe('Last 4 digits of card'),
});

export type ReceiptData = z.infer<typeof receiptSchema>;

export const receiptDescription = 'Simple retail receipt with store header, itemized purchases, tax calculation, and payment method.';

export const receiptFields: Record<string, string> = {
  receiptNumber: 'string - receipt identifier',
  date: 'string - purchase date',
  taxRate: 'number - tax rate as decimal',
  store: 'object - store name, address, phone, website',
  items: 'array - items with name, price, and optional quantity',
  paymentMethod: 'string - payment method name',
  cardLastFour: 'string? - last 4 digits of card',
};

export const receiptExample: ReceiptData = {
  receiptNumber: 'R-84291',
  date: 'February 14, 2026',
  taxRate: 0.0875,
  store: {
    name: 'Golden Gate Provisions',
    address: '742 Valencia Street',
    cityStateZip: 'San Francisco, CA 94110',
    phone: '(415) 555-0198',
    website: 'www.goldengateprovisions.com',
  },
  items: [
    { name: 'Organic Sourdough Loaf', price: 7.50, quantity: 2 },
    { name: 'Heirloom Tomatoes (1 lb)', price: 5.99, quantity: 1 },
    { name: 'Local Wildflower Honey (12 oz)', price: 12.00, quantity: 1 },
    { name: 'Free-Range Eggs (dozen)', price: 8.50, quantity: 1 },
    { name: 'Artisan Cheddar (8 oz)', price: 9.75, quantity: 1 },
    { name: 'Cold Brew Coffee (16 oz)', price: 6.50, quantity: 2 },
  ],
  paymentMethod: 'Visa',
  cardLastFour: '4821',
};
