#!/bin/sh
# Kopiert den Brücken-Code (forge-bridge/src/clash/forgebridge, ohne den eigenständigen Einstieg
# BridgeMain) in das NeoForge-Repo, wo er eingebettet läuft (forge.neo.clash.ClashTable).
# Maßgeblich ist immer die Fassung in MTG Clash; in NeoForge nicht von Hand ändern.
# NEOFORGE = NeoForge-Repo (Standard ~/Entwicklung/Java/NeoForge)
set -e
cd "$(dirname "$0")/.."
NEOFORGE="${NEOFORGE:-$HOME/Entwicklung/Java/NeoForge}"
DEST="$NEOFORGE/forge-gui-neo/src/main/java/clash/forgebridge"
mkdir -p "$DEST"
rm -f "$DEST"/*.java
for f in forge-bridge/src/clash/forgebridge/*.java; do
  [ "$(basename "$f")" = "BridgeMain.java" ] && continue
  cp "$f" "$DEST/"
done
echo "OK: $(ls "$DEST" | wc -l) Dateien nach $DEST"
