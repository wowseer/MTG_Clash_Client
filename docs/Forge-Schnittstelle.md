# Forge-Schnittstelle

MTG Clash kann die Regel-Engine von **Forge** (Java) nutzen, auf zwei Arten mit demselben
Protokoll:

**Eigenständig:** Godot startet Forge als eigenen Prozess und spricht über stdin/stdout mit ihm.

```
Godot (Rust)                                  Java-Prozess (forge-bridge/)
ForgeBridge-Node  ── JSON-Zeile auf stdin ──▶  clash.forgebridge.BridgeMain → ForgeBridge
  Signale        ◀── JSON-Zeile auf stdout ──    └─ Forge (forge-game, forge-ai, …)
                       stderr ──▶ user://forge-bridge.log
```

**Eingebettet in NeoForge:** NeoForge (JavaFX-Oberfläche für Forge) startet die Partie und
Godot als Spieltisch. Godot verbindet sich über einen lokalen Socket.

```
NeoForge (JVM)                                              Godot (gestartet von NeoForge)
NeoGame.play ── ClashTable.open(): Port öffnen, Godot starten ──▶ godot --path … -- --clash-port=N
   │            ForgeBridge.attach(Socket)  ◀── JSON-Zeilen über TCP 127.0.0.1:N ──▶ ForgeBridge-Node
   └─ HostedMatch: menschlicher Sitz = RemoteLobbyPlayer (RemotePlayerController)
```

- NeoForge-Seite: `forge.neo.clash.ClashTable` und ein kleiner Eingriff in `forge.neo.match.NeoGame`
  (NeoForge-Repo). Der Brücken-Code wird mit `tools/sync_neoforge.sh` nach
  `forge-gui-neo/src/main/java/clash/forgebridge/` kopiert; maßgeblich ist die Fassung hier.
- Eingeschaltet über `neo.clash=true` (oder Umgebung `NEOFORGE_CLASH=true`); Godot-Programm
  `neo.clash.godot` / `CLASH_GODOT`, Projektordner `neo.clash.project` / `CLASH_PROJECT`.
  Befehl zum Testen: `NeoMain clash [--deck=NAME] [--opponents=N] [--timeout=SEK]` bzw. `tools/neoforge_clash.sh`.
- Ablauf: `ready` (mit `embedded: true`) → `match_started` (ohne Zustand; der Zustand kommt mit der
  ersten Anfrage) → Anfragen wie gewohnt → `game_over` (aus `GameEventGameOutcome`) → NeoForge
  schließt den Socket, Godot beendet sich. `start_match` ist eingebettet nicht erlaubt.
- Schließt der Spieler das Godot-Fenster während der Partie, gibt sein Sitz auf.
- NeoMatchUI läuft unsichtbar als Zuschauer mit (wie NeoForges Modus OBSERVE ohne Tisch);
  Hot-Seat-Rivalen werden mit dem 3D-Tisch als KI gespielt.

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

Seit Protokoll 3 spricht die Brücke das **Protokoll-Modell** (`Decision` / `Choice`, Vorbild:
`forge-engine/src/decision.rs` aus forge-rules-rs, `~/Entwicklung/Java/forge/rust/` – nicht NeoForge). Die Java-Brücke übersetzt Forges
`PlayerController`-Aufrufe in `Decision`s (`Decisions.java`, `RemotePlayerController.java`).

