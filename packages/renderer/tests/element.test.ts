import { describe, it, expect, vi } from 'vitest';
import React from 'react';

vi.mock('node:fs/promises', () => ({
  readFile: vi.fn(),
}));

import { resolveElement } from '../src/element.js';

describe('resolveElement', () => {
  it('returns a static React element default export', async () => {
    const el = React.createElement('div', null, 'hello');
    const mod = { default: el };
    const result = await resolveElement(mod);
    expect(result).toBe(el);
  });

  it('calls a function default export with empty object when no data', async () => {
    const el = React.createElement('div', null, 'from fn');
    const fn = vi.fn().mockReturnValue(el);
    const mod = { default: fn };

    const result = await resolveElement(mod);
    expect(fn).toHaveBeenCalledWith({});
    expect(result).toBe(el);
  });

  it('calls a function default export with provided data', async () => {
    const el = React.createElement('div', null, 'data');
    const fn = vi.fn().mockReturnValue(el);
    const mod = { default: fn };

    const result = await resolveElement(mod, { data: { title: 'Test' } });
    expect(fn).toHaveBeenCalledWith({ title: 'Test' });
    expect(result).toBe(el);
  });

  it('throws when no default export', async () => {
    const mod = {};
    await expect(resolveElement(mod)).rejects.toThrow('No default export found');
  });

  it('throws when default export is not a valid element or function', async () => {
    const mod = { default: 42 };
    await expect(resolveElement(mod as any)).rejects.toThrow(
      'Default export is not a valid Forme element'
    );
  });

  it('throws when function returns non-element', async () => {
    const fn = vi.fn().mockReturnValue('not an element');
    const mod = { default: fn };
    await expect(resolveElement(mod as any)).rejects.toThrow(
      'did not return a valid Forme element'
    );
  });
});
