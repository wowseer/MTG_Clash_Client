# MTG Clash – Dokumentation

Ein Kartenspiel in einer 3D-Arena. Die Darstellung ist in **Rust** (godot-rust / gdext 0.5.5)
geschrieben und wird als GDExtension in **Godot 4.7** geladen. Die **Spielregeln kommen
vollständig von Forge** (Java-Regel-Engine, siehe Abschnitt 5); ein eigenes Regelwerk gibt es
nicht mehr. Das Spiel ist zweisprachig (**Deutsch / Englisch**, siehe Abschnitt 6).

Diese Datei gibt den Überblick. Jede einzelne Klasse, Eigenschaft, Funktion und jedes Signal
ist beschrieben

- **direkt in Godot:** Editor → *Hilfe → Hilfe durchsuchen* (F1) → Klassenname eingeben,
  z. B. `ForgeTable`, `Card3D`, `Zone`;
- **als Datei:** [API-Referenz.md](API-Referenz.md).

Beides wird aus denselben Rust-Doku-Kommentaren (`///`) erzeugt (gdext-Feature `register-docs`).

---

## 1. Projekt bauen und starten

| Schritt | Befehl |
|---|---|
| Rust bauen | `cd rust && cargo build` (Release: `cargo build --release`) |
| Unit-Tests | `cd rust && cargo test` |
| Forge-Brücke bauen | `forge-bridge/build.sh` (siehe Abschnitt 5) |
| Spiel starten | Godot-Projekt `godot/` öffnen, F5 (Hauptszene `table.tscn`; braucht Java 17+ und Forge) |
| API-Referenz neu erzeugen | `python3 tools/gen_api_doc.py` |

Nach jedem Rust-Build das Spiel neu starten, damit die neue Bibliothek geladen wird.
`godot/rust.gdextension` zeigt im Debug auf `rust/target/debug/`, im Export auf `rust/target/release/`.

---

## 2. Aufbau der Szene `table.tscn`

```
Table (TableController)          Wurzel: Maus-Eingabe, Drag-and-Drop, Kamera-Fokus
├── WorldEnvironment, Lantern    Licht und Atmosphäre
├── Arena (Arena)                prozedurale Arena mit Spielbrett (Kulisse)
├── Camera (TableCamera)         Übersicht / Nahaufnahme
├── Player (PlayerSeat)          Spieler unten (Forge-Sitzplatz 0)
│   ├── Battlefield (Zone)       Spielfeld (automatische Anordnung, `auto_arrange`)
│   ├── Deck, Discard (Zone)     Deck, Friedhof
│   ├── Exile (Zone)             Exil (offener Stapel zwischen Friedhof und Tischmitte)
│   ├── Command (Zone)           Kommandozone als Spalte (Spieler rechts unten, Gegner rechts oben)
│   ├── Resources (Zone)         Länder-Reihe
│   ├── ManaPool (ManaPool)      Manavorrat (Anzeige)
│   ├── HeroAnchor (Marker3D)    Position des Medaillons (direkt über der Hand)
│   ├── ManaAnchor (Marker3D)    Position der Mana-Symbole (unter der Länder-Reihe)
│   └── Hand (Zone)              Handkarten (Fächer, nah an der Kamera am unteren Bildrand)
├── Opponent (PlayerSeat)        Gegner oben (Forge-KI, Sitzplatz 1), gleicher Aufbau, um 180°
│                                gedreht; Hand am oberen Bildrand, Medaillon direkt darunter
├── Stack (Zone)                 Stack in der Tischmitte (Art 4, schwebt leicht über dem Brett)
├── TurnHud (TurnHud)            Bildschirm-Anzeige, liest vom ForgeTable
├── ForgeBridge (ForgeBridge)    Java-Prozess mit Forge
└── ForgeTable (ForgeTable)      gleicht den Tisch mit Forge ab, setzt Spielzüge um
```

Der `TableController` kennt keine Regeln: Er meldet jeden Drop per Signal `card_dropped`; der
`ForgeTable` spielt die Karte über Forge oder legt sie an ihren Platz zurück.

**Anordnung auf dem Spielfeld** (`Zone.auto_arrange`, beide Spielfelder): Karten werden automatisch
nach Art platziert, jeweils aus Sicht ihrer Seite („oben“ = zur Tischmitte; beim Gegner gespiegelt):

| Bereich | Karten |
|---|---|
| vorn, mittig an der Mittellinie | Kreaturen in einer Reihe; passen sie nicht, werden sie kleiner, dann zweireihig |
| hinten links (neben dem Medaillon) | Länder (eigene Länder-Reihe): von links nach rechts, gleichnamige Länder überlappend gestapelt, Gruppen nach Manafarbe; getappte Länder bilden einen eigenen Stapel direkt rechts neben den ungetappten gleichen Namens. Länder-Reihe, Deck, Friedhof und Exil liegen so hoch, dass die Hand sie nicht verdeckt (wie im Video „Spree und Stack“), der Manapool darunter zwischen Ländern und Hand |
| rechts, Mitte der eigenen Seite | alles andere (Planeswalker, Schlachten, Artefakte, Ausrüstung, Verzauberungen …) rechts von der Mitte, etwa in der Mitte der eigenen Spielfeldhälfte (über der Länder-Reihe); **gleiche Karten liegen gestapelt** wie gleichnamige Länder (gleicher Name, Tapp-Zustand, Stärke/Widerstand, Schaden und Marken; Kreaturen untereinander – jede weitere 0,2 Kartenlängen weiter zum Spieler hin und obenauf, die Kopfzeilen darunter bleiben sichtbar –, andere Karten um 0,3 Kartenbreiten nebeneinander versetzt; `Card3D::stack_key`), getappte bilden einen eigenen Stapel rechts neben den ungetappten, Träger angelegter Karten liegen allein; wird es voll, werden **alle Karten des Spielfelds gemeinsam kleiner**, sodass Kreaturen (mittig) und die anderen Karten (rechts daneben) je in einer Reihe nebeneinander bleiben – bis 45 % der Größe; erst danach zwei Kreaturen-Reihen und die anderen Karten ganz hinten, zuletzt überlappend |
| linker Rand | Exil, Friedhof, Bibliothek (von der Mitte nach unten); Kommandozone rechts |

**Form und Größe** (wie im Video des Users): Karten auf Spielfeld und Länder-Reihe sind **Kacheln**
(`Zone.tile_cards`, `Card3D.set_tile`): nur Name und Bild, Stärke/Widerstand unten rechts, etwas
breiter als hoch; beim Ziehen erscheint die ganze Karte, beim Hovern die Kartenansicht (siehe unten). Getappte Karten (auch Länder) sind grau und stehen leicht schräg, sobald sie benutzt sind. Größe `Zone.card_scale`
(Spielfeld 2,5, Länder 1,8); ist eine Reihe zu voll, werden die Karten automatisch kleiner
(bis 45 %), danach rücken sie zusammen.

### Kulisse und Bildschirm-Anzeige

Der Look orientiert sich an digitalen Sammelkartenspielen (Spielbrett von oben, Medaillons mit
Lebenspunkten, großer Aktionsbutton), ist aber eine eigene Gestaltung ohne fremde Grafiken –
alles wird prozedural in Rust erzeugt.

**`Arena` (`rust/src/table/arena.rs`)** – Standard-Kulisse von `table.tscn`:

| Element | Beschreibung |
|---|---|
| Spielbrett | Sandstein mit gemeißelten Rillen (Normal-Map aus einem Höhenbild): doppelter Rahmen mit Eck-Ringen, große Raute, Mittellinie (z = 0) |
| Runenkreis | in der Brettmitte; Ringe, Stern und Glyphen leuchten türkis (Emissionstextur, Stärke `rune_glow`) |
| Rand | flacher Steinrand (nur 0,06 über dem Brett) mit Goldeinlage; in den Ecken eingelassene Leuchtkristalle in Goldfassung – das Brett wirkt wie eine flache Spielmatte |
| Umgebung | gestufter Sockel, Schieferboden, zwei Feuerschalen, Säulenruinen, runde Arenamauer mit Bannern in Spieler- (`player_color`) und Gegnerfarbe (`opponent_color`) |

Das Brett ist breit angelegt (`board_size` = 19 × 9,6 in der Szene), damit es fast die ganze Bildbreite füllt; die Spielfelder sind 17 breit und reichen von der Mittellinie bis zum Rand (z 0,15 … 4,85); Bibliothek/Friedhof/Exil liegen links außen (x = −8,9, beim Gegner rechts oben), die Länder-Reihen hinten links (x = −4,8, beim Gegner gespiegelt), der Stack rechts (x = 6,0). Der Lichtkegel (`Lantern.spot_angle` 46°) deckt das ganze Brett ab.
Einstellbar im Inspector: `board_size`, `texture_resolution` (Pixel pro Einheit der Brett-Textur),
`rune_glow`, `player_color`, `opponent_color`. Die alte Kulisse `Tavern` existiert weiter: in
`table.tscn` den Knoten `Arena` durch einen `Tavern`-Knoten ersetzen (dann `Lantern.light_energy`
wieder auf ca. 10 stellen, die Arena braucht wegen des hellen Steins nur ca. 3).

