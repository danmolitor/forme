import { z } from 'zod';

const tableRowSchema = z.object({
  region: z.string(),
  q1: z.string(),
  q2: z.string(),
  q3: z.string(),
  q4: z.string(),
});

const recommendationSchema = z.object({
  title: z.string(),
  description: z.string(),
  priority: z.string(),
  timeline: z.string(),
});

const sectionSchema = z.object({
  title: z.string(),
  paragraphs: z.array(z.string()).optional(),
  intro: z.string().optional(),
  tableData: z.array(tableRowSchema).optional(),
  items: z.array(recommendationSchema).optional(),
});

export const reportSchema = z.object({
  title: z.string().describe('Report title'),
  subtitle: z.string().describe('Report subtitle'),
  author: z.string().describe('Author name'),
  department: z.string().describe('Department name'),
  company: z.string().describe('Company name'),
  date: z.string().describe('Report date'),
  classification: z.string().describe('Document classification, e.g. "Internal - Confidential"'),
  keyMetrics: z.array(z.object({
    value: z.string(),
    label: z.string(),
  })).optional().describe('Key metric cards shown after executive summary'),
  sections: z.array(sectionSchema).describe('Report sections: [0] Executive Summary (paragraphs), [1] Data (tableData), [2] Visual Analysis (intro only), [3] Recommendations (items)'),
});

export type ReportData = z.infer<typeof reportSchema>;

export const reportDescription = 'Multi-page business report with cover page, table of contents, executive summary, data tables with charts, and recommendations.';

export const reportFields: Record<string, string> = {
  title: 'string - report title',
  subtitle: 'string - report subtitle',
  author: 'string - author name',
  department: 'string - department',
  company: 'string - company name',
  date: 'string - report date',
  classification: 'string - confidentiality level',
  keyMetrics: 'array? - metric cards with value and label',
  sections: 'array - 4 sections: executive summary, data table, visual analysis, recommendations',
};

export const reportExample: ReportData = {
  title: 'Annual Performance Review',
  subtitle: 'Fiscal Year 2025 Results and Strategic Outlook',
  author: 'Rachel Torres',
  department: 'Strategy & Operations',
  company: 'Cascadia Holdings',
  date: 'January 28, 2026',
  classification: 'Internal - Confidential',
  keyMetrics: [
    { value: '$142M', label: 'Total Revenue' },
    { value: '+18%', label: 'Year-over-Year Growth' },
    { value: '94.2%', label: 'Customer Retention' },
  ],
  sections: [
    {
      title: 'Executive Summary',
      paragraphs: [
        'Fiscal year 2025 marked a period of sustained growth, with consolidated revenue reaching $142 million, an 18% increase over the prior year.',
        'Customer retention remained above target at 94.2%, reflecting continued investment in support infrastructure and product reliability.',
        'Operating margins expanded by 2.3 percentage points to 31.4%, driven by economies of scale.',
      ],
    },
    {
      title: 'Revenue by Region',
      intro: 'Quarterly revenue (in millions) by operating region for fiscal year 2025.',
      tableData: [
        { region: 'Pacific Northwest', q1: '$12.4M', q2: '$13.1M', q3: '$14.2M', q4: '$15.8M' },
        { region: 'Northern California', q1: '$8.7M', q2: '$9.2M', q3: '$9.8M', q4: '$10.1M' },
        { region: 'Southern California', q1: '$6.3M', q2: '$6.8M', q3: '$7.1M', q4: '$7.5M' },
        { region: 'Southeast', q1: '$3.2M', q2: '$4.1M', q3: '$4.8M', q4: '$5.6M' },
      ],
    },
    {
      title: 'Visual Analysis',
      intro: 'Charts illustrating key trends in regional performance and quarterly growth.',
    },
    {
      title: 'Recommendations',
      intro: 'Strategic initiatives for the coming fiscal year.',
      items: [
        { title: 'Accelerate Southeast Expansion', description: 'Open regional office in Atlanta with 8 new sales headcount.', priority: 'High', timeline: 'Q1 2026' },
        { title: 'Launch Enterprise Tier', description: 'Dedicated enterprise tier with custom SLAs and advanced analytics.', priority: 'High', timeline: 'Q2 2026' },
        { title: 'International Pilot Program', description: 'Evaluate expansion into Canada and the UK with 10-15 pilot customers.', priority: 'Medium', timeline: 'Q3 2026' },
      ],
    },
  ],
};
