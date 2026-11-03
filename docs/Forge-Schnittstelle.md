# Forge-Schnittstelle

MTG Clash kann die Regel-Engine von **Forge** (Java) nutzen. Forge läuft dabei als eigener
Prozess; Godot spricht über stdin/stdout mit ihm.

```
Godot (Rust)                                  Java-Prozess (forge-bridge/)
ForgeBridge-Node  ── JSON-Zeile auf stdin ──▶  clash.forgebridge.ForgeBridge
  Signale        ◀── JSON-Zeile auf stdout ──    └─ Forge (forge-game, forge-ai, …)
                       stderr ──▶ user://forge-bridge.log
```

## Lizenz

Forge steht unter **GPL-3.0**. Die Brücke (`forge-bridge/`) linkt Forge-Klassen und steht
deshalb ebenfalls unter GPL-3.0 (`forge-bridge/LICENSE`). Das Spiel selbst (Apache-2.0) startet
die Brücke nur als getrenntes Programm und kommuniziert über Pipes; es enthält keinen Forge-Code.
Forges Kartendatenbank (`res/cardsfolder`) enthält Karten von Wizards of the Coast, sie wird nicht
mit dem Spiel ausgeliefert, sondern aus der lokalen Forge-Installation gelesen.

## Einrichtung

1. Forge installieren (Desktop-Version), z. B. nach `~/Games/Forge`. Benötigt wird
   `forge-gui-desktop-*-jar-with-dependencies.jar` und der Ordner `res/`.
   **Projektkopie:** Liegen Jar und `res/` in `forge-bridge/forge/`, wird diese Kopie bevorzugt
   (von `build.sh`, `run.sh` und `ForgeBridge`). Der Ordner ist per `.gitignore` ausgeschlossen
   (GPL-Code und Wizards-Kartendaten, ca. 160 MB). Anlegen aus einer Installation:
   ```sh
   mkdir -p forge-bridge/forge
   rsync -a ~/Games/Forge/forge-gui-desktop-*-jar-with-dependencies.jar ~/Games/Forge/LICENSE.txt forge-bridge/forge/
   rsync -a --exclude adventure --exclude music --exclude 'languages/*' ~/Games/Forge/res forge-bridge/forge/
   cp ~/Games/Forge/res/languages/{en-US,de-DE}.properties ~/Games/Forge/res/languages/cardnames-de-DE.txt forge-bridge/forge/res/languages/
   ```
   Weggelassen werden Adventure-Modus, Musik und fremde Sprachdateien – für Partien über die
   Brücke nicht nötig.
2. Java 17 oder neuer (`java`, `javac`, `jar` im PATH).
3. Brücke bauen: `forge-bridge/build.sh` → `forge-bridge/build/forge-bridge.jar` (ca. 1–2 s).
   Anderer Forge-Pfad: `FORGE_JAR=/pfad/zum/forge.jar forge-bridge/build.sh`.
4. In Godot einen Knoten `ForgeBridge` anlegen und ggf. `forge_dir` / `forge_jar` setzen
   (leer = `FORGE_DIR`, sonst `forge-bridge/forge/`, sonst `~/Games/Forge`; Jar wird automatisch gesucht).

Manueller Test ohne Godot: `forge-bridge/run.sh` starten und JSON-Zeilen eintippen, z. B.
`{"cmd":"ping"}`.

## Was Forge entscheidet und was MTG Clash

| Entscheidung | wer |
|---|---|
| Priorität: Karte/Fähigkeit spielen oder passen | **MTG Clash** (`request` kind `priority`) |
| Mulligan | **MTG Clash** (`request` kind `mulligan`) |
| Allgemeine Auswahl: Modi, Optionen (Kreaturtyp, Marken-Typ, Schutz, Schlüsselwort, Ersatzeffekt, Kartenseite …), Ja/Nein (optionale Trigger, „Darfst du …“, Kopf/Zahl), Zahlen, Farben, Kartennamen, optionale Kosten, Startspieler, Abstimmungen, Würfel | **MTG Clash** (`request` kind `choice`) |
| Ziele, Manabezahlung (inkl. X-Wert), Angreifer, Blocker, Auswahl von Karten | vorerst Forge-KI |
| Gegner (nicht ferngesteuerte Sitzplätze) | Forge-KI |

Erweitern: in `RemotePlayerController.java` weitere Methoden von Forges `PlayerController`
überschreiben (z. B. `declareAttackers`, `declareBlockers`, `chooseTargetsFor`) und darin
`ForgeBridge.request("<kind>", seat, payload)` aufrufen.

