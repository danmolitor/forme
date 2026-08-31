import { describe, it, expect } from 'vitest';
import { friendlyDependencyError } from '../src/workspace.js';

/** The named-error contract: a missing workspace dependency must read as
 *  guidance naming the package and the file type it blocks, not a raw
 *  createRequire/ESM-loader stack trace. Applies to all four input paths. */
describe('friendlyDependencyError', () => {
  it('names a missing framework runtime from an ESM loader error', () => {
    const err = Object.assign(new Error("Cannot find package 'vue' imported from /proj/.forme-render-1.mjs"), {
      code: 'ERR_MODULE_NOT_FOUND',
    });
    const out = friendlyDependencyError(err, '.vue');
    expect(out.message).toBe('"vue" is not installed in this workspace. Run `npm install vue` to preview .vue files.');
  });

  it('names a missing compiler from a require resolution error', () => {
    const err = Object.assign(new Error("Cannot find module 'svelte/compiler'"), { code: 'MODULE_NOT_FOUND' });
    const out = friendlyDependencyError(err, '.svelte');
    // Subpath collapses to the installable package name.
    expect(out.message).toBe('"svelte" is not installed in this workspace. Run `npm install svelte` to preview .svelte files.');
  });

  it('preserves the scoped adapter package name', () => {
    const err = new Error("Cannot find package '@formepdf/preact' imported from /proj/x.mjs");
    const out = friendlyDependencyError(err, '.tsx');
    expect(out.message).toContain('"@formepdf/preact" is not installed');
    expect(out.message).toContain('npm install @formepdf/preact');
  });

  it('passes a non-resolution error through untouched', () => {
    const err = new Error('Build error: Unexpected token at line 4');
    expect(friendlyDependencyError(err, '.vue')).toBe(err);
  });
});
