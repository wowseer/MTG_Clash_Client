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
| `declareAttackers` | `DeclareAttackers { candidates, defenders }` | `Attackers([[karte, ziel], …])`; leer = kein Angriff |
| `declareBlockers` | `DeclareBlockers { candidates, attackers }` | `Blocks([[blocker, angreifer], …])`; leer = nicht blocken |
| `applyManaToCost` (über `payManaCost`) | `PayMana { remaining, options: [ActivateMana(karte, i), AutoPay, Cancel] }` | `Pay(ActivateMana(…))` → Quelle tappen, Mana einsetzen, erneut fragen bis bezahlt; `Pay(AutoPay)`; `Pay(Cancel)` = Wirken zurücknehmen |
| `chooseCardsForEffect`, `chooseCardsForZoneChange`, `chooseSingleCardForZoneChange`, `chooseCardsToDiscardFrom`, `chooseCardsToDiscardToMaximumHandSize`, `choosePermanentsToSacrifice`, `choosePermanentsToDestroy`, `chooseCardsToRevealFromHand`, `chooseSingleEntityForEffect` / `chooseEntitiesForEffect` (nur Karten) | `ChooseCards { reason, options, min, max }` (`reason`: `Choose`, `Search`, `Discard`, `DiscardToHandSize`, `Sacrifice`) | `Cards([…])` |
| `chooseSingleEntityForEffect` / `chooseEntitiesForEffect` mit Spielern | `ChooseOption` / `ChooseOptions` (**Erweiterung**) | `Number` / `Modes` |
| `arrangeForScry`, `arrangeForSurveil` | `Scry { cards, graveyard }` | `Arrange { top, bottom }` |
| `orderSimultaneousSa` (eigene Trigger), `orderMoveToZoneList` | `OrderTriggers { descriptions }` | `Order([…])` (erstes zuerst) |
| `tuckCardsViaMulligan` | `MulliganBottom { hand, count }` | `Cards([…])` |
| Kosten: Opfern, Abwerfen, Tappen, Zurückgeben (`getCostDecisionMaker` → `RemoteCostDecision`) | `ChooseCards { reason: SacrificeCost / DiscardCost / TapCost / ReturnCost }` | `Cards([…])` |
| `assignCombatDamage` | `AssignCombatDamage { attacker, amount, blockers, lethal, trample_to }` | `Damage([je Blocker…, Trampel])` |
| `chooseTargetsFor` bei „DividedAsYouChoose“ | `DivideAmount { source, amount, targets, min_each }` (**Erweiterung**) | `Damage([…])` |
| `payCostToPreventEffect` | `PayUnlessCost { cost }` (nur wenn bezahlbar) | `Yes` / `No` |
| Phyrexianisches Mana in `applyManaToCost` | `PayLifeForPhyrexian { symbol }` | `Yes` / `No` |
| `orderAndPlaySimultaneousSa`, `playTrigger` (eigene Trigger) | über `PlaySpellAbility.playSpellAbility` bzw. `playSpellAbilityNoStack` (wie Forges menschlicher Controller): Modi, Ziele, Kosten fragt der Spieler | – |
| Exil als Kosten, Delve, Convoke, Sonderkosten, Erschöpfen/Rekrutieren, Blocker-Reihenfolge, Kopien | – | **vorerst Forge-KI** |
| Gegner (nicht ferngesteuerte Sitzplätze) | – | Forge-KI |

**Ablauf beim Spielen** (`RemotePlayerController.playChosenSpellAbility`): Die Brücke ruft Forges
eigenen Ablauf für menschliche Spieler auf, `forge.game.player.PlaySpellAbility.playSpellAbility`
(im Jar enthalten). Er fragt den Controller der Reihe nach: Zusatzkosten/optionale Kosten
(`chooseOptionalCosts`) → Modi (`chooseModeForAbility`) → angesagter Typ → X-Wert
(`announceRequirements`) → Ziele (`setupTargets` → `chooseTargetsFor`) → Kosten
(`CostPayment` mit `getCostDecisionMaker`; Mana über `payManaCost` → `PlaySpellAbility.payManaCost`
→ `applyManaToCost` = `PayMana`; andere Kosten entscheidet vorerst die KI). Abbruch an jeder Stelle
→ Forge nimmt das Wirken zurück, die Brücke meldet `rejected`. Frei verteilten Schaden fragt
`chooseTargetsFor` nach der Zielwahl als `DivideAmount` (Forges GUI macht das ebenfalls bei der
Zielauswahl, `addDividedAllocation`).

