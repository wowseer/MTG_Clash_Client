# MTG Clash – Dokumentation

Ein Kartenspiel in einer 3D-Arena. Die Spiellogik ist in **Rust** (godot-rust / gdext 0.5.5)
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
├── Arena (Arena)                prozedurale Arena mit Spielbrett (Kulisse)
├── Camera (TableCamera)         Übersicht / Nahaufnahme
├── Player (PlayerSeat)          Spieler unten
│   ├── Battlefield (Zone)       Spielfeld (freie Platzierung)
│   ├── Deck, Discard (Zone)     Deck, Friedhof
│   ├── Resources (Zone)         Ressourcen-Reihe
│   ├── ManaPool (ManaPool)      Manavorrat
│   ├── HeroAnchor (Marker3D)    Position des Medaillons (direkt über der Hand)
│   └── Hand (Zone)              Handkarten (Fächer, nah an der Kamera am unteren Bildrand)
├── Opponent (PlayerSeat)        Gegner oben (KI), gleicher Aufbau, um 180° gedreht;
│                                Hand am oberen Bildrand, Medaillon direkt darunter
├── StackManager (StackManager)  Stack + Prioritätsschleife
├── StackZone (Zone)             sichtbarer Stack in der Tischmitte
├── TurnManager (TurnManager)    Rundenablauf und alle Spielregeln
└── TurnHud (TurnHud)            Bildschirm-Anzeige
```

In Test-Builds hängt der `TableController` zusätzlich ein `DebugPanel` an (nicht in der Szenendatei).

### Kulisse und Bildschirm-Anzeige

Der Look orientiert sich an digitalen Sammelkartenspielen (Spielbrett von oben, Medaillons mit
Lebenspunkten, großer Aktionsbutton), ist aber eine eigene Gestaltung ohne fremde Grafiken –
alles wird prozedural in Rust erzeugt.

**`Arena` (`rust/src/table/arena.rs`)** – Standard-Kulisse von `table.tscn`:

| Element | Beschreibung |
|---|---|
| Spielbrett | Sandstein mit gemeißelten Rillen (Normal-Map aus einem Höhenbild): doppelter Rahmen mit Eck-Ringen, große Raute, Mittellinie (z = 0) |
| Runenkreis | in der Brettmitte; Ringe, Stern und Glyphen leuchten türkis (Emissionstextur, Stärke `rune_glow`) |
| Kartenfächer | vertiefte Felder unter jeder Stapel- (Deck/Friedhof) und Ressourcen-`Zone` der Szene – werden beim Start aus den Zonen gelesen, passen sich also an verschobene Zonen an |
| Rand | flacher Steinrand (nur 0,06 über dem Brett) mit Goldeinlage; in den Ecken eingelassene Leuchtkristalle in Goldfassung – das Brett wirkt wie eine flache Spielmatte |
| Umgebung | gestufter Sockel, Schieferboden, zwei Feuerschalen, Säulenruinen, runde Arenamauer mit Bannern in Spieler- (`player_color`) und Gegnerfarbe (`opponent_color`) |

Einstellbar im Inspector: `board_size`, `texture_resolution` (Pixel pro Einheit der Brett-Textur),
`rune_glow`, `player_color`, `opponent_color`. Die alte Kulisse `Tavern` existiert weiter: in
`table.tscn` den Knoten `Arena` durch einen `Tavern`-Knoten ersetzen (dann `Lantern.light_energy`
wieder auf ca. 10 stellen, die Arena braucht wegen des hellen Steins nur ca. 3).

**Kamera und Aufteilung** (wie in digitalen Kartenspielen): Die Übersichtskamera schaut steil
von oben (ca. 75°) aus großer Entfernung mit kleinem Blickwinkel (`fov` 30°,
`Camera.overview_position` (0, 23.2, 6.4)) – dadurch fast keine Perspektive, das Spielfeld wirkt
flach wie in Arena. Nahaufnahme- und Mitführ-Abstand (`focus_distance` 2,4, `follow_distance` 6,6)
sind an den kleinen Blickwinkel angepasst. Die eigene Hand liegt vergrößert (Skalierung 2,8) knapp
über dem Brett vor dessen Vorderkante am unteren Bildrand (die untere Kartenhälfte ist
abgeschnitten, Hover zoomt die ganze Karte), die Gegnerhand am oberen Rand. Beide Spielfelder sind 3,0 Einheiten tief (Mittellinie bis
z = ±3,2), damit vor ihnen Platz für die Medaillons bleibt. Wer Kamera oder Hand verschiebt, muss
die `HeroAnchor`-Marker mitziehen.

**HUD (`TurnHud`)**

| Bereich | Inhalt |
|---|---|
| Bildmitte oben / unten | `SeatBadge`-Medaillon von Gegner (rot, direkt unter dessen Hand) / Spieler (blau, direkt über der eigenen Hand) – wie in MTGA. Die Position kommt von den `HeroAnchor`-Markern (`TurnHud.player_hero_anchor` / `opponent_hero_anchor`); ohne Marker sitzen die Medaillons links oben / unten. Gezoomte (gehoverte) oder gezogene Karten liegen sichtbar **über** beiden Medaillons: Die Medaillon-Zeilen hängen unter einer Maske (`CardCutout`, `card_cutout.rs`), die das auf den Bildschirm projizierte Kartenviereck (`Card3D.screen_quad()`) ausspart – nötig, weil das HUD sonst immer über der 3D-Szene gezeichnet wird. Raute mit Wappen und Anfangsbuchstabe, roter Edelstein mit Lebenspunkten; pulsierender Schein = am Zug; Edelstein blitzt bei Schaden rot, bei Heilung grün. Daneben Name, Ressourcen und Mana |
| unten links | `PhaseTimeline`: Zeitstrahl der Phasen – Punkte für die Phasen, ein Punkt „Kampf“ mit drei kleineren Unterpunkten (Angreifer, Blocker, Schaden). Eine leuchtende Markierung gleitet zur aktuellen Phase (bei neuer Runde springt sie zurück); erledigte Strecke und Punkte golden, kommende hohl. Darüber der Name der aktuellen Phase, darunter Runde und Seite am Zug |
| Bildmitte | nur bei Spielende: „Spielende – X gewinnt!“. Während des Spiels keine Hinweistexte (Phase steht im Zeitstrahl, Stack in der Stack-Anzeige); `TurnManager.get_hint()` liefert den Hinweis weiterhin für eigene Skripte |
| oben links | die letzten drei Protokollzeilen, dezent |
| Bildschirmmitte | Auswahlfenster für Heldenfähigkeiten (nach Klick auf den eigenen Helden) |
| unten rechts | großer Button „Nächste Phase“ / „Weiter (passen)“: blau, solange der Stack leer ist, orange, wenn Priorität abgegeben wird |

### Dateien

| Ordner / Datei | Inhalt |
|---|---|
| `rust/src/table/` | Darstellung: `Card3D`, `Zone`, `TableCamera`, `TableController`, Kulissen `Arena` (`arena.rs`) und `Tavern` (`tavern.rs`), prozedurale Texturen (`textures.rs`) |
| `rust/src/game/` | Regeln: `TurnManager`, Phasen, `GameState`, Timing, Mana, Stack, Karten, HUD, Spieler-Medaillon (`seat_badge.rs`), Stack-Anzeige (`stack_view.rs`), Karten-Aussparung der Medaillons (`card_cutout.rs`) |
| `rust/src/debug.rs` | Debug-Tool (nur Test-Build) |
| `rust/src/i18n.rs` | Zweisprachigkeit: Sprache, `tr!`/`trf!`-Makros, Namenstabelle |
| `docs/` | diese Dokumentation und die API-Referenz |
| `tools/gen_api_doc.py` | erzeugt `docs/API-Referenz.md` |

---

## 3. Bedienung

| Aktion | Wirkung |
|---|---|
| Maus über Karte | Karte neigt sich; offene Karten werden groß und lesbar angezeigt (Hover-Zoom). Nicht durch Buttons, Debug-Fenster oder die Stack-Anzeige hindurch |
| Karte aus der Hand ziehen und loslassen | ausspielen (Regeln prüfen Phase, Stack und Mana) |
| Aura ziehen | über einer gültigen Zielkarte loslassen (Ziel wird gelb markiert) |
| Klick auf eigene Ressource | tappen → +1 Mana in den Pool |
| Klick auf eigenen Helden | Fenster mit seinen Fähigkeiten |
| Klick auf Ausrüstung, dann auf eigene Kreatur | Ausrüstung anlegen (kostet Mana) |
| Klick auf Artefakt mit Fähigkeit | Fähigkeit aktivieren (tappt das Artefakt) |
| Angreifer-Phase: Klick auf Kreaturen | als Angreifer markieren (rot) |
| Blocker-Phase: eigene Kreatur, dann Angreifer | Blocker zuweisen (gelb → blau) |
| Rechtsklick auf Karte | Kamera-Nahaufnahme; erneut oder Esc = Übersicht |
| Button „Nächste Phase“ / „Weiter (passen)“ bzw. Leertaste | Phase beenden bzw. bei belegtem Stack Priorität abgeben |
| Maus über eine Karte der großen Stack-Anzeige | holt sie nach vorn (auch tiefer liegende Elemente lesbar) |

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

**Anzeige:** Solange etwas auf dem Stack liegt, zeigt das HUD den Stack **groß in der oberen
Bildschirmhälfte** (`StackView`). Das passiert bei jeder gespielten Karte außer Ressourcen (die
nicht über den Stack gehen) und in jeder Prioritätsrunde. Jedes Element ist eine große Karte mit
allen Texten; antwortet ein Spieler, wird seine Karte **versetzt darübergelegt**, die
darunterliegenden bleiben am Rand sichtbar. Das oberste Element (wird als Nächstes aufgelöst) hat
einen **goldenen Rahmen**. Ein Band zeigt *Spieler/Gegner · Zauber/Fähigkeit*; bei Fähigkeiten
steht im Textfeld der konkrete Effekt. Nach dem Auflösen verschwindet die Karte aus der Anzeige.
Eine zusätzliche Textanzeige der Stack-Anzahl gibt es nicht.
Die Anzeige liegt **über dem Gegner-Medaillon**: Dahinter wird die obere Bildhälfte leicht grau
abgedunkelt (weicher Übergang nach unten, klickdurchlässig), Medaillon und Tisch scheinen durch.
Die Karten beginnen direkt unter dem Lebens-Edelstein des Gegners, sodass sein Leben lesbar bleibt.

**Hovern im Stack-Bereich:** Die großen Stack-Karten fangen die Maus ab – Karten auf dem Tisch
**unter** der Anzeige werden dort weder gehovert noch angeklickt. In der kleinen Stack-Zone in der
Tischmitte liegen neuere Karten oben und vorn. **Fährt die Maus über eine Karte der großen
Stack-Anzeige – auch über den sichtbaren Rand einer tiefer liegenden –, kommt sie nach vorn**
und ist vollständig lesbar; beim Verlassen rutscht sie zurück an ihren Platz.
Allgemein gilt: Liegt die Maus über einer Bildschirmfläche, die Mausereignisse annimmt (Stack-Karten,
Buttons, Debug-Fenster), wird auf dem Tisch dahinter nichts gehovert.

- Wer etwas auf den Stack legt, behält die Priorität.
- Aufgelöst wird das **oberste** Item erst, wenn **beide nacheinander passen** (LIFO).
- **Der Stack läuft, bis beide Spieler „Weiter“ gedrückt haben.** Der Mensch passt nur per Button
  „Weiter (passen)“ bzw. Leertaste – es gibt kein automatisches Passen (einschaltbar über
  `TurnManager.auto_pass_human`, dann passt er automatisch, wenn er keinen spielbaren Sofortzauber
  hat). Die KI passt oder antwortet nach `priority_delay` Sekunden selbst.
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

**Verschieben:** Die rote Titelleiste „::: Debug – zum Verschieben ziehen“ mit gedrückter linker
Maustaste ziehen. Die große Kartenvorschau wandert rechts daneben mit; die Titelleiste bleibt
immer im Bild. Die Position bleibt beim Sprachwechsel erhalten (nicht über einen Neustart hinweg).

Oben im Fenster: **Sprache / Language** (Deutsch / English) – stellt das ganze Spiel um.

| Reiter | Funktion |
|---|---|
| Mana | „+“ für Blau, Schwarz, Grün, Rot, Gelb (= weißes Mana), Grau (= farblos) legt Mana **direkt in den Manapool** (Menge = „Anzahl pro Klick“, 1–99); es verfällt am Zugende wie jedes Mana. Dazu „Manapool leeren“ und „Alle Ressourcen enttappen“. **Unendlich Mana:** Aus / Spieler / Gegner / Beide (siehe unten) |
| Karten | alle Karten mit Suche; Doppelklick / „Auf die Hand“; große Vorschau mit Regeltext. **Empfänger:** Spieler (deine Hand) oder Gegner (seine Hand) |
| Erstellen | eigene Karte aus Name, Typ, Fähigkeit, Kosten, Angriff, Verteidigung; „Erstellen + auf die Hand“ beachtet den Empfänger |

Erstellte Karten gelten nur bis zum Neustart.

**Karten an den Gegner:** Ist als Empfänger „Gegner“ gewählt (die Auswahl in den Reitern *Karten*
und *Erstellen* ist gekoppelt), gleitet die Karte vom Gegner-Deck **verdeckt** auf seine Hand und
**gehört ihm** (`owner_seat = 1`, Friedhof des Gegners). Die KI spielt sie in ihrem Zug regelkonform
aus – zusammen mit „Unendlich Mana: Gegner“ lassen sich so gezielt Gegnerzüge testen.

### Unendlich-Mana-Modus

Auswahl im Reiter „Mana“: **Aus**, **Spieler**, **Gegner** oder **Beide**.

- Wer unendlich Mana hat, kann **jede Karte und jede Fähigkeit bezahlen** (auch Anlegekosten von
  Ausrüstung und Tapp-Fähigkeiten) – ohne Ressourcen zu tappen und ohne den Manapool zu verbrauchen.
- Alle anderen Regeln gelten weiter: eigener Zug, Phase, leerer Stack, 1 Ressource pro Runde, gültige Ziele.
- Das HUD zeigt bei „Mana“ dann *unendlich (Debug)*.
- Mit „Gegner“ oder „Beide“ spielt die KI alle Karten aus, die sie regelkonform spielen darf.
- Technisch: Eigenschaft `infinite_mana` am `PlayerSeat`; geprüft in `GameState.can_afford` / `auto_pay`.
  Nur das Debug-Tool schaltet sie ein – im Release-Build ist der Modus nicht erreichbar.

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