Angeboten werden nur Optionen, die jetzt spielbar und bezahlbar sind; Manafähigkeiten werden
nicht angeboten (Mana wird beim Ausführen automatisch bezahlt). Ist die gewählte Option nicht
ausführbar (nicht spielbar, nicht bezahlbar, kein gültiges Ziel), kommt `rejected` und eine neue
Anfrage. Ob die Forge-KI den Zug für sinnvoll hält, spielt keine Rolle (bis Protokoll 1 hat ihre
Einschätzung manche legalen Züge verhindert).

## Protokoll (Version 2)

JSON Lines, UTF-8: genau eine JSON-Nachricht pro Zeile.

### Befehle (Godot → Forge, stdin)

| `cmd` | Felder | Wirkung |
|---|---|---|
| `ping` | `nonce` (optional) | Antwort `pong` mit gleichem `nonce` |
| `start_match` | `decks`: Liste von `.dck`-Pfaden oder Forge-Decknamen (mind. 2) · `remote`: Liste bool je Sitzplatz (true = von MTG Clash gesteuert) · `names`: Spielernamen · `seed` (optional) · `starting_life` (optional) | startet eine Partie (Constructed, 1 Spiel) |
| `answer` | `id`: Anfrage-ID · `choice`: Antwort | beantwortet eine `request` |
| `state` | `viewer`: Sitzplatz mit offener Hand (-1 = keiner) | Antwort `state` |
| `quit` | – | beendet den Prozess |

### Nachrichten (Forge → Godot, stdout)

| `type` | Felder |
|---|---|
| `ready` | `protocol`, `working_dir` – Forge ist geladen (ca. 5 s nach Start) |
| `pong` | `nonce` |
| `match_started` | `state` |
| `event` | `event` (Forge-Klassenname, z. B. `GameEventCardChangeZone`, `GameEventTurnPhase`), `text`, `turn`, `phase` |
| `request` | `id`, `kind`, `player` (Sitzplatz), dazu je nach `kind`: **priority**: `options`, `state` → `choice` = Index oder -1 (passen) · **mulligan**: `cards_to_return`, `state` → `choice` = true (behalten) / false · **choice**: siehe unten |
| `rejected` | `player`, `option`, `reason` – gewählte Option nicht ausführbar |
| `state` | `state` |
| `game_over` | `winner` (Name oder null), `draw`, `reason` |
| `chat` | `from`, `text` |
| `error` | `code` (`init_failed`, `bad_command`, `unknown_command`, `deck_not_found`, `match_running`, `no_match`, `unknown_request`, `game_crashed`), `message` |

Der Spiel-Thread wartet bei jeder `request`, bis die passende `answer` kommt.

**`request` kind `choice`** (allgemeine Auswahl, `RemoteChoice.java`)

| Feld | Bedeutung |
|---|---|
| `mode` | `single` (eine Option) · `multi` (mehrere) · `confirm` (Ja/Nein) · `number` (Zahl) · `text` (freie Eingabe, z. B. Kartenname) |
| `title` | Frage (von Forge oder der Brücke; `CARDNAME` ist bereits ersetzt) |
| `source` | `{id, name}` der Karte, um die es geht (optional) |
| `ability` | Text der Fähigkeit (optional) |
| `options` | Liste `{text, color?}` für `single`/`multi`/`confirm`; `color` = `W U B R G C` bei Farbwahl |
| `min`, `max` | Anzahl bei `multi`, Grenzen bei `number` |
| `error` | bei `text`: Hinweis nach ungültiger Eingabe (die Frage kommt dann erneut, höchstens 5-mal) |
| `cancelable` | Abbrechen erlaubt |
| `state` | Spielzustand |

Antwort `choice`: `single` → Index · `multi` → Liste von Indizes · `confirm` → true (erste Option)
/ false · `number` → Zahl · `text` → Text · Abbrechen → null. Ungültige Antworten (falscher Typ,
Index außerhalb, falsche Anzahl) gelten als Abbruch; ist Abbrechen nicht erlaubt, entscheidet dann
die Forge-KI. Gibt es bei `single` nur eine Option und ist Abbrechen nicht erlaubt, wird gar
nicht gefragt; `multi` mit genau einer Wahl wird als `single` gefragt.

**`state`**

```json
{
  "turn": 3, "phase": "MAIN1", "active_player": 0,
  "players": [
    { "seat": 0, "name": "Spieler", "life": 20, "library": 50, "hand_count": 5,
      "hand": [ …nur für viewer… ],
      "battlefield": [ { "id": 12, "name": "Mountain", "type": "Basic Land - Mountain", "tapped": false } ],
      "graveyard": [], "exile": [] }
  ]
}
```