| Forge-Methode(n) | `Decision` | Antwort (`Choice`) |
|---|---|---|
| `chooseSpellAbilityToPlay` | `Priority { actions, elsewhere }` (`elsewhere`: spielbare Karten außerhalb von Hand und Spielfeld mit Zone, siehe Erweiterungen); ohne spielbare Aktion passt die Brücke selbst, außer in der eigenen zweiten Hauptphase und Endphase mit leerem Stack (dann `Priority` nur mit `Pass`, damit der Spieler dort anhält) (zusätzlich `ActivateMana(karte, i)` je Manafähigkeit, sobald es etwas zu spielen gibt, und `UndoMana(karte)` je Karte, deren Manafähigkeit Forge noch zurücknehmen kann) | `Action(…)`; `Pass`; `Action(ActivateMana)` → Fähigkeit sofort ausführen (Mana in den Pool), erneut fragen; `Action(UndoMana)` → `MagicStack.filterUndoStackByHost` + `sa.undo()` (Karte enttappt, Mana aus dem Pool), erneut fragen |
| `mulliganKeepHand` | `Mulligan { mulligans_taken, first_player }` | `Keep` / `Mulligan` |
| `chooseModeForAbility` | `ChooseModes { modes, min, max, can_repeat }`; bei Modi mit eigenen Zusatzkosten (Spree, Tiered: Parameter `ModeCost`) zusätzlich `costs` (je Modus, z. B. `["{1}", "{3}{W}{W}"]`), `base_cost` (`"{W}"`), `keyword` (`"Spree"` / `"Tiered"`) | `Modes([…])` |
| `chooseTargetsFor` (über `setupTargets`) | `ChooseTargets { source, description, candidates, min, max }` | `Targets([…])` / `Cancel` |
| `announceRequirements` (X ab 0) | `ChooseX { max }` (UI: 3D-Knöpfe am Stack, `ForgeTable.show_choose_x`) | `Number(n)` / `Cancel` |
| `chooseColor`, `chooseColors` mit genau einer Farbe | `ChooseColor { options }` | `Color("R")` |
| `chooseCardName` | `NameCard { options }` (alle erlaubten Namen) | `Number(index)` |
| `chooseSomeType` | `ChooseType { options }` | `Number(index)` |
| `confirmAction`, `confirmReplacementEffect`, `confirmStaticApplication` | `Confirm { prompt }` | `Yes` / `No` |
| `confirmTrigger` | `OptionalTrigger { description }` | `Yes` / `No` |
| Startspieler, Abstimmung, Marken-Typ, Schutz, Schlüsselwort, Sektor, Würfel, Zahl aus Liste, Ja/Nein mit eigenen Beschriftungen (Kopf/Zahl, Tappen/Enttappen …), Farbe inkl. farblos | `ChooseOption` (**Erweiterung**) | `Number(index)` |
| `chooseSingleReplacementEffect` (CR 616.1) | `ChooseOption { options: [{text, card}], purpose: "ReplacementEffect" }`: je Ersatzeffekt sein Text und die Karten-ID seiner Quelle, deren Daten in der Hülle `cards` | `Number(index)` |
| `chooseOptionalCosts`, `chooseColors` (mehrere), `chooseDiceToReroll` | `ChooseOptions` (**Erweiterung**) | `Modes([…])` |
| `addKeywordCost` (Schlüsselwort-Zusatzkosten beim Wirken, CR 601.2b: Casualty, Conspire, Offspring, Tappen bei Harmonize) | `ChooseOptions { options: [„Casualty“] (Schlüsselwort ohne Zahl; reine Manakosten mit Angabe, z. B. „Offspring {2}“), min: 0, max: 1, purpose: "OptionalCost" }` (wie Kicker; ohne Überschreiben fragte Forge über `chooseNumberForKeywordCost` mit Höchstwert 1) | `Modes([0])` = zahlen, `Modes([])` = normal |
| `chooseNumber`, `chooseNumberForCostReduction`, `chooseNumberForKeywordCost` (Multikicker, Replicate, Squad, „beliebig viel Mana zahlen“), angesagte Werte ≠ X | `ChooseNumber` (**Erweiterung**) | `Number(n)` |
| `declareAttackers` | `DeclareAttackers { candidates, defenders }` | `Attackers([[karte, ziel], …])`; leer = kein Angriff |
| `declareBlockers` | `DeclareBlockers { candidates, attackers }` | `Blocks([[blocker, angreifer], …])`; leer = nicht blocken |
| `applyManaToCost` (über `payManaCost`) | `PayMana { remaining, options: [ActivateMana(karte, i), AutoPay, Cancel] }` | `Pay(ActivateMana(…))` → Quelle tappen, Mana einsetzen, erneut fragen bis bezahlt; `Pay(AutoPay)`; `Pay(Cancel)` = Wirken zurücknehmen |
| `chooseCardsForEffect`, `chooseCardsForZoneChange`, `chooseSingleCardForZoneChange`, `chooseCardsToDiscardFrom`, `chooseCardsToDiscardToMaximumHandSize`, `choosePermanentsToSacrifice`, `choosePermanentsToDestroy`, `chooseCardsToRevealFromHand`, `chooseSingleEntityForEffect` / `chooseEntitiesForEffect` (nur Karten) | `ChooseCards { reason, options, min, max }` (`reason`: `Choose`, `Search`, `Discard`, `DiscardToHandSize`, `Sacrifice`) | `Cards([…])` |
| `chooseSingleEntityForEffect` / `chooseEntitiesForEffect` mit Spielern (z. B. „Choose a player“) | `ChooseTargets { source, description, candidates, min, max, purpose: "Choose" }` (**Erweiterung**: kein Ziel (CR 115.1), die UI zeigt Knöpfe je Spieler; das Basismodell kennt keine Spielerwahl für Effekte; `source` = −1, wenn es keine Quelle gibt) | `Targets([…])` |
| `chooseNewTargetsFor` (Umleiten, Kopien mit „you may choose new targets“) | bei optionaler Änderung zuerst `Confirm` („Neue Ziele wählen für …?“), dann `ChooseTargets` (mit Forges Zusatzfilter) | `Yes`/`No`, `Targets([…])` |
| `exertAttackers`, `enlistAttackers` (CR 701.43, 702.154) | `ChooseCards { reason: "Choose", options: Angreifer, min: 0 }` (Tisch-Modus) | `Cards([…])` |
| `chooseSingleSpellForEffect` / `chooseSpellAbilitiesForEffect` | `ChooseOption` (`purpose: "SpellForEffect"`) / `ChooseOptions` (`purpose: "SpellsForEffect"`, `min` = `max` = Anzahl) mit `options[].card` = Karte der Fähigkeit, Text ohne Kartennamen | `Number` / `Modes` |
| `chooseSingleStaticAbility` | `ChooseOption { purpose: "StaticAbility" }` mit `options[].card` = Quellkarte | `Number` |
| `chooseSingleCardFace` / `chooseSingleCardState` | `ChooseOption { purpose: "CardFace" }`, je Seite eigene Kartendaten in der Hülle `cards` (IDs ab 1 900 000 000, nicht im Spiel) | `Number` |
| `orderAndPlaySimultaneousSa` mit Kopien | Kopie auf den Stack wie Forges menschlicher Controller; neue Ziele über `chooseNewTargetsFor` | – |
| `playSaFromPlayEffect` (Kaskade, „you may cast“, Wunder …) | über Forges menschlichen Ablauf (Ziele, Modi, Kosten fragt der Spieler) | – |
| `payCombatCost` (Angriffs-/Blockkosten, CR 508.1h) | `Confirm`, dann Bezahlen wie beim Wirken | `Yes`/`No` |
| `confirmPayment` | `Confirm` | `Yes`/`No` |
| `chooseCardsPile` (Fact or Fiction u. a.) | `ChooseOption { purpose: "Piles" }`, je Stapel `cards` (IDs der offenen Karten, Daten in der Hülle `cards`) bzw. `hidden` (Anzahl verdeckter); Forges Parameter `FaceDown` („False“, „One“ = Stapel 1 verdeckt, „True“) | `Number` |
| `orderCosts` | `OrderTriggers` mit Kostentexten | `Order` |
| `chooseCardsForCost` (Kosten während des Auflösens) | `ChooseCards` mit Grund nach Kostenart (`DiscardCost`, `SacrificeCost`, `ExileCost`, `TapCost`, `ReturnCost`, sonst `Choose`) | `Cards` |
| `chooseCardsToDiscardUnlessType` | `ChooseCards { reason: "Discard", min: 1, max: n }`: n Karten oder eine Karte des Typs | `Cards` |
| `chooseSaToActivateFromOpeningHand` (Leylines …) | `ChooseOptions { purpose: "OpeningHand", min: 0 }` mit `options[].card` | `Modes` |
| `getAbilityToPlay` | `ChooseOption { purpose: "AbilityToPlay" }`, jede Option mit `card` = die Karte, abbrechbar | `Number` |
| `chooseManaFromPool` (mehrere passende Mana im Pool) | `ChooseOption` mit Mana und Quelle (**Erweiterung**: das Basismodell (`PayAction`) kennt keine Pool-Zahlung) | `Number` |
| `chooseCardsForSplice` (CR 702.47) | `ChooseCards { reason: "Choose", min: 0 }` aus der Hand | `Cards` |
| `chooseCardsForEffectMultiple` | je Gruppe ein `ChooseCards { min: 0/1, max: 1 }` | `Cards` |
| `chooseTarget` (welches Ziel geändert wird) | `ChooseOption` „Zauber → Ziel“ | `Number` |
| `reveal` (Karten werden aufgedeckt, CR 701.16) | `ChooseCards { reason: "Reveal", options, min: 0, max: 0 }` (**Erweiterung**: reine Anzeige; „Fertig“ = gesehen); eigene Handkarten werden nicht gezeigt | `Cards([])` |
| `notifyOfValue` (z. B. „wählt Rot“) | Nachricht `log` mit `kind: "INFORMATION"` (keine Entscheidung) | – |
| `divideShield` (Schutzschild auf mehrere betroffene Ziele, CR 615.7) | `DivideAmount { amount: Schildmenge (höchstens Gesamtschaden), targets, min_each: 0 }` | `Damage([…])` |
| `chooseRollToSwap`, `chooseRollSwapValue` (Würfelwurf tauschen) | `ChooseOption` | `Number` |
| `specifyManaCombo` („n Mana in beliebiger Farbkombination“) | je Mana ein `ChooseColor` | `Color` |
| `playSpellAbilityNoStack` | über Forges menschlichen Ablauf | – |
| `payCostDuringRoll` (Kosten, um einen Wurf zu ändern) | `Confirm`, dann Bezahlen wie beim Wirken | `Yes`/`No` |
| Außerhalb des Umfangs (bleiben bei der KI) | Ante, Contraptions, Planechase-Würfel, Assist (Mehrspieler), Varianten-Starthand, Sideboard, Gebote | – |
| `chooseCardsToDelve` (CR 702.66) | `ChooseCards { reason: "Delve", options: Friedhof, min: 0, max }` | `Cards([…])` |
| `chooseCardsForConvokeOrImprovise` (CR 702.51, 702.126, Waterbend) | `ChooseCards { reason: "Convoke" / "Improvise" / "Choose", options: ungetappte Kreaturen/Artefakte, min: 0, max }`; welches Mana jede Karte bezahlt, legt die Brücke fest (farbiges Symbol in einer Farbe der Kreatur, sonst generisch) | `Cards([…])` |
| `orderBlockers`, `orderBlocker`, `orderAttackers` (Schadens-Reihenfolge, nur mit `order_combatants`) | `OrderTriggers { descriptions }` mit Kartennamen (wie `orderMoveToZoneList`) | `Order([…])` |
| `arrangeForScry`, `arrangeForSurveil` | `Scry { cards, graveyard }` | `Arrange { top, bottom }` |
| `orderSimultaneousSa` (eigene Trigger), `orderMoveToZoneList` | `OrderTriggers { descriptions, sources }` (`sources` = Karten-ID der Quelle je Eintrag, −1 = keine) | `Order([…])` (erstes zuerst) |
| `tuckCardsViaMulligan` | `MulliganBottom { hand, count }` | `Cards([…])` |
| Kosten: Opfern, Abwerfen, Tappen, Zurückgeben, Exilieren (`getCostDecisionMaker` → `RemoteCostDecision`) | `ChooseCards { reason: SacrificeCost / DiscardCost / TapCost / ReturnCost / ExileCost }`; Exil-Sonderfälle (Karte selbst, alle, oberste Friedhofskarten, Mindest-Manawert, Bibliothek, „aus demselben Friedhof“) entscheidet die Engine; Fahrzeuge besetzen (Crew, Gesamtstärke ≥ N) als `TapCost` mit `min: 1`, zu wenig Stärke → erneut fragen | `Cards([…])` |
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

