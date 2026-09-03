#!/usr/bin/env bash
# Master release script — automates RELEASE.md end to end.
#
#   ./scripts/release.sh 0.14.0            # full release (confirm-gated)
#   ./scripts/release.sh 0.14.0 --dry-run  # everything except publish/push/tag
#   ./scripts/release.sh 0.14.0 --from publish   # jump to a phase
#
# Idempotent by design: every publish step checks the registry first, so
# re-running after a partial failure (an npm OTP timeout at package 9/15)
# skips what already shipped and continues. Phases:
#   preflight → bump → build → test → publish → tag → verify → reminders
#
# Deliberately NOT automated (printed as reminders at the end):
# forme-go (separate repo, manual by choice), pdf-testkit pin bump,
# forme-landing/playground dep bumps, docs.formepdf.com.

set -euo pipefail

VERSION="${1:-}"
DRY_RUN=false
FROM_PHASE="preflight"
shift || true
while [ $# -gt 0 ]; do
  case "$1" in
    --dry-run) DRY_RUN=true ;;
    --from) shift; FROM_PHASE="${1:-preflight}" ;;
    *) echo "unknown flag: $1"; exit 1 ;;
  esac
  shift || true
done

if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$ ]]; then
  echo "Usage: ./scripts/release.sh <version> [--dry-run] [--from <phase>]"
  exit 1
fi

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

GREEN='\033[0;32m'; YELLOW='\033[1;33m'; RED='\033[0;31m'; NC='\033[0m'
step() { printf "\n${GREEN}━━━ %s ━━━${NC}\n\n" "$1"; }
note() { printf "${YELLOW}%s${NC}\n" "$1"; }
fail() { printf "${RED}FAIL: %s${NC}\n" "$1"; exit 1; }
confirm() {
  local r
  read -r -p "$1 [y/N] " r
  [[ "$r" =~ ^[Yy]$ ]]
}

# npm packages in dependency/publish order. html joined the line at 0.14.0.
NPM_ORDER=(shared fonts-standard react core renderer svelte vue preact cli hono next resend mcp sdk tailwind templates html)
PARITY_FIXTURES=(letterhead report zebra-invoice statement)

phase_reached() {
  # Phases run in fixed order; --from skips earlier ones.
  local phases=(preflight bump build test publish tag verify reminders) i started=false
  for i in "${phases[@]}"; do
    [ "$i" = "$FROM_PHASE" ] && started=true
    [ "$i" = "$1" ] && { $started && return 0 || return 1; }
  done
  return 1
}

# ────────────────────────── preflight ──────────────────────────
if phase_reached preflight; then
  step "PREFLIGHT — credentials, clean trees, toolchain"

  [ -z "$(git status --porcelain)" ] || fail "working tree not clean (forme)"
  note "branch: $(git branch --show-current)"

  command -v wasm-pack >/dev/null || fail "wasm-pack not installed"
  note "wasm-pack $(wasm-pack --version | awk '{print $2}')  (CI pins 0.13.1 — published WASM builds HERE; keep this in mind)"
  note "node $(node --version), npm $(npm --version), $(cargo --version | cut -d' ' -f1-2)"

  if ! $DRY_RUN; then
    NPM_USER="$(npm whoami 2>/dev/null)" || fail "npm not logged in (npm login)"
    note "npm: $NPM_USER"
    [ -f "$HOME/.cargo/credentials.toml" ] || [ -f "$HOME/.cargo/credentials" ] \
      || note "WARN: no cargo credentials found — 'cargo login' before the crates.io step"
    [ -f "$HOME/.pypirc" ] || [ -n "${TWINE_PASSWORD:-}" ] \
      || note "WARN: no ~/.pypirc and no TWINE_PASSWORD — PyPI upload will prompt/fail"
  fi

  note ""
  note "Supply-chain: check current npm IoC advisories (socket.dev / snyk) and"
  note "grep lockfiles for affected package@version pairs before publishing."
  $DRY_RUN || confirm "Lockfile supply-chain audit done (or accepted)?" || fail "aborted at supply-chain gate"
fi

