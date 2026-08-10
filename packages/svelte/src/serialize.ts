/**
 * Serializer API: user-authored .svelte template in, document model out.
 *
 * Serialization runs the template through Svelte's server renderer
 * (`render` from `svelte/server`, a public, stable API) and parses the
 * placeholder markup the Forme components emit (see `parser.ts`).
 * `{#each}`, `{#if}`, snippets, and text interpolation are evaluated
 * by Svelte itself - nothing is reimplemented here.
 *
 * All functions are async-first: Svelte's render is synchronous today,
 * but the experimental async SSR can be adopted later without a
 * breaking change. Adapters serialize; only the engine renders PDFs -
 * the `render` export name mirrors the react adapter's historical API
 * and returns a JSON string, not PDF bytes.
 */

import { render as renderSvelteMarkup } from 'svelte/server';
import type { Component } from 'svelte';
import type { FormeDocument } from '@formepdf/shared';
import { parseMarkup } from './parser.js';

export interface SerializeOptions<Props extends Record<string, any>> {
  /** Props passed to the template component. */
  props?: Props;
}

/**
 * Serialize a Svelte template into a Forme JSON document object.
 * The template's top-level element must be a `<Document>`.
 */
export async function serialize<Props extends Record<string, any>>(
  template: Component<Props>,
  options?: SerializeOptions<Props>
): Promise<FormeDocument> {
  const { body } = renderSvelteMarkup(template, { props: (options?.props ?? {}) as Props });
  return parseMarkup(body);
}

/**
 * Serialize a Svelte template to a Forme JSON string.
 * The template's top-level element must be a `<Document>`.
 */
export async function render<Props extends Record<string, any>>(
  template: Component<Props>,
  options?: SerializeOptions<Props>
): Promise<string> {
  return JSON.stringify(await serialize(template, options));
}

/**
 * Serialize a Svelte template to a Forme document object.
 * The template's top-level element must be a `<Document>`.
 */
export async function renderToObject<Props extends Record<string, any>>(
  template: Component<Props>,
  options?: SerializeOptions<Props>
): Promise<FormeDocument> {
  return serialize(template, options);
}
