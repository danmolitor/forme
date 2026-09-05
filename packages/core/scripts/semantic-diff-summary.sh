#!/usr/bin/env bash
# Dogfood experiment (non-gating): when the template-regression suite
# fails, pull the pdf-testkit semantic diff blocks out of its captured
# output so they land in the GitHub job summary instead of dying in the
# log scrollback. The vitest matcher already computes and formats these;
# this script only surfaces them. It must never fail a build: any
# problem here prints nothing and exits 0. Remove the experiment by
# deleting this file and the two CI lines that reference it.
set +e
LOG="${1:-/tmp/templates-regression.log}"
[ -f "$LOG" ] || exit 0
grep -q "PDF snapshot changed" "$LOG" || exit 0

echo "## Template regression: semantic diff (pdf-testkit)"
echo ""
echo "The gate above is authoritative; this explains what moved."
echo ""
awk '
  / FAIL .*regression/ { name=$0; sub(/^.*> /, "", name); next }
  /PDF snapshot changed/ {
    inblock=1
    if (name != "") { print "### " name; name="" }
    print "```"
    sub(/^.*PDF snapshot changed/, "PDF snapshot changed")
    print
    next
  }
  inblock && /baseline: / { print; print "```"; print ""; inblock=0; next }
  inblock { print }
' "$LOG"
exit 0
