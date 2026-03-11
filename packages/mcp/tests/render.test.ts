import { describe, it, expect, afterEach } from 'vitest';
import { join } from 'node:path';
import { readFile, unlink, mkdir } from 'node:fs/promises';
import { renderPdf } from '../src/tools/render-pdf.js';
import { renderCustom } from '../src/tools/render-custom.js';
import { invoiceExample } from '../src/schemas/invoice.js';

// Write test outputs into a .test-output dir within cwd (passes path validation)
const testDir = join(process.cwd(), '.test-output');
const tmpFile = (name: string) => join(testDir, `${Date.now()}-${name}`);

const filesToClean: string[] = [];

afterEach(async () => {
  for (const f of filesToClean.splice(0)) {
    try { await unlink(f); } catch { /* ignore */ }
  }
});

// Ensure test output directory exists
await mkdir(testDir, { recursive: true });

describe('renderPdf', () => {
  it('renders invoice template to valid PDF', async () => {
    const out = tmpFile('invoice.pdf');
    filesToClean.push(out);
    const result = await renderPdf('invoice', invoiceExample as any, out);
    expect(result.path).toBe(out);
    expect(result.size).toBeGreaterThan(0);
    const bytes = await readFile(out);
    expect(bytes.slice(0, 5).toString()).toBe('%PDF-');
  });

  it('throws for unknown template', async () => {
    await expect(renderPdf('nonexistent', {}, tmpFile('x.pdf'))).rejects.toThrow('not found');
  });

  it('throws for invalid data', async () => {
    await expect(renderPdf('invoice', { bad: true }, tmpFile('x.pdf'))).rejects.toThrow();
  });

  it('rejects path traversal', async () => {
    await expect(renderPdf('invoice', invoiceExample as any, '../escape.pdf')).rejects.toThrow('outside');
  });
});

describe('renderCustom', () => {
  it('renders bare JSX expression', async () => {
    const out = tmpFile('custom.pdf');
    filesToClean.push(out);
    const jsx = `
      <Document>
        <Page size="Letter" margin={48}>
          <Text>Hello from test</Text>
        </Page>
      </Document>
    `;
    const result = await renderCustom(jsx, out);
    expect(result.size).toBeGreaterThan(0);
    const bytes = await readFile(out);
    expect(bytes.slice(0, 5).toString()).toBe('%PDF-');
  });

  it('renders Template function', async () => {
    const out = tmpFile('custom-fn.pdf');
    filesToClean.push(out);
    const jsx = `
      function Template() {
        return (
          <Document>
            <Page size="Letter" margin={48}>
              <Text>Template function</Text>
            </Page>
          </Document>
        );
      }
    `;
    const result = await renderCustom(jsx, out);
    expect(result.size).toBeGreaterThan(0);
  });

  it('throws for invalid JSX', async () => {
    const out = tmpFile('bad.pdf');
    await expect(renderCustom('<<< not valid jsx >>>', out)).rejects.toThrow();
  });

  it('blocks require calls', async () => {
    const out = tmpFile('blocked.pdf');
    const jsx = `
      const fs = require('fs');
      <Document>
        <Page size="Letter" margin={48}>
          <Text>test</Text>
        </Page>
      </Document>
    `;
    await expect(renderCustom(jsx, out)).rejects.toThrow();
  });
});
