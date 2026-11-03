#!/bin/sh
# Startet die Brücke von Hand (zum Testen). Protokoll über stdin/stdout, siehe
# docs/Forge-Schnittstelle.md. FORGE_DIR = Forge-Installationsordner (mit res/).
BRIDGE_DIR="$(cd "$(dirname "$0")" && pwd)"
FORGE_DIR="${FORGE_DIR:-$HOME/Games/Forge}"
FORGE_JAR="${FORGE_JAR:-$FORGE_DIR/forge-gui-desktop-2.0.15-SNAPSHOT-jar-with-dependencies.jar}"
cd "$FORGE_DIR" || exit 1
exec java -Xmx2g -Dfile.encoding=UTF-8 -Djava.awt.headless=true \
  -cp "$BRIDGE_DIR/build/forge-bridge.jar:$FORGE_JAR" clash.forgebridge.ForgeBridge
