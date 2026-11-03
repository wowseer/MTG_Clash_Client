#!/bin/sh
# Baut die Forge-Brücke: build/forge-bridge.jar
# FORGE_JAR = Forge-Desktop-Jar mit allen Abhängigkeiten (aus der Forge-Installation).
set -e
cd "$(dirname "$0")"
if [ -z "$FORGE_JAR" ]; then
  # Projektkopie (forge-bridge/forge/) bevorzugen, sonst ~/Games/Forge
  if [ -d forge/res ]; then FORGE_DIR="${FORGE_DIR:-$(pwd)/forge}"; else FORGE_DIR="${FORGE_DIR:-$HOME/Games/Forge}"; fi
  FORGE_JAR="$(ls "$FORGE_DIR"/forge-gui-desktop-*-jar-with-dependencies.jar 2>/dev/null | sort | tail -n 1)"
fi
if [ ! -f "$FORGE_JAR" ]; then
  echo "Forge-Jar nicht gefunden: $FORGE_JAR (Variable FORGE_JAR setzen)" >&2
  exit 1
fi
rm -rf build/classes
mkdir -p build/classes
javac --release 17 -encoding UTF-8 -cp "$FORGE_JAR" -d build/classes $(find src -name '*.java')
jar --create --file build/forge-bridge.jar -C build/classes .
echo "OK: build/forge-bridge.jar"