**Sichtbar im Editor:** `Arena`, `Tavern` und `FlickerLight` sind Tool-Klassen – die Kulisse wird
auch im Godot-Editor gebaut, wenn `table.tscn` geöffnet ist. Die erzeugten Knoten haben keinen
Owner, erscheinen nicht im Szenenbaum und werden nicht in die `.tscn` gespeichert. Ändert man im
Inspector eine Eigenschaft der `Arena`, baut sie sich sofort neu auf (`Arena.rebuild()`). Karten,
Medaillons und HUD entstehen erst beim Spielstart (F5). Nach einem neuen Rust-Build
ggf. die Szene neu laden (*Szene → Gespeicherte Szene neu laden*) oder den Editor neu starten.

**Kamera und Aufteilung** (wie in digitalen Kartenspielen): Die Übersichtskamera schaut steil
von oben (ca. 75°) aus großer Entfernung mit kleinem Blickwinkel (`fov` 30°,
`Camera.overview_position` (0, 23.2, 6.4)) – dadurch fast keine Perspektive, das Spielfeld wirkt
flach. Nahaufnahme-Abstand `focus_distance` 2,4. Die eigene Hand liegt vergrößert (Skalierung 2,8)
knapp über dem Brett vor dessen Vorderkante am unteren Bildrand (die untere Kartenhälfte ist
abgeschnitten, Hover zoomt die ganze Karte), die Gegnerhand am oberen Rand. Beide Spielfelder sind
3,0 Einheiten tief (Mittellinie bis z = ±3,2), damit vor ihnen Platz für die Medaillons bleibt. Wer
Kamera oder Hand verschiebt, muss die `HeroAnchor`-Marker mitziehen.

**HUD (`TurnHud`)** – liest jedes Frame die Anzeige-Struktur `HudView` vom `ForgeTable`
(Eigenschaft `forge_table`):

| Bereich | Inhalt |
|---|---|
| Bildmitte oben / unten | (Während einer 3D-Bühne – Startspieler, Mulligan, Abwerfen, „Wähle eins“ – ausgegraut und fast durchsichtig.) `SeatBadge`-Medaillon von Gegner (rot, direkt unter dessen Hand) / Spieler (blau, direkt über der eigenen Hand). Die Position kommt von den `HeroAnchor`-Markern (`TurnHud.player_hero_anchor` / `opponent_hero_anchor`); ohne Marker sitzen die Medaillons links oben / unten. Gezoomte (gehoverte) oder gezogene Karten liegen sichtbar **über** beiden Medaillons: Jede Medaillon-Zeile hängt unter einer Hülle (`CardCutout`, `card_cutout.rs`), deren Canvas-Shader alle Pixel im auf den Bildschirm projizierten Kartenviereck (`Card3D.screen_quad()`) verwirft. Solange eine Karte gehovert oder gezogen wird, werden beide Medaillon-Zeilen außerdem weich durchsichtig (30 % Deckkraft). Raute mit Wappen und Anfangsbuchstabe, roter Edelstein mit Lebenspunkten; pulsierender Schein = am Zug; Edelstein blitzt bei Schaden rot, bei Heilung grün |
| unten rechts, über dem Button | `PhaseTimeline`: Zeitstrahl der Phasen mit drei Stationen – (1) ein Kreis für Enttappen, Auffrischung, Ziehen und Hauptphase 1, der mit jedem dieser Schritte größer wird; (2) ein Punkt „Kampf“, dessen drei Schritte (Angreifer, Blocker, Schaden) erst beim Kampf aufklappen; (3) ein Kreis für Hauptphase 2 und Ende. Symbole: Enttappen = Kreispfeil, Auffrischung = Sonne, Ziehen = Karte mit Plus, Hauptphasen = aufgefächerte Karten, Ende = Mondsichel, Kampf = gekreuzte Schwerter (Angreifer = Schwert, Blocker = Schild, Schaden = Flamme). Die aktuelle Station leuchtet. Forges `PhaseType` wird darauf abgebildet (`forge/table.rs`, `phase_index`) |
| Bildmitte | nur bei Spielende: „Spielende – X gewinnt!“ |
| unter der eigenen Länder-Reihe | Manapool als runde Symbole (`ManaPoolView`, `mana_view.rs`), aus Forges `mana_pool` (schwebendes Mana, z. B. aus Ritualen) |
| linker Bildrand, oben / unten | Gegner (oben links) bzw. Spieler (unten links): Rundenzahl mit Sanduhr-Symbol (`HourglassIcon`, `hourglass.rs`), darunter der Spielername; der Name der Seite am Zug ist golden |
| Bildschirmmitte | Auswahlfenster (`ChoicePanel`, `choice_panel.rs`) für alle allgemeinen Forge-Entscheidungen: eine Option, mehrere Optionen, Ja/Nein, Zahl; Suchfeld bei langen Listen (höchstens 40 Knöpfe, z. B. bei allen Kartennamen); welche Engine-Entscheidung welches Fenster ergibt, steht in `decision_view.rs` (Bedienung siehe Abschnitt 3) |
| unten rechts | übereinander: Phasen-Zeitstrahl, **Phasen-Knopf** „Passen »“ (auch Leertaste; im Kampf „Angreifen (n) »“ bzw. „Blocken (n) »“, sonst ausgegraut) und darunter der kurze Knopf **„»»“** (Sofort beenden: nur die Symbole, Knopf nur so breit wie sie – `Button3D.body_half`; im Zug des Gegners ausgegraut). Beide sind orange animierte 3D-Knöpfe (`Button3D` mit `pin_to_screen`: kurz vor der Kamera, feste Pixelgröße, vor allen Karten); während einer 3D-Bühne (Startspieler, Mulligan) ausgeblendet. Hinweise der Engine zum Kampf (z. B. ungültiger Angriff) stehen golden über dem Zeitstrahl |

### Dateien

| Ordner / Datei | Inhalt |
|---|---|
| `rust/src/table/` | Darstellung: `Card3D`, `Zone`, `TableCamera`, `TableController`, Ziel- und Kampfpfeile `TargetArrows` (`target_arrows.rs`), Kulissen `Arena` (`arena.rs`) und `Tavern` (`tavern.rs`), prozedurale Texturen (`textures.rs`), Kartenbilder (`placeholder.rs`) |
| `rust/src/game/` | Anzeige-Bausteine: `PlayerSeat`, Kartenmodell aus den Kartendaten der Engine (`cards.rs`: `CardDef::from_state`, Kartenart, Wertefeld), Regeltext-Symbole und Schriftgrößen (`card_text.rs`), Markennamen (`counters.rs`), Mana-Typen und -Pool (`mana.rs`), Phasen-Liste (`phases.rs`), HUD (`hud.rs`), Spieler-Medaillon (`seat_badge.rs`), Sanduhr (`hourglass.rs`), Manapool-Anzeige (`mana_view.rs`), Phasen-Zeitstrahl (`phase_timeline.rs`), Kartenvorschau (`card_preview.rs`), Auswahlfenster (`choice_panel.rs`), 3D-Bühnen für Kartenwahl in `table/showcase.rs`, Engine-Entscheidung → Auswahlfenster → Antwort (`decision_view.rs`), Karten-Aussparung der Medaillons (`card_cutout.rs`) |
| `rust/src/rules/` | Engine-Grenze: Protokoll-Modell (`Decision`, `Choice`, `Action`, `Target` …, `decision.rs`), offene Anfrage `Request` und Schnittstelle `RulesEngine` (`mod.rs`). Die UI kennt nur diese Typen |
| `rust/src/forge/` | `ForgeBridge` (Java-Prozess mit Forge, setzt `RulesEngine` um) und `ForgeTable` (Partie auf dem 3D-Tisch) |
| `forge-bridge/` | Java-Brücke zu Forge (GPL-3.0): Quellcode, `build.sh`, `run.sh`, Testdecks; `forge/` = lokale Forge-Kopie (nicht im Git) |
| `rust/src/i18n.rs` | Zweisprachigkeit: Sprache, `tr!`/`trf!`-Makros, Namenstabelle |
| `docs/` | diese Dokumentation, die Forge-Schnittstelle, die Kartenabdeckung (was für alle Karten fehlt) und die API-Referenz |
| `docs/claude/` | Arbeitsanweisungen für Claude: Animationsdesign, Spielfelder |
| `CLAUDE.md` | Projektziel, Fahrplan und Arbeitsregeln für Claude |
| `tools/gen_api_doc.py` | erzeugt `docs/API-Referenz.md` |

---

## 3. Bedienung

