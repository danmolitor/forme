import { writeFile } from 'node:fs/promises';
import * as React from 'react';
import * as FormeReact from '@formepdf/react';
import { validateOutputPath } from '../utils/validate-output-path.js';
import { withTimeout } from '../utils/timeout.js';

export async function renderCustom(
  jsx: string,
  output?: string,
): Promise<{ path: string; size: number }> {
  // Lazy-import esbuild and core to avoid startup side effects with stdio
  const [{ transform }, { renderDocument }] = await Promise.all([
    import('esbuild'),
    import('@formepdf/core'),
  ]);

  // Transpile JSX to JS
  let result;
  try {
    result = await transform(jsx, {
      loader: 'tsx',
      jsx: 'transform',
      jsxFactory: 'React.createElement',
      jsxFragment: 'React.Fragment',
    });
  } catch (err: any) {
    throw new Error(
      `JSX transpilation failed: ${err.message}\n\nSource:\n${jsx.slice(0, 500)}`
    );
  }

  // Sanitize transpiled code to block imports/requires
  let sanitized = result.code;
  sanitized = sanitized.replace(/\bimport\s+.*?\s+from\s+['"][^'"]*['"]\s*;?/g, '');
  sanitized = sanitized.replace(/\bimport\s*\(/g, '(undefined)(');
  sanitized = sanitized.replace(/\bexport\s+default\s+/g, '');
  sanitized = sanitized.replace(/\bexport\s+/g, '');
  sanitized = sanitized.replace(/\brequire\s*\(/g, '(undefined)(');

  // Build the function body: provide all Forme components + React in scope
  const componentNames = [
    'Document', 'Page', 'View', 'Text', 'Image',
    'Table', 'Row', 'Cell', 'Fixed', 'Svg', 'PageBreak',
    'StyleSheet', 'Font', 'Watermark', 'QrCode',
    'BarChart', 'LineChart', 'PieChart', 'Canvas',
  ];

  const preamble = componentNames
    .map(name => `const ${name} = FormeReact.${name};`)
    .join('\n');

  const code = `${preamble}\n${sanitized}`;

  // Dangerous globals to shadow in the sandbox
  const shadowNames = ['globalThis', 'global', 'process', 'require'];

  // Strip trailing semicolons/whitespace and trailing line comments
  const trimmedCode = sanitized
    .replace(/\/\/[^\n]*$/, '')
    .replace(/;\s*$/, '')
    .trim();

  // Try evaluating as a bare JSX expression first (most common case).
  let element: any = null;

  try {
    const exprFn = new Function('React', 'FormeReact', ...componentNames, ...shadowNames, `
      return (${trimmedCode});
    `);
    element = exprFn(React, FormeReact, ...componentNames.map(n => (FormeReact as any)[n]), ...shadowNames.map(() => undefined));
  } catch {
    // Not a bare expression — try as a script with named exports
  }

  // If bare expression didn't work, try as a script defining Template/App
  if (!element || !React.isValidElement(element)) {
    try {
      const fn = new Function('React', 'FormeReact', ...shadowNames, `
        ${code}
        if (typeof Template !== 'undefined') return Template;
        if (typeof App !== 'undefined') return App;
        return null;
      `);
      element = fn(React, FormeReact, ...shadowNames.map(() => undefined));
    } catch (err: any) {
      throw new Error(
        `JSX evaluation failed: ${err.message}\n\nTranspiled code:\n${sanitized.slice(0, 500)}`
      );
    }
  }

  // If we got a function, call it to get the element
  if (typeof element === 'function') {
    element = element({});
  }

  if (!element || !React.isValidElement(element)) {
    throw new Error(
      'Could not find a React element to render. Your JSX should either:\n' +
      '- Be a single JSX expression (e.g. <Document>...</Document>)\n' +
      '- Define a function called Template or App that returns JSX\n' +
      '- Export a default function'
    );
  }

  const pdfBytes = await withTimeout(renderDocument(element), 30_000, 'PDF rendering');

  const outputPath = validateOutputPath(output || './custom.pdf');
  await writeFile(outputPath, pdfBytes);

  return { path: outputPath, size: pdfBytes.length };
}
