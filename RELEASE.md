# Release Process

## Version Strategy

- Engine (Cargo) + all npm packages share the same version (e.g. 0.9.0)
- Python SDK (`formepdf` on PyPI) follows the same version
- Rust crate (`forme-pdf` on crates.io) follows the same version
- Go SDK (`github.com/formepdf/forme-go`) uses a `v0.9.0` git tag
- VS Code extension follows the same version as of 0.13.0. It publishes to the Marketplace rather than npm, which is why it used to version itself — but it bundles the engine WASM and `@formepdf/renderer` wholesale, so a number that had drifted three minors behind (0.10.5 against a 0.12.1 monorepo) said nothing about what was in the VSIX. It jumped 0.10.5 → 0.13.0; 0.11.x and 0.12.x have no extension release.
- **`server/` and `rasterizer/` are frozen at 0.10.5 and no longer track the release.** They exist to build the two Docker images (`formepdf/forme`, `formepdf/rasterizer`), which served the hosted API — that product line is shut down. 0.10.5 is the last version that actually shipped as an image, and leaving the crates there keeps the number honest rather than inventing versions nobody can pull.

  Do not bump them "for consistency". `server/Dockerfile:3` is `FROM formepdf/rasterizer:0.10.5`, which resolves against a tag that really exists on Docker Hub; moving the crate version without publishing a matching image either strands a phantom version or — if the pin follows — breaks the server build outright. Skip the entire Docker section below unless you are deliberately reviving the hosted API.

- Docker image (`formepdf/forme`) follows `server/`'s version — tagged as `{version}` and `latest`. Not published since 0.10.5; see above.
- Rasterizer Docker image (`formepdf/rasterizer`) follows `rasterizer/`'s version. Not published since 0.10.5; see above.

---

## Build Order

Build order matters. Later packages depend on earlier ones.

```bash
# 1. Engine (Rust) — only if engine/ changed
cd engine
cargo fmt
cargo clippy -- -W clippy::all
cargo test

# 2. Shared (framework-neutral serialize core — no runtime deps)
cd packages/shared
npm run build

# 3. React (JSX components, serialize, types — depends on shared)
cd packages/react
npm run build
npm test

# 4. Core (WASM bridge — compiles engine to WebAssembly)
cd packages/core
npm run build    # runs wasm-pack + tsc

# 5. Renderer (shared render pipeline — depends on react + core)
cd packages/renderer
npm run build
npm test

# 6. Svelte adapter (SSR-then-parse — depends on shared, optional peer on core)
cd packages/svelte
npm run build    # runs embed-preview + svelte-package
npm run check    # svelte-check typecheck
npm test         # requires core built above

# 6b. Preact adapter (fork of react's serializer w/ Preact VNode APIs — depends on shared, peer on preact ^10)
cd packages/preact
npm run build
npm test         # parity tests require @formepdf/react also built

# 7. CLI (dev server + build command — depends on renderer)
cd packages/cli
npm run build

# 8. VS Code extension (depends on renderer)
cd packages/vscode
npm run build    # esbuild bundle + copies WASM + preview HTML

# 9. Integration and utility packages (depend on react + core)
cd packages/hono && npm run build
cd packages/next && npm run build
cd packages/mcp && npm run build
cd packages/resend && npm run build

# 8. Packages with no build step (verify they resolve correctly)
cd packages/sdk && npm run build       # TypeScript hosted API client
cd packages/tailwind && npm run build  # tw() function, Tailwind v3
cd packages/templates && npm run build # shared templates + Zod schemas

# 9. Python SDK — rebuild WASM (only if engine/ changed)
cd packages/python-sdk
bash build_wasm.sh   # builds wasm32-wasip1 target, copies to formepdf/forme.wasm

# 10. Go SDK — rebuild WASM (only if engine/ changed)
# The Go SDK is a separate git repo at packages/go-sdk/
# It uses //go:embed for the WASM binary (gitignored, must be present locally)
cd packages/go-sdk
bash templates/build_wasm.sh   # or copy from engine target:
# cp ../../engine/target/wasm32-wasip1/release/forme.wasm templates/forme.wasm
```

