/**
 * Serializer API: user-authored Vue template in, document model out.
 *
 * Serialization runs the template through Vue's server renderer
 * (`renderToString` from `vue/server-renderer`, a public, stable API) and
 * parses the placeholder markup the Forme components emit (the SAME
 * `parseMarkup` the Svelte adapter uses — it lives in `@formepdf/shared`).
 * `v-for`, `v-if`, slots, and interpolation are evaluated by Vue itself —
 * nothing is reimplemented here. Adapters serialize; only the engine
 * renders PDFs.
 */
import { createSSRApp } from 'vue';
import type { Component } from 'vue';
import { renderToString } from 'vue/server-renderer';
import { parseMarkup } from '@formepdf/shared';
import type { FormeDocument } from '@formepdf/shared';

export interface SerializeOptions<Props extends Record<string, any>> {
  /** Props passed to the root template component. */
  props?: Props;
}

/**
 * Serialize a Vue template into a Forme JSON document object.
 * The template's top-level element must be a `<Document>`.
 */
export async function serialize<Props extends Record<string, any>>(
  template: Component,
  options?: SerializeOptions<Props>,
): Promise<FormeDocument> {
  const app = createSSRApp(template, (options?.props ?? {}) as Record<string, unknown>);
  const markup = await renderToString(app);
  return parseMarkup(markup);
}

/** Serialize a Vue template to a Forme JSON string. */
export async function render<Props extends Record<string, any>>(
  template: Component,
  options?: SerializeOptions<Props>,
): Promise<string> {
  return JSON.stringify(await serialize(template, options));
}

/** Serialize a Vue template to a Forme document object. */
export async function renderToObject<Props extends Record<string, any>>(
  template: Component,
  options?: SerializeOptions<Props>,
): Promise<FormeDocument> {
  return serialize(template, options);
}
