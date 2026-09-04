#!/usr/bin/env bash
# The byte wall, with build freshness BY CONSTRUCTION.
#
# Renders the html fixture corpus through two forme-html binaries — the
# current working tree and a baseline git ref — and cmp's the PDFs.
#
# History: the stale-binary trap struck four times (cargo fingerprints
# twice, a stale dist, and a release binary built in engine/ while
# rendering with html's bin), each time making a wall comparison lie in
# one direction or the other. Memory is not a defense; this script is.
# It never trusts an existing binary: the current side is ALWAYS built,
# in the crate directory whose binary runs, immediately before
# rendering; the baseline side is ALWAYS built from a clean `git
# worktree` of the requested ref (cached target dir keeps rebuilds
# incremental). Same reasoning as the measure/layout agreement gate:
# enforce the invariant instead of remembering it.
#
# Usage:
#   scripts/byte-wall.sh <baseline-ref> [fixture ...]
#
# Fixtures default to the committed float-free corpus. Inputs are taken
# from the CURRENT tree for both sides (constant input, two binaries).
# Exit 0 = all byte-identical; exit 1 = at least one differs (each
# DIFFERS names both files so the diff can be examined — a legitimate
# behavior change is a finding to look at, not an error to suppress).
set -uo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)"
REF="${1:?usage: byte-wall.sh <baseline-ref> [fixture ...]}"
shift || true
FIXTURES=("$@")
if [ ${#FIXTURES[@]} -eq 0 ]; then
  FIXTURES=(letterhead report zebra-invoice statement dashed-borders invoice)
fi

OUT="$(mktemp -d /tmp/byte-wall.XXXXXX)"
WT="$HERE/.byte-wall-worktree"
CACHE="$HERE/html/target/byte-wall-baseline"

echo "── building CURRENT (html/, release)"
(cd "$HERE/html" && cargo build --release --quiet) || exit 2
CURRENT_BIN="$HERE/html/target/release/forme-html"

echo "── building BASELINE ($REF, clean worktree)"
git -C "$HERE" worktree remove --force "$WT" 2>/dev/null || true
git -C "$HERE" worktree add --detach --quiet "$WT" "$REF" || exit 2
(cd "$WT/html" && CARGO_TARGET_DIR="$CACHE" cargo build --release --quiet) || {
  git -C "$HERE" worktree remove --force "$WT"
  exit 2
}
BASELINE_BIN="$CACHE/release/forme-html"

status=0
for f in "${FIXTURES[@]}"; do
  src="$HERE/html/tests/fixtures/$f.html"
  if [ ! -f "$src" ]; then
    echo "$f: SKIPPED (no fixture at html/tests/fixtures/$f.html)"
    continue
  fi
  "$CURRENT_BIN" "$src" -o "$OUT/$f.current.pdf" 2>/dev/null
  "$BASELINE_BIN" "$src" -o "$OUT/$f.baseline.pdf" 2>/dev/null
  if cmp -s "$OUT/$f.baseline.pdf" "$OUT/$f.current.pdf"; then
    echo "$f: IDENTICAL"
  else
    echo "$f: DIFFERS — $OUT/$f.baseline.pdf vs $OUT/$f.current.pdf"
    status=1
  fi
done

git -C "$HERE" worktree remove --force "$WT"
exit $status
