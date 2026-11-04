import { z } from 'zod';

/// Converts a Zod schema to a JSON Schema object (draft 2020-12), with
/// Zod's own converter. A hand-written walk over Zod's internals used to do
/// this; Zod 4 reorganized those internals, and the walk would have
/// returned empty schemas without an error.
export function zodToJsonSchema(schema: z.ZodType): Record<string, unknown> {
  return z.toJSONSchema(schema, { io: 'input' }) as Record<string, unknown>;
}
