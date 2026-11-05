# NeoForge-Anbindung

Lässt NeoForge (JavaFX-Oberfläche für Forge, github.com/AdrianLopez98/NeoForge) seine Partien auf
dem 3D-Tisch von MTG Clash spielen. Menüs, Decks und Modi bleiben bei NeoForge; für den
menschlichen Sitz startet NeoForge Godot als Spieltisch und bedient ihn über die Forge-Brücke
(Protokoll v3, lokaler Socket). Partien mit mehr als zwei Spielern spielt NeoForge weiter mit
seinem eigenen Tisch.

Alles hier ist von den Änderungen an der Brücke getrennt und lässt sich so in jedes NeoForge-Repo
mitgeben.

## Inhalt

| Datei | Was |
|---|---|
| `src/forge/neo/clash/ClashTable.java` | Hülle für NeoForge: liest die Einstellungen (Systemeigenschaft → Umgebungsvariable → NeoForge-Einstellung), startet den Tisch über `clash.forgebridge.TableLauncher`, liefert den ferngesteuerten Spieler (`ClashBridge.remotePlayer`) |
| `neoforge-host.patch` | Änderungen an NeoForges eigenen Klassen gegen NeoForge upstream: `NeoGame` (3D-Tisch für den menschlichen Sitz, `NeoMatchUI` läuft unsichtbar als Zuschauer), `SafeAi` (ferngesteuerten Sitz nicht durch die KI-Absicherung ersetzen), `NeoMain` (Testbefehl `clash`), `SettingsPanel` (Abschnitt „3D-Tisch (MTG Clash)“), Sprachdateien |

Die Brücke selbst (`clash.forgebridge` aus `forge-bridge/src/`, ohne den Programmstart `app/`)
kopiert `tools/sync_neoforge.sh` dazu.

## Einbauen

```sh
NEOFORGE=~/Entwicklung/Java/NeoForge          # NeoForge-Checkout
git -C "$NEOFORGE" apply integrations/neoforge/neoforge-host.patch   # einmalig
NEOFORGE="$NEOFORGE" tools/sync_neoforge.sh    # Brücke + ClashTable kopieren (nach jeder Änderung)
```

Danach baut NeoForge wie gewohnt mit Maven. Ohne Maven gegen eine installierte NeoForge:
`tools/neoforge_patch.sh` → `forge-bridge/build/neoforge/neoforge-clash-patch.jar` vor die
NeoForge-Jars in den Klassenpfad legen (siehe `tools/neoforge_clash.sh`).

## Einschalten

In NeoForge unter *Einstellungen → 3D-Tisch (MTG Clash)*: Tisch einschalten, Godot-Programm
(Godot 4.7) und Projektordner (`godot/` von MTG Clash) eintragen. Oder beim Start:

| Systemeigenschaft | Umgebungsvariable | Bedeutung |
|---|---|---|
| `neo.clash=true` | `NEOFORGE_CLASH=true` | 3D-Tisch verwenden |
| `neo.clash.godot` | `CLASH_GODOT` | Godot-Programm (Standard `godot`) |
| `neo.clash.project` | `CLASH_PROJECT` | Godot-Projektordner von MTG Clash |

Verbindet sich der Tisch nicht innerhalb von 2 Minuten, spielt NeoForge mit seinem eigenen Tisch.

## Patch aktualisieren

Ändern sich NeoForges Klassen im Repo (lokal committet), den Patch neu schreiben:

```sh
tools/neoforge_export_patch.sh      # BASE=origin/HEAD, NEOFORGE=… anpassbar
```

Prüfen, ob er auf ein unverändertes NeoForge passt:
`git -C <frischer NeoForge-Checkout> apply --check integrations/neoforge/neoforge-host.patch`.

## Lizenz

GPL-3.0 wie NeoForge und Forge.
