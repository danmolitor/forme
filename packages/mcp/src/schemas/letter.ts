import { z } from 'zod';

export const letterSchema = z.object({
  sender: z.object({
    name: z.string(),
    title: z.string().optional(),
    company: z.string(),
    address: z.string(),
    cityStateZip: z.string(),
    phone: z.string().optional(),
    email: z.string().optional(),
  }).describe('Sender/letterhead details'),
  date: z.string().describe('Letter date'),
  recipient: z.object({
    name: z.string(),
    title: z.string().optional(),
    company: z.string().optional(),
    address: z.string(),
    cityStateZip: z.string(),
  }).describe('Recipient address'),
  salutation: z.string().describe('Greeting, e.g. "Dear Ms. Chen,"'),
  body: z.array(z.string()).describe('Body paragraphs'),
  closing: z.string().describe('Closing phrase, e.g. "Sincerely,"'),
  signatureName: z.string().describe('Printed name under signature'),
  signatureTitle: z.string().optional().describe('Title under signature'),
});

export type LetterData = z.infer<typeof letterSchema>;

export const letterDescription = 'Formal business letter with letterhead, recipient address, body paragraphs, and closing signature.';

export const letterFields: Record<string, string> = {
  sender: 'object - name, title?, company, address, cityStateZip, phone?, email?',
  date: 'string - letter date',
  recipient: 'object - name, title?, company?, address, cityStateZip',
  salutation: 'string - greeting line',
  body: 'array - body paragraphs as strings',
  closing: 'string - closing phrase',
  signatureName: 'string - printed name',
  signatureTitle: 'string? - title under name',
};

export const letterExample: LetterData = {
  sender: {
    name: 'James Mitchell',
    title: 'Director of Partnerships',
    company: 'Northwind Design Co.',
    address: '1847 Lakewood Boulevard, Suite 300',
    cityStateZip: 'Portland, OR 97205',
    phone: '(503) 555-0147',
    email: 'james.mitchell@northwinddesign.co',
  },
  date: 'February 24, 2026',
  recipient: {
    name: 'Sarah Chen',
    title: 'VP of Operations',
    company: 'Meridian Architecture Group',
    address: '520 Market Street, Floor 14',
    cityStateZip: 'San Francisco, CA 94105',
  },
  salutation: 'Dear Ms. Chen,',
  body: [
    'Thank you for our productive meeting last Thursday to discuss the upcoming Meridian rebrand initiative. We are excited about the opportunity to partner with your team on this project.',
    'As discussed, our proposal includes a comprehensive brand identity package, website redesign, and full suite of marketing collateral. The enclosed statement of work outlines the project timeline, deliverables, and investment summary for your review.',
    'We believe our experience with architectural and design firms positions us uniquely to capture the essence of Meridian\'s vision. Our team is prepared to begin discovery sessions as early as next month.',
    'Please don\'t hesitate to reach out if you have any questions. We look forward to hearing from you.',
  ],
  closing: 'Sincerely,',
  signatureName: 'James Mitchell',
  signatureTitle: 'Director of Partnerships',
};
