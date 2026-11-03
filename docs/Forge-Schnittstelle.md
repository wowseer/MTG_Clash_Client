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

## Was die Engine entscheidet und was der Spieler

Seit Protokoll 3 spricht die Brücke **NeoForges Entscheidungsmodell** (`Decision` / `Choice`,
siehe `forge-engine/src/decision.rs` in NeoForge). Die Java-Brücke übersetzt Forges
`PlayerController`-Aufrufe in `Decision`s (`Decisions.java`, `RemotePlayerController.java`).

| Forge-Methode(n) | NeoForge-`Decision` | Antwort (`Choice`) |
|---|---|---|
| `chooseSpellAbilityToPlay` | `Priority { actions }` | `Action(…)`; `Pass` |
| `mulliganKeepHand` | `Mulligan { mulligans_taken }` | `Keep` / `Mulligan` |
| `chooseModeForAbility` | `ChooseModes { modes, min, max, can_repeat }` | `Modes([…])` |
| `chooseTargetsFor` (über `setupTargets`) | `ChooseTargets { source, description, candidates, min, max }` | `Targets([…])` / `Cancel` |
| `announceRequirements` (X ab 0) | `ChooseX { max }` | `Number(n)` / `Cancel` |
| `chooseColor`, `chooseColors` mit genau einer Farbe | `ChooseColor { options }` | `Color("R")` |
| `chooseCardName` | `NameCard { options }` (alle erlaubten Namen) | `Number(index)` |
| `chooseSomeType` | `ChooseType { options }` | `Number(index)` |
| `confirmAction`, `confirmReplacementEffect`, `confirmStaticApplication` | `Confirm { prompt }` | `Yes` / `No` |
| `confirmTrigger` | `OptionalTrigger { description }` | `Yes` / `No` |
| Startspieler, Abstimmung, Marken-Typ, Schutz, Schlüsselwort, Ersatzeffekt, statische Fähigkeit, Kartenseite/-zustand, Sektor, Würfel, Zahl aus Liste, Ja/Nein mit eigenen Beschriftungen (Kopf/Zahl, Tappen/Enttappen …), Farbe inkl. farblos | `ChooseOption` (**Erweiterung**) | `Number(index)` |
| `chooseOptionalCosts`, `chooseColors` (mehrere), `chooseDiceToReroll` | `ChooseOptions` (**Erweiterung**) | `Modes([…])` |
| `chooseNumber`, `chooseNumberForCostReduction`, `chooseNumberForKeywordCost`, angesagte Werte ≠ X | `ChooseNumber` (**Erweiterung**) | `Number(n)` |
| Manabezahlung, Angreifer, Blocker, Schaden verteilen, Kartenauswahl, Ziele ausgelöster Fähigkeiten | (`PayMana`, `DeclareAttackers`, `DeclareBlockers`, `AssignCombatDamage`, `ChooseCards` …) | **vorerst Forge-KI** |
| Gegner (nicht ferngesteuerte Sitzplätze) | – | Forge-KI |

**Ablauf beim Spielen** (`RemotePlayerController.prepareRemote`, nachgebaut nach Forges Ablauf für
menschliche Spieler, `PlaySpellAbility` im Forge-Quellcode): Zusatzkosten/optionale Kosten →
Modi (`CharmEffect.makeChoices`) → angesagter Typ → X-Wert (`announceRequirements`) →
`SpellAbility.setupTargets()` (ruft je gezielter Teilfähigkeit `chooseTargetsFor`) → Bezahlen
durch Forge (vorerst automatisch). Zauber, deren Schaden frei verteilt wird, bereitet noch die KI vor.

**Priority-Aktionen**: Land → `PlayLand(card)` (weitere Land-Seiten derselben Karte `PlayLandAlt`),
Zauber → `Cast(card)` (weitere Zauber-Varianten derselben Karte `CastAlt(card, n)`), aktivierte
Fähigkeit → `Activate(card, n)` (n = laufender Index der angebotenen Fähigkeiten der Karte).
Angeboten wird nur, was jetzt spielbar und bezahlbar ist und mögliche Ziele hat; Manafähigkeiten
nicht (Mana wird beim Ausführen automatisch bezahlt). Ist die gewählte Aktion nicht ausführbar
(abgebrochen, kein gültiges Ziel), kommt `rejected` und eine neue `Priority`-Anfrage. Ob die
Forge-KI den Zug für sinnvoll hält, spielt keine Rolle.

