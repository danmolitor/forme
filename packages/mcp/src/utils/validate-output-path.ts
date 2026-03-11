import { resolve, sep } from 'node:path';

/// Validates that the output path stays within the current working directory.
/// Throws if the resolved path escapes cwd (e.g. "../escape.pdf", "/tmp/evil.pdf").
export function validateOutputPath(requestedPath: string): string {
  const cwd = process.cwd();
  const resolved = resolve(requestedPath);
  if (resolved !== cwd && !resolved.startsWith(cwd + sep)) {
    throw new Error(
      `Output path "${requestedPath}" resolves outside the working directory. ` +
      `Resolved: ${resolved}, cwd: ${cwd}`
    );
  }
  return resolved;
}