| Aktion | Wirkung |
|---|---|
| **Wer beginnt?** | Gewinnst du den Münzwurf, schweben über der Tischmitte zwei leuchtende 3D-Knöpfe untereinander: „Ich beginne“ (orange) und „Forge-KI beginnt“ (blau), darüber die Frage. Maus darüber = Knopf wächst, Leuchten wird heller; Klick = Wahl. Bausteine: `Button3D` (`table/button3d.rs`, Kollisions-Layer 3 `CONTROL_LAYER`, Vorrang vor Karten im `TableController`), Aussehen: Shader `table/button_shader.rs`, Nachbau der Blender-Materialien aus `Assets/Buttons/*.blend` samt Treibern – Feuer (orange) bzw. Wolken (blau) fließen in einer 4-s-Schleife, das Leuchten pulsiert; Farbstil `ButtonStyle` |
| **Mulligan** | Die Starthand erscheint groß vor der Kamera (Bühne `Showcase`, `table/showcase.rs`: Tisch abgedunkelt, Karten gleiten aus der Hand herein), darüber wer beginnt und die Frage, darunter „Mulligan“ (blau, links) und „Behalten“ (orange, rechts); darunter der Hinweis, wie viele Karten ein weiterer Mulligan unter die Bibliothek legt. Hover über eine Karte zeigt sie groß. Nach der Wahl gleiten die Karten zurück in die Hand. Forge fragt nach London-Regel sofort nach jedem Mulligan, welche Karten unter die Bibliothek kommen: auf derselben Bühne Karten anklicken (golden markiert, erneuter Klick hebt auf), der orange Knopf „Fertig (n/N)“ wird erst bei passender Anzahl aktiv; danach erneut „Mulligan“ oder „Behalten“ |
| Maus über Karte | **Handkarten** werden wie bisher selbst groß und lesbar vor die Kamera geholt (`Zone.zoom_cards` der eigenen Hand). **Alle anderen Karten** neigen sich nur leicht; für sie zeigt die **Kartenansicht** (`CardInspector`, `table/card_inspector.rs`, wie `Fähigkeiten.png`) zeigt eine helle **Kopie** der Karte groß am Bildschirmrand gegenüber der Maus – die Karte selbst bleibt an ihrem Platz. Daneben eine dunkle Tafel mit den **Fähigkeiten** (Schlüsselwörter mit Erklärung der Engine, z. B. Flying, Lifelink, Kicker) und den **Veränderungen** (Stärke/Widerstand gegenüber dem Aufdruck, Schaden, Marken, Commander/Ringträger, angelegte Karten). Nicht bei verdeckten Karten, nicht beim Ziehen und Zielen, nicht durch Buttons hindurch |
| Friedhof · Deck · Exil (unten links nebeneinander, beim Gegner oben rechts gespiegelt) | wie im Video „Exile and Grave“: links der **Friedhof** (offene Karten, so groß wie das Deck), in der Mitte das **Deck**, rechts das **Exil**: Liegen dort Karten, öffnet sich ein dunkel wirbelnder **Strudel** (`table/exile_portal.rs`, Shader: Rauschen in Polarkoordinaten, dreht sich nach innen), etwa kartengroß; Karten, die ins Exil gehen, gleiten hinein, schrumpfen und verschwinden (`Zone.swallow_cards`). Ist das Exil leer, schließt sich der Strudel (die Zone ist zu). **Klick** auf den Friedhof bzw. auf den Strudel (eigene oder des Gegners) öffnet die Karten darin als Cover-Flow wie beim Durchsuchen der Bibliothek („Friedhof (n)“ / „Exil (n)“, jüngste Karte zuerst, „Schlachtfeld ansehen“, „Fertig“ bzw. Leertaste/Escape schließt). Hat die angeklickte Karte eine eigene Aktion (z. B. Rückblende), wird statt dessen diese ausgeführt; in der Ansicht leuchten spielbare Karten, ein Klick darauf schließt die Ansicht und führt die Aktion aus |
| Deck | verdeckter Stapel, so groß wie deine Bibliothek in Forge; die Karten liegen größer als normal (`card_scale` 1,7 der Deck-Zonen), damit der Stapel neben den Spielfeldkarten gut zu sehen ist. Gezogen wird automatisch im Ziehschritt (Forge); die Karte gleitet sichtbar vom Deck auf die Hand |
| Spielbare Handkarten | leuchten, sobald Forge auf deine Priorität wartet und du sie jetzt spielen und bezahlen kannst |
| Karten aus anderen Zonen in der Hand | Wie in Arena: Jede Karte, die du gerade aus einer anderen Zone spielen kannst – Friedhof (Flashback, Escape …), Exil (Foretell, Abenteuer, „you may play“), Kommandozone (Commander), Sideboard (Companion), Bibliothek –, erscheint zusätzlich als Kopie fest rechts außen im Handfächer, mit Lücke zu den echten Handkarten (`Zone::add_tail_card`; neu gezogene Karten kommen davor): sie fliegt aus ihrer Zone dorthin, ihr Rahmen leuchtet weiß, darüber stehen die Kosten, mit denen sie aus dieser Zone gespielt wird (z. B. Flashback {2}{R} statt {R}), dazu eine Plakette („aus dem Friedhof“, „Begleiter“ …). Zieh sie auf den Tisch, um sie aus ihrer Zone zu spielen; ist sie nicht mehr spielbar, verschwindet die Kopie (`ForgeTable::sync_hand_proxies`) |
| Kosten der Handkarten | Über jeder eigenen Handkarte (außer Ländern) steht rechts oben ihre Manakosten als Reihe runder Mana-Symbole, die halb über die Oberkante ragt (wie im Video „Spree und Stack“; Zone mit `show_costs`, Baustein `table/cost_symbols.rs`) |
| **Kostensymbole** (überall, wo die 3D-Oberfläche Kosten oder Mana zeigt) | Die Symbole aus dem Symbolbogen `Assets/Mana/Mana.svg` (`game/mana_symbols.rs`, in die Bibliothek eingebaut und zur Laufzeit gerendert): Farben, Zahlen 0–20, X/Y/Z, Schnee, Hybride, 2/Farbe, phyrexianisch, Tappen/Enttappen – in Kosten über den Handkarten, auf Knöpfen und Beschriftungen, im Manapool, im Farbrad und in den Kostenpunkten gezeichneter Kartenbilder. Nur farbloses Mana {C} fehlt im Bogen und bleibt eine graue Raute. Knöpfe („Bezahlen {1}{U}“, „Kicker – {R}“, „Offspring {2}“) und Beschriftungen („+ {3}“ über Spree-Modi) zeigen statt „{…}“ diese Symbole |
| Leuchtende Karte aus der Hand ziehen und **irgendwo** auf dem Tisch loslassen (nicht über der Hand) | spielen: Forge bekommt die passende Option, zahlt das Mana automatisch und legt die Karte hin (Länder in die Länder-Reihe, bleibende Karten aufs Spielfeld, Zauber auf den Friedhof). Solange du ziehst, wird die Karte zusätzlich **groß ganz oben in der Bildmitte** angezeigt. Ungültige Züge springen an ihren Platz zurück |
| Phasen-Knopf „Passen“ bzw. Leertaste | Priorität abgeben; im Angriffs-/Blockschritt den geplanten Angriff bzw. die Blocks bestätigen. Hat man nichts zu spielen, passt das Spiel automatisch – **außer in der eigenen zweiten Hauptphase und der eigenen Endphase**: dort hält es immer an (der Zeitstrahl zeigt den Wechsel dorthin), bis man selbst passt |
| **Farbe wählen (Farbrad)** | Wie im Video „Mana Auswählen“: Verlangt Forge eine Manafarbe (`ChooseColor`, z. B. „ein Mana beliebiger Farbe“, „wähle eine Farbe“, je Mana bei Farbkombinationen) oder einen Grundland-Typ (`ChooseType` nur mit Ebene/Insel/Sumpf/Gebirge/Wald), erscheint auf der Karte ein **Farbrad** (`ManaWheel`, `table/mana_wheel.rs`): Segmente Weiß oben, im Uhrzeigersinn Blau, Schwarz, Rot, Grün (farblos grau) mit Sonne, Tropfen, Schädel, Flamme, Baum, in der Mitte das Kartenbild, darüber „(Karte) Wähle eine Farbe.“. Es wächst in 0,20 s aus der Karte und folgt ihr; das Segment unter der Maus hellt auf, Klick wählt. Hat ein Land mehrere Manafähigkeiten mit je einer Farbe, öffnet ein Klick auf das Land dasselbe Rad (Escape bzw. Klick daneben schließt es ohne Wahl) Bei Fragen der Engine (Farbe, Grundland-Typ, Schutz vor einer Farbe) steht das Rad in der **Mitte des Spielfelds**, mit dem Bild der Quellkarte in der Radmitte; nur das freiwillige Rad beim Anklicken eines Landes mit mehreren Manafähigkeiten sitzt an der Karte. |
| **Farbrad: weitere Fragen** | Dasselbe Rad (Motive wie im Manapool, farblos = graue Raute) auch für: **Mana aus dem Pool** („Mit welchem Mana aus dem Pool bezahlen?“), **Farbe einschließlich farblos**, **mehrere Farben** (Klick wählt bzw. hebt auf – gewählte Segmente hell mit goldenem Ring, die anderen gedämpft –, darunter orange „Fertig (n/…)“, erst bei gültiger Anzahl aktiv) und **Manafähigkeiten einer Karte** wie Cavern of Souls: je feste Manaart ein Segment, eine Fähigkeit „ein Mana beliebiger Farbe“ bekommt je Farbe ein Segment – die gewählte Farbe beantwortet Forges Folgefrage gleich mit. Das freiwillige Rad an einer Karte steht über ihr. Haben zwei Möglichkeiten dieselbe Farbe (z. B. Pool-Mana aus zwei Quellen), gibt es stattdessen Knöpfe |
| **Wirken / Aktivieren abbrechen** | Bei jeder Frage, die Forge beim Wirken einer eigenen Karte bzw. beim Aktivieren einer eigenen Fähigkeit abbrechbar stellt (Ziele, Modi, X, Optionen …): der **Phasen-Knopf** heißt „Abbrechen“ (auch Leertaste), oben rechts an der Karte sitzt ein grauer **✕-Knopf**, **Escape** bricht ab, und man kann die Karte per Pfeil **zurück auf die eigene Hand ziehen**. Forge gibt die Karte zurück und fragt wieder nach Priorität. Abdeckende Bühnen („Wähle eins“, Kicker …) haben ihren eigenen Abbrechen-Knopf; der Phasen-Knopf ist nur während abdeckender Bühnen ausgeblendet |
| Knopf „»»“ (Sofort beenden) | nur im eigenen Zug: bis zum Ende der Runde automatisch passen, ohne Angriff weiter. Blocker, Auswahlen und Ziele werden weiter gefragt; legt ein anderer Spieler etwas auf den Stack, endet das automatische Passen, damit man antworten kann |
| Auswahlfenster (Bildschirmmitte) | erscheint bei jeder allgemeinen Entscheidung von Forge (Mulligan, Startspieler, Modi, Kreaturtyp, Farbe, Kartenname, Ja/Nein, Zahl …). Die Karte, um die es geht, ist golden markiert. **Eine Option:** Knopf anklicken. **Mehrere:** Knöpfe an-/abwählen (golden = gewählt), dann „Fertig (n/…)“ – aktiv erst bei gültiger Anzahl. **Zahl:** Zahlenfeld, „OK“. Ab 13 Optionen gibt es ein **Suchfeld** (Treffer, die mit dem Suchtext beginnen, zuerst); angezeigt werden höchstens 40 Knöpfe – bei „Kartennamen nennen“ (alle über 34.000 Namen) also einfach den Namen eintippen und anklicken. „Abbrechen“ nur, wo Forge das erlaubt. Farb-Optionen sind in der Manafarbe eingefärbt |
| **Klick vs. Ziehen** | Kurz klicken (Maus bewegt sich weniger als 6 Pixel) = Klick; erst bei Bewegung wird die Karte gezogen |
| Klick auf eine leuchtende Karte auf dem Spielfeld | Fähigkeit aktivieren (z. B. Ausrüsten). Hat die Karte mehrere Fähigkeiten, erscheint die 3D-Bühne **„Wähle eins“** (siehe unten); bei mehreren Manafähigkeiten beim Bezahlen ein Menü |
| **Wähle eins** (mehrere Spielweisen) | Hat eine Karte mehrere Möglichkeiten, ins Spiel zu kommen bzw. benutzt zu werden (Wirken, Cycling, Abenteuer, andere Seite, alternative Kosten, Fähigkeiten …), erscheint beim Reinziehen bzw. Anklicken eine 3D-Bühne: Tisch abgedunkelt, Titel „Wähle eins“, je Möglichkeit eine Kopie der Karte, blau leuchtend, mit Beschriftung darüber (z. B. „Wirken“, „Cycling {2}“); Hover vergrößert. Klick wählt, der orange Knopf „Abbrechen“ schließt die Bühne (die Karte bleibt auf der Hand) |
| **Modus wählen** („Wähle eins —“, Confluence …) | 3D-Bühne: je Modus eine Kopie der Karte, auf der **nur dieser Modus** steht (Beschriftung „Modus 1“, „Modus 2“ …). Ein Modus: Klick wählt sofort. Mehrere: Klick wählt (orange) bzw. hebt auf, „Fertig (n/…)“; wiederholbare Modi zählen je Klick hoch (Plakette „×2“), „Zurücksetzen“ leert die Wahl |
| **Fähigkeit wählen** („Wähle eins“ bei Karten mit mehreren Spielweisen, z. B. zwei aktivierte Fähigkeiten) | Wie im Video „Choose One + Marke“: je Spielweise eine Kopie der Karte; „Wirken“ / „Land spielen“ zeigt die ganze Karte mit Beschriftung, eine Fähigkeit eine Kopie mit nur ihrem Text (auf dem echten Kartenbild auf dem hellen Feld über dem Textfeld); bei bis zu drei Möglichkeiten weiter auseinander. Klick wählt, „Abbrechen“ schließt |
| **Spree / Tiered** (Modi mit eigenen Zusatzkosten, CR 702.172; Video „Spree und Stack“) | Wie „Modus wählen“, aber: Titel = Schlüsselwort („Spree“), darunter „Wähle eine oder mehrere Zusatzkosten.“ (Tiered: „Wähle eine der Zusatzkosten.“), über jeder Modus-Kopie ihre Kosten **„+ {3}“**. Klick wählt (orange) bzw. hebt auf, bei Tiered tauscht er die Wahl. Unten orange **„Bezahlen {Gesamtkosten}“** – Grundkosten plus gewählte Modus-Kosten, rechnet bei jedem Klick mit, erst bei gültiger Wahl aktiv – und „Abbrechen“, wo Forge es erlaubt. Danach wird wie gewohnt bezahlt |
| **Gewirkter Zauber am Stack während der Moduswahl** | Bei jeder Moduswahl für eine Karte, die nicht auf dem Spielfeld liegt (sie wird gerade gewirkt; CR 601.2a: erst auf den Stack, dann Modi), steht die **ganze Karte hell am Stack-Platz** rechts auf der Bühne; die Auswahlreihe rückt nach links. Das Original auf der Hand ist so lange ausgeblendet. Nach der Wahl liegt der Zauber oben auf dem Stack-Stapel, ältere Einträge schauen versetzt dahinter hervor |
| **Ersatzeffekt wählen** (mehrere Ersatzeffekte auf dasselbe Ereignis, CR 616.1; allgemein jede Wahl, bei der jede Möglichkeit zu einer Karte gehört) | Wie die Spree-Bühne: Titel „Ersatzeffekt wählen“, darunter „Welcher Ersatzeffekt gilt zuerst?“, je Möglichkeit eine Kopie ihrer Quellkarte mit nur dem Text dieses Effekts (bei bis zu drei Möglichkeiten weiter auseinander, fast flach). Klick wählt (orange) bzw. hebt auf, orange **„Anwenden“** erst mit Wahl aktiv. Gehören mehrere Effekte zur selben Karte, steht „Effekt n“ darüber. Kein 2D-Fenster |
| **Karten-Optionen** (statische Fähigkeit zuerst, Kartenseite, Zauber/Fähigkeit für einen Effekt, Spielweise, wenn ein Effekt die Karte spielen lässt, mehrere Zauber/Fähigkeiten) | Dieselbe Bühne wie bei Ersatzeffekten: je Möglichkeit eine Kopie ihrer Karte mit nur ihrem Text (bei Kartenseiten die ganze Seite mit Name, Typ, Kosten, Text und Werten). Titel je Frage: „Statische Fähigkeit wählen“, „Seite wählen“, „Wähle eins“ („Wie soll die Karte gespielt werden?“), „Wähle n“; darunter Forges Fragetext. Eine Wahl: „Anwenden“; mehrere: Klick wählt bzw. hebt auf, „Fertig (n/…)“ erst bei gültiger Anzahl aktiv; „Abbrechen“, wo Forge es erlaubt |
| **Aus der Starthand** (Leylines u. a., CR 113.6b) | Karten-Optionen-Bühne mit den ganzen Karten: Titel „Aus der Starthand“, Klick wählt bzw. hebt auf, „Fertig (n/0–…)“ – auch ohne Wahl möglich |
| **Kartenstapel wählen** (Fact or Fiction …) | Bühne „Stapel wählen“: je Stapel ein eigener Kartenfächer nebeneinander mit „Stapel n (k)“ darüber, offene Karten mit Bild, verdeckte als Rückseiten. Klick auf eine Karte wählt ihren ganzen Stapel (alle Karten orange), „Anwenden“ bestätigt (`ForgeTable::show_piles`) |
| **Karten auf dem Tisch wählen** (opfern, tappen, Convoke, Improvise …) | Kein Fenster mehr: Klick auf die leuchtenden Karten wählt bzw. hebt auf (wird genau eine verlangt, sofort); der Phasen-Knopf zählt mit („Fertig (n/…) »“) und bestätigt, Forges Frage steht als Hinweis darüber; bei zu wenigen Karten „Abbrechen“, wo erlaubt |
| **Knopf-Spalte** (drei und mehr Möglichkeiten als Text: Markenart, Schlüsselwort, Schutz-Art, Zahl aus einer Liste, Abstimmung) | Titel je Frage („Marke wählen“, „Fähigkeit wählen“, „Schutz wählen“, „Zahl wählen“, „Abstimmen“), darunter Forges Fragetext. Gibt es eine Quellkarte, steht sie groß links, die Knöpfe rechts daneben untereinander (erster orange, die anderen blau; bei vielen enger, ab acht zwei Spalten), sonst mittig. Markenarten tragen ihre Münze (+1/+1 grün, −1/−1 rot, sonst golden mit Anfangsbuchstabe). Schutz vor einer Farbe: stattdessen das **Farbrad** mit den Farben zur Wahl. „Abbrechen“ grau darunter, wo erlaubt (`ForgeTable::show_option_buttons`) |
| **Ja/Nein** (optionale Fähigkeit, „… es sei denn, du zahlst“, phyrexianisches Mana, Entweder-oder) | Die Karte, um die es geht, groß und hell in der Mitte, darüber die Frage, darunter **zwei Knöpfe untereinander**: oben orange (erste Antwort, z. B. „Ja“, „Zahlen“), darunter blau |
| **Optionale Kosten** (Kicker, Buyback, Bargain, Gift …, auch Schlüsselwort-Zusatzkosten wie Casualty, Conspire, Offspring) | Die Karte groß und hell im Vordergrund, darüber „Kicker“ (bzw. „Casualty“ …), darunter orange **„Kicker – {R}“** (bzw. „Casualty“ ohne Zahl, „Offspring {2}“; andere Kosten als Mana erklärt der Kartentext) und grau **„Normal“**; Opfern/Tappen danach über die Kartenwahl; bei mehreren optionalen Kosten je Kosten ein Knopf zum An-/Abschalten (✓) und „Fertig“ |
| **Ziel wählen** | Kein Fenster: Sobald eine Karte oder Fähigkeit ein Ziel braucht, hängt an ihr (die Karte liegt dabei schon auf dem Stack in der Tischmitte) ein goldener **Pfeil**, der der Maus folgt. Gültige Ziele leuchten (Karten) bzw. bekommen einen pulsierenden goldenen Ring (Spieler-Medaillons); fährt man darüber, rastet der Pfeil dort ein. **Klick** (oder Pfeil von der Karte ziehen und loslassen) wählt das Ziel – je nach Karte eine Kreatur, einen Spieler oder anderes. Gibt es genau ein mögliches Ziel, wird es ohne Klick genommen. Das gilt für jede Karte, die zielt (CR 115), auch wenn nur Spieler infrage kommen (z. B. Lightning Bolt ohne Kreaturen): keine Abfrage, nur der Pfeil. Der Pfeil hat die Farbe der Karte. Bei nur einem Ziel geht es sofort weiter; bei mehreren erneuter Klick = abwählen, dann Phasen-Knopf „Ziele bestätigen (n) »“. Solange zu wenige Ziele gewählt sind, heißt der Knopf „Abbrechen“ und nimmt den Zauber zurück. Was die Karte als Ziel verlangt, steht golden über dem Phasen-Zeitstrahl |
| **X wählen** (X-Kosten) | Am Stack rechts (ohne Abdunkeln, `Showcase.open_at`): die Karte **hell und lesbar** (orange leuchtend; die Originalkarte auf dem Stack dahinter ist so lange ausgeblendet und erscheint erst wieder, wenn die X-Bühne verschwunden ist), darüber „Wähle einen Wert für X.“, darunter die Knopfreihe **−5**, **−**, orange **„Bezahlen X = n“**, **+**, **+5** (Minus erst ab 1 aktiv) und grau „Abbrechen“ (nimmt den Zauber zurück). „Bezahlen“ schickt den Wert an Forge, danach wird wie gewohnt bezahlt |
| **Zahl wählen** (`ChooseNumber`: „wähle eine Zahl“, „wie viele Marken“, „um wie viel verringern“ …) | Dieselbe Zahlen-Bühne wie bei X: die Karte groß und lesbar, darüber der Fragetext (das allgemeine „Choose a number“ der Engine wird zu „Wähle eine Zahl von 0 bis 13.“), darunter **−5**, **−**, orange **„Bestätigen: n“**, **+**, **+5**; Start ist der kleinste erlaubte Wert, „−“/„+“ nur aktiv, wo sich der Wert ändern kann; grau „Abbrechen“ nur, wo Forge es erlaubt. Liegt die Karte auf dem Stack, steht die Bühne dort (ohne Abdunkeln, der Phasen-Knopf zeigt „Auswahl treffen …“), sonst vor der Kamera. Kein 2D-Fenster |
| **Spieler wählen** (Wahl für einen Effekt **ohne Ziel**, bei der nur Spieler zur Wahl stehen und genau einer gewählt wird, z. B. „wähle einen Spieler“, CR 115.1) | Wie die Startspieler-Wahl, ohne Karte: über dem Tisch je Gegner ein **oranger** Knopf mit seinem Namen, darunter der **blaue** Knopf „Ich selbst“; darf man keinen wählen, zuletzt grau „Keinen Spieler“. Darüber „(Karte) Wähle einen Spieler.“. Medaillon-Klick und Pfeil gehen weiterhin; Abbrechen über den Phasen-Knopf bzw. das ✕ an der Karte. Echte Ziele (CR 115: „target player“, „any target“ wie bei Lightning Bolt) bekommen **nie** Knöpfe, auch wenn nur Spieler infrage kommen – dort zieht man nur den Pfeil (siehe „Ziel wählen“) |
| **Mana bezahlen** | **Immer automatisch:** Zieht man eine Karte auf den Tisch (bzw. aktiviert eine Fähigkeit), bezahlt Forge die Kosten sofort selbst – kein Fenster, auch nicht mit vorab getapptem Mana (das schwebende Mana im Pool wird dabei mitverwendet). **Vorab tappen:** Länder und andere Manaquellen kann man mit Priorität jederzeit anklicken (CR 605.3a; sie leuchten dafür nicht), das Mana schwebt dann im Manapool. **Wieder entappen:** Klick auf ein vorab getapptes Land, dessen Mana noch nicht ausgegeben ist, nimmt die Manafähigkeit zurück (Forge-Rückgängig für Manafähigkeiten, Aktion `UndoMana`) – das Land steht wieder auf, das Mana verschwindet aus dem Pool |
| **Karten aus der Hand wählen** (abwerfen, auch als Kosten oder wegen Handgröße; nach dem Mulligan unter die Bibliothek; Spleißen; andere Wahl nur aus der Hand) | 3D-Bühne wie beim Mulligan (Fragetext „Titel: Rest“ wird zu Titel und Untertitel, z. B. „Spleißen“): die wählbaren Handkarten groß nebeneinander vor der Kamera (blau leuchtend); passen sie nicht alle ins Bild, schiebt man sie mit dem **Schieberegler** darunter (`Slider3D`: orangen Griff ziehen oder auf die Schiene klicken) nach links und rechts. Titel z. B. „1 Karte(n) abwerfen“. Klick wählt (orange), erneuter Klick hebt auf; bei nur einer erlaubten Karte tauscht ein Klick die Wahl. Orange „Fertig (n/…)“ erst bei gültiger Anzahl aktiv, blau „Abbrechen“ nur wo Forge das erlaubt. Kein 2D-Fenster |
| **Hellsicht / Überwachen** | 3D-Bühne wie im Video `Scry.mp4`: Titel „Hellsicht“ (bzw. „Überwachen“) mit Anleitung, ein dunkler Kasten, darunter links **UNTEN** (bzw. **FRIEDHOF**) und rechts **OBEN**. Im Kasten liegt deine Bibliothek als **verdeckter Stapel**; sichtbar sind nur die aufgedeckten Karten. Sie liegen zuerst rechts **auf** dem Stapel (die rechte kommt zuoberst). Zieht man eine nach links (oder klickt sie an), rutscht sie **unter** den Stapel und schaut links hervor; beim Überwachen liegt sie links neben dem Stapel (Friedhof). Ein weiterer Klick holt sie zurück nach oben. Oben rechts „Schlachtfeld ansehen“. Orange „Fertig“: die Karten fliegen umgedreht zum Bibliotheksstapel auf dem Tisch (beim Überwachen nur in den Friedhof, wenn keine oben bleibt) |
| **Karten aus verdeckten Zonen wählen** (Bibliothek durchsuchen, aufgedeckte Karten, Friedhof …) | 3D-Bühne wie im Video „Chose from Deck“: Beim Durchsuchen der Bibliothek blitzt zuerst der eigene Bibliotheksstapel auf. Tisch abgedunkelt (Tischkarten dahinter reagieren nicht auf die Maus; solange eine abgedunkelte Bühne offen ist, sind Medaillons, Namen, Phasen-Zeitstrahl und Zielpfeile ausgeblendet), Titel („Wähle bis zu einer Karte“ …) mit Quelle und Text, die Karten groß als **Cover-Flow** (`Showcase.enable_coverflow`, `Zone.coverflow`): die mittleren gerade und vorn, die übrigen zur Seite gedreht, nach hinten versetzt und überlappend, am Rand dicht gestapelt. Die Karte unter der Maus kommt groß und gerade nach vorn. Bei vielen Karten läuft der Fächer mit dem **Schieberegler** darunter durch die Mitte. Oben rechts **„Schlachtfeld ansehen“** blendet die Bühne aus (Tisch sichtbar und hoverbar), „Zurück zur Auswahl“ holt sie zurück. Eine Karte: Klick wählt sofort; mehrere: Klick wählt (orange), „Fertig (n/…)“; darf man keine nehmen, steht blau **„Ablehnen“** unten rechts. Nur ansehen: „Fertig“. Die gewählte Karte gleitet danach vom Bibliotheksstapel an ihren Platz |
| **Hellsicht / Überwachen** | Kartenfächer mit zwei Reihen „Oben auf die Bibliothek“ und „Unter die Bibliothek“ bzw. „In den Friedhof“. Klick legt eine Karte ans Ende der anderen Reihe; die Reihenfolge oben gilt von oben nach unten. „Fertig“ |
| **Reihenfolge wählen** | Gleichzeitige eigene Trigger (CR 603.3b; bzw. Karten, die gleichzeitig in eine Zone wechseln, Kosten): 3D-Bühne „Reihenfolge festlegen“ wie bei „Wähle eins“ – je Eintrag eine Karte nebeneinander (bei Triggern eine Kopie der auslösenden Karte, die nur den Text dieser Fähigkeit zeigt), bei vielen Karten mit Schieberegler. Klick vergibt die nächste Nummer (**1**, **2**, … als Plakette auf der Karte, orange umrandet); erneuter Klick nimmt die Nummer zurück, die übrigen rücken nach. „Zurücksetzen“ löscht alle Nummern, orange „Fertig (n/N)“ ist aktiv, sobald alle nummeriert sind (Nummer 1 kommt zuerst) |
| **Kosten wählen** | Muss als Kosten eine Karte geopfert, abgeworfen, getappt, exiliert oder auf die Hand zurückgegeben werden (z. B. „Opfere eine Kreatur:“), erscheint der Kartenfächer mit allen passenden Karten. Gibt es genau so viele passende Karten wie nötig, wird nicht gefragt |
| **Phyrexianisches Mana** | Vor dem Bezahlen je Symbol die Frage „mit 2 Leben bezahlen?“ („2 Leben zahlen“ / „Mit Mana zahlen“) |
| **„…, es sei denn, du zahlst“** | Kann man die Kosten bezahlen, fragt das Spiel „Zahlen“ / „Nicht zahlen“; bezahlt wird wie beim Wirken (Mana über das Bezahl-Fenster). Kann man nicht zahlen, wird nicht gefragt |
| **Schaden verteilen** | Bei Zaubern wie „… beliebig auf ein oder zwei Ziele aufgeteilt“ und bei Kampfschaden gegen mehrere Blocker bzw. mit Trampeln: Fenster mit einem Zahlenfeld je Ziel, vorbelegt (Kampf: tödlicher Schaden der Reihe nach, Rest aufs Trampel-Ziel), Anzeige „Verteilt: n / gesamt“; „OK“ erst, wenn die Summe stimmt. Beim Wirken bekommt jedes Ziel mindestens 1 |
| **Ziele ausgelöster Fähigkeiten** | Eigene Trigger mit Ziel (z. B. „Wenn … ins Spiel kommt, fügt einer Kreatur 4 Schaden zu“) fragen das Ziel wie beim Wirken (Klick auf Karte/Medaillon) |
| **Aufgedeckte Karten** | Deckt ein Spieler Karten auf (z. B. eine gesuchte Karte aus der Bibliothek), erscheinen sie im Kartenfächer mit Überschrift „Aufgedeckt – …“; „Fertig“ schließt die Anzeige. Mitteilungen wie „wählt Rot“ stehen im Spielprotokoll |
| **Kopien, Umleiten, Erschöpfen, Rekrutieren** | Kopiert ein Effekt einen Zauber und erlaubt neue Ziele (z. B. Reverberate), fragt das Spiel „Neue Ziele wählen für …?“; bei „Ja“ wie beim Wirken Ziel anklicken. Gegenzauber und Kopiereffekte zielen auf den Zauber im Stack (Karte in der Tischmitte anklicken). Beim Angriff fragt das Spiel nach Angreifern, die erschöpft (Exert) bzw. rekrutiert (Enlist) werden sollen: auf dem Tisch anklicken, „Fertig“ |
| **Convoke, Improvise, Delve** | Beim Wirken solcher Zauber fragt das Spiel vor dem Bezahlen: ungetappte Kreaturen (Convoke) bzw. Artefakte (Improvise) leuchten auf dem Tisch, jede gewählte bezahlt ein Mana (passende Farbe zuerst); bei Delve erscheinen die Friedhofskarten im Kartenfächer, jede exilierte Karte spart {1}. „Fertig“ mit 0 Karten = normal bezahlen |
| **Karten auf dem Tisch wählen** | Muss man Karten wählen, die alle offen auf dem Tisch liegen (z. B. eine Kreatur als Kosten opfern, „wähle eine bleibende Karte“), leuchten sie direkt auf dem Tisch: Klick wählt (goldene Markierung). Das Fenster am rechten Rand zeigt die Namen als Ausweichweg und „Fertig“ bei mehreren. Muss man einen Spieler wählen („Choose a player“), pulsieren die Medaillons wie bei Zielen |
| **Angreifen** (Angriffsschritt) | **Kampfansicht:** Kreaturen, die angreifen können, leuchten blau. Klick (oder Ziehen) = angreifen: das Leuchten wird orange, die Karte rückt zum Gegner vor und trägt eine rote **Angriffsspitze** an der Oberkante; erneuter Klick = nicht angreifen. **Pfeil nur bei mehreren Angriffszielen:** liegt beim Gegner ein Planeswalker (oder eine Schlacht), zeigt ein orangefarbener Pfeil, wen der Angreifer angreift – Pfeil ziehen und auf Medaillon bzw. Planeswalker loslassen oder danach das Ziel anklicken. Ohne Planeswalker gibt es keinen Pfeil. Bestätigen mit dem Phasen-Knopf unten rechts („Angreifen (n) »“, auch Leertaste; ohne gewählte Angreifer heißt er „Angreifen »“ und geht ohne Angriff weiter). Ungültige Angriffe (z. B. eine Kreatur muss angreifen) werden mit Hinweis erneut gefragt |
| **Blocken** (Blockschritt) | Eigene Kreaturen, die blocken können, leuchten. **Pfeil ziehen** von der eigenen Kreatur auf einen Angreifer = blocken (ins Leere = Block aufheben). Alternativ Klick auf eine davon (goldene Markierung), dann Klick auf einen Angreifer = blocken (blauer Pfeil, blaue Markierung; ein blasser Pfeil zeigt vorher auf den Angreifer unter der Maus). Klick auf einen Blocker = Block aufheben. Mehrere Blocker auf denselben Angreifer sind möglich. Bestätigen mit dem Phasen-Knopf („Blocken (n) »“, ohne Blocker „Blocken »“); ungültige Blocks werden mit Forges Begründung erneut gefragt |
| **Marken, Spielsteine, angelegte Karten, Exil** | Marken stehen wie im Video „Choose One + Marke“ als **Münzen** rechts oben am Kartenrand (halb überstehend, je Markenart eine, untereinander): silbern mit der Anzahl, Rand golden (+1/+1 grün, −1/−1 rot); kommt eine Marke dazu, springt die Münze kurz auf und leuchtet blau (`Card3D::set_counters`, `cost_symbols::counter_coin`). Den Namen der Marke zeigt die Kartenansicht beim Hovern. Loyalität und Verteidigung stehen im Wertefeld. Spielsteine tragen die Plakette „Spielstein“. Auren und Ausrüstung liegen leicht versetzt unter ihrem Träger und wandern mit ihm. Exilierte Karten liegen offen auf dem Exil-Stapel neben dem Friedhof. Spieler-Marken (Gift, Energie, Erfahrung …) stehen am linken Bildrand unter dem Namen |
| **Kommandozone, doppelseitige und verdeckte Karten** | Commander, Embleme, Dungeons (Plakette „Raum: …“), The Ring, The Monarch, The Initiative u. a. liegen in der Kommandozone der Seite (Spalte am rechten Rand). Doppelseitige Karten drehen sich beim Transformieren sichtbar um (0,35 s, 180° um die Längsachse, `Card3D::flip_to`); die andere Seite steht als Plakette „↺ Name“. Verdeckte Karten (Morph, Manifest) zeigen das Rückseitenmuster mit ihren Werten (meist 2/2). Monarch, Initiative, Stadtsegen und Ring-Stufe stehen unter dem Spielernamen, Tag ☀ / Nacht ☾ neben der Rundenzahl |
| **Stack** | Rechts am Bildrand als **Kartenstapel**, groß und zur Kamera geneigt (Zone `Stack`, Anordnung `ZoneKind::Stack`): Zauber gleiten beim Wirken dorthin (schon während Ziele und Kosten gewählt werden, Eintrag „wird gewirkt“); aktivierte und ausgelöste Fähigkeiten erscheinen als Karte ihrer Quelle. Der neueste Eintrag liegt vorn und wird zuerst abgehandelt; jeder ältere liegt ein Stück weiter oben links, leicht gedreht dahinter. **Pfeile erklären, was worauf reagiert:** jeder Eintrag mit Zielen bekommt Pfeile in der Farbe seiner Karte (blau, rot, …; mehrfarbig gold, farblos grau) zu seinen Zielen; zielt er auf einen Eintrag darunter (z. B. Gegenzauber), wird der Pfeil zum Bogen, der aus der vorderen Karte aufsteigt und auf der Karte dahinter landet. Neue Pfeile wachsen in 0,33 s heraus. Die Liste am linken Bildrand nennt Quelle, Art und Text |
| Kampf der KI | Angreifer der KI rücken ebenso vor und tragen die Angriffsspitze; Pfeile nur bei Angriffen auf Planeswalker/Schlachten (orange), Blocks als blaue Pfeile |
| **Kampfschaden** | Jede Kreatur, die Kampfschaden zufügt, **schlägt zu**: sie holt in 0,22 s zum Ziel aus (zum Medaillon des Spielers, zum Planeswalker bzw. zur blockenden/geblockten Kreatur – Kreaturen treffen sich auf halbem Weg), roter Einschlag aus den Mana-Glow-Assets (Blitz, Stern, Schockwelle, Funken), dann zurück |
| **Leuchten der Karten** | Rahmen-Glow aus `Assets/Mana Glow Effekte` an jeder Karte: **blau** = spielbar bzw. wählbar (Standard), **orange** = aktiviert – Karte wird gezogen, gewirkt oder ihre Fähigkeit aktiviert, als Angreifer/Blocker/Ziel gewählt, Land für Mana getappt. Beim Aktivieren leuchtet der Rahmen kurz hell auf (`Card3D.flash`) |
| **Tempo der KI** | Die Forge-KI entscheidet sofort; damit man ihrem Zug zusehen kann, wartet die Brücke nach jeder ihrer Aktionen kurz (Land 0,7 s, Zauber/Fähigkeit 1 s, Angreifer 0,9 s, Blocker 0,7 s, jeder Phasenwechsel in ihrem Zug 0,25 s; ein KI-Zug dauert so etwa 3–4 s). Einstellbar über `ForgeBridge.ai_pace` (Faktor, 0 = ohne Pausen) |
| **Mana bezahlen (Effekt)** | Wird ein Land (bzw. eine Manaquelle) für Mana getappt – von dir oder der KI –, spielt der Effekt aus `Assets/Mana Glow Effekte` (`table/fx.rs`, `ManaFx`): das Land blitzt auf, Runenkreis und Mana-Kugel erscheinen, ein Komet mit Schweif fliegt im Bogen zur bezahlten Karte auf dem Stack, Einschlag mit Stern, Schockwelle und Funken, der Rahmen der Karte leuchtet auf. Farbe = erzeugtes Mana (Weiß, Blau, Schwarz, Rot, Grün, farblos golden). Ohne Zauber (Mana vorab getappt) fliegt er zur Manapool-Anzeige (`ManaAnchor`; bei der KI ohne Anker zum Medaillon) |
| **Pfeile** | Nachbau des Pfeils aus `Assets/Pfeil/MTG_Pfeil.blend` (`target_arrows.rs`): leuchtendes Band mit hellem Kern, Spitze und zur Spitze laufenden Lichtwellen; Farben aus der Palette des Blender-Materials (rot = Angriff, blau = Block, gold = Zielwahl, violett = Ziele auf dem Stack). Neue Pfeile wachsen in 0,33 s vom Start zum Ziel. In Ruhe gerade; bewegt sich ein Ende quer (z. B. beim Ziehen), biegt sich der Pfeil wie ein Seil und schwingt zurück |
| Rechtsklick auf Karte | Kamera-Nahaufnahme; erneut oder Esc = Übersicht |

