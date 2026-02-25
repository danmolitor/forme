import { templates } from '../templates/index.js';
import { zodToJsonSchema } from '../zod-to-json-schema.js';

export function getSchema(templateName: string) {
  const entry = templates[templateName];
  if (!entry) {
    const available = Object.keys(templates).join(', ');
    throw new Error(`Template "${templateName}" not found. Available templates: ${available}`);
  }

  return {
    name: templateName,
    description: entry.description,
    schema: zodToJsonSchema(entry.schema),
    example: entry.example,
  };
}
