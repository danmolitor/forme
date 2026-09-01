/**
 * Named errors for missing workspace dependencies.
 *
 * Every input path resolves its framework packages from the *user's*
 * workspace (JSX externalizes react/@formepdf/*; the SFC paths load the
 * compiler and runtime from the user's node_modules). That moves one failure
 * mode onto the user: a project with Forme imports but no framework installed.
 * When that happens the panel must show a named, actionable message —
 * "vue is not installed in this workspace. Run `npm install vue`…" — not a
 * raw createRequire/ESM-loader stack trace. This maps the resolution failure
 * to that message; any other error passes through untouched.
 */

const MISSING_RE = /Cannot find (?:package|module) '([^']+)'/;

/// Extract the bare package name from a specifier: `svelte/compiler` → svelte,
/// `@vue/compiler-sfc` → @vue/compiler-sfc, `@formepdf/svelte` → @formepdf/svelte.
function packageOf(spec: string): string {
  if (spec.startsWith('@')) {
    const parts = spec.split('/');
    return parts.slice(0, 2).join('/');
  }
  return spec.split('/')[0];
}

/// If `err` is a module-not-found failure, return a named error naming the
/// missing package and the file type it blocks. Otherwise return the original
/// error (coerced to an Error). `ext` is the input extension for the message,
/// e.g. `.vue`.
export function friendlyDependencyError(err: unknown, ext: string): Error {
  const e = err as NodeJS.ErrnoException & { message?: string };
  const message = e?.message ?? String(err);
  const isMissing =
    e?.code === 'ERR_MODULE_NOT_FOUND' ||
    e?.code === 'MODULE_NOT_FOUND' ||
    MISSING_RE.test(message);

  if (isMissing) {
    const match = message.match(MISSING_RE);
    if (match) {
      const pkg = packageOf(match[1]);
      return new Error(
        `"${pkg}" is not installed in this workspace. ` +
          `Run \`npm install ${pkg}\` to preview ${ext} files.`,
      );
    }
  }

  return err instanceof Error ? err : new Error(message);
}
