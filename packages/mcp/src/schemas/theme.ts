import { z } from 'zod';

export const themeSchema = z.object({
  primaryColor: z.string().optional().describe('Accent color (CSS hex), e.g. "#2563eb"'),
  fontFamily: z.string().optional().describe('Font family name, e.g. "Helvetica"'),
  margins: z.union([
    z.number(),
    z.object({
      top: z.number(),
      right: z.number(),
      bottom: z.number(),
      left: z.number(),
    }),
  ]).optional().describe('Page margins — number (all sides) or {top,right,bottom,left}'),
}).optional().describe('Optional theme customization');

export type Theme = z.infer<typeof themeSchema>;
