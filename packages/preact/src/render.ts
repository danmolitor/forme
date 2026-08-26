import type { VNode } from 'preact';
import { serialize } from './serialize.js';
import type { FormeDocument } from './types.js';

/**
 * Render a Preact element tree to a Forme JSON string.
 * The top-level element must be a <Document>.
 */
export function render(element: VNode): string {
  return JSON.stringify(serialize(element));
}

/**
 * Render a Preact element tree to a Forme document object.
 * The top-level element must be a <Document>.
 */
export function renderToObject(element: VNode): FormeDocument {
  return serialize(element);
}