---

## Version Bump Checklist

Files to update when bumping (e.g. 0.8.3 -> 0.9.0):

### npm packages
- [ ] `packages/shared/package.json`
- [ ] `packages/react/package.json`
- [ ] `packages/core/package.json`
- [ ] `packages/renderer/package.json`
- [ ] `packages/svelte/package.json`
- [ ] `packages/preact/package.json`
- [ ] `packages/cli/package.json`
- [ ] `packages/hono/package.json`
- [ ] `packages/next/package.json`
- [ ] `packages/resend/package.json`
- [ ] `packages/mcp/package.json`
- [ ] `packages/sdk/package.json`
- [ ] `packages/tailwind/package.json`
- [ ] `packages/templates/package.json`
- [ ] `packages/vscode/package.json` — on the shared version line since 0.13.0; `scripts/bump-version.sh` handles it

### Non-npm packages
- [ ] `engine/Cargo.toml` — `version = "0.9.0"`
- [ ] `packages/python-sdk/pyproject.toml` — `version = "0.9.0"` (handled by `scripts/bump-version.sh`)
- [ ] ~~`server/Cargo.toml`~~ — frozen at 0.10.5, do not bump (see Version Strategy)
- [ ] ~~`rasterizer/Cargo.toml`~~ — frozen at 0.10.5, do not bump (see Version Strategy)
- [ ] Go SDK `packages/go-sdk/` — no version file; versioned by git tag
- [ ] `engine/Cargo.lock` — auto-regenerates on the next `cargo build` after a `Cargo.toml` bump. Stage and commit the resulting diff with the version bump; CI will fail if `Cargo.lock` is stale.

### Dockerfile rasterizer pins

> **Not part of a normal release.** `server/` and `rasterizer/` are frozen at 0.10.5 and their images are no longer published, so both pins below correctly point at the last real tag. Leave them alone. The rest of this section applies only if the hosted API is revived.

After bumping the engine/server/rasterizer versions, **two Dockerfiles** still reference the old rasterizer tag and must be bumped to the new version:
- [ ] `server/Dockerfile` — `FROM formepdf/rasterizer:{version}` (line 3). Required: bump *after* the new rasterizer image is published to Docker Hub, *before* building the server image (server pulls rasterizer at build time).
- [ ] `forme-dashboard/packages/api/Dockerfile` — `FROM formepdf/rasterizer:{version}`. Different repo; commit + push separately so Railway's next deploy picks up the new tag.

### SDK WASM binaries (if engine/ changed)
- [ ] `packages/python-sdk/formepdf/forme.wasm` — rebuild via `bash build_wasm.sh`
- [ ] `forme-go/templates/forme.wasm` (separate `forme-go` git repo) — rebuild via `bash templates/build_wasm.sh` OR copy the artifact built by the python-sdk script (same target + flags)
- Both use the `wasm32-wasip1` target with `--features wasm-raw` (C-ABI exports for non-JS hosts)
- The Python SDK WASM is gitignored — use `git add -f` to commit it
- The Go SDK WASM is **tracked** in the forme-go repo (not gitignored) — `git add` it normally

> **Known bug**: `forme-go/templates/build_wasm.sh` computes `REPO_ROOT="$SCRIPT_DIR/../../.."` which assumed the old in-monorepo layout. After the Go SDK split into `forme-go/`, this resolves to `/path/to/forme/engine` instead of `/path/to/forme/forme/engine`. Until the script is fixed (point it at the `forme/` sibling, or accept an `ENGINE_DIR` env override), the workaround is: build the python-sdk WASM first (`cd packages/python-sdk && bash build_wasm.sh`), then `cp packages/python-sdk/formepdf/forme.wasm ../forme-go/templates/forme.wasm`. The python-sdk build script uses the same `wasm32-wasip1 + --features wasm-raw` target as the Go SDK, so the artifact is interchangeable.

