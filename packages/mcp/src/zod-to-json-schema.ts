import type { z } from 'zod';

/// Converts a Zod schema to a JSON Schema object.
/// Handles the subset of Zod types used in our template schemas.
export function zodToJsonSchema(schema: z.ZodType): Record<string, unknown> {
  return convert(schema as any);
}

function convert(schema: any): Record<string, unknown> {
  const def = schema._def;
  if (!def) return {};

  const typeName = def.typeName as string;

  switch (typeName) {
    case 'ZodString': {
      const result: Record<string, unknown> = { type: 'string' };
      if (def.description) result.description = def.description;
      if (def.checks) {
        for (const check of def.checks) {
          if (check.kind === 'min') result.minLength = check.value;
          else if (check.kind === 'max') result.maxLength = check.value;
          else if (check.kind === 'regex') result.pattern = check.regex.source;
        }
      }
      return result;
    }

    case 'ZodNumber': {
      const result: Record<string, unknown> = { type: 'number' };
      if (def.description) result.description = def.description;
      if (def.checks) {
        for (const check of def.checks) {
          if (check.kind === 'min') result.minimum = check.value;
          else if (check.kind === 'max') result.maximum = check.value;
          else if (check.kind === 'int') result.type = 'integer';
        }
      }
      return result;
    }

    case 'ZodBoolean': {
      const result: Record<string, unknown> = { type: 'boolean' };
      if (def.description) result.description = def.description;
      return result;
    }

    case 'ZodArray': {
      const result: Record<string, unknown> = {
        type: 'array',
        items: convert(def.type),
      };
      if (def.description) result.description = def.description;
      return result;
    }

    case 'ZodObject': {
      const shape = def.shape();
      const properties: Record<string, unknown> = {};
      const required: string[] = [];

      for (const [key, value] of Object.entries(shape)) {
        properties[key] = convert(value as any);
        // Check if the field is optional
        if (!isOptional(value as any)) {
          required.push(key);
        }
      }

      const result: Record<string, unknown> = {
        type: 'object',
        properties,
      };
      if (required.length > 0) result.required = required;
      if (def.description) result.description = def.description;
      return result;
    }

    case 'ZodOptional': {
      const inner = convert(def.innerType);
      return inner;
    }

    case 'ZodDefault': {
      const inner = convert(def.innerType);
      inner.default = def.defaultValue();
      return inner;
    }

    case 'ZodEnum': {
      const result: Record<string, unknown> = {
        type: 'string',
        enum: [...def.values],
      };
      if (def.description) result.description = def.description;
      return result;
    }

    case 'ZodUnion': {
      const options = (def.options as any[]).map((opt: any) => convert(opt));
      const result: Record<string, unknown> = { anyOf: options };
      if (def.description) result.description = def.description;
      return result;
    }

    case 'ZodLiteral': {
      const result: Record<string, unknown> = { const: def.value };
      if (def.description) result.description = def.description;
      return result;
    }

    case 'ZodRecord': {
      const result: Record<string, unknown> = {
        type: 'object',
        additionalProperties: convert(def.valueType),
      };
      if (def.description) result.description = def.description;
      return result;
    }

    case 'ZodUnknown':
    case 'ZodAny': {
      return {};
    }

    default:
      return {};
  }
}

function isOptional(schema: any): boolean {
  const typeName = schema._def?.typeName as string;
  return typeName === 'ZodOptional' || typeName === 'ZodNullable' || typeName === 'ZodDefault';
}
