#!/bin/sh
# Startet eine NeoForge-Partie mit dem 3D-Tisch (Befehl "clash") in einer TESTKOPIE der
# installierten NeoForge: Programm-Jars und Kartendaten werden mitbenutzt, die Daten (datos) aber
# kopiert, damit die eigenen Spielstände unberührt bleiben. Vorher tools/neoforge_patch.sh.
#   NEOFORGE_APP        installierte NeoForge (Standard ~/Games/NeoForge)
#   NEOFORGE_TEST_ROOT  Testkopie (Standard forge-bridge/build/neoforge/testroot)
#   CLASH_GODOT         Godot-Programm (Standard: Godot 4.7.2 aus ~/.appimages)
#   CLASH_JAVA          Java ab 17 (Standard: java im PATH; die mitgelieferte Laufzeit hat kein bin/java)
#   CLASH_CMD           NeoForge-Befehl: clash (Standard, Testpartie) oder ui (normales Menü; dann in
#                       Einstellungen → 3D-Tisch „Meine Partien am 3D-Tisch spielen“ einschalten)
# Weitere Argumente gehen an NeoForge, z. B. --deck="Food and Fellowship" --timeout=600
set -e
cd "$(dirname "$0")/.."
REPO="$(pwd)"
APP="${NEOFORGE_APP:-$HOME/Games/NeoForge}"
ROOT="${NEOFORGE_TEST_ROOT:-$REPO/forge-bridge/build/neoforge/testroot}"
GODOT="${CLASH_GODOT:-$HOME/.appimages/Godot_v4.7.2-stable_mono_linux_x86_64/Godot_v4.7.2-stable_mono_linux.x86_64}"
PATCH="$REPO/forge-bridge/build/neoforge/neoforge-clash-patch.jar"
mkdir -p "$ROOT"
if [ ! -d "$ROOT/datos" ]; then
  cp -r "$APP/datos" "$ROOT/datos"
fi
# Ein Commander-Deck für den Test (Vorkonstrukt aus NeoForges res/), falls keines da ist
mkdir -p "$ROOT/datos/decks/commander"
if [ -z "$(ls "$ROOT/datos/decks/commander")" ]; then
  cp "$APP/res/quest/commanderprecons/Food and Fellowship [LTC] [2023].dck" "$ROOT/datos/decks/commander/"
fi
ln -sfn "$APP/res" "$ROOT/res"
cd "$ROOT"
JAVA="${CLASH_JAVA:-java}"
exec "$JAVA" \
  -Dfile.encoding=UTF-8 -XX:MaxRAMPercentage=50 \
  -Dforge.assetsDir="$ROOT/" -Dneo.dataDir="$ROOT/datos" \
  --add-opens=java.base/java.util=ALL-UNNAMED --add-opens=java.base/java.lang=ALL-UNNAMED \
  --add-opens=java.base/java.lang.reflect=ALL-UNNAMED --add-opens=java.base/java.text=ALL-UNNAMED \
  --add-opens=java.base/java.util.concurrent=ALL-UNNAMED --add-opens=java.base/java.math=ALL-UNNAMED \
  --add-opens=java.base/java.net=ALL-UNNAMED \
  -Dneo.clash.godot="$GODOT" -Dneo.clash.project="$REPO/godot" \
  -cp "$PATCH:$APP/lib/app/*" forge.neo.NeoMain "${CLASH_CMD:-clash}" "$@"
