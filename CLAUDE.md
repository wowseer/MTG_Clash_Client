# CLAUDE.md – MTG Clash

## Ziel

**Eine 3D-Oberfläche, die das komplette Backend von Forge bedient – und am Ende in NeoForge lebt.**

- **NeoForge** ist die Neuentwicklung von Forges Regel-Engine in Rust
  (`~/Entwicklung/Java/forge/rust/`, Crates `forge-types` → `forge-script` → `forge-engine`,
  veröffentlicht als `github.com/wowseer/forge-rules-rs`). Sie soll später als GDExtension in Godot
  laufen; **diese 3D-UI wird dann in NeoForge übernommen.**
- Bis dahin läuft die UI gegen die Java-Engine von Forge (über `forge-bridge/`). Die Java-Brücke ist
  nur eine **Übergangs-Engine**: Sie übersetzt Forge in NeoForges Entscheidungsmodell.
- Jede Entscheidung, die die Engine einem Spieler abverlangt, trifft der Spieler in der 3D-Ansicht
  selbst; jeder Teil des Spielzustands ist auf dem Tisch oder im HUD sichtbar. Die KI steuert nur
  Gegner, nie den menschlichen Spieler.
- Ein eigenes Regelwerk wird **nicht** wieder eingeführt: Die UI prüft keine Regeln, sie zeigt den
  Zustand der Engine an und schickt ihr die Antworten des Spielers.

## Kompatibilität zu NeoForge (verbindlich)

- **Entscheidungsmodell = NeoForge.** Maßgeblich ist `forge-engine/src/decision.rs` in NeoForge:
  `Decision` (was die Engine fragt), `Choice` (Antwort), `Action` (Prioritäts-Aktion), `Target`,
  `PayAction`, `CardChoiceReason`; Ablauf `Engine::advance() → Status::NeedsDecision(Decision) |
  GameOver`, Fortsetzung mit `Engine::submit(Choice)` (`forge-engine/src/engine.rs`).
- Das Protokoll zur Java-Brücke bildet diese Typen **1:1** ab (Variantenname als `decision` /
  `choice`, Felder mit denselben Namen). Die Java-Brücke übersetzt Forges `PlayerController`-Aufrufe
  in NeoForge-`Decision`s und die `Choice`s zurück.
- Die Rust-UI spricht nur mit einer **Engine-Schnittstelle** (`advance`/`submit`/Zustand), nie
  direkt mit Java-Details. Heute: Umsetzung über den Java-Prozess; später: NeoForge direkt.
  UI-Code darf keine Annahmen über Java/Forge enthalten.
- Fragt Forge etwas, das NeoForge (noch) nicht kennt (z. B. Reihenfolge von Ersatzeffekten,
  Startspieler, Abstimmung, Marken-Typ), gibt es eine allgemeine Erweiterung (Liste von Optionen →
  `Choice::Number(index)`). Jede Erweiterung in `docs/Forge-Schnittstelle.md` unter
  „Erweiterungen gegenüber NeoForge“ auflisten, damit sie in NeoForge nachgezogen werden kann.
- IDs: `CardId` = Forge-Karten-ID, `PlayerId` = Sitzplatz-Index, bis NeoForge-IDs vorliegen.
- **Multiplayer mitdenken** (NeoForge-Regel): nie „den Gegner“ als einzelnen Spieler hartkodieren.
  Die 3D-Szene hat heute zwei Sitzplätze; neuer Code muss mit `PlayerId`s und Listen arbeiten.
- **Umfang wie NeoForge:** Formate Standard, Pioneer, Modern, Legacy, Vintage, Pauper, Commander,
  Booster Draft, Cube. **Keine UI bauen für** Archenemy, Planechase (Planar-Würfel), Vanguard,
  Conspiracy, Un-Sets (Contraptions, Attractions, Sticker), Alchemy/Digital-only, Ante. Im Scope
  sind u. a. Dungeons/Initiative, Monarch, Tag/Nacht, „The Ring“, Embleme, Energie/Gift/Erfahrung,
  Sagas/Klassen/Räume, Schlachten, Commander-Zone.

## Weitere Anweisungen (vor der Arbeit lesen)

| Bevor du … | lies |
|---|---|
| eine Forge-Entscheidung oder Zustandsanzeige einbaust | `docs/Kartenabdeckung.md` (alle 113 Forge-Entscheidungen → 13 Bedienmuster, nötiger Zustand, Reihenfolge) |
| eine Animation baust oder änderst | `docs/claude/ANIMATION.md` (Grundsätze, Timing-Tabelle, Vorlage) |
| ein Spielfeld / eine Kulisse baust oder überarbeitest | `docs/claude/SPIELFELD.md` (Detailgrad, Textur-Pipeline, Vorlage) |

