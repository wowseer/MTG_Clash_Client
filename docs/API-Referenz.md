# API-Referenz (automatisch erzeugt)

Alle Godot-Klassen der Rust-Erweiterung mit Eigenschaften, Funktionen und Signalen.
Erzeugt mit `python3 tools/gen_api_doc.py` aus den Rust-Doku-Kommentaren – dieselben
Texte zeigt der Godot-Editor unter **Hilfe → Hilfe durchsuchen (F1)**.

Sitzplätze werden als Zahl übergeben: `0` = Spieler (unten), `1` = Gegner (oben).

## Inhalt

- [DebugPanel](#debugpanel) – erbt von `CanvasLayer` *(nur Test-Build)*
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
- [StackItem](#stackitem) – erbt von `RefCounted`
- [StackManager](#stackmanager) – erbt von `Node`
- [TurnManager](#turnmanager) – erbt von `Node`
- [Hello](#hello) – erbt von `Node2D`
- [Arena](#arena) – erbt von `Node3D`
- [Card3D](#card3d) – erbt von `Area3D`
- [TableCamera](#tablecamera) – erbt von `Camera3D`
- [TableController](#tablecontroller) – erbt von `Node3D`
- [TargetArrows](#targetarrows) – erbt von `Node3D`
- [FlickerLight](#flickerlight) – erbt von `OmniLight3D`
- [Tavern](#tavern) – erbt von `Node3D`
- [Zone](#zone) – erbt von `Area3D`

## DebugPanel

*Erbt von `CanvasLayer` · Quelle: `rust/src/debug.rs`*

> Existiert nur in Test-Builds (`cargo build`), nicht in `cargo build --release`.

Debug-Tool (nur in Test-Builds): Sprache umschalten (Deutsch / English), Ressourcen hinzufügen, jede Karte auf die Hand geben und eigene Karten erstellen. Wird vom `TableController` automatisch erzeugt und existiert in Release-Builds (`cargo build --release`) nicht.

### Funktionen

| Funktion | Rückgabe | Beschreibung |
|---|---|---|
| `on_toggle()` | `void` | Fenster ein-/ausblenden. |
| `on_language_selected(index: int)` | `void` | Sprache gewählt (0 = Deutsch, 1 = English): stellt das ganze Spiel um, beschriftet alle Karten neu und baut das Debug-Fenster in der neuen Sprache auf. |
| `on_catalog_selected(index: int)` | `void` | Karte in der Liste ausgewählt: große Vorschau zeigen. |
| `on_header_input(event: InputEvent)` | `void` | Maus-Eingabe auf der Titelleiste: linke Taste gedrückt halten und ziehen verschiebt das Debug-Fenster (die Kartenvorschau wandert mit). |
| `on_recipient_selected(index: int)` | `void` | Empfänger gewählt (0 = Spieler, 1 = Gegner): gilt für „Auf die Hand“ und „Erstellen + auf die Hand“. Karten für den Gegner landen verdeckt auf seiner Hand, gehören ihm und werden von der KI regelkonform gespielt. |
| `on_tab_changed(tab: int)` | `void` | Reiter gewechselt: blendet die große Kartenvorschau ein oder aus. |
| `on_add_mana(index: int)` | `void` | "+"-Knopf: legt Mana direkt in den eigenen Manapool – Menge = „Anzahl pro Klick“. `index`: 0 = Blau, 1 = Schwarz, 2 = Grün, 3 = Rot, 4 = Gelb (weißes Mana), 5 = Grau (farblos). Das Mana verfällt am Ende des Zuges (außer mit einer Karte „Mana bleibt erhalten“). |
| `on_infinite_mana_selected(index: int)` | `void` | Auswahl „Unendlich Mana“: 0 = aus, 1 = Spieler, 2 = Gegner, 3 = beide. Wer unendlich Mana hat, kann jede Karte und Fähigkeit bezahlen, ohne Ressourcen zu tappen oder den Pool zu verbrauchen; alle anderen Regeln gelten weiter. |
| `on_clear_mana()` | `void` | Knopf „Manapool leeren“: setzt den eigenen Manapool auf null. |
| `on_untap_resources()` | `void` | Knopf „Alle Ressourcen enttappen“: enttappt alle Karten der eigenen Ressourcen-Reihe. |
| `on_filter_changed(text: String)` | `void` | Suchfeld geändert: filtert die Kartenliste nach Name oder Typ. |
| `on_catalog_activated(index: int)` | `void` | Doppelklick in der Kartenliste. |
| `on_give_selected()` | `void` | Knopf „Auf die Hand“: legt die in der Liste gewählte Karte auf die Hand. |
| `on_kind_selected(index: int)` | `void` | Typ gewechselt: passende Fähigkeiten und Hinweis anzeigen. |
| `on_create(to_hand: bool)` | `void` | Knopf „Erstellen“: baut eine Karte aus Name, Typ, Fähigkeit, Kosten, Angriff und Verteidigung. `to_hand` = true legt sie zusätzlich direkt auf die Hand. |

## ForgeBridge

*Erbt von `Node` · Quelle: `rust/src/forge/bridge.rs`*

Verbindung zur Forge-Regel-Engine: startet die Java-Brücke (`forge-bridge/`) als eigenen Prozess und spricht mit ihr über JSON-Zeilen (Protokoll: `docs/Forge-Schnittstelle.md`). Ablauf: `start_bridge()` → Signal [signal bridge_ready] → `start_match()` → Signale [signal forge_event] (jedes Spielereignis) und [signal decision_requested] (ferngesteuerter Spieler hat Priorität / Mulligan) → Antwort mit `answer()` → [signal game_over]. Ferngesteuert sind die Sitzplätze aus `remote_seats`; alle anderen spielt die Forge-KI. Zahlen in empfangenen Dictionaries sind ganzzahlig (int), wenn sie keinen Nachkommaanteil haben (Godots JSON liefert sonst immer float).

### Eigenschaften

| Name | Typ | Beschreibung |
|---|---|---|
| `java_path` | `String` | Java-Programm (Name im PATH oder absoluter Pfad). Java 17 oder neuer. |
| `forge_dir` | `String` | Forge-Installationsordner (enthält `res/`). Leer = Umgebungsvariable `FORGE_DIR`, sonst `~/Games/Forge`. |
| `forge_jar` | `String` | Forge-Jar mit allen Abhängigkeiten. Leer = erste Datei `forge-gui-desktop-*-jar-with-dependencies.jar` in `forge_dir`. |
| `bridge_jar` | `String` | Gebaute Brücke (`forge-bridge/build.sh`). `res://`-Pfade werden umgerechnet. |
| `java_max_memory_mb` | `int` | Maximaler Java-Heap in MB (`-Xmx`). |
| `autostart` | `bool` | Brücke automatisch in `ready()` starten. |
| `log_messages` | `bool` | Jede empfangene Nachricht in die Godot-Konsole schreiben (zum Debuggen). |

### Funktionen

| Funktion | Rückgabe | Beschreibung |
|---|---|---|
| `start_bridge()` | `bool` | Startet den Java-Prozess. Gibt false zurück, wenn er schon läuft oder nicht starten konnte (dann zusätzlich [signal bridge_error]). Fertig ist er erst mit [signal bridge_ready] (Forge braucht einige Sekunden zum Laden). |
| `stop_bridge()` | `void` | Beendet die Brücke (Befehl "quit", nach 2 s hart beendet). Sendet [signal bridge_stopped] mit -1, falls sie lief. |
| `is_running()` | `bool` | Läuft der Java-Prozess? |
| `is_ready()` | `bool` | Hat Forge fertig geladen ([signal bridge_ready] empfangen)? |
| `send_command(command: Dictionary)` | `bool` | Schickt einen beliebigen Befehl (Dictionary mit "cmd", siehe Protokoll). false, wenn die Brücke nicht läuft. |
| `ping(nonce: int)` | `bool` | Verbindungstest; Antwort kommt als Nachricht `{type: "pong", nonce}`. |
| `start_match(decks: PackedStringArray, remote_seats: PackedInt32Array, names: PackedStringArray, seed: int)` | `bool` | Startet eine Partie. `decks`: Pfade zu Forge-`.dck`-Dateien (`res://` erlaubt) oder Namen von Decks aus Forges Constructed-Sammlung, einer je Sitzplatz (mind. 2). `remote_seats`: Sitzplätze, die MTG Clash steuert (Rest = Forge-KI). `names`: Spielernamen (darf kürzer sein). `seed` < 0 = zufällig. |
| `answer(request_id: int, choice: Variant)` | `bool` | Beantwortet eine Anfrage aus [signal decision_requested]. `choice`: bei "priority" der Index der Option oder -1 (passen), bei "mulligan" true (behalten) / false. |
| `request_state(viewer: int)` | `bool` | Fragt den aktuellen Spielzustand ab → [signal state_received]. `viewer` = Sitzplatz, dessen Handkarten mitgeschickt werden (-1 = keiner). |

### Signale

| Signal | Beschreibung |
|---|---|
| `bridge_ready(protocol: int)` | Die Brücke ist gestartet und Forge geladen; `protocol` = Protokollversion. |
| `message_received(message: Dictionary)` | Jede empfangene Nachricht (vor den speziellen Signalen), z. B. für eigene Auswertung. |
| `forge_event(event: Dictionary)` | Ein Forge-Spielereignis: `{event, text, turn, phase}` (`event` = Forge-Klassenname, z. B. "GameEventCardChangeZone"). |
| `decision_requested(request: Dictionary)` | Ein ferngesteuerter Spieler muss entscheiden: `{id, kind, player, …}`. kind "priority": `options` (Liste `{index, card, card_id, land, spell, text}`) und `state` – Antwort mit `answer()` (Index oder -1 = passen). kind "mulligan": `state`, `cards_to_return` – Antwort true = behalten. |
| `decision_rejected(info: Dictionary)` | Die gewählte Option wurde von Forge abgelehnt (`{player, option, reason}`); es folgt eine neue Anfrage. |
| `match_started(state: Dictionary)` | Die Partie hat begonnen; `state` = Spielzustand (siehe Protokoll). |
| `state_received(state: Dictionary)` | Antwort auf `request_state()`. |
| `game_over(result: Dictionary)` | Die Partie ist vorbei: `{winner, draw, reason}`. |
| `bridge_error(code: String, message: String)` | Fehler der Brücke oder beim Start (`code` z. B. "deck_not_found", "spawn_failed"). |
| `bridge_stopped(exit_code: int)` | Der Java-Prozess ist beendet (`exit_code` -1 = unbekannt / von uns beendet). |

## ForgeTable

*Erbt von `Node` · Quelle: `rust/src/forge/table.rs`*

Spielt eine Partie mit der Forge-Regel-Engine auf dem 3D-Tisch: Karten, Zonen, Kamera, Drag-and-Drop und HUD sind dieselben wie im normalen Spiel, nur die Regeln kommen von Forge (über `ForgeBridge`). Forge-Sitzplatz 0 ist der Spieler (unten, von dir gesteuert), Sitzplatz 1 der Gegner (oben, Forge-KI). Länder liegen in der Ressourcen-Reihe, alle anderen bleibenden Karten auf dem Spielfeld, der Friedhof im Ablagestapel; die Gegnerhand wird verdeckt angezeigt. Bedienung: Hat der Spieler Priorität, leuchten alle Handkarten, die er jetzt spielen kann. Eine solche Karte aus der Hand auf den Tisch ziehen = spielen (Mana zahlt Forge automatisch). Hauptbutton / Leertaste = passen. Mulligan im Auswahlfenster. Ziele, Angreifer und Blocker wählt vorerst die Forge-KI für dich.

### Eigenschaften

| Name | Typ | Beschreibung |
|---|---|---|
| `bridge` | `ForgeBridge` | Verbindung zu Forge. |
| `controller` | `TableController` | Tischsteuerung (Drag-and-Drop, Kamera). Ohne `TurnManager` sind dort alle Karten frei beweglich; dieser Knoten prüft die Züge über Forge. |
| `player_seat` | `PlayerSeat` | Sitzplatz des Spielers (unten) = Forge-Sitzplatz 0. |
| `opponent_seat` | `PlayerSeat` | Sitzplatz des Gegners (oben) = Forge-Sitzplatz 1. |
| `player_deck` | `String` | Deck des Spielers: Forge-`.dck`-Datei (`res://` erlaubt) oder Forge-Deckname. |
| `opponent_deck` | `String` | Deck des Gegners: Forge-`.dck`-Datei (`res://` erlaubt) oder Forge-Deckname. |
| `seed` | `int` | Zufallsstartwert der Partie (< 0 = zufällig). |
| `autostart` | `bool` | Brücke und Partie automatisch beim Start der Szene starten. |
| `refresh_interval` | `float` | Mindestabstand (Sekunden) zwischen zwei Zustandsabfragen, während Forge Ereignisse sendet. |

### Funktionen

| Funktion | Rückgabe | Beschreibung |
|---|---|---|
| `start_match()` | `bool` | Startet die Partie mit `player_deck` gegen `opponent_deck` (Brücke muss bereit sein). Gibt false zurück, wenn das nicht ging. |
| `pass_priority()` | `void` | Passt (Hauptbutton / Leertaste): beantwortet eine offene Prioritätsanfrage mit -1. |
| `hud_choose(index: int)` | `void` | Auswahl im HUD-Fenster: beim Mulligan 0 = behalten, 1 = Mulligan. |
| `get_card(forge_id: int)` | `Card3D` | Karte auf dem Tisch zur Forge-Karten-ID (null, wenn nicht sichtbar). |
| `get_state()` | `Dictionary` | Zuletzt von Forge empfangener Spielzustand (Aufbau siehe `docs/Forge-Schnittstelle.md`). |
| `get_pending_kind()` | `String` | Hat der Spieler gerade eine offene Entscheidung? Liefert deren Art ("priority", "mulligan") oder einen leeren Text. |
| `on_bridge_ready(protocol: int)` | `void` | Brücke bereit → Partie starten (bei `autostart`). |
| `on_state(state: Dictionary)` | `void` | Neuer Spielzustand von Forge (Spielstart oder Abfrage): Tisch abgleichen. |
| `on_forge_event(event: Dictionary)` | `void` | Forge-Ereignis: Zustand bald neu abfragen (gebündelt, siehe `refresh_interval`). |
| `on_decision_requested(request: Dictionary)` | `void` | Entscheidung des Spielers nötig: Zustand übernehmen, spielbare Karten leuchten lassen. |
| `on_game_over(result: Dictionary)` | `void` | Partie vorbei: Sieger merken (HUD zeigt ihn an). |
| `on_bridge_error(code: String, message: String)` | `void` | Fehler der Brücke: als Warnung in die Godot-Konsole schreiben. |
| `on_card_dropped(card: Card3D, from_zone: Zone, to_zone: Zone)` | `void` | Karte wurde auf dem Tisch abgelegt: aus der Hand gespielt → passende Option an Forge; alles andere wird auf den Forge-Stand zurückgesetzt. |

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

Bildschirm-Overlay für den Rundenablauf: Zeitstrahl der Phasen (`PhaseTimeline`, unten rechts über dem Hauptbutton), Spielende-Meldung in der Bildmitte, je ein `SeatBadge`-Medaillon mit Leben (an `player_hero_anchor` / `opponent_hero_anchor`, sonst links oben / unten), am linken Bildrand Spielername und darüber Rundenzahl mit Sanduhr (`HourglassIcon`; Gegner oben, Spieler unten), großer Button „Nächste Phase“ / „Passen“ (unten rechts, auch Leertaste; blau, bei belegtem Stack orange) und das Auswahlfenster für Heldenfähigkeiten. Liest den Zustand jedes Frame vom `TurnManager` – oder, ist keiner gesetzt, von einem `ForgeTable` (Spiel mit der Forge-Regel-Engine; das Auswahlfenster dient dann für den Mulligan).

### Eigenschaften

| Name | Typ | Beschreibung |
|---|---|---|
| `turn_manager` | `TurnManager` | Der `TurnManager`, dessen Zustand angezeigt wird. |
| `forge_table` | `ForgeTable` | Alternative Datenquelle: Forge-Spiel (`ForgeTable`). Wird nur benutzt, wenn `turn_manager` leer ist. |
| `player_hero_anchor` | `Node3D` | 3D-Punkt, an dem das Spieler-Medaillon sitzt (Unterkante mittig, z. B. direkt über der Hand). Leer = Medaillon unten links. Gezoomte oder gezogene Karten liegen sichtbar über dem Medaillon (Aussparung per `CardCutout`). |
| `opponent_hero_anchor` | `Node3D` | 3D-Punkt, an dem das Gegner-Medaillon sitzt (Oberkante mittig, z. B. direkt unter dessen Hand). Leer = Medaillon oben links. |
| `player_mana_anchor` | `Node3D` | 3D-Punkt, an dem die Mana-Symbole des Spielers sitzen (Oberkante mittig, z. B. direkt unter der Ressourcen-Reihe). Leer = rechts neben dem Spieler-Medaillon. |

### Funktionen

| Funktion | Rückgabe | Beschreibung |
|---|---|---|
| `on_hero_ability_0()` | `void` | Knopf der ersten Heldenfähigkeit im Auswahlfenster. |
| `on_hero_ability_1()` | `void` | Knopf der zweiten Heldenfähigkeit im Auswahlfenster. |
| `on_hero_cancel()` | `void` | Knopf „Abbrechen“ im Auswahlfenster für Heldenfähigkeiten. |
| `on_next_pressed()` | `void` | Button „Nächste Phase“ / „Passen“ bzw. Leertaste: ruft `TurnManager.next_phase()` auf (im Forge-Spiel: `ForgeTable.pass_priority()`). |

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

Manapool als Reihe runder Symbole (HUD). Wird vom `TurnHud` jedes Frame über `set_amounts()` gefüttert; die Breite passt sich der Anzahl der Kreise an.

### Funktionen

| Funktion | Rückgabe | Beschreibung |
|---|---|---|
| `set_amounts(amounts: PackedInt64Array)` | `void` | Setzt die Mana-Mengen je Art (Reihenfolge Weiß, Blau, Schwarz, Rot, Grün, Farblos). Leeres Array = nichts anzeigen. Zuwachs lässt den Kreis aufspringen. |

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
| `resource_zone` | `Zone` | Reihe für ausgespielte Ressourcen-Karten. |
| `mana_pool` | `ManaPool` | Manapool dieses Spielers. |
| `life` | `int` | Aktuelle Lebenspunkte (≤ 0 = Partie verloren). |
| `resources_played` | `int` | In dieser Runde bereits ausgespielte Ressourcen-Karten. |
| `infinite_mana` | `bool` | Debug: unendlich Mana – jede Karte und Fähigkeit ist bezahlbar, ohne Ressourcen zu tappen oder den Pool zu verbrauchen. Wird nur vom Debug-Tool eingeschaltet. |

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

### Funktionen

| Funktion | Rückgabe | Beschreibung |
|---|---|---|
| `set_state(seat_name: String, life: int, active: bool)` | `void` | Setzt Name, Lebenspunkte und „am Zug“ auf einmal. Weicht `life` vom bisherigen Wert ab, blitzt der Edelstein kurz auf (rot bei Schaden, grün bei Heilung). |

## StackItem

*Erbt von `RefCounted` · Quelle: `rust/src/game/stack.rs`*

Ein Zauber oder eine Fähigkeit auf dem Stack.

### Eigenschaften

| Name | Typ | Beschreibung |
|---|---|---|
| `source` | `Variant` | Ausführende Karte oder Fähigkeit. |
| `target` | `Variant` | Ziel des Effekts (Karte, Spieler-Index, … oder null). |
| `effect` | `Callable` | Wird beim Auflösen aufgerufen: `func(item: StackItem)`. |
| `condition` | `Callable` | Optional: Prüfung direkt vor dem Auflösen, `func(item) -> bool`. false = Item verpufft ohne Wirkung (z. B. Ziel hat das Spielfeld verlassen). |
| `controller` | `int` | Spieler, der das Item kontrolliert (0/1). |
| `description` | `String` | Text für Anzeige und Protokoll. |
| `payload` | `Variant` | Freie Zusatzdaten (z. B. Index einer Heldenfähigkeit). |

### Funktionen

| Funktion | Rückgabe | Beschreibung |
|---|---|---|
| `static create(source: Variant, target: Variant, effect: Callable, controller: int, description: String)` | `StackItem` | Bequemer Konstruktor: `StackItem.create(quelle, ziel, effekt, controller, text)`. |
| `is_still_valid()` | `bool` | Ist das Ziel noch gültig (nicht freigegeben) und die Bedingung erfüllt? |

## StackManager

*Erbt von `Node` · Quelle: `rust/src/game/stack.rs`*

Verwaltet den Stack und die Prioritätsschleife.

### Funktionen

| Funktion | Rückgabe | Beschreibung |
|---|---|---|
| `counter_item(item: StackItem)` | `bool` | Entfernt ein bestimmtes Item ohne Auflösung (Counterspell). true = gefunden. |
| `push(item: StackItem)` | `void` | Legt ein Item oben auf den Stack. Sein Controller behält die Priorität. |
| `resolve_next()` | `StackItem` | Löst das oberste Item sofort auf (ohne Prioritätsrunde). Gibt das aufgelöste Item zurück (null bei leerem Stack). Die Callable darf selbst wieder `push` aufrufen (z. B. ausgelöste Fähigkeiten). |
| `pass_priority(player: int)` | `int` | Spieler passt. Rückgabe: -1 = nicht an der Reihe, 0 = weitergegeben, 1 = beide gepasst -> oberstes Item wurde aufgelöst, 2 = beide gepasst bei leerem Stack. |
| `set_active_player(player: int)` | `void` | Neuer Zug / neue Phase: aktiver Spieler erhält Priorität. |
| `get_priority_player()` | `int` | Spieler, der gerade Priorität hat (0 = Spieler, 1 = Gegner). |
| `is_empty()` | `bool` | Ist der Stack leer? |
| `size()` | `int` | Anzahl der Items auf dem Stack. |
| `top()` | `StackItem` | Oberstes Item (null bei leerem Stack). |
| `get_items()` | `Array[StackItem]` | Alle Items, unterstes zuerst. |
| `clear()` | `void` | Entfernt alles ohne Auflösung. |

### Signale

| Signal | Beschreibung |
|---|---|
| `item_pushed(item: StackItem)` | Ein Item wurde oben auf den Stack gelegt. |
| `item_resolved(item: StackItem)` | Ein Item wurde aufgelöst (seine Callable wurde ausgeführt). |
| `item_fizzled(item: StackItem)` | Item wurde ohne Wirkung entfernt (Ziel ungültig / Bedingung false). |
| `priority_changed(player: int)` | Die Priorität ging an `player` (0 = Spieler, 1 = Gegner). |
| `all_passed()` | Beide Spieler haben bei leerem Stack gepasst. |
| `item_countered(item: StackItem)` | Item wurde von einem anderen Effekt neutralisiert (Counterspell). |

## TurnManager

*Erbt von `Node` · Quelle: `rust/src/game/turn_manager.rs`*

Steuert die Partie: Rundenablauf als State Machine (Enttappen → Auffrischung → Ziehen → Hauptphase 1 → Angreifer bestimmen → Blocker zuweisen → Schadensberechnung → Hauptphase 2 → Ende), die Prioritätsschleife des Stacks und alle Spielregeln (Timing je Kartentyp, Mana, Kampf, Auren, zustandsbasierte Aktionen, KI). Schnittstelle für `TableController`, `TurnHud` und eigene Skripte. Sitzplätze werden als Zahl angegeben: 0 = Spieler (unten), 1 = Gegner (oben).

### Eigenschaften

| Name | Typ | Beschreibung |
|---|---|---|
| `player_seat` | `PlayerSeat` | Sitzplatz des menschlichen Spielers (unten). |
| `opponent_seat` | `PlayerSeat` | Sitzplatz des Gegners (oben, normalerweise KI). |
| `draw_count` | `int` | Karten, die in der Zieh-Phase gezogen werden. |
| `start_delay` | `float` | Wartezeit vor dem ersten Zug (Austeilen der Starthände). |
| `phase_delay_scale` | `float` | Faktor für alle automatischen Phasen-Verzögerungen. |
| `stack_manager` | `StackManager` | Stack mit Prioritätsschleife für Zauber und Fähigkeiten. |
| `stack_zone` | `Zone` | Zone, in der Zauberkarten liegen, solange sie auf dem Stack sind. |
| `priority_delay` | `float` | Bedenkzeit der KI, bevor sie passt oder antwortet (Sekunden). |
| `auto_pass_human` | `bool` | Passt der Mensch automatisch, wenn er keinen spielbaren Sofortzauber hat? Standard aus: Der Stack läuft, bis der Mensch „Weiter“ drückt. |

### Funktionen

| Funktion | Rückgabe | Beschreibung |
|---|---|---|
| `start_game()` | `void` | Startet die Partie (der Spieler beginnt). |
| `next_phase()` | `void` | Button „Nächste Phase“: nur wirksam, wenn die Phase auf einen Menschen wartet. |
| `resolve_stack_item(item: StackItem)` | `void` | Wird vom StackManager beim Auflösen eines Items aufgerufen. |
| `get_stack_items()` | `Array[StackItem]` | Alle Elemente auf dem Stack, unterstes zuerst (für die Stack-Anzeige). |
| `get_stack_lines()` | `PackedStringArray` | Stack-Einträge als Text, oberster zuerst. |
| `get_priority_seat()` | `int` | Spieler mit Priorität bei vollem Stack (-1 = Stack leer). |
| `get_button_text()` | `String` | Beschriftung für den HUD-Button. |
| `is_waiting_for_input()` | `bool` | Wartet das Spiel auf eine Eingabe des Menschen (Button „Nächste Phase“ bzw. „Passen“)? |
| `get_phase_index()` | `int` | Index der aktuellen Phase in `get_phase_titles()` (−1 = noch nicht gestartet oder Spielende). |
| `get_phase_title()` | `String` | Name der aktuellen Phase. |
| `get_phase_titles()` | `PackedStringArray` | Namen aller Phasen in ihrer Reihenfolge. |
| `get_hint()` | `String` | Hinweistext für das HUD: was der Spieler gerade tun kann. |
| `get_log()` | `PackedStringArray` | Die letzten Protokolleinträge, ältester zuerst. |
| `get_turn()` | `int` | Rundennummer (jeder Zug eines Spielers zählt als eine Runde). |
| `get_active_seat()` | `int` | 0 = Spieler, 1 = Gegner, -1 = noch nicht gestartet. |
| `get_seat_name(seat: int)` | `String` | Name des Sitzplatzes `seat` (0 = Spieler, 1 = Gegner). |
| `get_life(seat: int)` | `int` | Lebenspunkte von `seat` (0 = Spieler, 1 = Gegner). |
| `get_resource_counts(seat: int)` | `Vector2i` | Ressourcen-Karten: (ungetappt, gesamt). |
| `get_pool_amounts(seat: int)` | `PackedInt64Array` | Mana im Pool von `seat`, je Art in der Reihenfolge Weiß, Blau, Schwarz, Rot, Grün, Farblos (Indizes 0–5). Leer bei Debug „unendlich Mana“. |
| `get_pool_bbcode(seat: int)` | `String` | Manapool als BBCode (farbig) für das HUD, z. B. "[color=#e64d33]Rot 2[/color]". |
| `get_winner_name()` | `String` | Name des Gewinners (leer, solange die Partie läuft). |
| `can_drag(card: Card3D, zone: Zone)` | `bool` | Darf der Mensch diese Karte greifen? (eigene Hand / eigenes Spielfeld) |
| `can_play_card(card: Card3D, seat: int)` | `bool` | Darf `seat` (0 = Spieler, 1 = Gegner) diese Handkarte jetzt spielen? Prüft Zug, Phase, Stack und Mana je nach Kartentyp (siehe `timing.rs`). |
| `get_play_error(card: Card3D, seat: int)` | `String` | Grund, warum die Karte nicht gespielt werden darf ("" = erlaubt). |
| `resolve_drop(card: Card3D, from: Zone, to: Zone)` | `Zone` | Wird vor dem Ablegen gefragt und liefert die tatsächliche Zielzone (null = nicht erlaubt, Karte gleitet zurück). - Umsortieren innerhalb einer Zone: immer erlaubt. - Hand -> eigene Tischzone: Timing-Regeln je Kartentyp; Ressourcen landen in der Reihe, Zauber werden aufgelöst und landen auf dem Ablagestapel. |
| `resolve_drop_at(card: Card3D, from: Zone, to: Zone, point: Vector3)` | `Zone` | Wie `resolve_drop`, mit Ablagepunkt (Welt) als Wunschplatz für bleibende Karten. |
| `can_aim_from(card: Card3D)` | `bool` | Kann von dieser Karte ein Zielpfeil gezogen werden? (Zielwahl: eigene ungetappte Kreatur beim Blocken, eigene Ausrüstung in der eigenen Hauptphase.) |
| `aim_at(source: Card3D, target: Card3D)` | `bool` | Zielpfeil von `source` auf `target` losgelassen: Blocker zuweisen bzw. Ausrüstung anlegen. true = ausgeführt (bei ungültigem Ziel steht der Grund im Protokoll). |
| `is_human_hand(zone: Zone)` | `bool` | Ist `zone` die Hand eines menschlichen Spielers? |
| `resolve_hand_drop(card: Card3D, from: Zone, point: Vector3)` | `Zone` | Karte aus der Hand irgendwo auf dem Tisch losgelassen: wird auf das eigene Spielfeld ausgespielt (Ressourcen in die Ressourcen-Reihe), der Platz wird automatisch bestimmt. Liefert die Zielzone (Stack, Spielfeld, Ressourcen-Reihe) oder null (nicht erlaubt). |
| `is_legal_aura_target(aura: Card3D, target: Card3D)` | `bool` | Darf diese Aura die Karte `target` verzaubern? (für die Zielmarkierung beim Ziehen) |
| `resolve_aura_drop(card: Card3D, from: Zone, target: Card3D)` | `Zone` | Aura aus der Hand auf eine bleibende Karte ziehen. Liefert die Zielzone (Stack) oder null. |
| `on_card_clicked(card: Card3D)` | `bool` | Klick auf eine Karte. true = Klick verbraucht (kein Drag starten). - eigene Ressource in der Reihe: tappen (+1 Mana), jederzeit; erneuter Klick auf eine so getappte Ressource nimmt den Tapp zurück, solange ihr Mana unbenutzt im Pool liegt - eigener Held: Fähigkeitenauswahl im HUD öffnen - eigene Ausrüstung: auswählen, danach eigene Kreatur anklicken = anlegen - sonst entscheidet die Phase (z. B. Kampfauswahl) |
| `get_hero_choice_name()` | `String` | Name des Helden mit offener Fähigkeitenauswahl ("" = keine). |
| `get_hero_choices()` | `PackedStringArray` | Fähigkeiten des gewählten Helden als Text; nicht verfügbare mit Grund. |
| `is_hero_choice_available(index: int)` | `bool` | Ist Fähigkeit `index` des gewählten Helden gerade aktivierbar? |
| `activate_hero_ability(index: int)` | `bool` | Aktiviert Fähigkeit `index` des gewählten Helden. true = ausgeführt. |
| `cancel_hero_choice()` | `void` | Schließt das Auswahlfenster für Heldenfähigkeiten ohne Aktivierung. |

### Signale

| Signal | Beschreibung |
|---|---|
| `phase_changed(index: int, title: String)` | Neue Phase betreten (Index in Phase::ORDER, -1 = Spielende). |
| `game_over(winner: String)` | Die Partie ist entschieden; `winner` ist der Name des Gewinners. |

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

## Card3D

*Erbt von `Area3D` · Quelle: `rust/src/table/card3d.rs`*

Eine Spielkarte auf dem 3D-Tisch: Mesh mit Vorder-/Rückseite und Beschriftung, Hover-Neigung und Hover-Zoom (offene Karten werden groß und lesbar), Raycast-Ziel (Kollisions-Layer 1), Gleit-Animationen per Tween sowie alle Spielwerte (Typ, Kosten, Angriff/Verteidigung, Fähigkeiten, Tapp-Zustand).

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
| `card_name` | `String` | Kartenname. |
| `card_kind` | `int (Kartentyp)` | Kartentyp: 0 = Kreatur, 1 = Ressource, 2 = Sofortzauber, 3 = Hexerei, 4 = Held, 5 = Ausrüstung, 6 = Artefakt, 7 = Verzauberung, 8 = Aura. |
| `mana_cost` | `ManaCost` | Manakosten (leer = kostenlos). |
| `produces` | `int (Manafarbe)` | Nur Ressourcen: Farbe des Manas, das Tappen erzeugt. |
| `attack` | `int` | Grundangriff (ohne Boni durch Ausrüstung, Auren, Verzauberungen). |
| `defense` | `int` | Grundverteidigung (ohne Boni und ohne erlittenen Schaden). |
| `ability` | `int (Fähigkeit)` | Fähigkeit einer Kreatur: 0 = keine, 1 = Heiler, 2 = Feuer, 3 = Raufbold. |
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
| `get_current_attack()` | `int` | Angriff inklusive temporärer Boni. |
| `get_remaining_defense()` | `int` | Verteidigung abzüglich erlittenen Schadens in dieser Runde. |
| `set_ready_glow(on: bool)` | `void` | Dezenter goldener Rand „diese Karte kann eine Fähigkeit aktivieren“ an/aus (für Skripte; Rot/Blau setzt das Spiel selbst). Eine Markierung hat Vorrang. |
| `set_marker_kind(kind: int)` | `void` | Aura um die Karte setzen: 0 = keine, 1 = Angreifer (rot), 2 = Blocker (blau), 3 = ausgewählt/wählt ein Ziel (gold). |
| `set_tapped_state(tapped: bool)` | `void` | Setzt nur den Tapp-Zustand der Karte (schräg, ausgegraut, Tapp-Symbol), ohne Spielregeln (kein Mana, kein Neu-Anordnen) – für Skripte und Tests. |
| `is_tapped()` | `bool` | Ist die Karte getappt (leicht schräg, ausgegraut, Tapp-Symbol)? |
| `is_artifact()` | `bool` | Artefakt (inkl. Ausrüstung)? |
| `is_enchantment()` | `bool` | Verzauberung (inkl. Aura)? |
| `get_enchanted()` | `Card3D` | Kreatur, die diese Aura verzaubert (null = keine). |
| `get_willpower()` | `int` | Aktuelle Willenskraft (nur Helden). |
| `get_equipped_to()` | `Card3D` | Kreatur, an der diese Ausrüstung angelegt ist (null = frei). |
| `get_zone()` | `Zone` | Die Zone, in der die Karte aktuell liegt (None während des Ziehens). |

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

Wurzel der Tischszene: Maus-Eingabe (Hover, Klick, Drag-and-Drop per Raycast), Kamera-Fokus (Rechtsklick) und Erzeugung der Demo-Decks. Ist `turn_manager` gesetzt, gelten alle Spielregeln; ohne ihn lassen sich Karten frei bewegen. In Test-Builds erzeugt er außerdem das Debug-Tool.

### Eigenschaften

| Name | Typ | Beschreibung |
|---|---|---|
| `camera` | `TableCamera` | Die Tischkamera. |
| `turn_manager` | `TurnManager` | Optional: Regelwerk/Rundenablauf. Ohne ihn ist alles frei spielbar. |
| `hand_zone` | `Zone` | Zone, in die gezogene Karten (vom Deck) wandern. |
| `deck_zone` | `Zone` | Zone, aus der die Demo-Karten erzeugt werden. |
| `drag_height` | `float` | Höhe der Drag-Ebene über dem Tisch (Tischoberfläche = y dieses Nodes). |
| `drag_follow_speed` | `float` | Wie schnell die gezogene Karte der Maus folgt. |
| `ray_length` | `float` | Länge der Maus-Raycasts in Einheiten. |
| `demo_deck_size` | `int` | Anzahl der Demo-Karten im Deck (0 = keine Demo). |
| `demo_hand_size` | `int` | Anzahl der Demo-Karten, die zu Beginn ausgeteilt werden. |
| `opponent_hand_zone` | `Zone` | Handzone des Gegners (Demo: verdeckte Starthand). |
| `opponent_deck_zone` | `Zone` | Deckzone des Gegners (Demo: wird mit Karten gefüllt). |
| `opponent_battlefield_zone` | `Zone` | Spielfeld des Gegners (Demo: optionale Startkarten). |
| `demo_opponent_hand_size` | `int` | Demo: verdeckte Handkarten des Gegners. |
| `demo_opponent_battlefield_size` | `int` | Demo: offene Karten, die der Gegner schon ausgespielt hat. |

### Funktionen

| Funktion | Rückgabe | Beschreibung |
|---|---|---|
| `draw_card(delay: float)` | `Card3D` | Zieht die oberste Karte des Decks auf die Hand. |
| `static move_top_card(from: Zone, to: Zone, delay: float)` | `Card3D` | Bewegt die oberste Karte von `from` nach `to` (z. B. Gegner zieht / spielt aus). |
| `set_language(code: String)` | `void` | Stellt die Sprache des Spiels um: `"de"` = Deutsch, `"en"` = Englisch. HUD, Hinweise und neue Protokolleinträge wechseln sofort; alle Karten werden neu beschriftet. Bereits geschriebene Protokolleinträge bleiben unverändert. |
| `get_language()` | `String` | Aktuelle Sprache: `"de"` oder `"en"`. |
| `get_dragged_card()` | `Card3D` | Die Karte, die gerade gezogen wird. |

### Signale

| Signal | Beschreibung |
|---|---|
| `card_dropped(card: Card3D, from_zone: Zone, to_zone: Zone)` | Wird ausgelöst, wenn eine Karte per Drag-and-Drop in eine Zone gelegt wurde. |
| `card_drawn(card: Card3D)` | Wird ausgelöst, wenn eine Karte vom Deck auf die Hand gezogen wurde. |

## TargetArrows

*Erbt von `Node3D` · Quelle: `rust/src/table/target_arrows.rs`*

Zeichnet gebogene, leuchtende Zielpfeile über dem Tisch (Angriff, Block, Zielwahl). Wird vom `TableController` erzeugt und jedes Frame mit den aktuellen Pfeilen gefüttert; Höhe des Tisches = y dieses Nodes.

### Funktionen

| Funktion | Rückgabe | Beschreibung |
|---|---|---|
| `set_arrow_pairs(points: PackedVector3Array, colors: PackedColorArray)` | `void` | Setzt Pfeile aus Skripten: `points` enthält je zwei Punkte (Start, Ziel; Welt) pro Pfeil, `colors` eine Farbe pro Pfeil (fehlt sie: Gold). Der `TableController` überschreibt die Pfeile jedes Frame, solange er läuft. |

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