**Mana bezahlen** (`applyManaToCost`): schwebendes Mana im Pool wird zuerst eingesetzt
(`ManaPool.payManaCostFromPool`). Danach `PayMana` mit allen jetzt aktivierbaren Manafähigkeiten
eigener bleibender Karten. `ActivateMana`: Kosten der Manafähigkeit zahlen (Tappen; entscheidet
`AiCostDecision`), Fähigkeit auflösen, Mana mit `ManaPool.payManaFromAbility` einsetzen – wie
Forges KI für eine einzelne Quelle. `AutoPay`: `ComputerUtilMana.payManaCost`.

**Kampf** (`RemotePlayerController.declareAttackers` / `declareBlockers`): Kandidaten sind die
Kreaturen, für die Forge `CombatUtil.canAttack` bzw. `canBlock` erlaubt; jede Antwort wird mit
`canAttack(karte, ziel)` / `canBlock(angreifer, blocker)` und danach `CombatUtil.validateAttackers`
bzw. `validateBlocks` geprüft (Angriffspflichten, „kann nur mit zwei Kreaturen geblockt werden“ …).
Ungültige Erklärungen werden zurückgenommen und mit Hinweis im `prompt` erneut gefragt (höchstens
4-mal, dann entscheidet die KI).

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
| `Decision::DivideAmount { player, source, amount, targets, min_each }` → `Choice::Damage(mengen)` | Menge beim Wirken auf die gewählten Ziele verteilen (CR 601.2d) |
| `Choice::Cancel` | Ziel-/X-Wahl beim Wirken abbrechen (Wirken wird zurückgenommen); bei anderen Entscheidungen „Engine entscheidet selbst“ |
| Hülle der Anfrage: `prompt`, `source`, `ability`, `cancelable`, `labels`, `cards` | Anzeigetexte (Forges Fragetext, Quellkarte, Fähigkeitstext, Anzeigetext je Listeneintrag) und Kartendaten (`cards`: Liste wie im `state`) der Karten, um die es geht – auch aus verdeckten Zonen, z. B. bei `ChooseCards` mit `Search` |

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
| `log` | neuer Eintrag im Spielprotokoll (Forges `GameLog`, chronologisch): `kind` (`GameLogEntryType`, z. B. `TURN`, `LAND`, `STACK_ADD`, `STACK_RESOLVE`, `ZONE_CHANGE`, `DAMAGE`, `LIFE`, `COMBAT`, `PHASE`, `MANA`, `MULLIGAN`), `text` (englisch, Objekte als „Name (id)“), `card` (Karten-ID der Quelle, falls bekannt), `turn`. Godot-Signal `ForgeBridge.log_entry` |
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

Karten: `id`, `name`, `type`, `tapped`, `cost`, `colors` (Farben als `WUBRG`-Buchstaben, leer =
farblos), `text` (Regeltext mit aktuellen Änderungen, Symbole in `{…}`-Schreibweise); bei Kreaturen
zusätzlich `power`, `toughness` (aktuell) und `base_power`, `base_toughness` (aufgedruckt), bei
Planeswalkern `loyalty`, bei Schlachten `defense` (im Spiel die aktuellen Marken, sonst aufgedruckt),
`damage` (nur > 0, erlittener Schaden); nur wenn zutreffend: `token: true` (Spielstein), `attached_to` (Karten-ID des Trägers bei Auren, Ausrüstung,
Befestigungen), `commander: true`, `emblem: true`, `room` (aktueller Raum eines Dungeons),
`face_down: true` (verdeckt, CR 708: Name leer, Werte der verdeckten Seite), `face` (bei
doppelseitigen, modalen und Flip-Karten: 0 = Vorderseite, 1 = Rückseite/umgedreht) mit
`other_face` `{name, type}` (nur offen), `counters` (Marken wie NeoForges `CardObj.counters`: Name → Anzahl, Namen =
Forges `CounterEnumType`, z. B. `{"P1P1": 2, "LOYALTY": 4}`).

Spieler zusätzlich: `mana_pool` = Menge schwebenden Manas je Farbe in der Reihenfolge W U B R G C;
`counters` = Spieler-Marken (CR 122.1) wie bei Karten, z. B. `{"POISON": 3, "ENERGY": 2}`;
`command` = Karten der Kommandozone (CR 408: Commander, Embleme, Dungeons, „The Ring“, „The
Monarch“/„The Initiative“ und andere Effekt-Karten); `blessing` (Stadtsegen), `ring_level` (wie oft
der Ring in Versuchung geführt hat), `ring_bearer` (Karten-ID, falls vorhanden).

Spiel zusätzlich: `monarch` / `initiative` = Sitzplatz (−1 = niemand), `day` = true (Tag) / false
(Nacht), fehlt solange weder Tag noch Nacht ist (CR 726).