**Kartenbilder (JPG):** Wie Forge und NeoForge zeigen die Karten echte Kartenbilder von Scryfall.
Die Brücke sucht sie im Bildordner von Forge (`~/.cache/forge/pics/cards/`) und von NeoForge
(`~/Games/NeoForge/datos/cache/pics/cards/`, anderer Ort über die Umgebungsvariable
`CLASH_EXTRA_PICS`) und lädt fehlende im Hintergrund von Scryfall in den Forge-Bildordner (beim
ersten Spiel mit einem Deck erscheinen die Bilder nach und nach). Mit Bild blendet die Karte Name,
Kosten, Typzeile und Text aus (stehen im Bild); Stärke/Widerstand nur, wenn sie vom Aufdruck
abweichen. Gezeigt wird immer ein Druck mit **normalem Rahmen**: ist der Druck aus der Deckliste ein
Sonderdruck (Secret Lair, Showcase, Borderless, Comic-Ausgaben …), nimmt die Brücke für das Bild den
neuesten Druck aus einem Kernset oder einer Erweiterung (sonst Reprint-, Commander-, Draft-Sets;
nie „The List“) – nur das Bild, die Karte im Spiel bleibt unverändert. So liegen Name, Kosten und
Textfeld bei fast allen Karten an derselben Stelle. Bühnenkopien mit eigenem Text (ein Modus, ein
Ersatzeffekt) zeigen das echte Bild mit einem hellen Feld über dem gedruckten Textfeld, auf dem nur
dieser Text steht (wie im Video „Spree und Stack“; Schrift bis 56 Punkte). Gibt es von einer Karte
gar keinen Druck mit normalem Rahmen, schickt die Brücke zusätzlich den Bildausschnitt (`art`,
Scryfall `art_crop`); die Bühnenkopie bekommt dann den gezeichneten Rahmen mit diesem Ausschnitt im
Bildfeld und dem eigenen Text im Textfeld. Ohne Bild, bei verdeckten Karten und Spielsteinen bleibt der gezeichnete Platzhalter
(`placeholder::card_face`). Wird ein Bild erst während des Wartens fertig, schickt die Brücke
danach einen neuen Zustand, damit es sofort erscheint. Die Bilder
gehören Wizards of the Coast und kommen nie ins Git.

