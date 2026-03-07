import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { isValidElement, type ReactElement } from 'react';

export interface ResolveElementOptions {
  dataPath?: string;
  data?: unknown;
}

/// Validate a module's default export and resolve it to a React element.
/// Supports both static JSX exports and functions that accept data.
/// When `data` is provided, it takes precedence over `dataPath`.
export async function resolveElement(
  mod: Record<string, unknown>,
  options?: ResolveElementOptions,
): Promise<ReactElement> {
  const exported = mod.default;

  if (exported === undefined) {
    throw new Error(
      `No default export found.\n\n` +
      `  Your file must export a Forme element or a function that returns one:\n\n` +
      `    export default (\n` +
      `      <Document>\n` +
      `        <Text>Hello</Text>\n` +
      `      </Document>\n` +
      `    );\n\n` +
      `  Or with data:\n\n` +
      `    export default function Report(data) {\n` +
      `      return <Document><Text>{data.title}</Text></Document>\n` +
      `    }`
    );
  }

  if (typeof exported === 'function') {
    let data: unknown = {};
    if (options?.data !== undefined) {
      data = options.data;
    } else if (options?.dataPath) {
      data = await loadJsonData(options.dataPath);
    }
    const result = await (exported as (data: unknown) => ReactElement | Promise<ReactElement>)(data);
    if (!isValidElement(result)) {
      throw new Error(
        `Default export function did not return a valid Forme element.\n` +
        `  Got: ${typeof result}\n` +
        `  Make sure your function returns a <Document> element.`
      );
    }
    return result;
  }

  if (isValidElement(exported)) {
    if (options?.dataPath) {
      console.warn(
        `Warning: --data flag provided but default export is a static element, not a function.\n` +
        `  The data file will be ignored. Export a function to use --data.`
      );
    }
    return exported;
  }

  throw new Error(
    `Default export is not a valid Forme element.\n` +
    `  Got: ${typeof exported}\n` +
    `  Expected: a <Document> element or a function that returns one.`
  );
}

async function loadJsonData(dataPath: string): Promise<unknown> {
  const absolutePath = resolve(dataPath);
  const raw = await readFile(absolutePath, 'utf-8');
  try {
    return JSON.parse(raw);
  } catch {
    throw new Error(
      `Failed to parse data file as JSON: ${absolutePath}\n` +
      `  Make sure the file contains valid JSON.`
    );
  }
}
