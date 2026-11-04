#!/bin/sh
# Rendert die Knöpfe aus Assets/Buttons/*.blend nach godot/assets/buttons/ (Blender nötig).
# Die prozeduralen Blender-Materialien überträgt glTF nicht, deshalb werden sie als Bilder
# gerendert (Knopf und Leuchten getrennt, damit Button3D das Leuchten animieren kann).
set -e
cd "$(dirname "$0")/.."
OUT="$(pwd)/godot/assets/buttons"
mkdir -p "$OUT"
blender -b "Assets/Buttons/Button_Ornage.blend" --python tools/blender/render_button.py -- "$OUT" button_orange
blender -b "Assets/Buttons/Button_Blau.blend" --python tools/blender/render_button.py -- "$OUT" button_blue
echo "OK: $OUT"