### Erweiterungen gegenüber dem Basismodell

Diese Teile gibt es im Vorbild (`decision.rs` aus forge-rules-rs) (noch) nicht:

| Erweiterung | Zweck |
|---|---|
| `Decision::ChooseOption { player, prompt, options: [{text, color?}] }` → `Choice::Number(index)` | allgemeine Einfachauswahl (siehe Tabelle oben); `color` = `W U B R G` oder `C` (farblos) |
| `Decision::ChooseOptions { player, prompt, options, min, max }` → `Choice::Modes(indizes)` | allgemeine Mehrfachauswahl |
| `Decision::ChooseNumber { player, prompt, min, max }` → `Choice::Number(n)` | Zahl mit Untergrenze ≠ 0 bzw. außerhalb von X |
| `Decision::DivideAmount { player, source, amount, targets, min_each }` → `Choice::Damage(mengen)` | Menge beim Wirken auf die gewählten Ziele verteilen (CR 601.2d) |
| Ziele auf dem Stack in `ChooseTargets` | als `{"card": id}` der Quelle des Zaubers bzw. der Fähigkeit (das Basismodell (`Target`) hat keine Stack-Einträge; nachziehen: `Target::Stack(id)`) |
| `ChooseOption` für Mana aus dem Pool (`chooseManaFromPool`) | im Basismodell fehlt `PayAction::UsePool(Farbe)` |
| `Action::UndoMana { card }` in `Priority.actions` | vorab aktivierte Manafähigkeit der Karte zurücknehmen, solange ihr Mana noch nicht ausgegeben ist (Forges Rückgängig-Stapel für Manafähigkeiten); das Basismodell kennt kein Rückgängig |
| `OrderTriggers.sources` | Karten-ID der Quelle je Eintrag (−1 = keine), damit die UI Kartenkopien statt Texten zeigt |
| `Action::ActivateMana { card, index }` in `Priority.actions` | Manafähigkeit mit Priorität aktivieren (CR 605.3a), z. B. Länder vorab tappen; `index` wie bei `PayAction::ActivateMana`. Das Basismodell kennt Manafähigkeiten nur beim Bezahlen |
| `ChooseTargets.purpose` (`"Choose"` bei `chooseSingleEntityForEffect` / `chooseEntitiesForEffect`) | Wahl für einen Effekt ohne Ziel („choose a player“, CR 115.1); leer = echtes Ziel eines Zaubers oder einer Fähigkeit (CR 115), das die UI nur per Pfeil wählen lässt |
| `Priority.elsewhere` (`[{card, zone, cost}]`, `cost` = Manakosten der Spielweise aus dieser Zone, z. B. Flashback, Daten der Karten in der Hülle `cards`) | Karten, die gerade aus einer anderen Zone als Hand und Spielfeld spielbar sind (Friedhof, Exil, Kommandozone, Sideboard/Companion, Bibliothek); die UI zeigt sie zusätzlich im Handfächer |
| `ChooseOption.purpose`, `ChooseOptions.purpose`, `options[].card`, `options[].key` | wofür die Wahl ist (`"ReplacementEffect"`, `"StaticAbility"`, `"CardFace"`, `"SpellForEffect"`, `"SpellsForEffect"`, `"AbilityToPlay"`, `"CounterType"`, `"Keyword"`, `"Protection"` – Farben mit `color`, dann Farbrad –, `"NumberList"`, `"Vote"`, `"ManaPool"` bei `chooseManaFromPool`); `key` = Engine-Schlüssel einer Option (Markenart, z. B. `P1P1`) und zu welcher Karte jede Option gehört; mit Karte je Option zeigt die UI eine Kartenkopie je Möglichkeit statt Knöpfen |
| Stack-Eintrag `modes` | Texte der gewählten Modi eines modalen Zaubers (Forge hängt sie als Unterfähigkeiten mit SVar `CharmOrder` an) |
| `ChooseModes.costs`, `base_cost`, `keyword` | Modi mit eigenen Zusatzkosten (Spree CR 702.172, Tiered): Kosten je Modus, Kosten ohne Modi und Schlüsselwort, damit die UI „+ {Kosten}“ und „Bezahlen {Gesamtkosten}“ zeigt; ohne Modus-Kosten fehlen die Felder |
| `ChooseOptions.purpose` (`"OptionalCost"` bei `chooseOptionalCosts` und `addKeywordCost`) | wofür die Mehrfachwahl ist, damit die UI optionale Kosten (Kicker …) als eigene Bühne zeigt; sonst leer |
| `PayAction::UsePool { color }` (`W U B R G C`) in `PayMana.options` | schwebendes Mana einer Farbe einsetzen; die Brücke gibt Pool-Mana nicht mehr automatisch aus (nur über `UsePool` oder `AutoPay`) |
| `ChooseCards` mit `reason: "Reveal"` und `max: 0` | Anzeige aufgedeckter Karten; im Basismodell fehlt ein `Decision::Reveal { cards }` bzw. Ereignis |
| `Decision::ChooseStartingPlayer { player, players }` → `Choice::Number(index)` | wer beginnt die Partie (CR 103.1); `players` = Sitzplätze, `labels` = Spielernamen. Die UI zeigt dafür 3D-Knöpfe statt eines Fensters |
| `Decision::Mulligan` + Feld `first_player` | Sitzplatz des Startspielers (−1 = noch unbekannt), damit die Mulligan-Bühne „Du beginnst“ / „X beginnt“ zeigen kann; das Basismodell kennt nur `mulligans_taken` |
| `ChooseTargets` auch für Spieler-/Kartenwahl durch Effekte (`chooseSingleEntityForEffect` mit Spielern) | Das Basismodell hat dafür keine Entscheidung; die UI behandelt sie wie Ziele (Klick auf Medaillon/Karte) |
| `Choice::Cancel` | Ziel-/X-Wahl beim Wirken abbrechen (Wirken wird zurückgenommen); bei anderen Entscheidungen „Engine entscheidet selbst“ |
| Hülle der Anfrage: `prompt`, `source`, `ability`, `cancelable`, `labels`, `cards` | Anzeigetexte (Forges Fragetext, Quellkarte, Fähigkeitstext, Anzeigetext je Listeneintrag) und Kartendaten (`cards`: Liste wie im `state`) der Karten, um die es geht – auch aus verdeckten Zonen, z. B. bei `ChooseCards` mit `Search` |