# ────────────────────────── bump ──────────────────────────
if phase_reached bump; then
  step "BUMP — versions, changelogs, lockfile"

  CURRENT="$(grep -m1 '^version' engine/Cargo.toml | sed 's/.*"\(.*\)"/\1/')"
  if [ "$CURRENT" = "$VERSION" ]; then
    note "engine already at $VERSION — bump previously applied, skipping"
  else
    ./scripts/bump-version.sh "$VERSION"
    (cd "$ROOT" && npm install >/dev/null 2>&1)
    note "root package-lock.json regenerated"
  fi

  MISSING_CHANGELOG=()
  for f in engine/CHANGELOG.md packages/*/CHANGELOG.md; do
    [ -f "$f" ] || continue
    grep -q "\[$VERSION\]" "$f" || MISSING_CHANGELOG+=("$f")
  done
  if [ ${#MISSING_CHANGELOG[@]} -gt 0 ]; then
    note "CHANGELOGs without a [$VERSION] entry (retitle '## [Unreleased]' where content exists):"
    printf '  %s\n' "${MISSING_CHANGELOG[@]}"
    confirm "Continue anyway (unchanged packages are fine to skip)?" || fail "aborted for changelogs"
  fi

  if [ -n "$(git status --porcelain)" ]; then
    git status --short
    confirm "Commit the bump ('Bump all packages to $VERSION')?" || fail "bump uncommitted"
    git add -A
    git commit -m "Bump all packages to $VERSION"
  fi
fi

# ────────────────────────── build ──────────────────────────
if phase_reached build; then
  step "BUILD — full dependency order"

  (cd engine && cargo build --release && cargo fmt --check && cargo clippy -- -D warnings)
  for p in shared fonts-standard react; do (cd "packages/$p" && npm run build); done
  (cd packages/core && npm run build)          # wasm-pack ×3 targets + tsc
  (cd packages/renderer && npm run build)
  (cd packages/svelte && npm run build && npm run check)
  (cd packages/vue && npm run build)
  (cd packages/preact && npm run build)
  (cd packages/cli && npm run build)
  # html BEFORE vscode: the extension snapshots BOTH wasms (core + html),
  # so both must be current when it builds. (The gate below caught this
  # ordering wrong on the first real run.)
  (cd html && cargo build --release && cargo fmt --check && cargo clippy --all-targets -- -D warnings)
  (cd packages/html && ./build.sh)
  (cd packages/vscode && npm run build)        # snapshots BOTH wasms + preview
  for p in hono next mcp resend sdk tailwind templates; do (cd "packages/$p" && npm run build); done
  (cd packages/python-sdk && bash build_wasm.sh)

  step "BUILD — copied-artifact verification (the stale-copy gate)"
  verify_copy() {
    local a b
    a="$(shasum -a 256 "$1" | cut -d' ' -f1)"
    b="$(shasum -a 256 "$2" | cut -d' ' -f1)"
    [ "$a" = "$b" ] && echo "  ok: $2" || fail "STALE COPY: $2 != $1"
  }
  verify_copy packages/core/pkg-node/forme_bg.wasm packages/vscode/dist/forme_bg.wasm
  verify_copy packages/html/pkg-node/forme_pdf_html_bg.wasm packages/vscode/dist/forme_pdf_html_bg.wasm
  cmp -s packages/renderer/src/preview/index.html packages/renderer/dist/preview/index.html \
    && echo "  ok: renderer preview src == dist" || fail "renderer preview src != dist"
fi

# ────────────────────────── test ──────────────────────────
if phase_reached test; then
  step "TEST — every suite, including the ones the old gate missed"

  (cd engine && cargo test)
  (cd html && cargo test)
  for p in fonts-standard react core renderer svelte vue preact cli hono next resend mcp; do
    (cd "packages/$p" && npm test)
  done
  (cd packages/core && npm run test:templates)
  (cd packages/html && npm test)

  step "TEST — native vs WASM byte parity (${#PARITY_FIXTURES[@]} fixtures)"
  for f in "${PARITY_FIXTURES[@]}"; do
    ./html/target/release/forme-html "html/tests/fixtures/$f.html" -o "/tmp/rel-$f-native.pdf" -q
    node packages/html/bin/forme-html.js "html/tests/fixtures/$f.html" -o "/tmp/rel-$f-wasm.pdf" -q
    cmp -s "/tmp/rel-$f-native.pdf" "/tmp/rel-$f-wasm.pdf" && echo "  $f: byte-identical" \
      || fail "parity: $f differs between native and WASM"
  done
fi

if $DRY_RUN; then
  step "DRY RUN COMPLETE — everything built and green; nothing published"
  exit 0
fi

# ────────────────────────── publish ──────────────────────────
if phase_reached publish; then
  step "PUBLISH — npm (${#NPM_ORDER[@]} packages, in order, skip-if-published)"
  note "Registries are immutable per version. Broken publish = bump + redo."
  confirm "Begin npm publishes for $VERSION?" || fail "aborted before npm"

  for p in "${NPM_ORDER[@]}"; do
    name="@formepdf/$p"
    if npm view "$name@$VERSION" version >/dev/null 2>&1; then
      echo "  already published: $name@$VERSION"
    else
      (cd "packages/$p" && npm publish --access public)
      echo "  published: $name@$VERSION"
    fi
  done

  step "PUBLISH — VS Code Marketplace"
  if confirm "Package + publish the VSIX ($VERSION)?"; then
    (cd packages/vscode && npm run package && npx @vscode/vsce publish)
  else
    note "skipped vsce"
  fi

  step "PUBLISH — PyPI"
  if curl -sf "https://pypi.org/pypi/formepdf/$VERSION/json" >/dev/null 2>&1; then
    echo "  already published: formepdf==$VERSION"
  elif confirm "Build + upload formepdf==$VERSION to PyPI?"; then
    (cd packages/python-sdk && rm -rf dist && python3 -m build && twine upload "dist/formepdf-$VERSION"*)
  else
    note "skipped PyPI"
  fi

  step "PUBLISH — crates.io (engine only; forme-pdf-html is npm-only by policy)"
  if curl -sf -A "forme-release-script" "https://crates.io/api/v1/crates/forme-pdf/$VERSION" >/dev/null 2>&1; then
    echo "  already published: forme-pdf@$VERSION"
  elif confirm "cargo publish forme-pdf@$VERSION (dry-run first)?"; then
    (cd engine && cargo publish --dry-run && cargo publish)
  else
    note "skipped crates.io"
  fi
fi

# ────────────────────────── tag ──────────────────────────
if phase_reached tag; then
  step "TAG — monorepo"
  if git rev-parse "v$VERSION" >/dev/null 2>&1; then
    note "tag v$VERSION exists at $(git rev-parse --short "v$VERSION")"
  else
    confirm "Create tag v$VERSION at HEAD ($(git rev-parse --short HEAD))?" && git tag "v$VERSION"
  fi
  confirm "Push branch + tag to origin?" && { git push origin "$(git branch --show-current)"; git push origin "v$VERSION"; }
fi

# ────────────────────────── verify ──────────────────────────
if phase_reached verify; then
  step "VERIFY — clean-room installs against the live registries"
  CLEAN="/tmp/forme-release-verify-$VERSION"
  rm -rf "$CLEAN" && mkdir -p "$CLEAN" && pushd "$CLEAN" >/dev/null
  npm init -y >/dev/null 2>&1
  npm install "@formepdf/react@$VERSION" "@formepdf/core@$VERSION" "@formepdf/html@$VERSION" >/dev/null 2>&1 \
    && echo "  npm install: ok" || note "  npm install FAILED (propagation can lag ~1min; re-run --from verify)"
  printf '<h1 style="color:#1a365d">Release %s</h1>' "$VERSION" > t.html
  npx forme-html t.html -q 2>/dev/null && [ "$(head -c5 t.pdf)" = "%PDF-" ] \
    && echo "  npx forme-html: ok ($(wc -c < t.pdf | tr -d ' ') bytes)" || note "  npx forme-html FAILED"
  popd >/dev/null
  note "PyPI:      pip install formepdf==$VERSION   (verify separately)"
  note "crates.io: https://crates.io/crates/forme-pdf/$VERSION"
fi

# ────────────────────────── reminders ──────────────────────────
if phase_reached reminders; then
  step "MANUAL STEPS REMAINING (not automated by choice)"
  cat <<REMINDERS
  1. forme-go (../forme-go — separate repo, you do this by hand):
       cp packages/python-sdk/formepdf/forme.wasm ../forme-go/templates/forme.wasm
       cd ../forme-go && go clean -testcache && go test ./...
       git add templates/forme.wasm && git commit -m "Release v$VERSION"
       git tag v$VERSION && git push origin main v$VERSION
       # pkg.go.dev indexes within ~30 min

  2. pdf-testkit (../pdf-testkit): bump @formepdf/core devDep to $VERSION;
     the installed-vs-pinned drift check will name any ElementNodeType
     changes. Known for 0.14: 'Table' joins the pinned union (32 -> 33).

  3. forme-landing + forme-playground: bump @formepdf/* to $VERSION.

  4. docs.formepdf.com: update pages affected by this release.
REMINDERS
  step "RELEASE $VERSION COMPLETE"
fi
