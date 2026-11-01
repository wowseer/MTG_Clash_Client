# MTG Clash – Dokumentation

Ein Kartenspiel in einer 3D-Taverne. Die Spiellogik ist in **Rust** (godot-rust / gdext 0.5.5)
geschrieben und wird als GDExtension in **Godot 4.7** geladen. Das Spiel ist zweisprachig
(**Deutsch / Englisch**, siehe Abschnitt 6).

Diese Datei gibt den Überblick. Jede einzelne Klasse, Eigenschaft, Funktion und jedes Signal
ist beschrieben

- **direkt in Godot:** Editor → *Hilfe → Hilfe durchsuchen* (F1) → Klassenname eingeben,
  z. B. `TurnManager`, `Card3D`, `Zone`;
- **als Datei:** [API-Referenz.md](API-Referenz.md).

Beides wird aus denselben Rust-Doku-Kommentaren (`///`) erzeugt (gdext-Feature `register-docs`).

---

## 1. Projekt bauen und starten

| Schritt | Befehl |
|---|---|
| Test-Build (mit Debug-Tool) | `cd rust && cargo build` |
| Release-Build (ohne Debug-Tool) | `cd rust && cargo build --release` |
| Unit-Tests | `cd rust && cargo test` |
| Spiel starten | Godot-Projekt `godot/` öffnen, F5 (Hauptszene `table.tscn`) |
| API-Referenz neu erzeugen | `python3 tools/gen_api_doc.py` |

Nach jedem Rust-Build das Spiel neu starten, damit die neue Bibliothek geladen wird.
`godot/rust.gdextension` zeigt im Debug auf `rust/target/debug/`, im Export auf `rust/target/release/`.

---

## 2. Aufbau der Szene `table.tscn`

```
Table (TableController)          Wurzel: Maus-Eingabe, Drag-and-Drop, Demo-Decks
├── WorldEnvironment, Lantern    Licht und Atmosphäre
├── Tavern (Tavern)              prozedurale Taverne und Tisch
├── Camera (TableCamera)         Übersicht / Nahaufnahme
├── Player (PlayerSeat)          Spieler unten
│   ├── Battlefield (Zone)       Spielfeld (freie Platzierung)
│   ├── Deck, Discard (Zone)     Deck, Friedhof
│   ├── Resources (Zone)         Ressourcen-Reihe
│   ├── ManaPool (ManaPool)      Manavorrat
│   └── Hand (Zone)              Handkarten (Fächer)
├── Opponent (PlayerSeat)        Gegner oben (KI), gleicher Aufbau, um 180° gedreht
├── StackManager (StackManager)  Stack + Prioritätsschleife
├── StackZone (Zone)             sichtbarer Stack in der Tischmitte
├── TurnManager (TurnManager)    Rundenablauf und alle Spielregeln
└── TurnHud (TurnHud)            Bildschirm-Anzeige
```

In Test-Builds hängt der `TableController` zusätzlich ein `DebugPanel` an (nicht in der Szenendatei).

### Dateien

| Ordner / Datei | Inhalt |
|---|---|
| `rust/src/table/` | Darstellung: `Card3D`, `Zone`, `TableCamera`, `TableController`, `Tavern`, Texturen |
| `rust/src/game/` | Regeln: `TurnManager`, Phasen, `GameState`, Timing, Mana, Stack, Karten, HUD |
| `rust/src/debug.rs` | Debug-Tool (nur Test-Build) |
| `rust/src/i18n.rs` | Zweisprachigkeit: Sprache, `tr!`/`trf!`-Makros, Namenstabelle |
| `docs/` | diese Dokumentation und die API-Referenz |
| `tools/gen_api_doc.py` | erzeugt `docs/API-Referenz.md` |

---

## 3. Bedienung

| Aktion | Wirkung |
|---|---|
| Maus über Karte | Karte neigt sich; offene Karten werden groß und lesbar angezeigt (Hover-Zoom) |
| Karte aus der Hand ziehen und loslassen | ausspielen (Regeln prüfen Phase, Stack und Mana) |
| Aura ziehen | über einer gültigen Zielkarte loslassen (Ziel wird gelb markiert) |
| Klick auf eigene Ressource | tappen → +1 Mana in den Pool |
| Klick auf eigenen Helden | Fenster mit seinen Fähigkeiten |
| Klick auf Ausrüstung, dann auf eigene Kreatur | Ausrüstung anlegen (kostet Mana) |
| Klick auf Artefakt mit Fähigkeit | Fähigkeit aktivieren (tappt das Artefakt) |
| Angreifer-Phase: Klick auf Kreaturen | als Angreifer markieren (rot) |
| Blocker-Phase: eigene Kreatur, dann Angreifer | Blocker zuweisen (gelb → blau) |
| Rechtsklick auf Karte | Kamera-Nahaufnahme; erneut oder Esc = Übersicht |
| Button „Nächste Phase“ / „Passen“ bzw. Leertaste | Phase beenden bzw. Priorität abgeben |