## Protokoll (Version 3)

JSON Lines, UTF-8: genau eine JSON-Nachricht pro Zeile.

### Befehle (Godot → Forge, stdin)

| `cmd` | Felder | Wirkung |
|---|---|---|
| `ping` | `nonce` (optional) | Antwort `pong` mit gleichem `nonce` |
| `start_match` | `decks`: Liste von `.dck`-Pfaden oder Forge-Decknamen (mind. 2) · `remote`: Liste bool je Sitzplatz (true = von MTG Clash gesteuert) · `names`: Spielernamen · `seed` (optional) · `starting_life` (optional) · `game_type` (optional, `"Commander"` = Commander-Partie, Decks mit Abschnitt `[Commander]`, Startleben 40) · `ai_pace` (optional, Zahl, Standard 1: Faktor für die Pausen der Forge-KI nach ihren Aktionen – Land 0,7 s, Zauber/Fähigkeit 1 s, Angreifer 0,9 s, Blocker 0,7 s, Phasenwechsel im KI-Zug 0,25 s; 0 = ohne Pausen; `AiPace.java`) · `order_combatants` (optional, bool: Forges alte Regel „Kampfteilnehmer ordnen“, Schadens-Reihenfolge vor der CR-Änderung 2024) | startet eine Partie (Constructed, 1 Spiel) |
| `answer` | `id`: Anfrage-ID · `choice`: Antwort | beantwortet eine `request` |
| `state` | `viewer`: Sitzplatz mit offener Hand (-1 = keiner) | Antwort `state` |
| `debug_card_names` | – | Debug-Fenster: Antwort `debug_card_names` mit allen Kartennamen |
| `debug_add_card` | `seat`, `name` | Debug: Karte neu erzeugen und dem Spieler auf die Hand legen |
| `debug_infinite_mana` | `seat`, `on` | Debug: Manapool des Spielers bei jedem Phasenwechsel und Zauber auf 50 je Manaart (WUBRGC) auffüllen bzw. das abschalten |
| `quit` | – | beendet den Prozess |