Die Abfrage `state` liest Forges Daten aus einem anderen Thread als dem Spiel; ändert Forge
gleichzeitig etwas (`ConcurrentModificationException`), liest die Brücke bis zu zehnmal neu.

`combat` (laufender Kampf, sonst leere Listen): `attackers` = Liste `{card, target}` (`target` als
`Target`), `blocks` = Liste `[blocker, angreifer]`.

`stack` (CR 405, wie NeoForges `GameState.stack`): Liste der Einträge, **unterster zuerst, oberster
zuletzt**. Je Eintrag `id` (Forges Stack-ID), `kind` (`Spell` / `Ability` / `Trigger`), `source`
(Karten-ID der Quelle), `controller` (Sitzplatz), `text` (Forges Stack-Beschreibung),
`targets` (Ziele je Fähigkeit der Sub-Ability-Kette: Liste von Listen von `Target`) und bei Zaubern
`card` (Kartendaten der Karte in der Stack-Zone, wie in den Spielerzonen). Ein Zauber als Ziel
(z. B. Gegenzauber) erscheint als `{"card": id}` seiner Karte; eine Fähigkeit als Ziel als Karte
ihrer Quelle (NeoForges `Target` kennt keine Stack-Einträge).

```json
"stack": [
  { "id": 28, "kind": "Spell", "source": 52, "controller": 0, "targets": [[]],
    "text": "Flametongue Kavu - Creature 4 / 2", "card": { "id": 52, "name": "Flametongue Kavu", … } },
  { "id": 31, "kind": "Spell", "source": 92, "controller": 1, "targets": [[{"card": 52}]],
    "text": "Mana Leak (92) - Counter Flametongue Kavu (52) unless its controller pays {3}.", "card": { … } }
]
```

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
| `Priority` | Karten mit einer Aktion leuchten; Handkarte auf den Tisch ziehen → `PlayLand`/`Cast`; Klick auf eine Karte außerhalb der Hand → ihre `Activate`-Aktion (mehrere: Menü „Fähigkeit wählen“, nur UI); Hauptbutton / Leertaste → `Action::Pass` |
| `ChooseTargets` | gültige Karten leuchten, wählbare Spieler-Medaillons pulsieren golden; Klick wählt (Signal `ForgeTable.choice_option_clicked` → Auswahlfenster am rechten Rand); gewählte Ziele golden markiert, Pfeile Quelle → Ziel (`TargetArrows`) |
| `ChooseCards` / `MulliganBottom` / `Scry` | Kartenfächer-Overlay (`card_fan.rs`): Karten groß (Tischkarten bzw. temporäre Karten aus `cards`), Klick wählt bzw. wechselt die Reihe; „Fertig“ → `ForgeTable.submit_fan` → `Cards` / `Arrange` |
| `OrderTriggers` | Auswahlfenster, Optionen in Reihenfolge anklicken (nummeriert) → `Order` |
| `AssignCombatDamage` / `DivideAmount` | Auswahlfenster „Verteilen“: Zahlenfeld je Empfänger, vorbelegt, Summenanzeige, „OK“ nur bei richtiger Summe → `Damage` |
| `PayUnlessCost` / `PayLifeForPhyrexian` | Ja/Nein-Fenster („Zahlen“ / „Nicht zahlen“, „2 Leben zahlen“ / „Mit Mana zahlen“) |
| `PayMana` | Manaquellen leuchten; Klick = `Pay(ActivateMana)` (mehrere Manafähigkeiten: Menü); Fenster am rechten Rand mit offenen Kosten, „Automatisch bezahlen“ (`AutoPay`), „Abbrechen“ (`Cancel`); Manapool-Anzeige aus `mana_pool` |
| `DeclareAttackers` / `DeclareBlockers` | Kandidaten leuchten; Klick plant Angriff bzw. Block (rote / blaue Pfeile, Markierungen), Knopf „Angreifen (n)“ / „Blocken (n)“ im Fenster am rechten Rand sendet den Plan; `combat` aus dem Zustand zeigt den Kampf der KI als Pfeile |
| alle übrigen Entscheidungen | HUD-Auswahlfenster (`decision_view.rs`): Knöpfe, Mehrfachauswahl mit „Fertig“, Zahlenfeld, Suchfeld ab 13 Optionen (höchstens 40 Knöpfe, z. B. bei `NameCard` mit allen Kartennamen); Quellkarte golden markiert |
| `exile`, `attached_to`, `counters`, `token` | Exil-Stapel je Seite (`PlayerSeat.exile`); angelegte Karten versetzt unter dem Träger in dessen Zone; Marken und „Spielstein“ als Plaketten auf der Karte; Spieler-Marken unter dem Namen im HUD |
| `command`, `face`, `face_down`, `monarch`, `initiative`, `day`, `ring_level` | Kommandozone je Seite als Spalte (Spieler rechts unten, Gegner rechts oben); Seitenwechsel als Umdreh-Animation, verdeckte Karten mit Rückseitenmuster; Plaketten „Commander“, „Ringträger“, „Raum: …“, „↺ andere Seite“; Monarch, Initiative, Stadtsegen, Ring-Stufe unter dem Namen, Tag/Nacht neben der Rundenzahl |
| `log` | Spielprotokoll oben rechts im HUD: die letzten 7 Einträge ohne `PHASE`/`MANA`, IDs entfernt, Rundenbeginn golden |
| `stack` | Zone `Stack` in der Tischmitte: Zauber als ihre Karte, Fähigkeiten als Karte ihrer Quelle; violette Pfeile zu den Zielen; Liste im HUD (linker Rand) mit Quelle, Art, Text; Hauptbutton orange |
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
- Ziele per Klick (Karte, Medaillon, Mehrfachziele, Pfeile) und Aktivieren per Klick (inkl. Menü)
  getestet; Angriff und Block per Klick getestet (Angreifen mit 1–3 Kreaturen, „Nicht angreifen“,
  Blocken gegen 1–3 Angreifer, Kampfpfeile der KI aus `combat`).
