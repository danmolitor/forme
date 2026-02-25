import { templates } from '../templates/index.js';

export function listTemplates() {
  return Object.entries(templates).map(([name, entry]) => ({
    name,
    description: entry.description,
    fields: entry.fields,
  }));
}