### Erweiterungen gegenüber NeoForge

Diese Teile gibt es in NeoForges `decision.rs` (noch) nicht; sie sollten dort nachgezogen werden,
damit die UI unverändert mit NeoForge läuft:

| Erweiterung | Zweck |
|---|---|
| `Decision::ChooseOption { player, prompt, options: [{text, color?}] }` → `Choice::Number(index)` | allgemeine Einfachauswahl (siehe Tabelle oben); `color` = `W U B R G` oder `C` (farblos) |
| `Decision::ChooseOptions { player, prompt, options, min, max }` → `Choice::Modes(indizes)` | allgemeine Mehrfachauswahl |
| `Decision::ChooseNumber { player, prompt, min, max }` → `Choice::Number(n)` | Zahl mit Untergrenze ≠ 0 bzw. außerhalb von X |
| `Choice::Cancel` | Ziel-/X-Wahl beim Wirken abbrechen (Wirken wird zurückgenommen); bei anderen Entscheidungen „Engine entscheidet selbst“ |
| Hülle der Anfrage: `prompt`, `source`, `ability`, `cancelable`, `labels` | Anzeigetexte (Forges Fragetext, Quellkarte, Fähigkeitstext, Anzeigetext je Listeneintrag) |

## Protokoll (Version 3)

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
| `request` | `id`, `player`, `decision` (NeoForge-`Decision`), `prompt`, `source` `{id, name}`, `ability`, `cancelable`, `labels`, `state` – siehe unten |
| `rejected` | `player`, `option` (Index in `actions` ohne `Pass`), `action`, `reason` – gewählte Aktion nicht ausführbar |
| `state` | `state` |
| `game_over` | `winner` (Name oder null), `draw`, `reason` |
| `chat` | `from`, `text` |
| `error` | `code` (`init_failed`, `bad_command`, `unknown_command`, `deck_not_found`, `match_running`, `no_match`, `unknown_request`, `game_crashed`), `message` |

Der Spiel-Thread wartet bei jeder `request`, bis die passende `answer` kommt.

**`request`** (Protokoll v3)

```json
{"type": "request", "id": 7, "player": 0,
 "decision": {"type": "ChooseTargets", "player": 0, "source": 34, "description": "Ziel wählen: Select any target",
              "candidates": [{"player": 0}, {"player": 1}, {"card": 83}], "min": 1, "max": 1},
 "prompt": "Ziel wählen: Select any target", "source": {"id": 34, "name": "Lightning Bolt"},
 "ability": "Lightning Bolt deals 3 damage to any target.", "cancelable": true,
 "labels": ["Spieler (du)", "Forge-KI", "Llanowar Elves (Forge-KI)"], "state": {…}}
```

JSON-Form der NeoForge-Typen: Variante als `"type"`, Felder in `snake_case`; `Target` =
`{"card": id}` / `{"player": sitz}`; `Action` = `{"type": "Cast", "card": 34}` bzw. mit `"index"`
(`CastAlt`, `PlayLandAlt`, `Activate`, `CastWithAltCost`); `Color` = `"W" "U" "B" "R" "G"`;
`ChooseModes.modes` = Liste von `[index, text]`. `labels` = Anzeigetext je Eintrag der Hauptliste
(`actions` bei Priority, `candidates` bei ChooseTargets, `modes` bei ChooseModes).

