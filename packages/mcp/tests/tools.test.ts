import { describe, it, expect } from 'vitest';
import { listTemplates } from '../src/tools/list-templates.js';
import { getSchema } from '../src/tools/get-schema.js';

describe('listTemplates', () => {
  it('returns an array of template entries', () => {
    const result = listTemplates();
    expect(Array.isArray(result)).toBe(true);
    expect(result.length).toBeGreaterThan(0);

    for (const entry of result) {
      expect(entry).toHaveProperty('name');
      expect(entry).toHaveProperty('description');
      expect(entry).toHaveProperty('fields');
      expect(typeof entry.name).toBe('string');
      expect(typeof entry.description).toBe('string');
    }
  });

  it('includes known templates', () => {
    const names = listTemplates().map((t) => t.name);
    expect(names).toContain('invoice');
    expect(names).toContain('receipt');
  });
});

describe('getSchema', () => {
  it('returns schema for a known template', () => {
    const result = getSchema('invoice');
    expect(result.name).toBe('invoice');
    expect(result).toHaveProperty('description');
    expect(result).toHaveProperty('schema');
    expect(result).toHaveProperty('example');
    expect(result.schema).toHaveProperty('type', 'object');
  });

  it('throws for unknown template', () => {
    expect(() => getSchema('nonexistent')).toThrow('not found');
  });
});
