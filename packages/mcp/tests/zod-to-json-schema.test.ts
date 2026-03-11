import { describe, it, expect } from 'vitest';
import { z } from 'zod';
import { zodToJsonSchema } from '../src/zod-to-json-schema.js';

describe('zodToJsonSchema', () => {
  it('converts ZodString', () => {
    expect(zodToJsonSchema(z.string())).toEqual({ type: 'string' });
  });

  it('converts ZodNumber', () => {
    expect(zodToJsonSchema(z.number())).toEqual({ type: 'number' });
  });

  it('converts ZodBoolean', () => {
    expect(zodToJsonSchema(z.boolean())).toEqual({ type: 'boolean' });
  });

  it('converts ZodArray', () => {
    expect(zodToJsonSchema(z.array(z.string()))).toEqual({
      type: 'array',
      items: { type: 'string' },
    });
  });

  it('converts ZodObject with required fields', () => {
    const schema = z.object({ name: z.string(), age: z.number() });
    const result = zodToJsonSchema(schema);

    expect(result).toEqual({
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
    const result = zodToJsonSchema(schema);

    expect(result).toEqual({
      type: 'object',
      properties: {
        name: { type: 'string' },
        nickname: { type: 'string' },
      },
      required: ['name'],
    });
  });

  it('converts ZodRecord', () => {
    const schema = z.record(z.number());
    expect(zodToJsonSchema(schema)).toEqual({
      type: 'object',
      additionalProperties: { type: 'number' },
    });
  });

  it('preserves descriptions', () => {
    const schema = z.string().describe('A name');
    expect(zodToJsonSchema(schema)).toEqual({
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
    const result = zodToJsonSchema(schema);

    expect(result).toEqual({
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

  it('converts ZodAny to empty object', () => {
    expect(zodToJsonSchema(z.any())).toEqual({});
  });

  it('converts ZodUnknown to empty object', () => {
    expect(zodToJsonSchema(z.unknown())).toEqual({});
  });

  it('converts ZodEnum', () => {
    expect(zodToJsonSchema(z.enum(['a', 'b', 'c']))).toEqual({
      type: 'string',
      enum: ['a', 'b', 'c'],
    });
  });

  it('converts ZodUnion', () => {
    const schema = z.union([z.string(), z.number()]);
    expect(zodToJsonSchema(schema)).toEqual({
      anyOf: [{ type: 'string' }, { type: 'number' }],
    });
  });

  it('converts ZodDefault', () => {
    const schema = z.string().default('hello');
    expect(zodToJsonSchema(schema)).toEqual({
      type: 'string',
      default: 'hello',
    });
  });

  it('treats ZodDefault fields as optional in objects', () => {
    const schema = z.object({
      name: z.string(),
      color: z.string().default('blue'),
    });
    const result = zodToJsonSchema(schema);
    expect(result.required).toEqual(['name']);
  });

  it('converts ZodLiteral', () => {
    expect(zodToJsonSchema(z.literal('foo'))).toEqual({ const: 'foo' });
    expect(zodToJsonSchema(z.literal(42))).toEqual({ const: 42 });
  });

  it('includes string constraints', () => {
    const schema = z.string().min(3).max(10);
    expect(zodToJsonSchema(schema)).toEqual({
      type: 'string',
      minLength: 3,
      maxLength: 10,
    });
  });

  it('includes number constraints', () => {
    const schema = z.number().min(0).max(100).int();
    expect(zodToJsonSchema(schema)).toEqual({
      type: 'integer',
      minimum: 0,
      maximum: 100,
    });
  });

  it('includes string regex pattern', () => {
    const schema = z.string().regex(/^[A-Z]+$/);
    const result = zodToJsonSchema(schema);
    expect(result.pattern).toBe('^[A-Z]+$');
  });
});
