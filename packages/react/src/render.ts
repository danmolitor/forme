import type { ReactElement } from 'react';
import { serialize } from './serialize.js';
import type { FormeDocument } from './types.js';

/**
 * Render a React element tree to a Forme JSON string.
 * The top-level element must be a <Document>.
 */
export function render(element: ReactElement): string {
  return JSON.stringify(serialize(element));
}

/**
 * Render a React element tree to a Forme document object.
 * The top-level element must be a <Document>.
 */
export function renderToObject(element: ReactElement): FormeDocument {
  return serialize(element);
}