### Nachrichten (Forge → Godot, stdout)

| `type` | Felder |
|---|---|
| `ready` | `protocol`, `embedded` (true = in NeoForge eingebettet), `working_dir` – Forge ist geladen (ca. 5 s nach Start) |
| `pong` | `nonce` |
| `match_started` | `state` |
| `event` | `event` (Forge-Klassenname, z. B. `GameEventCardChangeZone`, `GameEventTurnPhase`), `text`, `turn`, `phase`; zusätzliche Felder: `GameEventCardTapped` → `card`, `tapped`; `GameEventManaPool` → `player`, `mode` (`Added` …), `colors` (WUBRGC); `GameEventCardDamaged` → `card`, `source`, `amount`; `GameEventPlayerDamaged` → `player`, `source`, `amount`, `combat` (für Mana- und Kampfeffekte der UI) |
| `log` | neuer Eintrag im Spielprotokoll (Forges `GameLog`, chronologisch): `kind` (`GameLogEntryType`, z. B. `TURN`, `LAND`, `STACK_ADD`, `STACK_RESOLVE`, `ZONE_CHANGE`, `DAMAGE`, `LIFE`, `COMBAT`, `PHASE`, `MANA`, `MULLIGAN`), `text` (englisch, Objekte als „Name (id)“), `card` (Karten-ID der Quelle, falls bekannt), `turn`. Godot-Signal `ForgeBridge.log_entry` |
| `request` | `id`, `player`, `decision` (`Decision`), `prompt`, `source` `{id, name}`, `ability`, `cancelable`, `labels`, `state` – siehe unten |
| `rejected` | `player`, `option` (Index in `actions` ohne `Pass`), `action`, `reason` – gewählte Aktion nicht ausführbar |
| `state` | `state` |
| `game_over` | `winner` (Name oder null), `draw`, `reason` |
| `debug_card_names` | `names`: alle Kartennamen der Kartendatenbank, sortiert |
| `chat` | `from`, `text` |
| `error` | `code` (`init_failed`, `bad_command`, `unknown_command`, `deck_not_found`, `match_running`, `no_match`, `unknown_request`, `game_crashed`, `unknown_card`, `bad_seat`, `debug_failed`), `message` |

Der Spiel-Thread wartet bei jeder `request`, bis die passende `answer` kommt. **Debug-Befehle**
(`debug_add_card`, `debug_infinite_mana`) ändern das Spiel im Spiel-Thread, während er so wartet
(vorher nicht – solange die KI nachdenkt, warten sie). Danach schickt die Brücke `state`; eine
offene `Priority`-Anfrage beantwortet sie selbst mit `{"type":"Refresh"}` und stellt sie neu (mit
den neuen Karten bzw. dem neuen Mana) – die UI bekommt also eine neue `request` mit neuer `id`.