### Cross-package dependency references
Update peer/runtime dependencies that pin to the formepdf packages:
- [ ] `packages/react/package.json` — `@formepdf/shared`
- [ ] `packages/core/package.json` — `@formepdf/react`
- [ ] `packages/renderer/package.json` — `@formepdf/core`, `@formepdf/react`
- [ ] `packages/svelte/package.json` — `@formepdf/shared` (dep), `@formepdf/core` (optional peer, `^` range)
- [ ] `packages/preact/package.json` — `@formepdf/shared` (dep), `@formepdf/react` (devDep for parity tests)
- [ ] `packages/cli/package.json` — `@formepdf/renderer`
- [ ] `packages/vscode/package.json` — `@formepdf/renderer`
- [ ] `packages/hono/package.json` — `@formepdf/react`, `@formepdf/core`
- [ ] `packages/next/package.json` — `@formepdf/react`, `@formepdf/core`
- [ ] `packages/resend/package.json` — `@formepdf/react`, `@formepdf/core`
- [ ] `packages/mcp/package.json` — `@formepdf/react`, `@formepdf/core`
- [ ] `packages/sdk/package.json` — `@formepdf/react`, `@formepdf/core` if referenced
- [ ] `packages/tailwind/package.json` — `@formepdf/react` if referenced
- [ ] `packages/templates/package.json` — `@formepdf/react`, `@formepdf/core`

### Changelogs
- [ ] `engine/CHANGELOG.md`
- [ ] `server/CHANGELOG.md`
- [ ] `packages/shared/CHANGELOG.md`
- [ ] `packages/react/CHANGELOG.md`
- [ ] `packages/core/CHANGELOG.md`
- [ ] `packages/renderer/CHANGELOG.md`
- [ ] `packages/svelte/CHANGELOG.md`
- [ ] `packages/preact/CHANGELOG.md`
- [ ] `packages/cli/CHANGELOG.md`
- [ ] `packages/hono/CHANGELOG.md`
- [ ] `packages/next/CHANGELOG.md`
- [ ] `packages/resend/CHANGELOG.md`
- [ ] `packages/mcp/CHANGELOG.md`
- [ ] `packages/sdk/CHANGELOG.md`
- [ ] `packages/tailwind/CHANGELOG.md`
- [ ] `packages/templates/CHANGELOG.md`
- [ ] `packages/vscode/CHANGELOG.md`

### READMEs (if new components, APIs, or capabilities were added)
- [ ] `README.md` (root) — features list, component table
- [ ] `packages/react/README.md` — component list, usage examples
- [ ] `packages/svelte/README.md` — component list, usage examples (Svelte adapter)
- [ ] `packages/preact/README.md` — usage + JSX runtime notes (Preact adapter)
- [ ] `packages/core/README.md` — API surface, render functions
- [ ] `packages/cli/README.md` — CLI commands, flags

### Docs (if user-facing behavior changed)
- [ ] `docs/` — Mintlify docs at docs.formepdf.com. Update pages affected by the release (e.g. `components.mdx`, `charts.mdx`, `styles.mdx`, `svg.mdx`). New props, changed defaults, and new components all need doc updates.

### Lockfile
- [ ] Run `npm install` from root to update `package-lock.json`

---

## Publish

### Pre-Publish Gate

**Do not start the publish steps below until every item here passes.** The npm/PyPI/crates.io registries are immutable per version — if you ship broken code, the only fix is bumping the version and republishing.