**Platzhalter-Kartenbild:** Name, Kostensymbole, Typzeile und Regeltext kommen von der Engine. Symbole im
Regeltext werden ersetzt (`{T}` → ↷, `{R}` → Ⓡ, `{2}` → ②, hybrid als „(R/G)“), die Schriftgröße
von Name, Typzeile und Regeltext passt sich der Textlänge an (`card_text::fit_line` / `fit_font`).
Wertefeld unten rechts: Kreaturen Stärke/Widerstand (grün = über dem Aufdruck, rot = darunter oder
beschädigt), Planeswalker Loyalität, Schlachten Verteidigung. Rahmenfarbe = Kartenfarbe (Gold =
mehrfarbig, Grau = farblos, Länder nach ihrer Manafarbe); Typsymbol im Bildfeld je Kartenart
(Planeswalker = Krone, Schlacht = Schild). Planeswalker und Schlachten liegen links auf dem Spielfeld.

**Getappte** Karten werden leicht schräg gestellt (15°, `zone::TAP_ANGLE_DEGREES`), ausgegraut und
zeigen mittig ein graues Tapp-Symbol (`textures::tap_icon()`).

**Zauber und Fähigkeiten mit Zielen:** Nach dem Ausspielen fragt das Spiel nacheinander alles, was
der Zauber braucht: optionale Kosten (z. B. Kicker), Modus, X-Wert und das **Ziel** (Klick auf
Karte oder Medaillon, siehe oben). Aktivierte Fähigkeiten bleibender Karten (z. B. Ausrüsten)
leuchten, wenn sie möglich sind, und werden per Klick aktiviert.

