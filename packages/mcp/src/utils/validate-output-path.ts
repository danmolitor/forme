import { resolve } from 'node:path';

/// Resolves the output path to an absolute path.
export function validateOutputPath(requestedPath: string): string {
  return resolve(requestedPath);
}
