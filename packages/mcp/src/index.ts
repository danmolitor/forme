#!/usr/bin/env node

import { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import { z } from 'zod';
import { listTemplates } from './tools/list-templates.js';
import { getSchema } from './tools/get-schema.js';
import { renderPdf } from './tools/render-pdf.js';
import { renderCustom } from './tools/render-custom.js';

const server = new McpServer({
  name: 'forme',
  version: '0.4.4',
});

// ── list_templates ──────────────────────────────────────────────────

server.tool(
  'list_templates',
  'List available built-in PDF templates with descriptions and field summaries',
  {},
  async () => {
    const result = listTemplates();
    return {
      content: [{
        type: 'text' as const,
        text: JSON.stringify(result, null, 2),
      }],
    };
  },
);

// ── get_template_schema ─────────────────────────────────────────────

server.tool(
  'get_template_schema',
  'Get full JSON Schema and example data for a specific template',
  { template: z.string().describe('Template name (e.g. "invoice", "receipt", "report", "shipping-label", "letter")') },
  async ({ template }) => {
    try {
      const result = getSchema(template);
      return {
        content: [{
          type: 'text' as const,
          text: JSON.stringify(result, null, 2),
        }],
      };
    } catch (err: any) {
      return {
        isError: true,
        content: [{
          type: 'text' as const,
          text: err.message,
        }],
      };
    }
  },
);

// ── render_pdf ──────────────────────────────────────────────────────

server.tool(
  'render_pdf',
  'Render a built-in template with data and write PDF to disk',
  {
    template: z.string().describe('Template name (e.g. "invoice", "receipt", "report", "shipping-label", "letter")'),
    data: z.record(z.unknown()).describe('Template data matching the template schema'),
    output: z.string().optional().describe('Output file path (defaults to ./{template}.pdf)'),
  },
  async ({ template, data, output }) => {
    try {
      const result = await renderPdf(template, data, output);
      return {
        content: [{
          type: 'text' as const,
          text: `PDF rendered successfully.\nPath: ${result.path}\nSize: ${(result.size / 1024).toFixed(1)} KB`,
        }],
      };
    } catch (err: any) {
      return {
        isError: true,
        content: [{
          type: 'text' as const,
          text: `Failed to render PDF: ${err.message}`,
        }],
      };
    }
  },
);

// ── render_custom_pdf ───────────────────────────────────────────────

server.tool(
  'render_custom_pdf',
  'Render arbitrary JSX to PDF. Use Forme components: Document, Page, View, Text, Image, Table, Row, Cell, Fixed, Svg, PageBreak, StyleSheet, Font',
  {
    jsx: z.string().describe('JSX/TSX source code using Forme components (Document, Page, View, Text, etc.)'),
    output: z.string().optional().describe('Output file path (defaults to ./custom.pdf)'),
  },
  async ({ jsx, output }) => {
    try {
      const result = await renderCustom(jsx, output);
      return {
        content: [{
          type: 'text' as const,
          text: `PDF rendered successfully.\nPath: ${result.path}\nSize: ${(result.size / 1024).toFixed(1)} KB`,
        }],
      };
    } catch (err: any) {
      return {
        isError: true,
        content: [{
          type: 'text' as const,
          text: `Failed to render custom PDF: ${err.message}`,
        }],
      };
    }
  },
);

// ── Start server ────────────────────────────────────────────────────

async function main() {
  const transport = new StdioServerTransport();
  await server.connect(transport);
  console.error('Forme MCP server running on stdio');
}

main().catch((err) => {
  console.error('Fatal error:', err);
  process.exit(1);
});
