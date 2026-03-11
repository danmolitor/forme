import type { GetPromptResult } from '@modelcontextprotocol/sdk/types.js';

export function generateInvoicePrompt(): GetPromptResult {
  return {
    messages: [{
      role: 'user',
      content: {
        type: 'text',
        text: [
          'I need to generate a professional PDF invoice. Please help me collect the following details:',
          '',
          '1. **Invoice number** — e.g. "INV-2026-0142"',
          '2. **Date** and **due date**',
          '3. **Tax rate** — as a decimal, e.g. 0.08 for 8%',
          '4. **Company details** — name, initials (1-3 letters for logo badge), address, email. Optionally a logoUrl (URL to company logo image).',
          '5. **Bill-to address** — customer name, company, address, email',
          '6. **Ship-to address** — name, address',
          '7. **Line items** — each with description, quantity, and unit price',
          '8. **Payment terms** — e.g. "Net 30"',
          '9. **Notes** (optional)',
          '',
          'Once I have these details, use the `render_pdf` tool with template "invoice" to generate the PDF.',
          'You can also pass a `theme` object with `primaryColor`, `fontFamily`, and/or `margins` to customize the look.',
        ].join('\n'),
      },
    }],
  };
}

export function generateReportPrompt(): GetPromptResult {
  return {
    messages: [{
      role: 'user',
      content: {
        type: 'text',
        text: [
          'I need to generate a multi-page business report PDF. Please help me collect:',
          '',
          '1. **Title** and **subtitle**',
          '2. **Author**, **department**, **company**',
          '3. **Date** and **classification** (e.g. "Internal - Confidential")',
          '4. **Key metrics** (optional) — array of {value, label} cards',
          '5. **Sections** (4 required):',
          '   - **Executive Summary** — paragraphs of text',
          '   - **Data section** — intro text + table rows with region, q1-q4 values',
          '   - **Visual Analysis** — intro text (charts auto-generated from table data)',
          '   - **Recommendations** — items with title, description, priority, timeline',
          '',
          'Use the `render_pdf` tool with template "report" to generate the PDF.',
          'You can also pass a `theme` object with `primaryColor`, `fontFamily`, and/or `margins`.',
        ].join('\n'),
      },
    }],
  };
}

export function createCustomPdfPrompt(): GetPromptResult {
  return {
    messages: [{
      role: 'user',
      content: {
        type: 'text',
        text: [
          'I want to create a custom PDF using JSX. Here are the available Forme components:',
          '',
          '**Layout**: `<Document>`, `<Page>`, `<View>`, `<PageBreak>`',
          '**Text**: `<Text>` — supports fontSize, fontWeight, color, textAlign, lineHeight, textDecoration, textTransform',
          '**Tables**: `<Table columns={[...]}>`, `<Row>`, `<Cell>`',
          '**Media**: `<Image src="..." />`, `<Svg width={} height={} content="..." />`',
          '**Fixed**: `<Fixed position="header|footer">` — repeats on every page',
          '**Watermark**: `<Watermark text="DRAFT" />` — rotated text behind content',
          '**Charts**: `<BarChart>`, `<LineChart>`, `<PieChart>`',
          '**QR Code**: `<QrCode data="..." size={100} />`',
          '',
          '**Styles** use a React Native/CSS-like object: `style={{ flexDirection: "row", gap: 16, padding: 12, backgroundColor: "#f8fafc" }}`',
          '',
          '**Example JSX**:',
          '```tsx',
          '<Document>',
          '  <Page size="Letter" margin={48}>',
          '    <View style={{ flexDirection: "row", justifyContent: "space-between" }}>',
          '      <Text style={{ fontSize: 24, fontWeight: 700 }}>Title</Text>',
          '      <Text style={{ fontSize: 10, color: "#64748b" }}>Subtitle</Text>',
          '    </View>',
          '  </Page>',
          '</Document>',
          '```',
          '',
          'Describe the PDF you want and I\'ll write the JSX and render it using `render_custom_pdf`.',
        ].join('\n'),
      },
    }],
  };
}
