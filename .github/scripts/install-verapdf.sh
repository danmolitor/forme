#!/usr/bin/env bash
# Headless install of veraPDF for the PDF/UA-1 conformance gate.
#
# veraPDF ships as an IzPack installer JAR driven by an auto-install XML.
# Pinned to 1.30.2 to match the version the corpus was validated against
# locally. Installs to $VERAPDF_HOME (default /opt/verapdf); the CLI lands
# at $VERAPDF_HOME/verapdf.
set -euo pipefail

VERSION="${VERAPDF_VERSION:-1.30.2}"
MINOR="${VERSION%.*}"                       # e.g. 1.30
DEST="${VERAPDF_HOME:-/opt/verapdf}"
WORK="$(mktemp -d)"

url="https://software.verapdf.org/releases/${MINOR}/verapdf-greenfield-${VERSION}-installer.zip"
echo "Downloading veraPDF ${VERSION} from ${url}"
curl -fSL "$url" -o "$WORK/verapdf.zip"
unzip -q "$WORK/verapdf.zip" -d "$WORK"

# The greenfield zip ships verapdf-izpack-installer-<version>.jar (the version
# trails "installer"), nested under a verapdf-greenfield-<version>/ dir.
installer_jar="$(find "$WORK" -name 'verapdf-*installer*.jar' | head -n1)"
if [ -z "$installer_jar" ]; then
  echo "veraPDF installer JAR not found in archive" >&2
  exit 1
fi

cat > "$WORK/auto-install.xml" <<XML
<?xml version="1.0" encoding="UTF-8" standalone="no"?>
<AutomatedInstallation langpack="eng">
  <com.izforge.izpack.panels.htmlhello.HTMLHelloPanel id="welcome"/>
  <com.izforge.izpack.panels.target.TargetPanel id="install_dir">
    <installpath>${DEST}</installpath>
  </com.izforge.izpack.panels.target.TargetPanel>
  <com.izforge.izpack.panels.packs.PacksPanel id="sdk_pack_select">
    <pack index="0" name="veraPDF Mac and *nix Scripts" selected="true"/>
    <pack index="1" name="veraPDF GUI" selected="true"/>
    <pack index="2" name="veraPDF Validation model" selected="true"/>
  </com.izforge.izpack.panels.packs.PacksPanel>
  <com.izforge.izpack.panels.install.InstallPanel id="install"/>
  <com.izforge.izpack.panels.finish.FinishPanel id="finish"/>
</AutomatedInstallation>
XML

echo "Running headless install to ${DEST}"
java -jar "$installer_jar" "$WORK/auto-install.xml"

"${DEST}/verapdf" --version
echo "veraPDF installed at ${DEST}/verapdf"