```bash
# 1. Working tree must be clean (no dangling uncommitted changes)
cd forme && git status --short    # should be empty
cd forme-go && git status --short # should be empty (the Go SDK is its own repo)

# 2. Engine + all packages must build cleanly
cd forme/engine && cargo build --release && cargo fmt --check && cargo clippy -- -D warnings
cd forme/packages/shared && npm run build    # must build first — react depends on it
cd forme/packages/react && npm run build
cd forme/packages/core && npm run build      # rebuilds WASM (pkg/ + pkg-node/)
cd forme/packages/renderer && npm run build
cd forme/packages/svelte && npm run build    # embed-preview + svelte-package
cd forme/packages/svelte && npm run check    # svelte-check typecheck
cd forme/packages/preact && npm run build
cd forme/packages/cli && npm run build
cd forme/packages/vscode && npm run build    # copies WASM from core
cd forme/packages/hono && npm run build
cd forme/packages/next && npm run build
cd forme/packages/mcp && npm run build
cd forme/packages/resend && npm run build
cd forme/packages/sdk && npm run build
cd forme/packages/tailwind && npm run build
cd forme/packages/templates && npm run build

# 3. Every test suite must pass
cd forme/engine && cargo test
cd forme/packages/react && npm test
cd forme/packages/core && npm test
cd forme/packages/renderer && npm test
cd forme/packages/svelte && npm test          # requires core built above
cd forme/packages/preact && npm test          # parity tests require @formepdf/react built
cd forme/packages/cli && npm test
cd forme/packages/hono && npm test
cd forme/packages/next && npm test
cd forme/packages/resend && npm test
cd forme/packages/mcp && npm test
cd forme-go && go clean -testcache && go test ./...

# 4. Lockfile must be regenerated after version bumps
cd forme && npm install
cd forme && git diff --stat package-lock.json   # should show the @formepdf/* deps moved to the new version
```

### Supply-chain audit (pre-publish)

Active npm supply-chain campaigns (mini Shai-Hulud and successors) periodically poison maintainer accounts and re-publish trojaned versions of widely-used packages. Before any publish:

```bash
# 1. Pull the current IoC list — check the dated advisories on:
#    - https://socket.dev/supply-chain-attacks/
#    - https://snyk.io/advisor/ (search "shai-hulud")
#    - Cloud-vendor advisories if you saw a public incident
#
# 2. Audit our lockfiles for any matching package@version pairs
cd forme && grep -cE '"@<affected-ns>/|"<affected-pkg>"' package-lock.json packages/*/package-lock.json
cd forme-dashboard && grep -cE '"@<affected-ns>/|"<affected-pkg>"' packages/*/package-lock.json
#
# 3. If any matches: STOP. Do not publish. Rotate npm tokens, GitHub tokens, and any
#    cloud credentials that were resident on the publishing machine. Coordinate with
#    upstream maintainers and the security tracker before any further action.
```

A clean audit is not a guarantee — it only proves the *lockfile* doesn't reference known-bad versions. A workstation that ran `npm install` against an unpinned poisoned dep in the window between publish and detection is independently compromised. If in doubt, run the publish from a fresh CI runner or container, not a developer machine.

### npm packages

```bash
cd packages/shared && npm publish --access public   # must publish first — react depends on it
cd packages/react && npm publish --access public
cd packages/core && npm publish --access public
cd packages/renderer && npm publish --access public
cd packages/svelte && npm publish --access public
cd packages/preact && npm publish --access public
cd packages/cli && npm publish --access public
cd packages/hono && npm publish --access public
cd packages/next && npm publish --access public
cd packages/resend && npm publish --access public
cd packages/mcp && npm publish --access public
cd packages/sdk && npm publish --access public
cd packages/tailwind && npm publish --access public
cd packages/templates && npm publish --access public
```

### VS Code extension

```bash
cd packages/vscode
npm run package    # creates forme-pdf-{version}.vsix
npx @vscode/vsce publish
```

### PyPI