Alle Entscheidungen im Umfang (siehe `docs/Kartenabdeckung.md`) trifft der Spieler selbst; die
Forge-KI springt nur ein, wenn eine Antwort ungültig ist. **Mana aus dem Pool:** Schwebendes Mana (z. B. aus Ritualen oder vorab
getappten Ländern) setzt die automatische Bezahlung mit ein (siehe „Mana bezahlen“).
**Modi mit Wiederholung** („you may choose the same mode more than once“): jeder Klick zählt den
Modus erneut („2× …“), bei voller Auswahl nimmt ein Klick ihn wieder heraus. Fahrzeuge besetzen:
Kreaturen auf dem Tisch anklicken, bis die nötige Stärke erreicht ist, dann „Fertig“.

### Debug-Fenster (nur Debug-Builds)

Startet man das Spiel aus dem Editor (bzw. einen Debug-Export; `OS.is_debug_build()`), legt der
`ForgeTable` ein **Debug-Fenster** an (`DebugPanel`, `game/debug_panel.rs`). In Release-Exporten
gibt es es nicht.

| Bedienung | Wirkung |
|---|---|
| **F10** | Fenster ein-/ausblenden (beim ersten Öffnen lädt es alle Kartennamen von Forge) |
| Suchfeld + Liste | Kartennamen filtern (Anfang zuerst, dann Teilwort; höchstens 200 Treffer) |
| „Auf meine Hand“ / Doppelklick, „Auf die Hand der KI“ | Karte wird neu erzeugt und auf die Hand gelegt (`debug_add_card`); eine offene Priorität wird neu gestellt, die Karte ist sofort spielbar |
| „Unendlich Mana: ich“ / „… KI“ | Forge füllt den Manapool bei jedem Phasenwechsel und Zauber auf 50 je Manaart auf (`debug_infinite_mana`) |