---

## 4. Spielregeln

### 4.1 Rundenablauf (`TurnManager`, `phases.rs`)

```
Enttappen → Auffrischung → Ziehen → Hauptphase 1 → Angreifer bestimmen ─┬→ Blocker zuweisen → Schadensberechnung ─┬→ Hauptphase 2 → Ende ─┐
    ↑                                                                    └──────────── (keine Angreifer) ──────────┘                       │
    └──────────────────────────────────── nächster Spieler ─────────────────────────────────────────────────────────────────────────────┘
```

| Phase | Was passiert | Weiter |
|---|---|---|
| Enttappen | alle eigenen Karten enttappen (außer gesperrten), „1 Ressource pro Runde“ zurücksetzen | automatisch |
| Auffrischung | Fähigkeiten „zu Beginn der Runde“ (Heiler, Feuer) | automatisch |
| Ziehen | 1 Karte (der Startspieler zieht in Runde 1 nicht) | automatisch |
| Hauptphase 1 / 2 | Karten ausspielen, Helden, Ausrüstung, Artefakte | Button |
| Angreifer bestimmen | Angreifer wählen; sie werden getappt | Button |
| Blocker zuweisen | Verteidiger weist Blocker zu | Button (wenn Mensch verteidigt) |
| Schadensberechnung | Kampfschaden; ungeblockt trifft den Spieler | automatisch |
| Ende | temporäre Effekte, Schaden und Markierungen entfernen, Spielerwechsel | automatisch |

Mana im Pool bleibt über alle Phasen eines Zuges erhalten und wird **am Ende des Zuges auf null gesetzt** –
außer ein Spieler hat eine Karte mit „Mana bleibt erhalten“ im Spiel (siehe 4.3).
Solange etwas auf dem Stack liegt, pausiert die Phase.
Beide Spieler starten mit 20 Leben; bei 0 Leben ist die Partie verloren.

### 4.2 Kartentypen und Timing (`cards.rs`, `timing.rs`)

| Typ | Wann spielbar | Wohin |
|---|---|---|
| Ressource | eigene Hauptphase, max. 1 pro Runde, kostenlos | Ressourcen-Reihe |
| Sofortzauber | **jederzeit**, auch im gegnerischen Zug | Stack → Wirkung → Friedhof |
| Hexerei | eigene Hauptphase, Stack leer | Stack → Wirkung → Friedhof |
| Kreatur | eigene Hauptphase, Stack leer | Stack → Spielfeld |
| Held | eigene Hauptphase, Stack leer | Stack → Spielfeld |
| Ausrüstung (Artefakt) | eigene Hauptphase, Stack leer | Stack → Spielfeld; danach für Mana anlegen |
| Artefakt | eigene Hauptphase, Stack leer; nur generische Kosten | Stack → Spielfeld |
| Verzauberung | eigene Hauptphase, Stack leer | Stack → Spielfeld |
| Aura | eigene Hauptphase, Stack leer, **braucht ein gültiges Ziel** | Stack → hängt an ihrem Ziel |

Prüfung im Code: `TurnManager.can_play_card(karte, sitz)` / `get_play_error(karte, sitz)`.

**Helden:** einmal pro Runde eine Fähigkeit (ändert die Willenskraft); bei 0 Willenskraft zerstört.
**Ausrüstung:** Klick auf Ausrüstung, dann eigene Kreatur → Bonus auf die Kreatur. Stirbt die Kreatur, bleibt die Ausrüstung liegen.
**Auren:** haben eine Zielklasse (Kreatur, Ressource, Artefakt, Verzauberung, Held oder jede bleibende Karte)
und eine Wirkung (Werte ±, Zusatzmana, enttappt nicht, Willenskraft).

### 4.3 Mana (`mana.rs`)

Farben: Weiß, Blau, Schwarz, Rot, Grün, Farblos (als Zahl 0–5).
Kostenschreibweise: Zahl = generisch, Buchstaben `W B S R G F` je Farbe – z. B. `2RG`.
Farbige Anteile müssen mit genau dieser Farbe bezahlt werden, generische mit beliebigem Mana.
Beim Ausspielen wird zuerst aus dem Pool bezahlt; fehlt etwas, tappt das Spiel passende Ressourcen automatisch.

**Zugende:** Am Ende jedes Zuges werden die Manapools **beider** Spieler auf null gesetzt.
Ausnahme: Ein Spieler hat eine bleibende Karte mit dem Dauereffekt **„Mana bleibt erhalten“**
(`StaticEffect::KeepMana`) im Spiel – dann behält er sein unverbrauchtes Mana (Protokoll:
„Spieler behält 3 Mana (Sparstrumpf)“). Beispielkarte: Artefakt **Sparstrumpf** (Kosten 2,
im Debug-Katalog); im Debug-Reiter „Erstellen“ als Artefakt-Fähigkeit „Passiv: Mana bleibt am Zugende erhalten“.