```bash
cd packages/python-sdk
python -m build
twine upload dist/*
# Requires PyPI API token in ~/.pypirc or TWINE_PASSWORD env var
# Package name: formepdf
# Verify at: https://pypi.org/project/formepdf/
```

### crates.io

```bash
cd engine
cargo publish
# Requires `cargo login` with crates.io token (one-time)
# Crate name: forme-pdf
# Verify at: https://crates.io/crates/forme-pdf
# Note: cargo publish does a dry run check first; add --dry-run to verify before publishing
```

### Docker images

The two images **must be published in order**: rasterizer first, then server. The server's Dockerfile (`server/Dockerfile:3`) starts with `FROM formepdf/rasterizer:{version}` and pulls the sidecar binary at build time — building server first will either pull the OLD rasterizer tag (silent regression) or fail outright if the new tag isn't on Docker Hub yet.

#### Step 1 — Rasterizer image

Build and push from the monorepo root:

```bash
# One-time builder setup (if not already done)
docker buildx create --name multiplatform --use
docker buildx inspect --bootstrap

docker login

docker buildx build \
  --platform linux/amd64,linux/arm64 \
  --provenance=true \
  --sbom=true \
  -f rasterizer/Dockerfile \
  -t formepdf/rasterizer:{version} \
  -t formepdf/rasterizer:latest \
  --push \
  .

# Verify
docker run --rm -p 3001:3001 formepdf/rasterizer:{version}
curl http://localhost:3001/health
```

#### Step 2 — Bump both Dockerfile pins

After the rasterizer image is on Docker Hub, update the `FROM formepdf/rasterizer:...` line in **both** Dockerfiles to the new version, then commit:

```bash
# forme repo — server image
sed -i '' "s|FROM formepdf/rasterizer:[0-9.]\+|FROM formepdf/rasterizer:{version}|" server/Dockerfile

# forme-dashboard repo — API image (separate repo, separate commit)
cd ../forme-dashboard
sed -i '' "s|FROM formepdf/rasterizer:[0-9.]\+|FROM formepdf/rasterizer:{version}|" packages/api/Dockerfile
```

Don't bump these before publishing the rasterizer image — between the pin update and the actual publish, any Railway deploy or fresh `docker build` will fail to pull the (not-yet-existing) tag.

#### Step 3 — Server image

Build and push, also from the monorepo root:

```bash
docker buildx build \
  --platform linux/amd64,linux/arm64 \
  --provenance=true \
  --sbom=true \
  -f server/Dockerfile \
  -t formepdf/forme:{version} \
  -t formepdf/forme:latest \
  --push \
  .

# Verify
docker run --rm -p 3000:3000 formepdf/forme:{version}
curl http://localhost:3000/health
```

Note: The Dockerfile requires Rust 1.88+ due to dependencies. Use `rust:latest` in the Dockerfile if the pinned version is too old.

### Go SDK

The Go SDK is published via git tag — pkg.go.dev indexes it automatically.

```bash
cd packages/go-sdk
# Verify tests pass
go test ./...

# Push to the Go SDK repo
git add .
git commit -m "Release v0.9.0"
git push origin main

# Tag the release (Go modules use the tag as the version)
git tag v0.9.0
git push origin v0.9.0

# pkg.go.dev will index it automatically within ~30 minutes
# Verify at: https://pkg.go.dev/github.com/formepdf/forme-go
```

---

## Git Tag (monorepo)

```bash
git tag v0.9.0
git push origin main
git push origin v0.9.0
```

---

## Post-Publish Verification

Verify the published packages actually work before announcing:

```bash
# npm — fresh install test
mkdir /tmp/test-forme-090 && cd /tmp/test-forme-090
npm init -y
npm install @formepdf/react @formepdf/core @formepdf/cli
# Run a minimal render

# Docker image
docker run --rm -p 3000:3000 formepdf/forme:{version}
curl http://localhost:3000/health
# Should return {"status":"ok","version":"{version}"}

# PyPI
pip install formepdf==0.9.0
python -c "import formepdf; print(formepdf.__version__)"

# crates.io
cargo add forme-pdf@0.9.0
# or check https://crates.io/crates/forme-pdf

# Go
go get github.com/formepdf/forme-go@v0.9.0
# Check https://pkg.go.dev/github.com/formepdf/forme-go@v0.9.0
```

