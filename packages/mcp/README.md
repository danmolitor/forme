# @formepdf/mcp

MCP server that lets AI tools generate PDFs. Add one line to your config, then ask Claude, Cursor, or Windsurf to generate invoices, receipts, reports, and more.

## Setup

Add to your MCP config:

```json
{
  "mcpServers": {
    "forme": {
      "command": "npx",
      "args": ["@formepdf/mcp"]
    }
  }
}
```

Restart your AI tool. Done.

## What you can say

- "Generate an invoice for Acme Corp, 10 hours of consulting at $150/hour"
- "Create a shipping label from Seattle to NYC, 3 lbs, fragile"
- "Make a receipt for 2 lattes and a muffin"
- "Write a business letter to Jane Smith about our partnership proposal"
- "Create a custom PDF with a big centered title that says Hello World"

The agent figures out the data shape from the tool schema. You don't need to know the template fields.

## Tools

### `list_templates`

Returns all available templates with descriptions and field summaries.

### `get_template_schema`

Returns the full JSON Schema and example data for a specific template. This is how the agent knows what data to construct from your request.

### `render_pdf`

Renders a built-in template with data and writes the PDF to disk.

```
Input: { template: "invoice", data: { ... }, output: "invoice.pdf" }
Output: PDF file at the specified path
```

### `render_custom_pdf`

Renders arbitrary JSX to PDF. The agent writes Forme JSX on the fly, the server transpiles it with esbuild and renders it.

```
Input: { jsx: "<Document><Page>...</Page></Document>", output: "custom.pdf" }
Output: PDF file at the specified path
```

## Built-in Templates

| Template | Description |
|----------|-------------|
| `invoice` | Line items, tax, totals, company/customer info |
| `receipt` | Payment confirmation, items, total, payment method |
| `report` | Multi-section document with title, headings, body text |
| `letter` | Business letter with letterhead, date, recipient, body |
| `shipping-label` | From/to addresses, weight, 4x6 format |

## How it works

The MCP server runs locally. PDF rendering happens in-process via Forme's Rust/WASM engine. No network calls, no API keys, no browser. The agent calls the tool, gets a file path back.

## Works with

- Claude Code
- Cursor
- Windsurf
- Any tool supporting the Model Context Protocol

## Links

- [Forme](https://github.com/formepdf/forme) -- PDF generation with JSX
- [Docs](https://docs.formepdf.com)
- [MCP Specification](https://modelcontextprotocol.io)
