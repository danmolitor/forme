import { describe, it, expect } from 'vitest';
import { z } from 'zod';
import { zodToJsonSchema } from '../src/zod-to-json-schema.js';

const DRAFT = 'https://json-schema.org/draft/2020-12/schema';

/// The schema without its top-level `$schema` key.
function body(schema: z.ZodType): Record<string, unknown> {
  const { $schema, ...rest } = zodToJsonSchema(schema);
  expect($schema).toBe(DRAFT);
  return rest;
}

describe('zodToJsonSchema', () => {
  it('declares JSON Schema draft 2020-12', () => {
    expect(zodToJsonSchema(z.string()).$schema).toBe(DRAFT);
  });

  it('converts ZodString', () => {
    expect(body(z.string())).toEqual({ type: 'string' });
  });

  it('converts ZodNumber', () => {
    expect(body(z.number())).toEqual({ type: 'number' });
  });

  it('converts ZodBoolean', () => {
    expect(body(z.boolean())).toEqual({ type: 'boolean' });
  });

  it('converts ZodArray', () => {
    expect(body(z.array(z.string()))).toEqual({
      type: 'array',
      items: { type: 'string' },
    });
  });

  it('converts ZodObject with required fields', () => {
    const schema = z.object({ name: z.string(), age: z.number() });
    expect(body(schema)).toEqual({
      type: 'object',
      properties: {
        name: { type: 'string' },
        age: { type: 'number' },
      },
      required: ['name', 'age'],
    });
  });

  it('handles optional fields', () => {
    const schema = z.object({
      name: z.string(),
      nickname: z.string().optional(),
    });
    expect(body(schema)).toEqual({
      type: 'object',
      properties: {
        name: { type: 'string' },
        nickname: { type: 'string' },
      },
      required: ['name'],
    });
  });

  it('converts ZodRecord', () => {
    const schema = z.record(z.string(), z.number());
    expect(body(schema)).toEqual({
      type: 'object',
      propertyNames: { type: 'string' },
      additionalProperties: { type: 'number' },
    });
  });

  it('preserves descriptions', () => {
    const schema = z.string().describe('A name');
    expect(body(schema)).toEqual({
      type: 'string',
      description: 'A name',
    });
  });

  it('handles nested objects', () => {
    const schema = z.object({
      address: z.object({
        street: z.string(),
        city: z.string(),
      }),
    });
    expect(body(schema)).toEqual({
      type: 'object',
      properties: {
        address: {
          type: 'object',
          properties: {
            street: { type: 'string' },
            city: { type: 'string' },
          },
          required: ['street', 'city'],
        },
      },
      required: ['address'],
    });
  });

  it('converts ZodAny to an unconstrained schema', () => {
    expect(body(z.any())).toEqual({});
  });

  it('converts ZodUnknown to an unconstrained schema', () => {
    expect(body(z.unknown())).toEqual({});
  });

  it('converts ZodEnum', () => {
    expect(body(z.enum(['a', 'b', 'c']))).toEqual({
      type: 'string',
      enum: ['a', 'b', 'c'],
    });
  });

  it('converts ZodUnion', () => {
    const schema = z.union([z.string(), z.number()]);
    expect(body(schema)).toEqual({ type: ['string', 'number'] });
  });

  it('converts ZodDefault', () => {
    const schema = z.string().default('hello');
    expect(body(schema)).toEqual({
      type: 'string',
      default: 'hello',
    });
  });

  it('treats ZodDefault fields as optional in objects', () => {
    // The schema describes what a caller sends (Zod's input type), where a
    // defaulted field may be left out. The output type would require it.
    const schema = z.object({
      name: z.string(),
      color: z.string().default('blue'),
    });
    const result = body(schema);
    expect(result.required).toEqual(['name']);
    expect(result.additionalProperties).toBeUndefined();
  });

  it('converts ZodLiteral', () => {
    expect(body(z.literal('foo'))).toEqual({ type: 'string', const: 'foo' });
    expect(body(z.literal(42))).toEqual({ type: 'number', const: 42 });
  });

  it('includes string constraints', () => {
    const schema = z.string().min(3).max(10);
    expect(body(schema)).toEqual({
      type: 'string',
      minLength: 3,
      maxLength: 10,
    });
  });

  it('includes number constraints', () => {
    const schema = z.number().min(0).max(100).int();
    expect(body(schema)).toEqual({
      type: 'integer',
      minimum: 0,
      maximum: 100,
    });
  });

  it('includes string regex pattern', () => {
    const schema = z.string().regex(/^[A-Z]+$/);
    expect(body(schema).pattern).toBe('^[A-Z]+$');
  });
});
