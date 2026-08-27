#!/usr/bin/env bash
set -euo pipefail

if [ $# -ne 1 ]; then
  echo "Usage: ./scripts/bump-version.sh <version>"
  echo "Example: ./scripts/bump-version.sh 0.8.0"
  exit 1
fi

VERSION="$1"

# Validate semver format
if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$ ]]; then
  echo "Error: '$VERSION' is not a valid semver version"
  exit 1
fi

ROOT="$(cd "$(dirname "$0")/.." && pwd)"

echo "Bumping all packages to $VERSION"
echo ""

# ── Rust engine ──────────────────────────────────────────────
echo "  engine/Cargo.toml"
sed -i '' -E "1,/^version = \"[^\"]+\"/s/^version = \"[^\"]+\"/version = \"$VERSION\"/" "$ROOT/engine/Cargo.toml"

# Regenerate Cargo.lock
(cd "$ROOT/engine" && cargo check --quiet 2>/dev/null)

# ── Python SDK (same version line as the engine and npm packages) ─
echo "  packages/python-sdk/pyproject.toml"
sed -i '' -E "1,/^version = \"[^\"]+\"/s/^version = \"[^\"]+\"/version = \"$VERSION\"/" \
  "$ROOT/packages/python-sdk/pyproject.toml"

# ── NPM packages (version + interdependencies) ──────────────
# Every workspace that publishes on the shared version line. Keep this in sync
# with the checklist in RELEASE.md — a package missing here is silently left on
# the old version, which is how `shared`, `svelte`, `preact`, and `templates`
# rode through two releases unbumped.
NPM_PACKAGES=(shared react core renderer svelte preact cli hono next resend mcp sdk tailwind templates)
for pkg in "${NPM_PACKAGES[@]}"; do
  pkgfile="$ROOT/packages/$pkg/package.json"
  [ -f "$pkgfile" ] || continue

  echo "  packages/$pkg/package.json"
  node -e "
    const fs = require('fs');
    const p = JSON.parse(fs.readFileSync(process.argv[1], 'utf8'));
    p.version = process.argv[2];
    const internalPrefixes = ['@formepdf/'];
    // peerDependencies included deliberately. A caret range on a 0.x version
    // does NOT admit the next minor — npm reads '^0.12.1' as '>=0.12.1 <0.13.0'
    // — so leaving @formepdf/svelte's optional peer on core at the old range
    // hands every consumer a peer conflict the moment we ship a minor.
    for (const section of ['dependencies', 'devDependencies', 'peerDependencies', 'optionalDependencies']) {
      if (!p[section]) continue;
      for (const [dep, ver] of Object.entries(p[section])) {
        if (!internalPrefixes.some(pfx => dep.startsWith(pfx))) continue;
        // Preserve the range operator; only the version moves.
        p[section][dep] = ver.startsWith('^') ? '^' + process.argv[2] : process.argv[2];
      }
    }
    fs.writeFileSync(process.argv[1], JSON.stringify(p, null, 2) + '\n');
  " "$pkgfile" "$VERSION"
done

# ── VS Code extension (dep only, version managed separately) ─
VSCODE_PKG="$ROOT/packages/vscode/package.json"
if [ -f "$VSCODE_PKG" ]; then
  node -e "
    const fs = require('fs');
    const raw = fs.readFileSync(process.argv[1], 'utf8');
    const updated = raw.replace(/\"@formepdf\/renderer\": \"[^\"]+\"/, '\"@formepdf/renderer\": \"' + process.argv[2] + '\"');
    if (updated !== raw) {
      fs.writeFileSync(process.argv[1], updated);
      console.log('  packages/vscode/package.json: updated renderer dep');
    }
  " "$VSCODE_PKG" "$VERSION"
fi

echo ""
echo "Done. All packages at $VERSION (vscode version unchanged, dep updated)"
echo ""
echo "Not covered by this script — check by hand:"
echo "  - server/Cargo.toml, rasterizer/Cargo.toml (own version line)"
echo "  - forme-go (separate repo, versioned by git tag)"
echo ""
echo "Next steps:"
echo "  1. Update changelogs"
echo "  2. git add -A && git commit -m 'Bump all packages to $VERSION'"
echo "  3. git tag v$VERSION"