---

## 4. Regeln

Alle Regeln (Zugablauf, Timing, Mana, Stack, Kampf, zustandsbasierte Aktionen) kommen von **Forge**.
Das Spiel selbst prüft nichts: Es zeigt Forges Spielzustand an und schickt deine Entscheidungen
(Priorität, Mulligan, Auswahlfenster) an Forge. Die frühere eigene Regel-Engine (`TurnManager`, `GameState`,
`StackManager`, Timing, Demo-Decks, Debug-Tool) wurde entfernt.

---

## 5. Forge-Regel-Engine (Java)

**Ziel ist NeoForge** (JavaFX-Oberfläche für Forge, `~/Entwicklung/Java/NeoForge`): Dort soll
der 3D-Tisch die Partien spielen, die NeoForge startet. Die UI spricht das Protokoll-Modell
(`rust/src/rules/`, angelehnt an forge-rules-rs); die Java-Brücke übersetzt Forge dorthin
(Protokoll v3). Forge läuft als eigener Java-Prozess (`forge-bridge/`, GPL-3.0). Der Godot-Knoten `ForgeBridge`
(`rust/src/forge/bridge.rs`) startet ihn und tauscht JSON-Zeilen über stdin/stdout aus.
Entscheidungen des Spielers (Priorität, Mulligan, allgemeine Auswahl) kommen als Signal
`decision_requested` und werden mit `answer()` beantwortet; alle Forge-Spielereignisse kommen als `forge_event`.

