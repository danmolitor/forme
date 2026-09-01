/**
 * One-call rendering convenience: template in, PDF bytes out. Mirrors the
 * Svelte adapter's wrapper — `@formepdf/core` is an optional peer, imported
 * dynamically so hosted-API users who POST JSON never download WASM. Render
 * options forward to core untouched, identical to the React/Svelte paths.
 */
import type { Component } from 'vue';
import type {
  RenderDocumentOptions as CoreRenderDocumentOptions,
  RenderWithLayoutResult,
} from '@formepdf/core';
import { serialize } from './serialize.js';
import type { SerializeOptions } from './serialize.js';

export type RenderDocumentOptions<Props extends Record<string, any>> = SerializeOptions<Props> &
  CoreRenderDocumentOptions;

export type RenderDocumentWithLayoutResult = Omit<RenderWithLayoutResult, 'pdf'> & {
  pdf: Uint8Array<ArrayBuffer>;
};

async function importCore(): Promise<typeof import('@formepdf/core')> {
  try {
    return await import('@formepdf/core');
  } catch (cause) {
    throw new Error(
      'renderDocument requires @formepdf/core, an optional peer dependency of ' +
        '@formepdf/vue. Install it with: npm install @formepdf/core',
      { cause },
    );
  }
}

/** Serialize a Vue template and render it to PDF bytes. */
export async function renderDocument<Props extends Record<string, any>>(
  template: Component,
  options?: RenderDocumentOptions<Props>,
): Promise<Uint8Array<ArrayBuffer>> {
  const { props, ...renderOptions } = options ?? ({} as RenderDocumentOptions<Props>);
  const doc = await serialize(template, { props });
  const core = await importCore();
  const pdf = await core.renderSerializedDoc(doc as unknown as Record<string, unknown>, renderOptions);
  return pdf as Uint8Array<ArrayBuffer>;
}

/** Like `renderDocument` but also returns layout info for overlays. */
export async function renderDocumentWithLayout<Props extends Record<string, any>>(
  template: Component,
  options?: RenderDocumentOptions<Props>,
): Promise<RenderDocumentWithLayoutResult> {
  const { props, ...renderOptions } = options ?? ({} as RenderDocumentOptions<Props>);
  const doc = await serialize(template, { props });
  const core = await importCore();
  const result = await core.renderSerializedDocWithLayout(
    doc as unknown as Record<string, unknown>,
    renderOptions,
  );
  return result as RenderDocumentWithLayoutResult;
}