- Mana bezahlen getestet: Land-Klicks bis bezahlt (z. B. {2}{R} → {2} → {1}), „Automatisch“,
  Kicker/Modi/X/Ziele über Forges menschlichen Ablauf.
- Kartenfächer getestet mit `testdecks/kartenwahl.dck`: `MulliganBottom` (7 Karten, 1 unter die
  Bibliothek), `Scry` (Magma Jet, eine Karte nach unten), `ChooseCards` mit `Search`
  (Wayfarer's Bauble, 20 Karten aus der Bibliothek).
- Kosten, Verteilen, phyrexianisches Mana und Trigger-Ziele getestet mit `testdecks/kosten.dck`
  gegen `testdecks/gegner_counter.dck`: Gut Shot (2 Leben), Forked Bolt (1+1), Cathartic Reunion
  (2 Karten abwerfen als Kosten), Flametongue Kavu (Trigger-Ziel); Kampfschaden-Verteilung mit
  künstlicher Anfrage (Vorbelegung 2/3/1 bei Trampeln, Summenprüfung).
- Noch nicht ferngesteuert: Exil als Kosten, Delve, Convoke, Sonderkosten,
  ausgelöster Fähigkeiten.
- Stack getestet mit `testdecks/kosten.dck` gegen `testdecks/gegner_counter.dck`: Zauber beider
  Seiten in der Stack-Zone, Mana Leak auf Flametongue Kavu (2 Einträge, Ziel = Zauber im Stack);
  ausgelöste Fähigkeit (Trigger von Flametongue Kavu mit Ziel) als Karte der Quelle, Priorität mit
  belegtem Stack. Optik (Liste im HUD, Pfeile) noch nicht im Fenster geprüft.
- Exil, Marken, Spielsteine und angelegte Karten getestet mit einem Testdeck (Dragon Fodder,
  Bonesplitter, Chandra, Torch of Defiance, Magma Spray) gegen `testdecks/gegner.dck` (Rancor):
  Spielsteine, `LOYALTY`, Exil beider Seiten, Rancor an der Gegnerkreatur, Ausrüstung am Träger.
  Spieler-Marken nur über den Unit-Test geprüft.
- Kartenbild aus den Kartendaten getestet (headless und Fenster-Nahaufnahme): Regeltext, Symbole
  (↷, Ⓡ, ①), Typzeilen, Planeswalker-Loyalität (auf der Hand aufgedruckt, im Spiel aktuell),
  Schriftanpassung bei langem Text (Chandra, Bonesplitter).
- Kommandozone, Seiten und Kennzeichen getestet mit einem Testdeck (Brutal Cathar, Soul Summons,
  Seasoned Dungeoneer, Palace Jailer, Call of the Ring) gegen `gegner.dck`: The Ring, The Monarch,
  The Initiative, Undercity mit Raum, Effekt von Palace Jailer in der Kommandozone; Manifest
  (verdeckt 2/2); Tag/Nacht und Transformieren Brutal Cathar ↔ Moonrage Brute.
- Spielprotokoll getestet: 440 Einträge in einer Partie, chronologisch (Mulligan, Runden, Länder,
  Zauber mit Zielen, Auflösen, Zonenwechsel), Anzeige oben rechts im Fenster geprüft.
- Forges eigene Fragetexte sind englisch (Forge-Spracheinstellung); die Texte der Brücke deutsch.