Einrichtung, Protokoll, GDScript-Beispiel und Grenzen: [Forge-Schnittstelle.md](Forge-Schnittstelle.md).
Forge-Dateien: Liegt eine Kopie (Jar + `res/`) in `forge-bridge/forge/` (nicht im Git), wird sie
vor `~/Games/Forge` verwendet.

**`ForgeTable` (`rust/src/forge/table.rs`)**

- Startet Brücke und Partie automatisch (`autostart`): `player_deck` gegen `opponent_deck`
  (Standard: gemischte Testdecks `forge-bridge/testdecks/spieler.dck` – Rot mit Spontanzaubern, Hexereien, X-Zauber, modalen Zaubern, Kicker, Ausrüstung – gegen `gegner.dck` – Grün mit Auren, Kampf, Rampe). Forge-Sitzplatz 0 = Spieler unten
  (du), Sitzplatz 1 = Gegner oben (Forge-KI).
- Baut bei jedem neuen Forge-Zustand den Tisch ab: Für jede Forge-Karte eine `Card3D` (Bild aus
  Name, Typ, Kosten, Stärke/Widerstand); Hand → Hand, Länder → Länder-Reihe, übrige
  bleibende Karten → Spielfeld, Friedhof → Ablagestapel, Exil → Exil-Stapel, Gegnerhand verdeckt;
  getappt wird übernommen. Angelegte Karten (`attached_to`: Auren, Ausrüstung) liegen in der Zone
  ihres Trägers versetzt darunter (`Zone.set_attached`), auch auf der Gegnerseite. Marken als
  Münzen rechts oben (`Card3D::set_counters`), „Spielstein“ als Plakette über dem Bildfeld
  (`Card3D::set_badges`, Texte aus `game/counters.rs`); Stack-Einträge → Zone `stack_zone` (Zauber als ihre Karte, Fähigkeiten als Karte
  ihrer Quelle, die auf der Quelle erscheint). Karten gleiten animiert an ihren Platz. **Deck:** Die Bibliothek liegt als verdeckter
  Stapel in der Deck-Zone (Anzahl = Forges `library`). Zieht Forge eine Karte, gleitet die oberste
  Deckkarte auf die Hand und wird dort aufgedeckt (beim Gegner bleibt sie verdeckt). Spielt der
  Gegner eine Karte aus, gleitet sie aus seiner verdeckten Hand auf den Tisch. Während Forge Ereignisse
  sendet, wird der Zustand gebündelt alle `refresh_interval` Sekunden neu abgefragt.
- Liefert dem HUD Phase, Leben, Namen, Runde, Sieger, Button und Auswahlfenster (`hud_view()`).

---

## 5a. Spielen in NeoForge (3D-Tisch)

NeoForge (`~/Entwicklung/Java/NeoForge`, JavaFX-Oberfläche für Forge) kann seine Partien auf
diesem 3D-Tisch spielen lassen: Menüs, Decks und Modi bleiben in NeoForge, der Spieltisch ist
Godot. Technisch: NeoForge öffnet einen lokalen Port, startet Godot mit `-- --clash-port=N`, und
der menschliche Sitz wird über die Brücke (Protokoll v3) bedient (Details:
`docs/Forge-Schnittstelle.md`, Abschnitt „Eingebettet in NeoForge“).

**Einschalten:** In NeoForge unter *Einstellungen → 3D-Tisch (MTG Clash)*:
„Meine Partien am 3D-Tisch spielen“ = Ja, „Godot-Programm“ (Pfad zur Godot-4.7-Datei) und
„Projektordner von MTG Clash“ (der Ordner `godot/` dieses Projekts). Alternativ beim Start
`-Dneo.clash=true -Dneo.clash.godot=… -Dneo.clash.project=…` bzw. die Umgebungsvariablen
`NEOFORGE_CLASH`, `CLASH_GODOT`, `CLASH_PROJECT`. Startet sich Godot nicht innerhalb von 2 Minuten,
spielt NeoForge wie gewohnt mit seinem eigenen Tisch.

**Ablauf:** Partie in NeoForge starten → Godot-Fenster öffnet sich mit der Partie → Spielende wird
4 s angezeigt, dann schließt sich Godot und NeoForge zeigt das Ergebnis. Wird das Godot-Fenster
mitten in der Partie geschlossen, gibt der eigene Sitz auf.

**Entwickeln und Testen ohne Maven:** NeoForges Quellen bauen nur in einem Forge-Checkout mit
Maven. Für die Integration reicht die installierte NeoForge (`~/Games/NeoForge`):
- `tools/sync_neoforge.sh` kopiert den Brücken-Code nach NeoForge (`clash/forgebridge/`),
- `tools/neoforge_patch.sh` kompiliert ihn mit den geänderten NeoForge-Klassen (`ClashTable`,
  `NeoGame`, `SafeAi`, `NeoMain`, `SettingsPanel`, Sprachdateien) zu einem Patch-Jar,
- `tools/neoforge_clash.sh [--deck=NAME] [--timeout=SEK]` spielt eine Commander-Partie mit dem
  3D-Tisch in einer Testkopie (eigene NeoForge-Daten bleiben unberührt; Befehl `NeoMain clash`).
  Mit `CLASH_CMD=ui tools/neoforge_clash.sh` startet stattdessen NeoForges normales Menü (Testkopie,
  Patch geladen, Godot-Pfade schon gesetzt); dort nur noch den 3D-Tisch in den Einstellungen
  einschalten.

## 6. Sprache (Deutsch / Englisch)

- **Umschalten** per Code: `$Table.set_language("en")` / `$Table.set_language("de")`; abfragen mit
  `$Table.get_language()`. Standard ist Deutsch.
- **Sofort umgestellt:** HUD, Phasen und Kartenbeschriftungen.
- **Kartennamen** kommen von Forge (Englisch).
- **Dokumentation** (Godot-Hilfe, diese Datei) bleibt Deutsch.

### Neue Texte hinzufügen (`rust/src/i18n.rs`)

| Fall | Schreibweise |
|---|---|
| fester Text | `tr!("Abbrechen", "Cancel")` |
| Text mit Werten | `trf!("{n} Karten", "{n} cards")` oder `trf!("{} zieht", "{} draws", name)` |

---

## 7. Eigene Skripte (GDScript) – Beispiel

```gdscript
@onready var forge: ForgeBridge = $Table/ForgeBridge
@onready var ft: ForgeTable = $Table/ForgeTable

func _ready() -> void:
    forge.forge_event.connect(func(e): print(e.event, ": ", e.text))
    forge.game_over.connect(func(r): print(r.winner, " gewinnt"))

func status() -> void:
    print(ft.get_pending_kind())          # z. B. "Priority", "Mulligan", "ChooseTargets" oder ""
    print(ft.get_state().get("phase"))    # z. B. "MAIN1"
```
