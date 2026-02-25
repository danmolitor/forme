import { writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import * as React from 'react';
import * as FormeReact from '@formepdf/react';

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
  const result = await transform(jsx, {
    loader: 'tsx',
    jsx: 'transform',
    jsxFactory: 'React.createElement',
    jsxFragment: 'React.Fragment',
  });

  // Build the function body: provide all Forme components + React in scope
  const componentNames = [
    'Document', 'Page', 'View', 'Text', 'Image',
    'Table', 'Row', 'Cell', 'Fixed', 'Svg', 'PageBreak',
    'StyleSheet', 'Font',
  ];

  const preamble = componentNames
    .map(name => `const ${name} = FormeReact.${name};`)
    .join('\n');

  const code = `${preamble}\n${result.code}`;

  // Try evaluating as a bare JSX expression first (most common case).
  // Strip trailing semicolons/whitespace so `return (expr)` works.
  const trimmedCode = result.code.replace(/;\s*$/, '').trim();
  let element: any = null;

  try {
    const exprFn = new Function('React', ...componentNames, `
      return (${trimmedCode});
    `);
    element = exprFn(React, ...componentNames.map(n => (FormeReact as any)[n]));
  } catch {
    // Not a bare expression — try as a script with named exports
  }

  // If bare expression didn't work, try as a script defining Template/App
  if (!element || !React.isValidElement(element)) {
    try {
      const fn = new Function('React', 'FormeReact', `
        ${code}
        if (typeof Template !== 'undefined') return Template;
        if (typeof App !== 'undefined') return App;
        return null;
      `);
      element = fn(React, FormeReact);
    } catch {
      // Fall through
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

  const pdfBytes = await renderDocument(element);

  const outputPath = resolve(output || './custom.pdf');
  await writeFile(outputPath, pdfBytes);

  return { path: outputPath, size: pdfBytes.length };
}
