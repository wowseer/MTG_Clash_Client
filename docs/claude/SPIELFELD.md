# Anweisungen für Claude: hochaufgelöste Spielfelder

Lies diese Datei, bevor du ein Spielfeld (Kulisse um den Tisch, z. B. `Arena`, `Tavern`) baust
oder überarbeitest.

## Ziel

Spielfelder auf dem Detailgrad moderner digitaler Kartenspiele: ein beleuchtetes Spielbrett als
Diorama, gestaltet wie ein aufwendig gefertigtes Objekt, mit thematischer Umgebung, weichem Licht,
feinen Materialien und kleinen lebendigen Details. **Gleicher Detailgrad wie MTG Arena, aber
eigene Gestaltung:** keine Motive, Formen, Embleme, Manasymbole, Farbschemata oder Layouts von
konkreten Arena-Brettern nachbauen. Wiedererkennbar als *unser* Spiel.

## Warum die bisherigen Felder klobig wirken

| Ursache | Abhilfe |
|---|---|
| Grundkörper (`BoxMesh`, `CylinderMesh`) mit harten 90°-Kanten | eigene Meshes mit **Fasen/Rundungen** (SurfaceTool oder glTF aus Blender), Kanten fangen Licht |
| Texturen auf der CPU erzeugt, 80 px pro Einheit | **4K-Texturen** (4096² fürs Brett), offline erzeugt und als Dateien gespeichert |
| nur Albedo + Normal | volles **PBR**: Albedo, Normal, ORM (Occlusion/Roughness/Metallic), Höhe, Emission |
| gleichmäßige Oberflächen | **Mehrstufiges Detail**: Großform → Mittelform (Ornamente, Fugen) → Feindetail (Kratzer, Poren, Staub in Vertiefungen) |
| wenig Licht-Tiefe | gebackenes Licht (`LightmapGI`) oder SDFGI, SSAO, Kontaktschatten, Vignette, sanfte Farbkorrektur |
| alles statisch | dezente Bewegung: Flammen, Partikel, Wasser, Stoff im Wind, pulsierendes Leuchten |

## Aufbau eines Spielfelds (Gestaltungsregeln)

1. **Spielfläche zuerst.** Die Spielbereiche (Spielfelder beider Seiten, Länder-Reihen, Deck,
   Friedhof, Stack-Mitte) liegen auf einer **ruhigen, kontrastarmen Fläche**. Karten müssen davor
   immer klar lesbar sein. Muster in der Spielfläche: höchstens 15 % Helligkeitsunterschied.
2. **Detail nach außen.** Je weiter vom Kartenbereich entfernt, desto mehr Detail, Kontrast und
   Farbe: Rahmen, Ecken, Umgebung. Die vier Ecken sind Blickfänge (Objekte, Lichtquellen).
3. **Klare Seiten.** Spieler- und Gegnerhälfte subtil unterscheidbar (Lichtfarbe, Banner,
   Material), symmetrisch im Aufbau.
4. **Ein Thema pro Feld.** Ein Ort mit einer Geschichte (z. B. Steinkreis im Wald,
   Alchemistenlabor, Eis-Zitadelle, Uhrwerk-Werkstatt), eine Hauptfarbe + eine Akzentfarbe.
5. **Licht erzählt.** Eine Hauptlichtquelle (warm oder kalt) von schräg oben, Füll- und
   Randlicht, leuchtende Akzente (Runen, Kristalle, Feuer) für Tiefe. Kein flaches Rundumlicht.
6. **Maßstab.** 1 Einheit = 10 cm, eine Karte ist 0,63 × 0,88. Brett aktuell 19 × 9,6, Oberfläche
   bei y = 0, Mittellinie z = 0 (Spieler z > 0). Zonen-Positionen aus `table.tscn` freihalten.
7. **Kamera bedenken.** Die Übersichtskamera schaut steil von oben (ca. 75°, `fov` 30°). Was sie
   nicht sieht, braucht kein Detail; was am Bildrand liegt, trägt die Stimmung.

## Herstellung (Pipeline)

**Texturen** – offline erzeugen, nicht beim Spielstart:
- Ein Generator-Skript in `tools/` (z. B. Python mit NumPy/Pillow oder ein Godot-Bake-Skript mit
  Shader in einem `SubViewport`) erzeugt die Karten als PNG:
  `godot/assets/boards/<name>/albedo.png`, `normal.png`, `orm.png`, `height.png`, `emission.png`.
