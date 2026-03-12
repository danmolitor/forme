import { describe, it, expect } from 'vitest';
import { validateOutputPath } from '../src/utils/validate-output-path.js';

describe('validateOutputPath', () => {
  it('resolves relative paths', () => {
    const result = validateOutputPath('./ok.pdf');
    expect(result).toContain('ok.pdf');
  });

  it('resolves absolute paths as-is', () => {
    const result = validateOutputPath('/tmp/output.pdf');
    expect(result).toBe('/tmp/output.pdf');
  });

  it('resolves subdirectory paths', () => {
    const result = validateOutputPath('subdir/ok.pdf');
    expect(result).toContain('subdir/ok.pdf');
  });
});
