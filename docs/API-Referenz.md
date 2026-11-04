# API-Referenz (automatisch erzeugt)

Alle Godot-Klassen der Rust-Erweiterung mit Eigenschaften, Funktionen und Signalen.
Erzeugt mit `python3 tools/gen_api_doc.py` aus den Rust-Doku-Kommentaren – dieselben
Texte zeigt der Godot-Editor unter **Hilfe → Hilfe durchsuchen (F1)**.

Sitzplätze werden als Zahl übergeben: `0` = Spieler (unten), `1` = Gegner (oben).

## Inhalt

- [ForgeBridge](#forgebridge) – erbt von `Node`
- [ForgeTable](#forgetable) – erbt von `Node`
- [CardCutout](#cardcutout) – erbt von `Control`
- [HourglassIcon](#hourglassicon) – erbt von `Control`
- [TurnHud](#turnhud) – erbt von `CanvasLayer`
- [ManaCost](#manacost) – erbt von `Resource`
- [ManaPool](#manapool) – erbt von `Node`
- [ManaPoolView](#manapoolview) – erbt von `Control`
- [PhaseTimeline](#phasetimeline) – erbt von `Control`
- [PlayerSeat](#playerseat) – erbt von `Node3D`
- [SeatBadge](#seatbadge) – erbt von `Control`
- [Hello](#hello) – erbt von `Node2D`
- [Arena](#arena) – erbt von `Node3D`
- [Button3D](#button3d) – erbt von `Area3D`
- [Card3D](#card3d) – erbt von `Area3D`
- [Showcase](#showcase) – erbt von `Node3D`
- [TableCamera](#tablecamera) – erbt von `Camera3D`
- [TableController](#tablecontroller) – erbt von `Node3D`
- [TargetArrows](#targetarrows) – erbt von `Node3D`
- [FlickerLight](#flickerlight) – erbt von `OmniLight3D`
- [Tavern](#tavern) – erbt von `Node3D`
- [Zone](#zone) – erbt von `Area3D`

## ForgeBridge

*Erbt von `Node` · Quelle: `rust/src/forge/bridge.rs`*

Verbindung zur Forge-Regel-Engine: startet die Java-Brücke (`forge-bridge/`) als eigenen Prozess und spricht mit ihr über JSON-Zeilen (Protokoll: `docs/Forge-Schnittstelle.md`). Ablauf: `start_bridge()` → Signal [signal bridge_ready] → `start_match()` → Signale [signal forge_event] (jedes Spielereignis) und [signal decision_requested] (ferngesteuerter Spieler hat Priorität / Mulligan) → Antwort mit `answer()` → [signal game_over]. Ferngesteuert sind die Sitzplätze aus `remote_seats`; alle anderen spielt die Forge-KI. Zahlen in empfangenen Dictionaries sind ganzzahlig (int), wenn sie keinen Nachkommaanteil haben (Godots JSON liefert sonst immer float).

### Eigenschaften

| Name | Typ | Beschreibung |
|---|---|---|
| `java_path` | `String` | Java-Programm (Name im PATH oder absoluter Pfad). Java 17 oder neuer. |
| `forge_dir` | `String` | Forge-Installationsordner (enthält `res/`). Leer = Umgebungsvariable `FORGE_DIR`, sonst die Projektkopie `forge-bridge/forge/` (falls vorhanden), sonst `~/Games/Forge`. |
| `forge_jar` | `String` | Forge-Jar mit allen Abhängigkeiten. Leer = erste Datei `forge-gui-desktop-*-jar-with-dependencies.jar` in `forge_dir`. |
| `bridge_jar` | `String` | Gebaute Brücke (`forge-bridge/build.sh`). `res://`-Pfade werden umgerechnet. |
| `java_max_memory_mb` | `int` | Maximaler Java-Heap in MB (`-Xmx`). |
| `autostart` | `bool` | Brücke automatisch in `ready()` starten. |
| `log_messages` | `bool` | Jede empfangene Nachricht in die Godot-Konsole schreiben (zum Debuggen). |

### Funktionen

| Funktion | Rückgabe | Beschreibung |
|---|---|---|
| `start_bridge()` | `bool` | Startet den Java-Prozess. Gibt false zurück, wenn er schon läuft oder nicht starten konnte (dann zusätzlich [signal bridge_error]). Fertig ist er erst mit [signal bridge_ready] (Forge braucht einige Sekunden zum Laden). |
| `is_hosted()` | `bool` | Läuft die Brücke eingebettet in einem Gastgeber (NeoForge startet die Partie)? |
| `stop_bridge()` | `void` | Beendet die Brücke (Befehl "quit", nach 2 s hart beendet). Sendet [signal bridge_stopped] mit -1, falls sie lief. |
| `is_running()` | `bool` | Läuft der Java-Prozess? |
| `is_ready()` | `bool` | Hat Forge fertig geladen ([signal bridge_ready] empfangen)? |
| `send_command(command: Dictionary)` | `bool` | Schickt einen beliebigen Befehl (Dictionary mit "cmd", siehe Protokoll). false, wenn die Brücke nicht läuft. |
| `ping(nonce: int)` | `bool` | Verbindungstest; Antwort kommt als Nachricht `{type: "pong", nonce}`. |
| `start_match(decks: PackedStringArray, remote_seats: PackedInt32Array, names: PackedStringArray, seed: int)` | `bool` | Startet eine Partie. `decks`: Pfade zu Forge-`.dck`-Dateien (`res://` erlaubt) oder Namen von Decks aus Forges Constructed-Sammlung, einer je Sitzplatz (mind. 2). `remote_seats`: Sitzplätze, die MTG Clash steuert (Rest = Forge-KI). `names`: Spielernamen (darf kürzer sein). `seed` < 0 = zufällig. |
| `answer(request_id: int, choice: Variant)` | `bool` | Beantwortet eine Anfrage aus [signal decision_requested]. `choice` ist eine Antwort im Format des Protokoll-Modells (Protokoll v3), z. B. `{"type": "Action", "action": {"type": "Pass"}}`; Aufbau siehe `docs/Forge-Schnittstelle.md`. |
| `request_state(viewer: int)` | `bool` | Fragt den aktuellen Spielzustand ab → [signal state_received]. `viewer` = Sitzplatz, dessen Handkarten mitgeschickt werden (-1 = keiner). |

### Signale

| Signal | Beschreibung |
|---|---|
| `bridge_ready(protocol: int)` | Die Brücke ist gestartet und Forge geladen; `protocol` = Protokollversion. |
| `message_received(message: Dictionary)` | Jede empfangene Nachricht (vor den speziellen Signalen), z. B. für eigene Auswertung. |
| `forge_event(event: Dictionary)` | Ein Forge-Spielereignis: `{event, text, turn, phase}` (`event` = Forge-Klassenname, z. B. "GameEventCardChangeZone"). |
| `log_entry(entry: Dictionary)` | Neuer Eintrag im Spielprotokoll der Engine: `{kind, text, card, turn}` (`kind` z. B. "ZONE_CHANGE", "DAMAGE", "STACK_ADD", "TURN"; `card` = Karten-ID der Quelle, falls bekannt). |
| `decision_requested(request: Dictionary)` | Ein ferngesteuerter Spieler muss entscheiden (Protokoll v3): `{id, player, decision, prompt, source, ability, cancelable, labels, state}`; `decision` ist eine `Decision` (z. B. `{"type": "Priority", "actions": […]}`). Antwort mit `answer()` und einer `Choice`. Aufbau siehe `docs/Forge-Schnittstelle.md`. |
| `decision_rejected(info: Dictionary)` | Die gewählte Option wurde von Forge abgelehnt (`{player, option, reason}`); es folgt eine neue Anfrage. |
| `match_started(state: Dictionary)` | Die Partie hat begonnen; `state` = Spielzustand (siehe Protokoll). |
| `state_received(state: Dictionary)` | Antwort auf `request_state()`. |
| `game_over(result: Dictionary)` | Die Partie ist vorbei: `{winner, draw, reason}`. |
| `bridge_error(code: String, message: String)` | Fehler der Brücke oder beim Start (`code` z. B. "deck_not_found", "spawn_failed"). |
| `bridge_stopped(exit_code: int)` | Der Java-Prozess ist beendet (`exit_code` -1 = unbekannt / von uns beendet). |

## ForgeTable

*Erbt von `Node` · Quelle: `rust/src/forge/table.rs`*

Spielt eine Partie mit der Forge-Regel-Engine auf dem 3D-Tisch: Karten, Zonen, Kamera, Drag-and-Drop und HUD sind dieselben wie im normalen Spiel, nur die Regeln kommen von Forge (über `ForgeBridge`). Forge-Sitzplatz 0 ist der Spieler (unten, von dir gesteuert), Sitzplatz 1 der Gegner (oben, Forge-KI). Länder liegen in der Ressourcen-Reihe, alle anderen bleibenden Karten auf dem Spielfeld, der Friedhof im Ablagestapel; die Gegnerhand wird verdeckt angezeigt. Bedienung: Hat der Spieler Priorität, leuchten alle Handkarten, die er jetzt spielen kann. Eine solche Karte aus der Hand auf den Tisch ziehen = spielen (Mana zahlt Forge automatisch). Hauptbutton / Leertaste = passen. Mulligan und alle allgemeinen Entscheidungen (Optionen, Modi, Ja/Nein, Zahlen, Farben, Kartennamen) im Auswahlfenster des HUD; die Karte, um die es geht, wird dabei golden markiert. Ziele, Angreifer, Blocker und Manabezahlung wählt vorerst die Forge-KI für dich.

### Eigenschaften

| Name | Typ | Beschreibung |
|---|---|---|
| `bridge` | `ForgeBridge` | Verbindung zu Forge. |
| `controller` | `TableController` | Tischsteuerung (Drag-and-Drop, Kamera). Sie kennt keine Regeln; dieser Knoten prüft die Züge über Forge. |
| `player_seat` | `PlayerSeat` | Sitzplatz des Spielers (unten) = Forge-Sitzplatz 0. |
| `opponent_seat` | `PlayerSeat` | Sitzplatz des Gegners (oben) = Forge-Sitzplatz 1. |
| `stack_zone` | `Zone` | Stack-Zone (Art 4) in der Tischmitte: Zauber liegen dort als ihre Karte, aktivierte und ausgelöste Fähigkeiten als Karte ihrer Quelle; der oberste Eintrag liegt rechts oben. Pfeile zeigen die Ziele jedes Eintrags. Leer = Stack nur in der Liste im HUD. |
| `player_deck` | `String` | Deck des Spielers: Forge-`.dck`-Datei (`res://` erlaubt) oder Forge-Deckname. |
| `opponent_deck` | `String` | Deck des Gegners: Forge-`.dck`-Datei (`res://` erlaubt) oder Forge-Deckname. |
| `seed` | `int` | Zufallsstartwert der Partie (< 0 = zufällig). |
| `autostart` | `bool` | Brücke und Partie automatisch beim Start der Szene starten. |
| `refresh_interval` | `float` | Mindestabstand (Sekunden) zwischen zwei Zustandsabfragen, während Forge Ereignisse sendet. |

### Funktionen

| Funktion | Rückgabe | Beschreibung |
|---|---|---|
| `start_match()` | `bool` | Startet die Partie mit `player_deck` gegen `opponent_deck` (Brücke muss bereit sein). Gibt false zurück, wenn das nicht ging. |
| `pass_priority()` | `void` | Passt (Hauptbutton / Leertaste): beantwortet eine offene `Priority`-Entscheidung mit `Action::Pass`. |
| `submit_choice(choice: Variant)` | `void` | Eingabe aus dem Auswahlfenster des HUD für die offene Entscheidung: eine Option = Index, mehrere = Array von Indizes, Ja/Nein und Mulligan = bool (true = erste Option / behalten), Zahl = int, Abbrechen = null. Wird in die passende `Choice` übersetzt; unpassende Eingaben werden ignoriert. Ist das Fähigkeiten-Menü offen, wählt die Eingabe eine Aktion daraus (null = Menü schließen). |
| `pool_clicked(mana: int)` | `void` | Ein Mana-Symbol der Manapool-Anzeige wurde angeklickt (`mana` = Manaart 0 = Weiß … 5 = Farblos): beim Bezahlen dieses schwebende Mana einsetzen (`PayAction::UsePool`), sonst nichts. |
| `on_bottom_done()` | `void` | „Fertig“ bei `MulliganBottom`: gewählte Karten an die Engine. |
| `on_mulligan_pressed(keep: bool)` | `void` | Ein Mulligan-Knopf wurde gedrückt: `keep` = Starthand behalten, sonst Mulligan. |
| `on_starting_player_pressed(index: int)` | `void` | Ein Startspieler-Knopf wurde gedrückt (`index` = Eintrag in `players` der Entscheidung `ChooseStartingPlayer`). |
| `player_clicked(seat: int)` | `void` | Ein Spieler-Medaillon wurde angeklickt (`seat` = Sitzplatz): ist der Spieler ein gültiges Ziel der offenen Zielwahl, wird er gewählt bzw. abgewählt. |
| `submit_fan(first: PackedInt64Array, second: PackedInt64Array)` | `void` | Antwort aus dem Kartenfächer: `first` = gewählte Karten (`ChooseCards`, `MulliganBottom`) bzw. Karten oben auf der Bibliothek (`Scry`, von oben nach unten), `second` = Karten unter die Bibliothek bzw. in den Friedhof. |
| `set_choice_selection(selection: PackedInt64Array)` | `void` | Aktuelle Auswahl im Auswahlfenster (vom HUD jedes Frame gemeldet): gewählte Ziele werden auf dem Tisch golden markiert und bekommen einen Pfeil. |
| `get_card(forge_id: int)` | `Card3D` | Karte auf dem Tisch zur Forge-Karten-ID (null, wenn nicht sichtbar). |
| `get_state()` | `Dictionary` | Zuletzt von Forge empfangener Spielzustand (Aufbau siehe `docs/Forge-Schnittstelle.md`). |
| `get_pending_kind()` | `String` | Hat der Spieler gerade eine offene Entscheidung? Liefert den Namen der `Decision` (z. B. "Priority", "Mulligan", "ChooseTargets", "ChooseOption") oder einen leeren Text. |
| `on_bridge_ready(protocol: int)` | `void` | Brücke bereit → Partie starten (bei `autostart`). |
| `on_state(state: Dictionary)` | `void` | Neuer Spielzustand von Forge (Spielstart oder Abfrage): Tisch abgleichen. |
| `on_forge_event(event: Dictionary)` | `void` | Forge-Ereignis: Zustand bald neu abfragen (gebündelt, siehe `refresh_interval`). |
| `on_log_entry(entry: Dictionary)` | `void` | Neuer Eintrag im Spielprotokoll der Engine: für das HUD merken (ohne Phasen- und Mana-Einträge, siehe `log_view::HIDDEN_KINDS`). |
| `on_decision_requested(request: Dictionary)` | `void` | Entscheidung des Spielers nötig (Protokoll v3): Zustand übernehmen, Quellkarte markieren, spielbare Karten leuchten lassen. Unbekannte Entscheidungen werden mit `Cancel` beantwortet (die Engine entscheidet dann selbst). |
| `on_game_over(result: Dictionary)` | `void` | Partie vorbei: Sieger merken (HUD zeigt ihn an). |
| `on_bridge_error(code: String, message: String)` | `void` | Fehler der Brücke: als Warnung in die Godot-Konsole schreiben. |
| `on_card_clicked(card: Card3D)` | `void` | Karte angeklickt: bei der Zielwahl = Ziel wählen/abwählen; mit Priorität = Aktion der Karte (nicht aus der Hand – die spielt man durch Ziehen): eine Aktion sofort, mehrere über das Fähigkeiten-Menü. |
| `on_card_dropped(card: Card3D, from_zone: Zone, to_zone: Zone)` | `void` | Karte wurde auf dem Tisch abgelegt: Hat sie eine Aktion in der offenen `Priority`- Entscheidung (aus der Hand: Land spielen / wirken; auf dem Spielfeld: Fähigkeit aktivieren), wird diese gewählt. Aus der Hand zurück auf die Hand = abbrechen. Alles andere wird auf den Engine-Stand zurückgesetzt. |

### Signale

| Signal | Beschreibung |
|---|---|
| `choice_option_clicked(index: int)` | Eine Option wurde auf dem Tisch gewählt (Klick auf ein gültiges Ziel – Karte oder Medaillon): `index` = Option im Auswahlfenster. Das HUD behandelt das wie einen Klick auf diese Option (zurückgestellt verbunden). |

## CardCutout

*Erbt von `Control` · Quelle: `rust/src/game/card_cutout.rs`*

Vollbild-Hülle für HUD-Elemente: Ihre Nachfahren (mit `use_parent_material = true`) werden überall gezeichnet, außer innerhalb des konvexen Vierecks `cutout` (Bildschirmkoordinaten, 4 Punkte). So wirkt eine 3D-Karte, die vor einem Medaillon schwebt, als läge sie darüber. Leeres `cutout` = nichts ausgespart. Ist eine Aussparung gesetzt (Karte gehovert/gezogen), wird der Inhalt zusätzlich weich auf 30 % Deckkraft ausgeblendet. `greyed` graut den Inhalt aus. Der `TurnHud` setzt beides jedes Frame.

### Eigenschaften

| Name | Typ | Beschreibung |
|---|---|---|
| `cutout` | `PackedVector2Array` | Konvexes Viereck (4 Punkte, Bildschirmkoordinaten), in dem die Nachfahren unsichtbar sind. Leer = keine Aussparung. |
| `greyed` | `bool` | Inhalt ausgrauen (graustufig und abgedunkelt), z. B. während die Stack-Anzeige darüber liegt. |

### Funktionen

| Funktion | Rückgabe | Beschreibung |
|---|---|---|
| `set_cutout(quad: PackedVector2Array)` | `void` | Setzt die Aussparung (4 Punkte oder leer). |
| `set_greyed(on: bool)` | `void` | Schaltet das Ausgrauen ein oder aus. |

## HourglassIcon

*Erbt von `Control` · Quelle: `rust/src/game/hourglass.rs`*

Sanduhr-Symbol (HUD), z. B. neben der Rundenzahl am linken Bildrand. Größe = `icon_size` (Breite × Höhe in Pixeln; die Mindestgröße des Controls).

### Eigenschaften

| Name | Typ | Beschreibung |
|---|---|---|
| `icon_size` | `Vector2` | Breite × Höhe des Symbols in Pixeln. |

## TurnHud

*Erbt von `CanvasLayer` · Quelle: `rust/src/game/hud.rs`*

Bildschirm-Overlay für den Rundenablauf: Zeitstrahl der Phasen (`PhaseTimeline`, unten rechts über dem Hauptbutton), Spielende-Meldung in der Bildmitte, je ein `SeatBadge`-Medaillon mit Leben (an `player_hero_anchor` / `opponent_hero_anchor`, sonst links oben / unten), am linken Bildrand Spielername und darüber Rundenzahl mit Sanduhr (`HourglassIcon`; Gegner oben, Spieler unten), großer Button „Nächste Phase“ / „Passen“ (unten rechts, auch Leertaste; blau, bei belegtem Stack orange) und ein Auswahlfenster (z. B. Mulligan). Liest den Zustand jedes Frame vom `ForgeTable` (Spiel mit der Forge-Regel-Engine).

### Eigenschaften

| Name | Typ | Beschreibung |
|---|---|---|
| `forge_table` | `ForgeTable` | Das Forge-Spiel (`ForgeTable`), dessen Zustand angezeigt wird. |
| `player_hero_anchor` | `Node3D` | 3D-Punkt, an dem das Spieler-Medaillon sitzt (Unterkante mittig, z. B. direkt über der Hand). Leer = Medaillon unten links. Gezoomte oder gezogene Karten liegen sichtbar über dem Medaillon (Aussparung per `CardCutout`). |
| `opponent_hero_anchor` | `Node3D` | 3D-Punkt, an dem das Gegner-Medaillon sitzt (Oberkante mittig, z. B. direkt unter dessen Hand). Leer = Medaillon oben links. |
| `player_mana_anchor` | `Node3D` | 3D-Punkt, an dem die Mana-Symbole des Spielers sitzen (Oberkante mittig, z. B. direkt unter der Ressourcen-Reihe). Leer = rechts neben dem Spieler-Medaillon. |

### Funktionen

| Funktion | Rückgabe | Beschreibung |
|---|---|---|
| `on_choice_option(index: int)` | `void` | Option `index` im Auswahlfenster angeklickt (eine Option / Ja-Nein: Antwort senden; Mehrfachauswahl: an-/abwählen). |
| `on_choice_done()` | `void` | Knopf „Fertig“ / „OK“ im Auswahlfenster (Mehrfachauswahl, Zahl, Text). |
| `on_fan_card(index: int)` | `void` | Karte `index` im Kartenfächer angeklickt (wählen bzw. in die andere Reihe legen). |
| `on_fan_done()` | `void` | „Fertig“ im Kartenfächer: Auswahl bzw. Anordnung an den `ForgeTable`. |
| `on_fan_cancel()` | `void` | „Abbrechen“ im Kartenfächer. |
| `on_pool_clicked(mana: int)` | `void` | Mana-Symbol im Manapool angeklickt → an den `ForgeTable` (beim Bezahlen einsetzen). |
| `on_badge_clicked(seat: int)` | `void` | Ein Medaillon (Spieler als Ziel) wurde angeklickt: an den `ForgeTable` weitergeben. |
| `on_choice_values_changed(value: float)` | `void` | Ein Zahlenfeld beim Verteilen wurde geändert: Summe und „Fertig“ aktualisieren. |
| `on_choice_filter_changed(text: String)` | `void` | Suchfeld des Auswahlfensters geändert: Optionen filtern. |
| `on_choice_cancel()` | `void` | Knopf „Abbrechen“ im Auswahlfenster: Antwort null. |
| `on_next_pressed()` | `void` | Button „Passen“ bzw. Leertaste: ruft `ForgeTable.pass_priority()` auf. |

## ManaCost

*Erbt von `Resource` · Quelle: `rust/src/game/mana.rs`*

Kosten einer Karte, als Resource im Inspector editierbar.

### Eigenschaften

| Name | Typ | Beschreibung |
|---|---|---|
| `generic` | `int` | Beliebiges Mana. |
| `white` | `int` | Weißes Mana, das mit Weiß bezahlt werden muss. |
| `blue` | `int` | Blaues Mana, das mit Blau bezahlt werden muss. |
| `black` | `int` | Schwarzes Mana, das mit Schwarz bezahlt werden muss. |
| `red` | `int` | Rotes Mana, das mit Rot bezahlt werden muss. |
| `green` | `int` | Grünes Mana, das mit Grün bezahlt werden muss. |
| `colorless` | `int` | Muss mit farblosem Mana bezahlt werden. |

### Funktionen

| Funktion | Rückgabe | Beschreibung |
|---|---|---|
| `static parse(text: String)` | `ManaCost` | Erzeugt Kosten aus der Textschreibweise, z. B. `"2RR"`. Ungültig -> null. |
| `static from_dictionary(dict: Dictionary)` | `ManaCost` | Erzeugt Kosten aus einem Dictionary `{ ManaType: Menge, "generic": Menge }`. |
| `can_pay(pool: ManaPool)` | `bool` | Prüft, ob der Pool die Kosten bezahlen kann. |
| `get_total()` | `int` | Gesamter Manawert. |
| `to_text()` | `String` | Kurzschreibweise, z. B. "2RR". |

## ManaPool

*Erbt von `Node` · Quelle: `rust/src/game/mana.rs`*

Manavorrat eines Spielers.

### Funktionen

| Funktion | Rückgabe | Beschreibung |
|---|---|---|
| `add_mana(mana_type: int (Manafarbe), amount: int)` | `void` | Fügt Mana hinzu. |
| `remove_mana(mana_type: int (Manafarbe), amount: int)` | `bool` | Nimmt `amount` Mana der Art `mana_type` wieder aus dem Pool (z. B. Tapp zurückgenommen). Alles oder nichts: false, wenn nicht genug davon im Pool liegt. |
| `pay_mana(cost: ManaCost)` | `bool` | Bezahlt die Kosten (alles oder nichts). true = bezahlt. |
| `pay_mana_dict(cost: Dictionary)` | `bool` | Wie `pay_mana`, mit Kosten als Dictionary `{ ManaType: Menge, "generic": Menge }`. |
| `can_pay(cost: ManaCost)` | `bool` | Kann dieser Pool die Kosten `cost` bezahlen? |
| `get_amount(mana_type: int (Manafarbe))` | `int` | Vorrat einer Farbe (0 = Weiß, 1 = Blau, 2 = Schwarz, 3 = Rot, 4 = Grün, 5 = Farblos). |
| `get_total()` | `int` | Gesamtes Mana im Pool (alle Farben zusammen). |
| `empty_pool()` | `void` | Leert den Pool (z. B. am Ende jeder Phase). |
| `to_dictionary()` | `Dictionary` | Aktueller Vorrat als Dictionary `{ ManaType: Menge }`. |
| `static get_mana_color(mana_type: int (Manafarbe))` | `Color` | Anzeigefarbe eines Manatyps (für HUD-Symbole). |
| `static get_mana_name(mana_type: int (Manafarbe))` | `String` | Anzeigename eines Manatyps, z. B. tr!("Rot", "Red"). |
| `to_text()` | `String` | Lesbare Anzeige, z. B. "Rot 2 · Grün 1". |

### Signale

| Signal | Beschreibung |
|---|---|
| `pool_changed()` | Wird nach jeder Änderung ausgelöst. |

## ManaPoolView

*Erbt von `Control` · Quelle: `rust/src/game/mana_view.rs`*

Manapool als Reihe runder Symbole (HUD). Wird vom `TurnHud` jedes Frame über `set_amounts()` gefüttert; die Breite passt sich der Anzahl der Kreise an. Beim Bezahlen sind einsetzbare Kreise golden umrandet und anklickbar ([signal mana_clicked]).

### Funktionen

| Funktion | Rückgabe | Beschreibung |
|---|---|---|
| `set_amounts(amounts: PackedInt64Array)` | `void` | Setzt die Mana-Mengen je Art (Reihenfolge Weiß, Blau, Schwarz, Rot, Grün, Farblos). Leeres Array = nichts anzeigen. Zuwachs lässt den Kreis aufspringen. |

### Signale

| Signal | Beschreibung |
|---|---|
| `mana_clicked(mana: int)` | Ein anklickbares Mana-Symbol wurde angeklickt (`mana` = Manaart: 0 = Weiß, 1 = Blau, 2 = Schwarz, 3 = Rot, 4 = Grün, 5 = Farblos). |

## PhaseTimeline

*Erbt von `Control` · Quelle: `rust/src/game/phase_timeline.rs`*

Zeitstrahl der Phasen (HUD, unten rechts über dem Hauptbutton): ein mit Enttappen, Auffrischung, Ziehen und Hauptphase 1 wachsender Kreis, ein Punkt „Kampf“, der seine drei Schritte (Phasen-Index 4–6) erst während des Kampfs aufklappt, und ein Kreis für Hauptphase 2 und Ende, der im Endschritt kleiner wird. Die aktuelle Station leuchtet; erledigte sind golden, kommende hohl. Darüber steht der Name der aktuellen Phase. Wird vom `TurnHud` jedes Frame über `set_state()` gefüttert.

### Funktionen

| Funktion | Rückgabe | Beschreibung |
|---|---|---|
| `set_state(titles: PackedStringArray, current: int)` | `void` | Setzt Phasennamen (in Spielreihenfolge) und Index der aktuellen Phase (-1 = vor Spielbeginn). |

## PlayerSeat

*Erbt von `Node3D` · Quelle: `rust/src/game/seat.rs`*

Ein Sitzplatz am Tisch (Mensch oder KI) mit seinen Zonen, Lebenspunkten und dem Manapool. Die Tischszene enthält zwei: `Player` (unten) und `Opponent` (oben).

### Eigenschaften

| Name | Typ | Beschreibung |
|---|---|---|
| `display_name` | `String` | Anzeigename in HUD und Protokoll. |
| `is_ai` | `bool` | Wird dieser Platz von der KI gespielt? |
| `starting_life` | `int` | Lebenspunkte zu Spielbeginn. |
| `hand` | `Zone` | Zone der Handkarten. |
| `deck` | `Zone` | Zone des Nachziehstapels (Deck). |
| `battlefield` | `Zone` | Zone des Spielfelds (Kreaturen, Helden, Artefakte, Verzauberungen, Ausrüstung, Auren). |
| `discard` | `Zone` | Zone des Friedhofs (Ablagestapel). |
| `exile` | `Zone` | Zone des Exils (CR 406): offener Stapel neben dem Friedhof. |
| `command` | `Zone` | Kommandozone (CR 408): Commander, Embleme, Dungeons, „The Ring“ als Spalte. |
| `resource_zone` | `Zone` | Reihe für ausgespielte Ressourcen-Karten. |
| `mana_pool` | `ManaPool` | Manapool dieses Spielers. |
| `life` | `int` | Aktuelle Lebenspunkte (≤ 0 = Partie verloren). |

### Funktionen

| Funktion | Rückgabe | Beschreibung |
|---|---|---|
| `owns_zone(zone: Zone)` | `bool` | Gehört die Zone zu diesem Sitzplatz? |

## SeatBadge

*Erbt von `Control` · Quelle: `rust/src/game/seat_badge.rs`*

Rautenförmiges Spieler-Medaillon (HUD): Wappen in der Seitenfarbe `accent`, darunter ein roter Edelstein mit den Lebenspunkten `life`. Ist `active` gesetzt (Spieler am Zug), pulsiert ein Schein um die Raute; Lebensänderungen blitzen kurz rot (Schaden) bzw. grün (Heilung). Wird vom `TurnHud` jedes Frame über `set_state()` aktualisiert.

### Eigenschaften

| Name | Typ | Beschreibung |
|---|---|---|
| `seat_name` | `String` | Name der Seite; sein Anfangsbuchstabe steht im Wappen. |
| `life` | `int` | Angezeigte Lebenspunkte. |
| `active` | `bool` | Ist diese Seite am Zug? (pulsierender Schein) |
| `accent` | `Color` | Farbe des Wappens (Seitenfarbe, z. B. blau für den Spieler, rot für den Gegner). |
| `targetable` | `bool` | Ist dieser Spieler gerade ein gültiges Ziel? Dann pulsiert ein goldener Ring und ein Klick löst [signal clicked] aus (sonst lässt das Medaillon Mausklicks durch). |
| `target_selected` | `bool` | Ist dieser Spieler als Ziel gewählt? (heller, ruhiger Ring) |

### Funktionen

| Funktion | Rückgabe | Beschreibung |
|---|---|---|
| `is_targetable()` | `bool` | Getter für `targetable`. |
| `set_targetable(value: bool)` | `void` | Setter für `targetable`: nur ein wählbares Medaillon fängt Mausklicks ab. |
| `set_state(seat_name: String, life: int, active: bool)` | `void` | Setzt Name, Lebenspunkte und „am Zug“ auf einmal. Weicht `life` vom bisherigen Wert ab, blitzt der Edelstein kurz auf (rot bei Schaden, grün bei Heilung). |

### Signale

| Signal | Beschreibung |
|---|---|
| `clicked()` | Das Medaillon wurde angeklickt, während `targetable` gesetzt war. |

## Hello

*Erbt von `Node2D` · Quelle: `rust/src/hello.rs`*

Beispiel-Node: zeigt, dass die Rust-Extension geladen wird.

### Eigenschaften

| Name | Typ | Beschreibung |
|---|---|---|
| `greeting` | `String` | Text, der beim Start ausgegeben wird. |

### Funktionen

| Funktion | Rückgabe | Beschreibung |
|---|---|---|
| `add(a: int, b: int)` | `int` | Beispielfunktion: gibt `a + b` zurück. |

## Arena

*Erbt von `Node3D` · Quelle: `rust/src/table/arena.rs`*

Arena-Kulisse: Sandstein-Spielbrett mit gemeißeltem, leuchtendem Runenkreis in der Mitte, großer Raute und Mittellinie (ohne eingelassene Felder für Deck, Ablage und Ressourcen), flacher Steinrand mit Goldeinlage, Leuchtkristalle in den Ecken, gestufter Sockel, Schieferboden, Säulenruinen, Feuerschalen und Arenamauer mit Bannern. Alles wird beim Start prozedural erzeugt. Maßstab: 1 Einheit = 10 cm; Brettoberfläche bei y = 0, Mittellinie bei z = 0 (Spieler z > 0, Gegner z < 0). Tool-Klasse: Die Kulisse wird auch im Godot-Editor gebaut, damit die Szene dort sichtbar ist. Die erzeugten Kinder haben keinen Owner und werden daher nicht in die `.tscn` gespeichert. Nach Änderungen an den Eigenschaften im Inspektor baut sich die Kulisse neu auf.

### Eigenschaften

| Name | Typ | Beschreibung |
|---|---|---|
| `board_size` | `Vector2` | Größe des Spielbretts (x = Breite, y = Tiefe) in Einheiten. |
| `texture_resolution` | `float` | Auflösung der Brett-Textur in Pixeln pro Einheit (höher = schärfere Ornamente, längerer Start). |
| `rune_glow` | `float` | Leuchtstärke des Runenkreises (0 = aus). |
| `player_color` | `Color` | Bannerfarbe der Spielerseite. |
| `opponent_color` | `Color` | Bannerfarbe der Gegnerseite. |

### Funktionen

| Funktion | Rückgabe | Beschreibung |
|---|---|---|
| `rebuild()` | `void` | Baut die gesamte Kulisse neu auf (entfernt vorher alle erzeugten Kinder). Wird in `ready()` aufgerufen und im Editor automatisch nach Änderungen im Inspektor. |
| `set_board_size(value: Vector2)` | `void` | Setter für `board_size`; baut die Kulisse (verzögert) neu auf, wenn sie schon steht. |
| `set_texture_resolution(value: float)` | `void` | Setter für `texture_resolution`; baut die Kulisse (verzögert) neu auf, wenn sie schon steht. |
| `set_rune_glow(value: float)` | `void` | Setter für `rune_glow`; baut die Kulisse (verzögert) neu auf, wenn sie schon steht. |
| `set_player_color(value: Color)` | `void` | Setter für `player_color`; baut die Kulisse (verzögert) neu auf, wenn sie schon steht. |
| `set_opponent_color(value: Color)` | `void` | Setter für `opponent_color`; baut die Kulisse (verzögert) neu auf, wenn sie schon steht. |

## Button3D

*Erbt von `Area3D` · Quelle: `rust/src/table/button3d.rs`*

Anklickbarer Knopf in der 3D-Szene: bewegte Oberfläche und Leuchten im Farbstil `style` (Nachbau der Blender-Materialien) mit Beschriftung (`text`). Wird vom `TableController` per Raycast angeklickt (Kollisions-Layer 3) und meldet das mit [signal pressed]. Richtet sich auf Wunsch zur Kamera aus (`face_camera`).

### Eigenschaften

| Name | Typ | Beschreibung |
|---|---|---|
| `style` | `ButtonStyle` | Farbstil: 0 = Orange (Feuer), 1 = Blau (Wolken). |
| `text` | `String` | Beschriftung auf dem Knopf. |
| `width` | `float` | Breite der Bildfläche in Welteinheiten (Höhe folgt aus dem Seitenverhältnis). |
| `face_camera` | `bool` | Knopf immer parallel zur Bildebene der Kamera ausrichten. |
| `enabled` | `bool` | Anklickbar? (Ausgegraut und ohne Klickwirkung, wenn nicht.) |

### Funktionen

| Funktion | Rückgabe | Beschreibung |
|---|---|---|
| `set_hovered(hovered: bool)` | `void` | Maus über dem Knopf (vom `TableController` gesetzt): Knopf wächst, Leuchten wird heller. |
| `is_hovered()` | `bool` | Ist die Maus über dem Knopf? |
| `activate()` | `void` | Klick auslösen (vom `TableController` bzw. von Skripten): kurzes Eindrücken und [signal pressed]. Ohne Wirkung, wenn der Knopf nicht `enabled` ist oder gerade verschwindet. |
| `dismiss()` | `void` | Knopf ausblenden (0,15 s) und danach freigeben; nicht mehr anklickbar. |
| `set_label_text(text: String)` | `void` | Beschriftung ändern. |

### Signale

| Signal | Beschreibung |
|---|---|
| `pressed()` | Der Knopf wurde angeklickt. |

## Card3D

*Erbt von `Area3D` · Quelle: `rust/src/table/card3d.rs`*

Eine Spielkarte auf dem 3D-Tisch: Mesh mit Vorder-/Rückseite und Beschriftung, Hover-Neigung und Hover-Zoom (offene Karten werden groß und lesbar), Raycast-Ziel (Kollisions-Layer 1), Gleit-Animationen per Tween sowie alle Spielwerte (Name, Typzeile, Kosten, Regeltext, Werte aus den Kartendaten der Engine; Tapp-Zustand).

### Eigenschaften

| Name | Typ | Beschreibung |
|---|---|---|
| `face_texture` | `Texture2D` | Textur der Vorderseite. |
| `back_texture` | `Texture2D` | Textur der Rückseite. |
| `max_tilt_degrees` | `float` | Maximale Neigung bei Mouseover (Grad). |
| `tilt_speed` | `float` | Glättung von Tilt/Zoom (größer = schneller). |
| `move_duration` | `float` | Dauer der Gleit-Animation zu einem Slot (Sekunden). |
| `zoom_enabled` | `bool` | Offene Karten beim Hovern groß und lesbar anzeigen. |
| `zoom_screen_height` | `float` | Anteil der Bildschirmhöhe, den die gezoomte Karte einnimmt. |
| `zoom_distance` | `float` | Abstand der gezoomten Karte zur Kamera (liegt damit vor allem anderen). |
| `zoom_margin_px` | `float` | Rand zum Bildschirmrand in Pixeln. |
| `hover_lift` | `float` | Anheben (entlang der Kartennormalen) für verdeckte Karten / ohne Zoom. |
| `hover_scale` | `float` | Skalierung für verdeckte Karten / ohne Zoom. |
| `card_name` | `String` | Kartenname (wie von der Engine). |
| `card_kind` | `int (Kartentyp)` | Kartenart: 0 = Kreatur, 1 = Land, 2 = Spontanzauber, 3 = Hexerei, 4 = Planeswalker, 5 = Ausrüstung, 6 = Artefakt, 7 = Verzauberung, 8 = Aura, 9 = Schlacht. |
| `owner_seat` | `int` | Besitzer (0 = Spieler, 1 = Gegner): auf dessen Friedhof geht die Karte. |

### Funktionen

| Funktion | Rückgabe | Beschreibung |
|---|---|---|
| `set_face_texture(tex: Texture2D)` | `void` | Setzt die Textur der Vorderseite. |
| `set_back_texture(tex: Texture2D)` | `void` | Setzt die Textur der Rückseite. |
| `set_hovered(hovered: bool)` | `void` | Aktiviert/deaktiviert den Hover-Zustand. |
| `is_hovered()` | `bool` | Liegt der Mauszeiger gerade auf der Karte? |
| `is_zoomed()` | `bool` | Wird die Karte gerade vergrößert angezeigt? |
| `screen_quad()` | `PackedVector2Array` | Die vier Ecken der Karte, wie sie gerade auf dem Bildschirm erscheint (inklusive Hover-Zoom, Neigung und Drag-Schwung), in Pixeln. Leer, wenn die Karte hinter der Kamera liegt. |
| `update_hover_point(world_point: Vector3)` | `void` | Übergibt den Raycast-Trefferpunkt (Weltkoordinaten) für den Tilt-Effekt. |
| `set_dragging(dragging: bool)` | `void` | Markiert die Karte als gezogen (Hover aus, laufende Animation stoppt). Setzt der `TableController`. |
| `is_dragging()` | `bool` | Wird die Karte gerade gezogen? |
| `set_drag_sway(sway: Vector2)` | `void` | Neigung während des Ziehens (x = Rollen um Z, y = Nicken um X), Radiant. |
| `move_to(target: Transform3D, delay: float)` | `void` | Lässt die Karte per Tween sanft zu `target` (globale Transform) gleiten. |
| `snap_to(target: Transform3D)` | `void` | Setzt die Karte sofort ohne Animation. |
| `stop_motion()` | `void` | Bricht eine laufende Gleit-Animation ab. |
| `set_ready_glow(on: bool)` | `void` | Dezenter goldener Rand „diese Karte kann eine Fähigkeit aktivieren“ an/aus (für Skripte; Rot/Blau setzt das Spiel selbst). Eine Markierung hat Vorrang. |
| `set_marker_kind(kind: int)` | `void` | Aura um die Karte setzen: 0 = keine, 1 = Angreifer (rot), 2 = Blocker (blau), 3 = ausgewählt/wählt ein Ziel (gold). |
| `set_tapped_state(tapped: bool)` | `void` | Setzt nur den Tapp-Zustand der Karte (schräg, ausgegraut, Tapp-Symbol), ohne Spielregeln (kein Mana, kein Neu-Anordnen) – für Skripte und Tests. |
| `is_tapped()` | `bool` | Ist die Karte getappt (leicht schräg, ausgegraut, Tapp-Symbol)? |
| `get_zone()` | `Zone` | Die Zone, in der die Karte aktuell liegt (None während des Ziehens). |

## Showcase

*Erbt von `Node3D` · Quelle: `rust/src/table/showcase.rs`*

Bühne vor der Kamera: Abdunkelung, Kartenfächer (`Zone`), Titel und `Button3D`-Knöpfe. `open()` stellt sie vor die aktuelle Kamera, `show_cards()` holt Karten herein, `close()` gibt sie an ihre Zonen zurück und blendet aus.

### Funktionen

| Funktion | Rückgabe | Beschreibung |
|---|---|---|
| `open()` | `void` | Stellt die Bühne vor die aktuelle Kamera (Abstand fest, parallel zur Bildebene). |
| `set_texts(title: String, subtitle: String, note: String)` | `void` | Titel, Untertitel und Hinweis setzen (leer = ausblenden). |
| `static button_width()` | `float` | Breite eines Knopfes in dieser Bühne (Welteinheiten). |
| `holds(card: Card3D)` | `bool` | Liegt `card` gerade in der Bühne? |
| `close()` | `void` | Karten zurückgeben, Knöpfe ausblenden, abdunkeln aufheben und danach freigeben. |

## TableCamera

*Erbt von `Camera3D` · Quelle: `rust/src/table/table_camera.rs`*

Tischkamera: Übersicht über den Tisch, Nahaufnahme einer Karte (Rechtsklick) und leichtes Mitführen beim Ziehen einer Karte. Alle Übergänge sind weich.

### Eigenschaften

| Name | Typ | Beschreibung |
|---|---|---|
| `overview_position` | `Vector3` | Kameraposition in der Übersicht. |
| `overview_target` | `Vector3` | Blickpunkt in der Übersicht. |
| `focus_distance` | `float` | Abstand zur Karte in der Nahaufnahme. |
| `follow_strength` | `float` | Wie stark die Kamera einer gezogenen Karte folgt (0 = gar nicht, 1 = voll). |
| `follow_distance` | `float` | Abstand zur gezogenen Karte, wenn follow_strength = 1. |
| `smoothing` | `float` | Glättung der Kamerabewegung (größer = schneller). |

### Funktionen

| Funktion | Rückgabe | Beschreibung |
|---|---|---|
| `show_overview()` | `void` | Zurück zur Tischübersicht. |
| `focus_on(node: Node3D)` | `void` | Nahaufnahme einer Karte (oder eines beliebigen Node3D). |
| `follow(node: Node3D)` | `void` | Leichtes Mitführen (z. B. während eine Karte gezogen wird). |
| `is_focused_on(node: Node3D)` | `bool` | Ist die Kamera gerade auf genau diesen Node fokussiert? |
| `is_overview()` | `bool` | Ist die Kamera gerade in der Übersicht? |

## TableController

*Erbt von `Node3D` · Quelle: `rust/src/table/table_controller.rs`*

Wurzel der Tischszene: Maus-Eingabe (Hover, Drag-and-Drop per Raycast) und Kamera-Fokus (Rechtsklick). Kennt keine Regeln: Jeder Drop wird per [signal card_dropped] gemeldet; der `ForgeTable` spielt die Karte dann über Forge oder legt sie an ihren Platz zurück.

### Eigenschaften

| Name | Typ | Beschreibung |
|---|---|---|
| `camera` | `TableCamera` | Die Tischkamera. |
| `drag_height` | `float` | Höhe der Drag-Ebene über dem Tisch (Tischoberfläche = y dieses Nodes). |
| `drag_follow_speed` | `float` | Wie schnell die gezogene Karte der Maus folgt. |
| `ray_length` | `float` | Länge der Maus-Raycasts in Einheiten. |

### Funktionen

| Funktion | Rückgabe | Beschreibung |
|---|---|---|
| `static move_top_card(from: Zone, to: Zone, delay: float)` | `Card3D` | Bewegt die oberste Karte von `from` nach `to` (z. B. Gegner zieht / spielt aus). |
| `set_language(code: String)` | `void` | Stellt die Sprache des Spiels um: `"de"` = Deutsch, `"en"` = Englisch. HUD, Hinweise und neue Protokolleinträge wechseln sofort; alle Karten werden neu beschriftet. Bereits geschriebene Protokolleinträge bleiben unverändert. |
| `get_language()` | `String` | Aktuelle Sprache: `"de"` oder `"en"`. |
| `get_hovered_card()` | `Card3D` | Die Karte, die gerade unter dem Mauszeiger liegt. |
| `get_dragged_card()` | `Card3D` | Die Karte, die gerade gezogen wird. |

### Signale

| Signal | Beschreibung |
|---|---|
| `card_dropped(card: Card3D, from_zone: Zone, to_zone: Zone)` | Wird ausgelöst, wenn eine Karte per Drag-and-Drop in eine Zone gelegt wurde. |
| `card_clicked(card: Card3D)` | Eine Karte wurde angeklickt (gedrückt und ohne nennenswerte Bewegung losgelassen) – z. B. als Ziel wählen oder Fähigkeit aktivieren. Funktioniert in allen Zonen, auch bei Karten, die man nicht ziehen kann. |

## TargetArrows

*Erbt von `Node3D` · Quelle: `rust/src/table/target_arrows.rs`*

Zeichnet gebogene, leuchtende Zielpfeile über dem Tisch (Angriff, Block, Zielwahl). Wird vom `ForgeTable` erzeugt und jedes Frame mit den aktuellen Pfeilen gefüttert; Höhe des Tisches = y dieses Nodes.

### Funktionen

| Funktion | Rückgabe | Beschreibung |
|---|---|---|
| `set_arrow_pairs(points: PackedVector3Array, colors: PackedColorArray)` | `void` | Setzt Pfeile aus Skripten: `points` enthält je zwei Punkte (Start, Ziel; Welt) pro Pfeil, `colors` eine Farbe pro Pfeil (fehlt sie: Gold). Der `ForgeTable` überschreibt die Pfeile jedes Frame, solange er läuft. |

## FlickerLight

*Erbt von `OmniLight3D` · Quelle: `rust/src/table/tavern.rs`*

Punktlicht, das wie eine Kerzen-/Kaminflamme flackert. Tool-Klasse: flackert auch im Editor (wird von `Arena` und `Tavern` dort mit erzeugt).

### Eigenschaften

| Name | Typ | Beschreibung |
|---|---|---|
| `base_energy` | `float` | Grundhelligkeit des Lichts. |
| `flicker_strength` | `float` | Relative Stärke des Flackerns (0 = ruhig). |
| `flicker_speed` | `float` | Geschwindigkeit des Flackerns (1 = normal). |

## Tavern

*Erbt von `Node3D` · Quelle: `rust/src/table/tavern.rs`*

Tavernen-Kulisse: Holztisch mit Messing-Trennlinie, Kerzen, Krüge, Münzen sowie Gastraum mit Kamin, Fachwerk, Regalen, Fässern, Bank und Hockern – alles prozedural erzeugt. Maßstab: 1 Einheit = 10 cm; Tischoberfläche bei y = 0, Trennlinie bei z = 0. Tool-Klasse: Die Kulisse wird auch im Godot-Editor gebaut (Kinder ohne Owner, werden nicht in die `.tscn` gespeichert).

### Eigenschaften

| Name | Typ | Beschreibung |
|---|---|---|
| `table_size` | `Vector2` | Größe der Tischplatte (x = Breite, y = Tiefe). |
| `table_height` | `float` | Höhe der Tischplatte über dem Boden. |

## Zone

*Erbt von `Area3D` · Quelle: `rust/src/table/zone.rs`*

Ein Bereich auf dem Tisch, der Karten hält und anordnet: Hand, Deck, Friedhof, Spielfeld, Ressourcen-Reihe oder Stack. Karten gleiten per Tween an ihre Plätze; angelegte Karten (Ausrüstung, Auren) liegen versetzt unter ihrem Träger. Die Zone ist eine Area3D auf Kollisions-Layer 2 (Drop-Ziel für Raycasts).

### Eigenschaften

| Name | Typ | Beschreibung |
|---|---|---|
| `kind` | `int (Zonenart)` | Art der Anordnung: 0 = Hand, 1 = Stapel, 2 = Spielfeld, 3 = Ressourcen-Reihe, 4 = Stack. |
| `size` | `Vector3` | Ausdehnung der Drop-Fläche (x = Breite, y = Höhe der Kollisionsbox, z = Tiefe). |
| `accepts_drops` | `bool` | Dürfen Karten hier abgelegt werden? |
| `cards_draggable` | `bool` | Dürfen Karten aus dieser Zone gegriffen werden? |
| `click_to_draw` | `bool` | Klick auf eine Karte zieht die oberste Karte auf die Hand (Deck). |
| `face_up` | `bool` | Karten offen (true) oder verdeckt (false) anzeigen. |
| `show_area` | `bool` | Zonenfläche sichtbar markieren und beim Drüberziehen hervorheben. |
| `hand_radius` | `float` | Hand: Radius des Fächerbogens. |
| `hand_card_angle_degrees` | `float` | Hand: Winkel zwischen zwei Karten (Grad). |
| `hand_max_spread_degrees` | `float` | Hand: maximaler Gesamtwinkel des Fächers (Grad). |
| `pile_spacing` | `float` | Stapel: Höhe je Karte. |
| `grid_columns` | `int` | Spielfeld im Rastermodus: Anzahl Spalten. |
| `grid_spacing` | `Vector2` | Spielfeld im Rastermodus: Abstand der Karten (x, z). |
| `free_placement` | `bool` | Karten bleiben dort liegen, wo sie abgelegt werden (statt Raster). |
| `auto_arrange` | `bool` | Automatische Anordnung nach Kartenart (hat Vorrang vor Raster und freier Ablage), aus Sicht der Seite (lokal −z = zur Tischmitte = „oben“): Helden oben links, Kreaturen oben in der Mitte (mehrere Reihen bei Bedarf), alles andere rechts. Ressourcen liegen in einer eigenen Ressourcen-Zone (unten links). |

### Funktionen

| Funktion | Rückgabe | Beschreibung |
|---|---|---|
| `set_attached(card: Card3D, host: Card3D)` | `void` | Legt `card` an `host` an (Aura, Ausrüstung, Befestigung; CR 301.5, 303.4): Sie liegt dann versetzt unter dem Träger und folgt ihm beim Anordnen. `host` = null löst die Verbindung. Beide Karten müssen in dieser Zone liegen; ordnet nur bei Änderung neu an. |
| `add_card(card: Card3D, index: int, delay: float)` | `void` | Fügt eine Karte ein (index < 0 = ans Ende) und ordnet alle Karten neu an. `delay` verzögert nur die Animation der neuen Karte (z. B. zum Austeilen). |
| `add_card_at(card: Card3D, world_point: Vector3, delay: float)` | `void` | Legt eine Karte frei am Weltpunkt ab (nur mit `free_placement`, sonst wie `add_card` ans Ende). Die Position wird in der Zone begrenzt. |
| `is_free_placement()` | `bool` | Bleiben Karten dort liegen, wo sie abgelegt werden (statt Raster)? |
| `remove_card(card: Card3D)` | `int` | Entfernt eine Karte; gibt ihren bisherigen Index zurück (-1 falls nicht enthalten). |
| `layout()` | `void` | Ordnet alle Karten neu an (Tween zu ihren Slots). |
| `get_card_count()` | `int` | Anzahl der Karten in der Zone. |
| `get_top_card()` | `Card3D` | Oberste/letzte Karte der Zone. |
| `get_cards()` | `Array[Card3D]` | Alle Karten der Zone, unterste bzw. erste zuerst. |
| `accepts_drops()` | `bool` | Dürfen hier Karten abgelegt werden? |
| `cards_draggable()` | `bool` | Dürfen Karten aus dieser Zone gegriffen werden? |
| `click_to_draw()` | `bool` | Zieht ein Klick auf eine Karte dieser Zone eine Karte? (nur ohne Regeln) |
| `set_highlighted(on: bool)` | `void` | Hebt die Zonenfläche hervor (nur sichtbar, wenn `show_area` aktiv ist). |
| `insertion_index(world_point: Vector3)` | `int` | Einfüge-Index für eine Karte, die am Weltpunkt `world_point` abgelegt wird. |