- Auflösung: Brett 4096 × 2048 (Seitenverhältnis wie das Brett), Rand/Props 2048², Kleinteile 1024².
- Vorgehen: Höhenkarte zuerst (Großform + Ornamente als Signed-Distance-Formen + Rauschen in
  mehreren Oktaven + Erosion/Kratzer) → Normal aus Höhe → Ambient Occlusion/Kantenabnutzung aus
  Höhe (Vertiefungen dunkler, Kanten heller/abgenutzt) → Albedo aus Materialfarben × Variation ×
  AO → Roughness (Vertiefungen rauer, abgegriffene Stellen glatter).
- Freie Fremdtexturen nur mit **CC0-Lizenz** (z. B. ambientCG, Poly Haven), Herkunft in
  `godot/assets/LICENSES.md` eintragen. Keine Texturen aus anderen Spielen.

**Geometrie:**
- Brett und Rand als eigene Meshes mit Fasen (SurfaceTool, `rust/src/table/`) oder als glTF aus
  Blender (`godot/assets/boards/<name>/*.glb`).
- Detaillierte Props (Statuen, Kristalle, Kerzen, Bücher …) als glTF; jedes Prop mit LODs, wenn
  es mehr als ~20 000 Dreiecke hat.
- Ornamente auf der Spielfläche nicht als Geometrie, sondern über Normal/Höhe (Parallax nur am
  Rand, nie unter den Karten).

**Materialien und Shader:**
- `StandardMaterial3D` mit allen PBR-Karten; für große Flächen **Detail-Textur** (zweite,
  gekachelte Feinstruktur) gegen Unschärfe in der Nahaufnahme.
- Leuchten über Emission + `glow` in der `WorldEnvironment`, nicht über Zusatzlichter.
- Bewegte Details per Shader (Flammen, Wasser, Runenpuls) statt per CPU.

**Licht und Umgebung:**
- `WorldEnvironment`: Tonemap ACES/Filmic, SSAO, Glow dezent, leichter Nebel für Tiefe, Vignette.
- Feste Kulissen: `LightmapGI` backen (Ergebnis unter `godot/assets/boards/<name>/`).
- Schatten der Karten auf dem Brett müssen weich und sichtbar sein (Kontaktschatten).

**Leistung:** 60 fps auf Mittelklasse-Hardware. Brett-Texturen komprimiert (VRAM-Kompression im
Import), Mipmaps an, Anisotropie 16×. Ladezeit eines Felds unter 2 s.

## Integration ins Projekt

- Jedes Spielfeld ist eine eigene Klasse oder Szene, austauschbar in `table.tscn` (wie `Arena` /
  `Tavern`), als Tool-Klasse auch im Editor sichtbar.
- Gemeinsame Maße (Brettgröße, Zonen-Positionen) aus einer Stelle lesen, nicht kopieren.
- Exportierte Einstellungen: Thema-Farben, Leuchtstärke, Detailstufe (für schwächere Rechner).
- Doku: Abschnitt „Kulisse“ in `docs/DOCUMENTATION.md` ergänzen, `///`-Doku an der Klasse,
  API-Referenz neu erzeugen, committen.

## Vorlage für ein neues Spielfeld

```
Spielfeld: <Name>
Thema / Ort: <ein Satz>
Hauptfarbe / Akzent: <Farbe> / <Farbe>
Licht: <Hauptlicht Richtung + Farbe, Akzentlichter>
Spielfläche: <Material, Muster, Kontrast ≤ 15 %>
Rahmen: <Material, Ornamente, Ecken-Blickfänge>
Umgebung: <Objekte außerhalb des Bretts, was die Kamera am Rand sieht>
Bewegte Details: <Flammen, Partikel, Pulsieren …>
Spieler-/Gegnerseite: <wie unterscheidbar>
Texturen: <Liste, Auflösung, Generator oder CC0-Quelle>
Meshes: <selbst erzeugt / glTF, Dreiecke>
Abgrenzung: <warum das kein Nachbau eines bestehenden Bretts ist>
```

## Prüfen

- Lesbarkeit: Karten aller Farben auf der Spielfläche gut erkennbar (auch getappte, ausgegraute).
- Nahaufnahme (Rechtsklick auf eine Karte) zeigt scharfe Details, keine verwaschenen Texturen.
- Sichtprüfung macht der User (Fenster-Renders öffnen sich auf seinem Desktop, nur nach Absprache).
- Bildrate und Ladezeit messen und in der Doku notieren.