**`request`** (Protokoll v3)

```json
{"type": "request", "id": 7, "player": 0,
 "decision": {"type": "ChooseTargets", "player": 0, "source": 34, "description": "Ziel wählen: Select any target",
              "candidates": [{"player": 0}, {"player": 1}, {"card": 83}], "min": 1, "max": 1},
 "prompt": "Ziel wählen: Select any target", "source": {"id": 34, "name": "Lightning Bolt"},
 "ability": "Lightning Bolt deals 3 damage to any target.", "cancelable": true,
 "labels": ["Spieler (du)", "Forge-KI", "Llanowar Elves (Forge-KI)"], "state": {…}}
```

JSON-Form der Typen: Variante als `"type"`, Felder in `snake_case`; `Target` =
`{"card": id}` / `{"player": sitz}`; `Action` = `{"type": "Cast", "card": 34}` bzw. mit `"index"`
(`CastAlt`, `PlayLandAlt`, `Activate`, `CastWithAltCost`); `Color` = `"W" "U" "B" "R" "G"`;
`ChooseModes.modes` = Liste von `[index, text]`. `labels` = Anzeigetext je Eintrag der Hauptliste
(`actions` bei Priority, `candidates` bei ChooseTargets, `modes` bei ChooseModes).

**Antwort** `{"cmd": "answer", "id": 7, "choice": {…}}` mit einer `Choice`:
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

