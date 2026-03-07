# Release Process

## Version Strategy

- Engine (Cargo) + all npm packages share the same version (e.g. 0.7.3)
- VS Code extension has its own version (e.g. 0.7.5) since it publishes to the Marketplace independently

## Build Order

Build order matters. Later packages depend on earlier ones.

```bash
# 1. Engine (Rust) - only if engine/ changed
cd engine
cargo fmt
cargo clippy -- -W clippy::all
cargo test

# 2. React (JSX components, serialize, types)
cd packages/react
npm run build
npm test

# 3. Core (WASM bridge - compiles engine to WebAssembly)
cd packages/core
npm run build    # runs wasm-pack + tsc

# 4. Renderer (shared render pipeline - depends on react + core)
cd packages/renderer
npm run build    # tsc + copy preview HTML
npm test

# 5. CLI (dev server + build command - depends on renderer)
cd packages/cli
npm run build

# 6. VS Code extension (depends on renderer)
cd packages/vscode
npm run build    # esbuild bundle + copy WASM + preview HTML

# 7. Integration packages (depend on react + core, no build step usually)
cd packages/hono && npm run build
cd packages/next && npm run build
cd packages/mcp && npm run build
cd packages/resend && npm run build
```

## Quick Build (all packages)

```bash
cd /path/to/forme

# Engine
(cd engine && cargo fmt && cargo clippy -- -W clippy::all && cargo test)

# Packages in order
(cd packages/react && npm run build && npm test)
(cd packages/core && npm run build)
(cd packages/renderer && npm run build && npm test)
(cd packages/cli && npm run build)
(cd packages/vscode && npm run build)
```

## Version Bump Checklist

Files to update when bumping (e.g. 0.7.2 -> 0.7.3):

### Package versions
- [ ] `engine/Cargo.toml` - `version = "0.7.3"`
- [ ] `packages/react/package.json` - `"version": "0.7.3"`
- [ ] `packages/core/package.json` - `"version": "0.7.3"`
- [ ] `packages/renderer/package.json` - `"version": "0.7.3"`
- [ ] `packages/cli/package.json` - `"version": "0.7.3"`
- [ ] `packages/hono/package.json` - `"version": "0.7.3"`
- [ ] `packages/next/package.json` - `"version": "0.7.3"`
- [ ] `packages/resend/package.json` - `"version": "0.7.3"`
- [ ] `packages/mcp/package.json` - `"version": "0.7.3"`
- [ ] `packages/vscode/package.json` - separate version (e.g. `"0.7.5"`)

### Cross-package dependency references
- [ ] `packages/core/package.json` - `@formepdf/react`
- [ ] `packages/renderer/package.json` - `@formepdf/core`, `@formepdf/react`
- [ ] `packages/cli/package.json` - `@formepdf/renderer`
- [ ] `packages/vscode/package.json` - `@formepdf/renderer`
- [ ] `packages/hono/package.json` - `@formepdf/react`, `@formepdf/core`
- [ ] `packages/next/package.json` - `@formepdf/react`, `@formepdf/core`
- [ ] `packages/resend/package.json` - `@formepdf/react`, `@formepdf/core`
- [ ] `packages/mcp/package.json` - `@formepdf/react`, `@formepdf/core`

### Changelogs
- [ ] `engine/CHANGELOG.md`
- [ ] `packages/react/CHANGELOG.md`
- [ ] `packages/core/CHANGELOG.md`
- [ ] `packages/renderer/CHANGELOG.md`
- [ ] `packages/cli/CHANGELOG.md`
- [ ] `packages/hono/CHANGELOG.md`
- [ ] `packages/next/CHANGELOG.md`
- [ ] `packages/resend/CHANGELOG.md`
- [ ] `packages/mcp/CHANGELOG.md`
- [ ] `packages/vscode/CHANGELOG.md`

### Lockfile
- [ ] Run `npm install` from root to update `package-lock.json`

## Publish

```bash
# npm packages (from each package directory)
cd packages/react && npm publish --access public
cd packages/core && npm publish --access public
cd packages/renderer && npm publish --access public
cd packages/cli && npm publish --access public
cd packages/hono && npm publish --access public
cd packages/next && npm publish --access public
cd packages/resend && npm publish --access public
cd packages/mcp && npm publish --access public

# VS Code extension
cd packages/vscode
npm run package    # creates forme-pdf-{version}.vsix
npx @vscode/vsce publish
# or: code --install-extension forme-pdf-{version}.vsix (local only)
```

## Git Tag

```bash
git tag v0.7.3
git push origin main
git push origin v0.7.3
```

## Common Mistakes

- **Stale WASM**: If engine/ changed, must rebuild `packages/core` (`npm run build`) before anything else. The WASM file is 3.4MB without Noto Sans, ~4.6MB with.
- **Stale dist/**: Always rebuild `packages/renderer` before VS Code or CLI. A stale `dist/` can silently ship broken code (e.g. missing function parameters).
- **Lockfile**: Run `npm install` from root after version bumps to update `package-lock.json`.
- **VS Code copies**: The VS Code esbuild config copies WASM from `packages/core/pkg/` and preview HTML from `packages/renderer/dist/preview/`. These are snapshots - rebuild VS Code after rebuilding core or renderer.
- **npm cache**: Can't republish the same version. If you published broken code, you must bump the version.
