#!/bin/sh
# Baut die Forge-Brücke: build/forge-bridge.jar
# FORGE_JAR = Forge-Desktop-Jar mit allen Abhängigkeiten (aus der Forge-Installation).
set -e
cd "$(dirname "$0")"
FORGE_JAR="${FORGE_JAR:-$HOME/Games/Forge/forge-gui-desktop-2.0.15-SNAPSHOT-jar-with-dependencies.jar}"
if [ ! -f "$FORGE_JAR" ]; then
  echo "Forge-Jar nicht gefunden: $FORGE_JAR (Variable FORGE_JAR setzen)" >&2
  exit 1
fi
rm -rf build/classes
mkdir -p build/classes
javac --release 17 -encoding UTF-8 -cp "$FORGE_JAR" -d build/classes $(find src -name '*.java')
jar --create --file build/forge-bridge.jar -C build/classes .
echo "OK: build/forge-bridge.jar"