**Antwort** `{"cmd": "answer", "id": 7, "choice": {…}}` mit einer NeoForge-`Choice`:
`{"type": "Keep"}`, `{"type": "Mulligan"}`, `{"type": "Action", "action": {…}}`,
`{"type": "Targets", "targets": [{"card": 83}]}`, `{"type": "Number", "value": 2}`,
`{"type": "Yes"}` / `{"type": "No"}`, `{"type": "Color", "color": "R"}`,
`{"type": "Modes", "modes": [0, 2]}`, `{"type": "Cancel"}` (Erweiterung); außerdem (noch ohne
Gegenstück in der Brücke) `Pay`, `Attackers`, `Blocks`, `Damage`, `Cards`, `Order`, `Arrange`.
Unpassende oder ungültige Antworten gelten als Abbruch; wo das nicht erlaubt ist, entscheidet die
Forge-KI. Eine Einfachauswahl mit nur einer Option und ohne Abbrechen wird gar nicht gefragt.

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
    match r.decision.type:
        "Mulligan":
            forge.answer(r.id, {"type": "Keep"})
        "Priority":
            # hier die Auswahl in der UI anbieten; erste Aktion ist immer {"type": "Pass"}
            forge.answer(r.id, {"type": "Action", "action": r.decision.actions[0]})
        _:
            forge.answer(r.id, {"type": "Cancel"})   # Engine entscheidet selbst
```

Alle Signale und Methoden stehen in der [API-Referenz](API-Referenz.md) unter `ForgeBridge`.

## Forge auf dem 3D-Tisch

`godot/table.tscn` (Hauptszene) spielt eine Forge-Partie auf dem 3D-Tisch (Karten, Zonen,
Drag-and-Drop, HUD). Der Knoten `ForgeTable` (`rust/src/forge/table.rs`) übersetzt dabei:

| Forge | 3D-Tisch |
|---|---|
| `state` (Spielstart, jede Anfrage, gebündelt nach Ereignissen) | `Card3D` je Karten-`id` in Hand / Ressourcen-Reihe (Länder) / Spielfeld / Ablagestapel, Gegnerhand verdeckt, getappt; `library` = verdeckter Stapel in der Deck-Zone, neue Handkarten werden von dort gezogen |
| `Priority` | Karten mit einer Aktion leuchten; Karte ziehen und loslassen → `Action` dieser Karte (aus der Hand `PlayLand`/`Cast`, auf dem Spielfeld `Activate`); Hauptbutton / Leertaste → `Action::Pass` |
| alle übrigen Entscheidungen | HUD-Auswahlfenster (`decision_view.rs`): Knöpfe, Mehrfachauswahl mit „Fertig“, Zahlenfeld, Suchfeld ab 13 Optionen (höchstens 40 Knöpfe, z. B. bei `NameCard` mit allen Kartennamen); Quellkarte golden markiert |
| `phase`, `life`, `turn`, `game_over` | Zeitstrahl, Medaillons, Rundenzahl, Spielende-Meldung im `TurnHud` |

## Stand und nächste Schritte

- Getestet (headless): Godot-Node → Brücke → Forge 2.0.15, Partie in `table.tscn` bis
  zum Spielende; Karten wandern korrekt zwischen Hand, Ressourcen, Spielfeld und Friedhof.
- Getestet mit der Projektkopie `forge-bridge/forge/` (KI gegen KI bis zum Spielende).
- Ein eigenes Regelwerk gibt es nicht mehr: Forge ist die einzige Regel-Engine.
- Allgemeine Auswahl (`choice`) getestet mit `forge-bridge/testdecks/auswahl.dck` (Startspieler,
  Kreaturtyp, Kartenname, Modus, Farbe) und mit künstlichen Anfragen (Mehrfachauswahl, Ja/Nein,
  Zahl, Abbrechen).
- Ziele, X-Wert, Kicker, Modi und Ausrüsten getestet mit `testdecks/spieler.dck` gegen
  `testdecks/gegner.dck` (Standard-Decks von `ForgeTable`).
- Protokoll v3 (NeoForge-Modell) getestet: Startspieler (`ChooseOption`), `Mulligan`, `NameCard`
  (34.457 Namen, Suche), Kicker (`ChooseOptions`), Farbe, `ChooseTargets` für Ausrüsten, `Priority`
  mit `Cast` und `Activate`.
- Noch nicht ferngesteuert: Angreifer, Blocker, Manabezahlung, Auswahl von Karten, Ziele
  ausgelöster Fähigkeiten; Ziele werden noch als Liste gewählt, nicht per Klick auf dem Tisch;
  Stack und Manapool werden im Forge-Spiel noch nicht angezeigt.
- Forges eigene Fragetexte sind englisch (Forge-Spracheinstellung); die Texte der Brücke deutsch.