## Architektur

```
Godot 4.7 (godot/)                Rust-GDExtension (rust/)                 Java (forge-bridge/)
table.tscn  ─────────────▶  TableController  Hover, Drag-and-Drop, Kamera
                            ForgeTable       Forge-Zustand → Card3D/Zonen,
                                             Spielerzüge → Antworten
                            TurnHud          liest HudView vom ForgeTable
                            ForgeBridge  ── JSON Lines stdin/stdout ──▶  clash.forgebridge.ForgeBridge
                                                                              └─ Forge 2.0.15 (forge-bridge/forge/)
```

| Ort | Inhalt |
|---|---|
| `rust/src/forge/bridge.rs` | `ForgeBridge`: startet den Java-Prozess, Signale je Nachricht, `answer()` |
| `rust/src/forge/table.rs` | `ForgeTable`: gleicht den Tisch mit `state` ab (Karten-ID → `Card3D`), Deck-Stapel, Leuchten spielbarer Karten, Drops → Optionen, `hud_view()` |
| `rust/src/table/` | Darstellung: `Card3D`, `Zone`, `TableCamera`, `TableController` (regelfrei), Kulisse |
| `rust/src/game/` | Anzeige-Bausteine: HUD, Medaillons, Phasen-Zeitstrahl, Mana-Typen, Kartendaten für das Kartenbild |
| `rust/src/rules/` | Engine-Grenze: NeoForge-Modell (`Decision`, `Choice` …), `Request`, `RulesEngine` |
| `rust/src/game/decision_view.rs` | `Decision` → Auswahlfenster → `Choice` (engine-neutral) |
| `forge-bridge/src/clash/forgebridge/` | `RemotePlayerController` (Forge-Aufrufe), `Decisions` (→ NeoForge-`Decision`, Antwort-`Choice` lesen), `StateJson` (Zustand → JSON), `LogForwarder` (Spielprotokoll), `ForgeBridge` (Befehle, Partie, Anfragen) |
| `forge-bridge/forge/` | lokale Kopie von Forge (Jar + `res/`), **nicht im Git** (GPL, Kartendaten von WotC) |
| `docs/Forge-Schnittstelle.md` | Protokoll (Befehle, Nachrichten, `state`-Aufbau). **Bei jeder Protokolländerung pflegen** |

Forge-Sitzplatz 0 = Spieler unten (ferngesteuert), Sitzplatz 1 = Gegner oben (Forge-KI).

## Stand

**Protokoll v3 steht:** NeoForges `Decision`/`Choice` in `rust/src/rules/` (Engine-Grenze,
`RulesEngine`), Java-Übersetzer `forge-bridge/.../Decisions.java`, UI-Abbildung
`rust/src/game/decision_view.rs`. Jedes weitere Bedienmuster direkt gegen die passende
`Decision`-Variante bauen. Fertig: `ChooseTargets` per Klick/Pfeil, Aktivieren per Klick,
`DeclareAttackers` / `DeclareBlockers` per Klick mit Kampfpfeilen, `PayMana` (Quellen anklicken,
Automatik). Gespielt wird über Forges menschlichen Ablauf `PlaySpellAbility.playSpellAbility`
(im Jar), der die überschriebenen Controller-Methoden fragt. Kartenfächer (`card_fan.rs`) für
`ChooseCards`, `MulliganBottom`, `Scry`; `OrderTriggers` im Auswahlfenster. Kosten-Auswahlen
(`RemoteCostDecision`), `AssignCombatDamage`, `DivideAmount`, `PayUnlessCost`,
`PayLifeForPhyrexian`, Trigger über den menschlichen Ablauf. Nächster Schritt laut Fahrplan:
offene Spieler-Entscheidungen aus dem Fahrplan (Ziele von `chooseSingleEntityForEffect` /
`chooseEntitiesForEffect`, Delve, Convoke, Blocker-Reihenfolge …); der Zustand ist vollständig.

Ferngesteuert, also vom Spieler in der UI entschieden:
- [x] Priorität: Karte oder Fähigkeit spielen bzw. passen (`chooseSpellAbilityToPlay`): Karte aus der Hand auf den Tisch ziehen, Button/Leertaste = passen
- [x] Mulligan (`mulliganKeepHand`): Auswahlfenster im HUD
- [x] Wirken über Forges menschlichen Ablauf (`PlaySpellAbility.playSpellAbility`): optionale Kosten, Modi, Typ, X, Ziele per Klick (`ChooseTargets`), Mana bezahlen (`PayMana`)
- [x] Allgemeine Auswahl (`Decisions.java`, `decision_view.rs`, `choice_panel.rs`): Modi, Optionen, Ja/Nein, Zahlen, Farben, Kartennamen, optionale Kosten, Startspieler (Muster M9–M12)
- [x] Kampf: Angreifer und Blocker per Klick; Kartenfächer für Kartenwahl, Mulligan, Hellsicht; Reihenfolge von Triggern