### 4.4 Stack und Priorität (`stack.rs`)

- Wer etwas auf den Stack legt, behält die Priorität.
- Aufgelöst wird das **oberste** Item erst, wenn **beide nacheinander passen** (LIFO).
- Wer keinen spielbaren Sofortzauber hat, passt automatisch nach kurzer Bedenkzeit.
- Die KI antwortet höchstens einmal pro Item mit einem sinnvollen Sofortzauber.
- **Hausverbot** (Counterspell, BB) neutralisiert den obersten gegnerischen Zauber.
- Ein Item verpufft, wenn sein Ziel vor dem Auflösen verschwindet.

### 4.5 Zustandsbasierte Aktionen (`GameState.check_state_based_actions`)

Werden jeden Frame geprüft:
1. globale Boni aus Artefakten und Verzauberungen neu berechnen,
2. Kreaturen mit Verteidigung ≤ 0 und Helden ohne Willenskraft zerstören,
3. Auren, deren Karte das Spiel verlassen hat, auf den Friedhof ihres Besitzers legen.

---

## 5. Debug-Tool (nur Test-Build)

Button **DEBUG** am linken Bildschirmrand. Existiert nur in `cargo build`, nicht in `cargo build --release`.

Oben im Fenster: **Sprache / Language** (Deutsch / English) – stellt das ganze Spiel um.

| Reiter | Funktion |
|---|---|
| Mana | „+“ für Blau, Schwarz, Grün, Rot, Gelb (= weißes Mana), Grau (= farblos) legt Mana **direkt in den Manapool** (Menge = „Anzahl pro Klick“, 1–99); es verfällt am Zugende wie jedes Mana. Dazu „Manapool leeren“ und „Alle Ressourcen enttappen“ |
| Karten | alle Karten mit Suche; Doppelklick / „Auf die Hand“; große Vorschau mit Regeltext |
| Erstellen | eigene Karte aus Name, Typ, Fähigkeit, Kosten, Angriff, Verteidigung |

Erstellte Karten gelten nur bis zum Neustart.

---

## 6. Sprache (Deutsch / Englisch)

- **Umschalten:** im Debug-Tool (*Sprache / Language*) oder per Code
  `$Table.set_language("en")` / `$Table.set_language("de")`; abfragen mit `$Table.get_language()`.
  Standard ist Deutsch.
- **Sofort umgestellt:** HUD, Phasen, Hinweise, Fehlermeldungen, Kartenbeschriftungen,
  Kartenvorschau und das Debug-Fenster.
- **Protokoll:** neue Einträge erscheinen in der neuen Sprache; ältere bleiben, wie sie geschrieben wurden.
- **Dokumentation** (Godot-Hilfe, diese Datei) bleibt Deutsch.

### Neue Texte hinzufügen (`rust/src/i18n.rs`)

| Fall | Schreibweise |
|---|---|
| fester Text | `tr!("Abbrechen", "Cancel")` |
| Text mit Werten | `trf!("{n} Karten", "{n} cards")` oder `trf!("{} zieht", "{} draws", name)` |
| Kartenname / Eigenname | deutscher Name im Code, englischer Eintrag in `i18n::name()` |

Kartennamen sind intern immer deutsch (sie dienen als Schlüssel) und werden erst bei der
Anzeige übersetzt. Selbst erstellte Karten aus dem Debug-Tool behalten ihren Namen.

---

## 7. Eigene Skripte (GDScript) – Beispiele

```gdscript
@onready var tm: TurnManager = $Table/TurnManager

func _ready() -> void:
    tm.phase_changed.connect(func(index, title): print("Neue Phase: ", title))
    tm.game_over.connect(func(winner): print(winner, " gewinnt"))

func karte_pruefen(karte: Card3D) -> void:
    if not tm.can_play_card(karte, 0):
        print(tm.get_play_error(karte, 0))   # z. B. "Nicht genug Mana."
```

```gdscript
# Eigener Zauber über den Stack
var item := StackItem.create(quelle, null, func(it): print("aufgelöst"), 0, "Mein Zauber")
$Table/StackManager.push(item)
```

---

## 8. Bekannte Vereinfachungen

- Bei leerem Stack muss nicht beidseitig gepasst werden; die Phasen steuern Button und Automatik.
- Zauber wählen ihr Ziel automatisch (z. B. „stärkste gegnerische Kreatur“); nur Auren werden gezielt gelegt.
- Kreaturen können keine Helden angreifen.
- Neu ausgespielte Kreaturen dürfen sofort angreifen.
- Die Demo-Decks (je 30 Karten, Spieler Rot/Grün, Gegner Blau/Schwarz) sind Platzhalter.