Karten: `id`, `name`, `type`, `tapped`, `cost`, `keywords` (Liste `{title, reminder}`: Schlüsselwörter mit Erinnerungstext der Engine, für die Kartenansicht), `colors` (Farben als `WUBRG`-Buchstaben, leer =
farblos), `text` (Regeltext mit aktuellen Änderungen, Symbole in `{…}`-Schreibweise); bei Kreaturen
zusätzlich `power`, `toughness` (aktuell) und `base_power`, `base_toughness` (aufgedruckt), bei
Planeswalkern `loyalty`, bei Schlachten `defense` (im Spiel die aktuellen Marken, sonst aufgedruckt),
`damage` (nur > 0, erlittener Schaden); nur wenn zutreffend: `token: true` (Spielstein), `attached_to` (Karten-ID des Trägers bei Auren, Ausrüstung,
Befestigungen), `commander: true`, `emblem: true`, `room` (aktueller Raum eines Dungeons),
`sick` (Kreatur noch nicht einsatzbereit, CR 302.6), `attacking` (greift im laufenden Kampf an), `class_level` (Klassen, CR 716), `unlocked_doors` (Räume: Namen der offenen Türen), `image` (absoluter Pfad des Kartenbilds, JPG von Scryfall im Forge-Bildordner `pics/cards/<SET>/<Name>.fullborder.jpg` oder im Bildordner von NeoForge; fehlt es, lädt die Brücke es im Hintergrund von Scryfall – `CardImages.java` – und schickt nach dem letzten fertigen Bild einer Reihe von sich aus eine Nachricht `state` mit den Pfaden; nie bei verdeckten Karten und Spielsteinen; für das Bild nimmt die Brücke einen Druck mit normalem Rahmen, siehe DOKUMENTATION.md), `art` (nur bei Karten ohne Druck mit normalem Rahmen: Pfad des Bildausschnitts `<SET>/<Name>.artcrop.jpg`, Scryfall `art_crop`),
`face_down: true` (verdeckt, CR 708: Name leer, Werte der verdeckten Seite), `face` (bei
doppelseitigen, modalen und Flip-Karten: 0 = Vorderseite, 1 = Rückseite/umgedreht) mit
`other_face` `{name, type}` (nur offen), `counters` (Marken wie `CardObj.counters` in forge-rules-rs: Name → Anzahl, Namen =
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

`stack` (CR 405, wie `GameState.stack` in forge-rules-rs): Liste der Einträge, **unterster zuerst, oberster
zuletzt**. Je Eintrag `id` (Forges Stack-ID), `kind` (`Spell` / `Ability` / `Trigger`), `source`
(Karten-ID der Quelle), `controller` (Sitzplatz), `text` (Forges Stack-Beschreibung),
`targets` (Ziele je Fähigkeit der Sub-Ability-Kette: Liste von Listen von `Target`) und bei Zaubern
`card` (Kartendaten der Karte in der Stack-Zone, wie in den Spielerzonen). Ein Zauber als Ziel
(z. B. Gegenzauber) erscheint als `{"card": id}` seiner Karte; eine Fähigkeit als Ziel als Karte
ihrer Quelle (das Basismodell (`Target`) kennt keine Stack-Einträge).
Zuoberst stehen Karten, die **gerade gewirkt werden** (CR 601.2a: schon in der Stack-Zone, Ziele und
Kosten noch offen): `kind` `"Casting"`, `id` = minus Karten-ID, `text` leer, `targets` leer, `card`
wie bei Zaubern. So bleibt die Karte während der Zielwahl sichtbar (Startpunkt des Zielpfeils).

```json
"stack": [
  { "id": 28, "kind": "Spell", "source": 52, "controller": 0, "targets": [[]],
    "text": "Flametongue Kavu - Creature 4 / 2", "card": { "id": 52, "name": "Flametongue Kavu", … } },
  { "id": 31, "kind": "Spell", "source": 92, "controller": 1, "targets": [[{"card": 52}]],
    "text": "Mana Leak (92) - Counter Flametongue Kavu (52) unless its controller pays {3}.", "card": { … } }
]
```

Sitzplätze (`players`, `seat`, `controller`, `player` in Zielen und Ereignissen) sind die Position in Forges `getRegisteredPlayers()` und bleiben fest, auch wenn ein Spieler ausscheidet.

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
| `Priority` | Karten mit einer Aktion leuchten; Handkarte auf den Tisch ziehen → `PlayLand`/`Cast`, bei mehreren Aktionen der Karte (ohne Manafähigkeiten) 3D-Bühne „Wähle eins“ mit einer Kartenkopie je Aktion; Klick auf eine Karte außerhalb der Hand → ihre `Activate`- bzw. `ActivateMana`-Aktion (mehrere: „Wähle eins“-Bühne, Manafähigkeiten beim Bezahlen: Menü; reine Manaquellen leuchten nicht); Hauptbutton / Leertaste → `Action::Pass` |
| `ChooseTargets` | genau ein Kandidat bei `min` = `max` = 1: sofort `Targets([…])` (keine echte Wahl). Sonst kein Fenster: von der Quelle (Karte auf dem Stack bzw. Karte der Fähigkeit) hängt ein goldener Pfeil an der Maus und rastet auf gültigen Zielen ein (Karten leuchten, Medaillons pulsieren golden); Klick bzw. Pfeil ziehen und loslassen wählt (`ForgeTable.pick_candidate`), erneut = abwählen. Sind `max` Ziele gewählt, geht `Targets` sofort an Forge; sonst Phasen-Knopf „Ziele bestätigen (n) »“ (ab `min`) bzw. „Abbrechen“ (`Cancel`). Was die Karte verlangt (`description`), steht golden über dem Zeitstrahl |
| `ChooseCards` mit Karten, die alle offen auf dem Tisch liegen (Spielfeld, Länder-Reihe, Kommandozone, Stack) | Tisch-Modus: Karten leuchten, Klick wählt (gold), Fenster am rechten Rand mit Kartennamen und „Fertig“ → `Cards` |
| `Mulligan` / `MulliganBottom` | 3D-Bühne (`Showcase`): Hand groß vor der Kamera, `Button3D` „Mulligan“/„Behalten“ → `Mulligan` / `Keep`; bei `MulliganBottom` Karten anklicken, „Fertig (n/N)“ → `Cards` |
| `ChooseCards`, alle Optionen auf der eigenen Hand (Abwerfen, `DiscardToHandSize`, Kosten …) | Hand-Bühne wie `MulliganBottom` (`ForgeTable.show_hand_pick`): Karten anklicken, „Fertig (n/min–max)“ → `Cards`, „Abbrechen“ → `Cancel` (nur wenn `cancelable`) |
| `ChooseModes` | 3D-Bühne: Kartenkopie je Modus mit nur diesem Modus als Regeltext → `Modes` |
| `Confirm`, `OptionalTrigger`, `PayUnlessCost`, `PayLifeForPhyrexian`, `ChooseOption` mit 2 Optionen | 3D-Bühne: Quellkarte groß, zwei Knöpfe untereinander (orange = erste Antwort, blau = zweite) → `Yes`/`No` bzw. `Number` |
| `ChooseOptions` mit `purpose` `OptionalCost` | 3D-Bühne: Karte groß, orange „Kicker – {R}“ → `Modes([0])`, grau „Normal“ → `Modes([])` |
| `Scry` | 3D-Bühne: Kasten mit „UNTEN“/„FRIEDHOF“ und „OBEN“, Karten ziehen → `Arrange { top, bottom }` (rechte Karte zuoberst) |
| `ChooseCards` (nicht Hand, nicht Tisch) | 3D-Bühne mit Kartenfächer und Schieberegler; Klick → `Cards([id])` bei einer Karte, sonst Auswahl + „Fertig“; „Ablehnen“ → `Cards([])` |
| `OrderTriggers` | 3D-Bühne „Reihenfolge festlegen“: je Eintrag eine Karte (Kopie der Quelle aus `sources`, Text = Beschreibung), Klick vergibt 1, 2, …, erneuter Klick nimmt zurück; „Fertig“ → `Order` |
| `AssignCombatDamage` / `DivideAmount` | Auswahlfenster „Verteilen“: Zahlenfeld je Empfänger, vorbelegt, Summenanzeige, „OK“ nur bei richtiger Summe → `Damage` |
| `PayUnlessCost` / `PayLifeForPhyrexian` | Ja/Nein-Fenster („Zahlen“ / „Nicht zahlen“, „2 Leben zahlen“ / „Mit Mana zahlen“) |
| `PayMana` | `ForgeTable` antwortet immer sofort `Pay(AutoPay)` (kein Fenster; vorab getapptes Mana im Pool setzt Forge mit ein). Vorab getappte Länder mit unverbrauchtem Mana: Klick bei Priorität → `Action(UndoMana)`; Manapool-Anzeige aus `mana_pool` |
| `DeclareAttackers` / `DeclareBlockers` | Kandidaten leuchten blau, gewählte orange; Angreifer rücken vor (Angriffsspitze), Angriffspfeile (orange) nur bei mehr als einem Angriffsziel; Pfeil ziehen (Kreatur → Spieler/Planeswalker bzw. Blocker → Angreifer; `TableController.aim_released`) oder Klick plant Angriff bzw. Block (rote / blaue Pfeile, Markierungen), der Phasen-Knopf unten rechts („Angreifen (n) »“ / „Blocken (n) »“) sendet den Plan (`ForgeTable.pass_priority`); „Sofort beenden“ (`ForgeTable.end_turn`) antwortet bis Rundenende selbst `Pass` bzw. `Attackers([])`; `combat` aus dem Zustand zeigt den Kampf der KI als Pfeile |
| alle übrigen Entscheidungen | HUD-Auswahlfenster (`decision_view.rs`): Knöpfe, Mehrfachauswahl mit „Fertig“, Zahlenfeld, Suchfeld ab 13 Optionen (höchstens 40 Knöpfe, z. B. bei `NameCard` mit allen Kartennamen); Quellkarte golden markiert |
| `exile`, `attached_to`, `counters`, `token` | Exil-Stapel je Seite (`PlayerSeat.exile`); angelegte Karten versetzt unter dem Träger in dessen Zone; Marken und „Spielstein“ als Plaketten auf der Karte; Spieler-Marken unter dem Namen im HUD |
| `command`, `face`, `face_down`, `monarch`, `initiative`, `day`, `ring_level` | Kommandozone je Seite als Spalte (Spieler rechts unten, Gegner rechts oben); Seitenwechsel als Umdreh-Animation, verdeckte Karten mit Rückseitenmuster; Plaketten „Commander“, „Ringträger“, „Raum: …“, „↺ andere Seite“; Monarch, Initiative, Stadtsegen, Ring-Stufe unter dem Namen, Tag/Nacht neben der Rundenzahl |
| `log` | wird zurzeit nicht angezeigt (das Protokoll-Fenster oben rechts ist auf Wunsch entfernt; geplant: aufklappbare 3D-Schriftrolle) |
| `stack` | Zone `Stack` rechts als Kartenstapel (neuester Eintrag vorn, ältere versetzt dahinter): Zauber (auch `Casting`) als ihre Karte, Fähigkeiten als Karte ihrer Quelle; Pfeile zu den Zielen in der Farbe der Quellkarte, auf Einträge darunter als Bogen; keine eigene Liste im HUD, der 3D-Stapel zeigt alle Einträge |
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
- Protokoll v3 (Protokoll-Modell) getestet: Startspieler (`ChooseOption`), `Mulligan`, `NameCard`
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
  belegtem Stack. Optik (Pfeile) noch nicht im Fenster geprüft.
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
- Effekt-Wahl getestet mit einem Testdeck (Sawhorn Nemesis, Goblin Bombardment, Raging Goblin):
  „Choose a player“ und „any target“ per Klick auf das Medaillon, Opfern als Kosten per Klick auf
  die Kreatur (Tisch-Modus). Reihenfolge der Blocker nicht ausgelöst (die KI blockte nie doppelt).
- Delve, Convoke, Improvise getestet mit einem Testdeck (Stoke the Flames, Treasure Cruise,
  Reverse Engineer, Raging Goblin, Ornithopter) gegen `gegner.dck`: Kreaturen bzw. Artefakte im
  Tisch-Modus, Friedhofskarten im Kartenfächer; nur im echten Wirken gefragt (nicht bei
  Spielbarkeits-Prüfungen).
- Exil als Kosten getestet mit Grim Lavamancer (zwei Karten aus dem eigenen Friedhof, Kartenfächer).
- Ziele auf dem Stack sind jetzt die Zauber/Fähigkeiten selbst (vorher die Karte, wodurch
  Kopieren nicht griff); in `ChooseTargets` erscheinen sie als Karte ihrer Quelle. Getestet mit
  Reverberate auf Shock: Kopie, „Neue Ziele wählen?“, neue Zielwahl. Erschöpfen (Ahn-Crop Crasher)
  und Rekrutieren (Yavimaya Steelcrusher) im Tisch-Modus getestet.
- Kaskade (Bloodbraid Elf → „Do you want to play Shock?“ → Ziel), Fact or Fiction (Stapelwahl) und
  Leyline of Punishment aus der Starthand getestet.
- Besetzen getestet mit Smuggler's Copter (Crew 1) und Raging Goblin (Tisch-Modus).
- `chooseManaFromPool`, Spleißen, Gruppenwahl und Zielwechsel ferngesteuert, aber nicht gezielt
  ausgelöst (nur Regressionstest mit den Standard-Testdecks).
- Aufdecken getestet: Forge-KI deckt die gesuchte Karte aus der Bibliothek auf (Kartenfächer ohne
  Auswahl).
- Pool-Mana per Klick getestet mit Rite of Flame (RR in den Pool) und Lightning Bolt (`UsePool R`).
- Modi mit Wiederholung (`can_repeat`) getestet mit Fiery Confluence: dreimal derselbe Modus über
  das HUD (vorher nicht beantwortbar, wenn weniger Modi verfügbar waren als zu wählen sind).
- Schutzschild verteilen und Würfel tauschen ferngesteuert, nicht gezielt ausgelöst.
- Farbkombination getestet mit Manamorphose (zwei Farbwahlen, danach Pool-Mana per Klick).
- Spielprotokoll getestet: 440 Einträge in einer Partie, chronologisch (Mulligan, Runden, Länder,
  Zauber mit Zielen, Auflösen, Zonenwechsel), Anzeige oben rechts im Fenster geprüft.
- Forges eigene Fragetexte sind englisch (Forge-Spracheinstellung); die Texte der Brücke deutsch.
