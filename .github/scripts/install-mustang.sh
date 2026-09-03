#!/usr/bin/env bash
# Headless install of the Mustangproject CLI (the ZUGFeRD/Factur-X
# reference validator) for the e-invoice container gate.
#
# Pinned by version AND sha256 — a downloaded artifact nobody inspects is
# how the sRGB.icc incident happened; the hash makes a swapped or
# truncated jar a loud failure instead of a silent one. Validation is
# fully offline (XSDs + compiled schematron ship inside the jar).
# License: Apache-2.0 (https://github.com/ZUGFeRD/mustangproject).
set -euo pipefail

VERSION="${MUSTANG_VERSION:-2.26.0}"
SHA256="${MUSTANG_SHA256:-42d7868cb68264874a7b8cab4c3587b03b23ccc7cd72373da917f66758bb9736}"
DEST="${MUSTANG_HOME:-$HOME/mustang}"

mkdir -p "$DEST"
jar="$DEST/Mustang-CLI.jar"
url="https://github.com/ZUGFeRD/mustangproject/releases/download/core-${VERSION}/Mustang-CLI-${VERSION}.jar"

echo "Downloading Mustang CLI ${VERSION} from ${url}"
curl -fSL "$url" -o "$jar"

actual="$(sha256sum "$jar" | cut -d' ' -f1)"
if [ "$actual" != "$SHA256" ]; then
  echo "Mustang CLI sha256 mismatch: expected $SHA256, got $actual" >&2
  exit 1
fi
echo "Mustang CLI installed at $jar (sha256 verified)"