Karten: `id`, `name`, `type`, `tapped`, bei Kreaturen zusätzlich `power`, `toughness`.

**Option (`priority`)**: `index`, `card`, `card_id`, `land` (Land spielen), `spell` (Zauber, sonst
aktivierte Fähigkeit), `text`.

Phasen (`phase`): Forges `PhaseType`-Namen, z. B. `UNTAP`, `UPKEEP`, `DRAW`, `MAIN1`,
`COMBAT_BEGIN`, `COMBAT_DECLARE_ATTACKERS`, `COMBAT_DECLARE_BLOCKERS`, `COMBAT_DAMAGE`,
`COMBAT_END`, `MAIN2`, `END_OF_TURN`, `CLEANUP`.

## Godot-Seite: `ForgeBridge` (`rust/src/forge/bridge.rs`)

Beispiel (GDScript), Spieler 0 wird ferngesteuert, Spieler 1 ist die Forge-KI:

```gdscript
@onready var forge: ForgeBridge = $ForgeBridge

func _ready():
    forge.bridge_ready.connect(func(_p):
        forge.start_match(
            PackedStringArray(["res://../forge-bridge/testdecks/rot.dck",
                               "res://../forge-bridge/testdecks/gruen.dck"]),
            PackedInt32Array([0]), PackedStringArray(["Spieler", "Gegner"]), -1))
    forge.forge_event.connect(func(e): print(e.event, ": ", e.text))
    forge.decision_requested.connect(_on_request)
    forge.game_over.connect(func(r): print("Sieger: ", r.winner))
    forge.start_bridge()

func _on_request(r: Dictionary):
    if r.kind == "mulligan":
        forge.answer(r.id, true)
    elif r.kind == "priority":
        # hier die Auswahl in der UI anbieten; -1 = passen
        forge.answer(r.id, r.options[0].index if r.options.size() > 0 else -1)
```

Alle Signale und Methoden stehen in der [API-Referenz](API-Referenz.md) unter `ForgeBridge`.

## Forge auf dem 3D-Tisch

`godot/table.tscn` (Hauptszene) spielt eine Forge-Partie auf dem 3D-Tisch (Karten, Zonen,
Drag-and-Drop, HUD). Der Knoten `ForgeTable` (`rust/src/forge/table.rs`) übersetzt dabei:

| Forge | 3D-Tisch |
|---|---|
| `state` (Spielstart, jede Anfrage, gebündelt nach Ereignissen) | `Card3D` je Karten-`id` in Hand / Ressourcen-Reihe (Länder) / Spielfeld / Ablagestapel, Gegnerhand verdeckt, getappt; `library` = verdeckter Stapel in der Deck-Zone, neue Handkarten werden von dort gezogen |
| `request` priority | spielbare Handkarten leuchten; Handkarte auf den Tisch ziehen → `answer(index)`; Hauptbutton / Leertaste → `answer(-1)` |
| `request` mulligan | HUD-Auswahlfenster „Behalten“ / „Mulligan“ |
| `request` choice | HUD-Auswahlfenster je `mode` (Knöpfe, Mehrfachauswahl mit „Fertig“, Zahlenfeld, Textfeld, Suchfeld ab 13 Optionen); Quellkarte golden markiert |
| `phase`, `life`, `turn`, `game_over` | Zeitstrahl, Medaillons, Rundenzahl, Spielende-Meldung im `TurnHud` |

## Stand und nächste Schritte

- Getestet (headless): Godot-Node → Brücke → Forge 2.0.15, Partie in `table.tscn` bis
  zum Spielende; Karten wandern korrekt zwischen Hand, Ressourcen, Spielfeld und Friedhof.
- Getestet mit der Projektkopie `forge-bridge/forge/` (KI gegen KI bis zum Spielende).
- Ein eigenes Regelwerk gibt es nicht mehr: Forge ist die einzige Regel-Engine.
- Allgemeine Auswahl (`choice`) getestet mit `forge-bridge/testdecks/auswahl.dck` (Startspieler,
  Kreaturtyp, Kartenname, Modus, Farbe) und mit künstlichen Anfragen (Mehrfachauswahl, Ja/Nein,
  Zahl, Abbrechen).
- Noch nicht ferngesteuert: Angreifer, Blocker, Ziele, Manabezahlung (auch der X-Wert: den legt
  die KI beim Bezahlen fest), Auswahl von Karten, aktivierte Fähigkeiten bleibender Karten;
  Stack und Manapool werden im Forge-Spiel noch nicht angezeigt.
- Forges eigene Fragetexte sind englisch (Forge-Spracheinstellung); die Texte der Brücke deutsch.