---

## Common Mistakes

- **Stale WASM**: If `engine/` changed, must rebuild `packages/core` (`npm run build`) before anything else. The WASM binary is ~5.1MB (grew from 4.8MB in 0.7.x when signatures were added). **Also rebuild** the Python SDK (`packages/python-sdk/build_wasm.sh`) and Go SDK (`packages/go-sdk/templates/build_wasm.sh`) WASM binaries — these are separate wasm32-wasip1 builds, not the wasm-pack JS build.
- **SDK WASM is gitignored**: Both `packages/python-sdk/formepdf/forme.wasm` and `packages/go-sdk/templates/forme.wasm` are in `.gitignore`. Use `git add -f` to stage them. The Go SDK is a separate git repo — commit and tag there independently.
- **Stale dist/**: Always rebuild `packages/renderer` before VS Code or CLI. A stale `dist/` can silently ship broken code.
- **VS Code copies**: The VS Code esbuild config copies WASM from `packages/core/pkg/` and preview HTML from `packages/renderer/dist/preview/`. These are snapshots — rebuild VS Code after rebuilding core or renderer.
- **Lockfile**: Run `npm install` from root after version bumps to update `package-lock.json`.
- **npm cache**: Can't republish the same version. If you published broken code, bump the version.
- **crates.io is permanent**: Same rule — can't yank and republish the same version. Use `--dry-run` first.
- **Go tag must be on the right repo**: The Go SDK is at `github.com/formepdf/forme-go`, not the monorepo. Tag there, not in the monorepo.
- **Docker Rust version**: The server Dockerfile requires Rust 1.88+ due to dependencies. If the pinned version errors, check the current minimum required version and update `FROM rust:X.XX-bookworm` accordingly. Using `rust:latest` is a safe fallback.
- **Docker buildx context**: Run the buildx build from the monorepo root (`forme/`), not from `server/`. The Dockerfile copies both `engine/` and `server/` directories.
- **PyPI stale dist/**: `twine upload dist/*` uploads everything in `dist/`, including old versions. Clean old builds first (`rm dist/formepdf-0.*.whl dist/formepdf-0.*.tar.gz`) or upload only the target version (`twine upload dist/formepdf-{version}*`).
- **PyPI token scope**: Make sure the PyPI token has upload rights for the `formepdf` project specifically (project-scoped token), not just your account.
- **Rasterizer version pins (TWO Dockerfiles)**: After publishing a new `formepdf/rasterizer` tag, update the `FROM formepdf/rasterizer:{version}` line in BOTH `server/Dockerfile` (this repo) AND `forme-dashboard/packages/api/Dockerfile` (separate repo). Missing the server one means the published `formepdf/forme:{version}` image will silently bundle the old rasterizer binary.
- **Docker publish order**: Always publish rasterizer BEFORE server. The server Dockerfile pulls the rasterizer image at build time — if you publish server first, you'll either bundle the old rasterizer or fail the build outright.
- **forme-go build script is broken**: `forme-go/templates/build_wasm.sh` has a stale `REPO_ROOT="$SCRIPT_DIR/../../.."` that assumes the old in-monorepo layout. After the Go SDK was split into its own repo, this path no longer resolves to the engine. Workaround: build the python-sdk WASM (`cd packages/python-sdk && bash build_wasm.sh`), then `cp packages/python-sdk/formepdf/forme.wasm ../forme-go/templates/forme.wasm`. Same target + flags, interchangeable artifact. A real fix is small — point the script at `../../forme/engine` or accept an `ENGINE_DIR` env override.
