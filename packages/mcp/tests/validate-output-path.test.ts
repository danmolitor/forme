import { describe, it, expect } from 'vitest';
import { validateOutputPath } from '../src/utils/validate-output-path.js';

describe('validateOutputPath', () => {
  it('allows relative paths within cwd', () => {
    const result = validateOutputPath('./ok.pdf');
    expect(result).toContain('ok.pdf');
  });

  it('allows subdirectory paths within cwd', () => {
    const result = validateOutputPath('subdir/ok.pdf');
    expect(result).toContain('subdir/ok.pdf');
  });

  it('rejects paths that escape cwd with ../', () => {
    expect(() => validateOutputPath('../escape.pdf')).toThrow('outside the working directory');
  });

  it('rejects absolute paths outside cwd', () => {
    expect(() => validateOutputPath('/tmp/evil.pdf')).toThrow('outside the working directory');
  });
});