Angezeigt:
- [x] Hand, Spielfeld, Länder-Reihe, Friedhof, verdeckte Gegnerhand, getappt
- [x] Bibliothek als Deck-Stapel; Ziehen gleitet animiert vom Deck auf die Hand
- [x] Phase, Leben, Runde, Spieler am Zug, Spielende
- [x] Stack: Zauber und Fähigkeiten in der Tischmitte, Ziele als violette Pfeile, Liste im HUD
- [x] Spielprotokoll oben rechts
- [x] Kommandozone, Monarch/Initiative/Stadtsegen/Ring, Tag/Nacht, doppelseitige Karten (Umdreh-Animation), verdeckte Karten
- [x] Kartenbild aus Engine-Daten: Regeltext mit Symbolen, Typzeile, Farben, Stärke/Widerstand, Loyalität, Verteidigung
- [x] Exil-Stapel, Marken- und Spielstein-Plaketten, angelegte Karten unter dem Träger, Spieler-Marken

## Fahrplan

Jeder Punkt ist ein eigener Schritt: Brücke (Java), Protokoll, Rust/UI, Doku, Test, Commit.
Die vollständige Liste mit Zuordnung zu Bedienmustern steht in `docs/Kartenabdeckung.md`.

**Entscheidungen des Spielers** (NeoForge-`Decision` → Forge-Methode in `RemotePlayerController`):
1. `ChooseTargets` (`chooseTargetsFor` ✔ beim Wirken; offen: Ziele ausgelöster Fähigkeiten, `chooseSingleEntityForEffect`, `chooseEntitiesForEffect`)
2. ~~`DeclareAttackers`~~ ✔ (offen: Erschöpfen/Rekrutieren)
3. ~~`DeclareBlockers`~~ ✔ (offen: Reihenfolge der Blocker)
4. ~~`PayMana`~~ ✔ (offen: Pool-Mana gezielt wählen, `chooseManaFromPool`, andere Kosten wie Opfern/Abwerfen)
5. ~~`Priority` mit `Action::Activate` per Klick, Menü bei mehreren~~ ✔
6. ~~`ChooseCards`, `Scry`, `OrderTriggers`, `MulliganBottom`~~ ✔ (offen: Kosten-Auswahlen wie Opfern/Abwerfen als Kosten, Delve, Convoke)
7. ~~`AssignCombatDamage`, `PayUnlessCost`, `PayLifeForPhyrexian`, Kosten, Trigger-Ziele~~ ✔ (offen: Exil als Kosten, Delve, Convoke, Erschöpfen/Rekrutieren, Blocker-Reihenfolge, Kopien)

**Zustand, der noch fehlt:**
- ~~Stack mit Zaubern und Fähigkeiten, Ziele als Pfeile~~ ✔ (Zone `Stack`, Liste im HUD)
- ~~Exil, Marken (Counter), angelegte Karten (Auren, Ausrüstung), Spielsteine~~ ✔
- ~~Regeltext auf dem Kartenbild; Kartenmodell verschlankt~~ ✔ (`CardDef::from_state`, `card_text.rs`)
- ~~Spielprotokoll~~ ✔ (Nachricht `log` aus Forges `GameLog`, oben rechts im HUD)
- ~~Kommandozone (Embleme, Commander, Dungeons, Ring), doppelseitige/verdeckte Karten~~ ✔

**Später:** Deckauswahl, mehr als 2 Spieler (Commander), Formate, Umzug der UI nach NeoForge.

## Quellcode von Forge und NeoForge

- Laufzeit ist das Jar von **Forge 2.0.15** (`forge-bridge/forge/`). Signaturen immer gegen dieses
  Jar prüfen (`javap -cp <jar> <Klasse>`); der Compiler in `build.sh` meldet Abweichungen.
- Java-Quellcode zum Nachlesen: `forge-bridge/forge-game/`, `forge-bridge/forge-ai/`,
  `forge-bridge/forge-core/` (aus dem Fork `~/Entwicklung/Java/forge`, Branch `rust-godot`).
  Der Ablauf für menschliche Spieler steht in `forge-game/.../player/PlaySpellAbility.java`.
  Nur lesen, nie ändern. Der Fork kann vom Jar abweichen: Logik aus dem Quellcode übernehmen,
  aber nur Methoden verwenden, die es in 2.0.15 gibt.
