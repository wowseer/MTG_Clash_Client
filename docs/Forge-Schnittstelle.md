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
2. Java 17 oder neuer (`java`, `javac`, `jar` im PATH).
3. Brücke bauen: `forge-bridge/build.sh` → `forge-bridge/build/forge-bridge.jar` (ca. 1–2 s).
   Anderer Forge-Pfad: `FORGE_JAR=/pfad/zum/forge.jar forge-bridge/build.sh`.
4. In Godot einen Knoten `ForgeBridge` anlegen und ggf. `forge_dir` / `forge_jar` setzen
   (leer = `FORGE_DIR` bzw. `~/Games/Forge`, Jar wird automatisch gesucht).

Manueller Test ohne Godot: `forge-bridge/run.sh` starten und JSON-Zeilen eintippen, z. B.
`{"cmd":"ping"}`.

## Was Forge entscheidet und was MTG Clash

| Entscheidung | wer |
|---|---|
| Priorität: Karte/Fähigkeit spielen oder passen | **MTG Clash** (`request` kind `priority`) |
| Mulligan | **MTG Clash** (`request` kind `mulligan`) |
| Ziele, Manabezahlung, Angreifer, Blocker, sonstige Auswahl | vorerst Forge-KI |
| Gegner (nicht ferngesteuerte Sitzplätze) | Forge-KI |

Erweitern: in `RemotePlayerController.java` weitere Methoden von Forges `PlayerController`
überschreiben (z. B. `declareAttackers`, `declareBlockers`, `chooseTargetsFor`) und darin
`ForgeBridge.request("<kind>", seat, payload)` aufrufen.

Angeboten werden nur Optionen, die jetzt spielbar und bezahlbar sind; Manafähigkeiten werden
nicht angeboten (Mana wird beim Ausführen automatisch bezahlt). Lehnt Forge die gewählte Option
ab (z. B. kein gültiges Ziel), kommt `rejected` und eine neue Anfrage.

## Protokoll (Version 1)

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
| `request` | `id`, `kind`, `player` (Sitzplatz), dazu je nach `kind`: **priority**: `options`, `state` → `choice` = Index oder -1 (passen) · **mulligan**: `cards_to_return`, `state` → `choice` = true (behalten) / false |
| `rejected` | `player`, `option`, `reason` – gewählte Option nicht ausführbar |
| `state` | `state` |
| `game_over` | `winner` (Name oder null), `draw`, `reason` |
| `chat` | `from`, `text` |
| `error` | `code` (`init_failed`, `bad_command`, `unknown_command`, `deck_not_found`, `match_running`, `no_match`, `unknown_request`, `game_crashed`), `message` |

Der Spiel-Thread wartet bei jeder `request`, bis die passende `answer` kommt.

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

`godot/forge_table.tscn` spielt eine Forge-Partie auf dem normalen 3D-Tisch (Karten, Zonen,
Drag-and-Drop, HUD). Der Knoten `ForgeTable` (`rust/src/forge/table.rs`) übersetzt dabei:

| Forge | 3D-Tisch |
|---|---|
| `state` (Spielstart, jede Anfrage, gebündelt nach Ereignissen) | `Card3D` je Karten-`id` in Hand / Ressourcen-Reihe (Länder) / Spielfeld / Ablagestapel, Gegnerhand verdeckt, getappt |
| `request` priority | spielbare Handkarten leuchten; Handkarte auf den Tisch ziehen → `answer(index)`; Hauptbutton / Leertaste → `answer(-1)` |
| `request` mulligan | HUD-Auswahlfenster „Behalten“ / „Mulligan“ |
| `phase`, `life`, `turn`, `game_over` | Zeitstrahl, Medaillons, Rundenzahl, Spielende-Meldung im `TurnHud` |

## Stand und nächste Schritte

- Getestet (headless): Godot-Node → Brücke → Forge 2.0.15, Partie in `forge_table.tscn` bis
  zum Spielende; Karten wandern korrekt zwischen Hand, Ressourcen, Spielfeld und Friedhof.
- `table.tscn` mit dem eigenen Regelwerk (`TurnManager`) ist unverändert.
- Noch nicht ferngesteuert: Angreifer, Blocker, Ziele, aktivierte Fähigkeiten bleibender Karten;
  Stack und Manapool werden im Forge-Spiel noch nicht angezeigt.
