import { z } from 'zod';

export const shippingLabelSchema = z.object({
  tracking: z.string().describe('Tracking number'),
  service: z.string().describe('Shipping service level, e.g. "Priority"'),
  weight: z.string().describe('Package weight, e.g. "4.2 lbs"'),
  dimensions: z.string().describe('Package dimensions, e.g. "12 x 8 x 6 in"'),
  from: z.object({
    name: z.string(),
    address: z.string(),
    cityStateZip: z.string(),
  }).describe('Sender address'),
  to: z.object({
    name: z.string(),
    address: z.string(),
    address2: z.string().optional(),
    cityStateZip: z.string(),
  }).describe('Recipient address'),
  stamps: z.array(z.string()).optional().describe('Handling stamps, e.g. ["FRAGILE", "THIS SIDE UP"]'),
});

export type ShippingLabelData = z.infer<typeof shippingLabelSchema>;

export const shippingLabelDescription = 'Compact 4x6 shipping label with sender/recipient addresses, barcode placeholder, tracking number, and handling stamps.';

export const shippingLabelFields: Record<string, string> = {
  tracking: 'string - tracking number',
  service: 'string - shipping service level',
  weight: 'string - package weight',
  dimensions: 'string - package dimensions',
  from: 'object - sender name, address, cityStateZip',
  to: 'object - recipient name, address, optional address2, cityStateZip',
  stamps: 'array? - handling stamps like "FRAGILE"',
};

export const shippingLabelExample: ShippingLabelData = {
  tracking: '1Z 999 AA1 0123 4567 890',
  service: 'Priority',
  weight: '4.2 lbs',
  dimensions: '12 x 8 x 6 in',
  from: {
    name: 'Cascade Electronics',
    address: '2901 Third Avenue, Unit 4B',
    cityStateZip: 'Seattle, WA 98121',
  },
  to: {
    name: 'Marcus Webb',
    address: '1455 Pennsylvania Ave NW',
    address2: 'Apt 712',
    cityStateZip: 'Washington, DC 20004',
  },
  stamps: ['FRAGILE', 'THIS SIDE UP'],
};