- NeoForge (Rust) zum Nachlesen: `~/Entwicklung/Java/forge/rust/` – `forge-engine/src/decision.rs`
  (Entscheidungsmodell), `engine.rs` (`advance`/`submit`), `docs/01-architektur.md`. Nur lesen;
  dort gelten eigene Regeln (eigene `CLAUDE.md`, Commits/Pushes dort nicht aus diesem Projekt heraus).

## Bauen, starten, testen

| Was | Befehl |
|---|---|
| Rust bauen | `cd rust && cargo build` |
| Unit-Tests | `cd rust && cargo test` |
| Brücke bauen | `forge-bridge/build.sh` (nutzt `forge-bridge/forge/`, sonst `~/Games/Forge`) |
| Brücke von Hand | `forge-bridge/run.sh`, dann JSON-Zeilen tippen, z. B. `{"cmd":"ping"}` |
| Godot | `~/.appimages/Godot_v4.7.2-stable_mono_linux_x86_64/Godot_v4.7.2-stable_mono_linux.x86_64` (nicht im PATH) |
| Spiel starten | Godot öffnen, F5 (Hauptszene `godot/table.tscn`) |
| API-Referenz | `python3 tools/gen_api_doc.py` → muss „ohne Beschreibung: 0“ melden |

**Testdecks:** `forge-bridge/testdecks/` (`spieler.dck` / `gegner.dck` = gemischte Standard-Decks; `auswahl.dck` löst viele Auswahl-Dialoge aus; `kartenwahl.dck` Hellsicht, Abwerfen, Bibliothek durchsuchen; `kosten.dck` + `gegner_counter.dck` Kosten, Verteilen, phyrexianisches Mana, Trigger-Ziele, Mana Leak; `rot.dck` / `gruen.dck` nur Länder + eine Kreatur).

**Partie ohne Fenster testen:** ein temporäres SceneTree-Skript (z. B. `godot/_probe.gd`, danach
wieder löschen), das `res://table.tscn` lädt, sich an `ForgeBridge.decision_requested` hängt und im
nächsten `_process` über `ForgeTable` bzw. das HUD antwortet (`submit_choice`, `pass_priority`, `TurnHud.on_choice_option`). Nicht direkt im
Signal-Handler antworten. Start:
`godot --headless --path godot -s res://_probe.gd`. Forge braucht ca. 5 s zum Laden; das
Brücken-Log steht in `~/.local/share/godot/app_userdata/MTG Clash/forge-bridge.log`.

Fenster-Renders (`--write-movie`) öffnen sich auf dem Desktop des Users und bekommen dessen echte
Mausklicks: nur für Optik verwenden, Spiellogik ohne Fenster prüfen, nie `warp_mouse`.

## Regeln für die Arbeit

- **Sprache:** Code-Kommentare, `///`-Doku und Dokumentation auf **Deutsch**; Commit-Messages auf Englisch.
- **Doku immer mitpflegen**, im selben Schritt wie die Änderung:
  1. `///`-Doku an jeder neuen oder geänderten Godot-Klasse, `#[func]`, `#[export]`, `#[var]`, `#[signal]` (erscheint in der Godot-Hilfe, F1)
  2. `docs/DOKUMENTATION.md` (Bedienung, Szenenaufbau, Stand)
  3. `docs/Forge-Schnittstelle.md` bei jeder Protokolländerung
  4. `python3 tools/gen_api_doc.py`
- **Nach jeder Aufgabe committen** auf `main` (`git add . && git commit`). Vorher `git status`
  ansehen: nur eigene Änderungen committen, fremde (z. B. vom User gelöschte Dateien) erwähnen.
- Shell ist zsh: Platzhalter ohne Treffer (`rm godot/_probe*.uid`) brechen den ganzen Befehl ab.
  Doku-Skripte in einem eigenen Aufruf ausführen und ihre Ausgabe prüfen, nie an `rm` & Co. hängen.
- Rust-Build ohne Warnungen halten; toten Code löschen statt mit `allow` zu verstecken. Einzige
  Ausnahme: noch ungenutzte Teile des NeoForge-Modells (`rules/decision.rs`) mit
  `#[expect(dead_code, reason = …)]` – `expect` meldet sich, sobald sie benutzt werden.
- Texte im Spiel zweisprachig: `tr!("Deutsch", "English")` / `trf!` aus `rust/src/i18n.rs`.
- Godot-Logik in Rust (gdext), kein GDScript im Spiel (nur temporäre Testskripte).
- Lizenz: `forge-bridge/` ist GPL-3.0 und läuft als getrennter Prozess; keinen Forge-Code in
  `rust/` übernehmen. `forge-bridge/forge/` (Jar + Kartendaten) nie committen.
- Begriffe in Code und Doku wie in NeoForge und den Comprehensive Rules (CR-Nummern in Kommentaren,
  wo eine Regel umgesetzt bzw. angezeigt wird).
