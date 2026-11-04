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
| unten links | Länder (eigene Länder-Reihe, Spalten nach Manafarbe) |
| oben Mitte | Kreaturen – eine Reihe, bei vielen Kreaturen weitere Reihen darunter (bei Platzmangel überlappend) |
| rechts | alles andere (Artefakte, Verzauberungen …) – rechtsbündiges Raster, eine Spalte je 3 Karten, höchstens 4 Spalten |

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

Das Brett ist breit angelegt (`board_size` = 19 × 9,6 in der Szene), damit es fast die ganze Bildbreite füllt; die Spielfelder sind 16,4 breit, Deck/Friedhof liegen rechts außen (x = ±8,6), die Länder-Reihen links außen (x = ∓6,6). Der Lichtkegel (`Lantern.spot_angle` 46°) deckt das ganze Brett ab.
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
| Bildmitte oben / unten | `SeatBadge`-Medaillon von Gegner (rot, direkt unter dessen Hand) / Spieler (blau, direkt über der eigenen Hand). Die Position kommt von den `HeroAnchor`-Markern (`TurnHud.player_hero_anchor` / `opponent_hero_anchor`); ohne Marker sitzen die Medaillons links oben / unten. Gezoomte (gehoverte) oder gezogene Karten liegen sichtbar **über** beiden Medaillons: Jede Medaillon-Zeile hängt unter einer Hülle (`CardCutout`, `card_cutout.rs`), deren Canvas-Shader alle Pixel im auf den Bildschirm projizierten Kartenviereck (`Card3D.screen_quad()`) verwirft. Solange eine Karte gehovert oder gezogen wird, werden beide Medaillon-Zeilen außerdem weich durchsichtig (30 % Deckkraft). Raute mit Wappen und Anfangsbuchstabe, roter Edelstein mit Lebenspunkten; pulsierender Schein = am Zug; Edelstein blitzt bei Schaden rot, bei Heilung grün |
| unten rechts, über dem Button | `PhaseTimeline`: Zeitstrahl der Phasen mit drei Stationen – (1) ein Kreis für Enttappen, Auffrischung, Ziehen und Hauptphase 1, der mit jedem dieser Schritte größer wird; (2) ein Punkt „Kampf“, dessen drei Schritte (Angreifer, Blocker, Schaden) erst beim Kampf aufklappen; (3) ein Kreis für Hauptphase 2 und Ende. Symbole: Enttappen = Kreispfeil, Auffrischung = Sonne, Ziehen = Karte mit Plus, Hauptphasen = aufgefächerte Karten, Ende = Mondsichel, Kampf = gekreuzte Schwerter (Angreifer = Schwert, Blocker = Schild, Schaden = Flamme). Die aktuelle Station leuchtet. Forges `PhaseType` wird darauf abgebildet (`forge/table.rs`, `phase_index`) |
| Bildmitte | nur bei Spielende: „Spielende – X gewinnt!“ |
| unter der eigenen Länder-Reihe | Manapool als runde Symbole (`ManaPoolView`, `mana_view.rs`), aus Forges `mana_pool` (schwebendes Mana, z. B. aus Ritualen) |
| oben rechts | Spielprotokoll (`GameLogList`, `log_view.rs`): die letzten 7 Einträge des Engine-Protokolls (`ForgeBridge.log_entry`), z. B. „Spieler cast Dragon Fodder“, Schaden, Zonenwechsel; ohne Phasen- und Mana-Einträge, Rundenbeginn golden, Objekt-IDs entfernt |
| linker Bildrand, Mitte | Stack-Liste (`StackList`, `stack_view.rs`): je Eintrag „Quelle · Art“ (Zauber, aktivierte bzw. ausgelöste Fähigkeit) und Forges Beschreibung mit Zielen; oberster Eintrag oben und golden, Rahmen in der Farbe der kontrollierenden Seite. Leerer Stack = ausgeblendet |
| linker Bildrand, oben / unten | Gegner (oben links) bzw. Spieler (unten links): Rundenzahl mit Sanduhr-Symbol (`HourglassIcon`, `hourglass.rs`), darunter der Spielername; der Name der Seite am Zug ist golden |
| Bildschirmmitte | Auswahlfenster (`ChoicePanel`, `choice_panel.rs`) für alle allgemeinen Forge-Entscheidungen: eine Option, mehrere Optionen, Ja/Nein, Zahl; Suchfeld bei langen Listen (höchstens 40 Knöpfe, z. B. bei allen Kartennamen); welche Engine-Entscheidung welches Fenster ergibt, steht in `decision_view.rs` (Bedienung siehe Abschnitt 3) |
| ganz oben, mittig | große Vorschau der Karte, die gerade aus der Hand gezogen wird (`card_preview.rs`); das Gegner-Medaillon liegt dann durchsichtig darüber |
| unten rechts | großer Button „Passen“ (auch Leertaste), nur aktiv, wenn Forge auf deine Priorität wartet |

### Dateien

| Ordner / Datei | Inhalt |
|---|---|
| `rust/src/table/` | Darstellung: `Card3D`, `Zone`, `TableCamera`, `TableController`, Ziel- und Kampfpfeile `TargetArrows` (`target_arrows.rs`), Kulissen `Arena` (`arena.rs`) und `Tavern` (`tavern.rs`), prozedurale Texturen (`textures.rs`), Kartenbilder (`placeholder.rs`) |
| `rust/src/game/` | Anzeige-Bausteine: `PlayerSeat`, Kartenmodell aus den Kartendaten der Engine (`cards.rs`: `CardDef::from_state`, Kartenart, Wertefeld), Regeltext-Symbole und Schriftgrößen (`card_text.rs`), Markennamen (`counters.rs`), Stack-Liste (`stack_view.rs`), Mana-Typen und -Pool (`mana.rs`), Phasen-Liste (`phases.rs`), HUD (`hud.rs`), Spieler-Medaillon (`seat_badge.rs`), Sanduhr (`hourglass.rs`), Manapool-Anzeige (`mana_view.rs`), Phasen-Zeitstrahl (`phase_timeline.rs`), Kartenvorschau (`card_preview.rs`), Auswahlfenster (`choice_panel.rs`), Kartenfächer (`card_fan.rs`), Engine-Entscheidung → Auswahlfenster → Antwort (`decision_view.rs`), Karten-Aussparung der Medaillons (`card_cutout.rs`) |
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
| **Wer beginnt?** | Gewinnst du den Münzwurf, schweben über der Tischmitte zwei leuchtende 3D-Knöpfe untereinander: „Ich beginne“ (orange) und „Forge-KI beginnt“ (blau), darüber die Frage. Maus darüber = Knopf wächst, Leuchten wird heller; Klick = Wahl. Bausteine: `Button3D` (`table/button3d.rs`, Kollisions-Layer 3 `CONTROL_LAYER`, Vorrang vor Karten im `TableController`), Bilder aus `Assets/Buttons/*.blend` (`tools/render_buttons.sh` → `godot/assets/buttons/`) |
| Maus über Karte | Karte neigt sich; offene Karten werden groß und lesbar angezeigt (Hover-Zoom). Nicht durch Buttons hindurch |
| Deck (rechts außen) | verdeckter Stapel, so groß wie deine Bibliothek in Forge. Gezogen wird automatisch im Ziehschritt (Forge); die Karte gleitet sichtbar vom Deck auf die Hand |
| Spielbare Handkarten | leuchten, sobald Forge auf deine Priorität wartet und du sie jetzt spielen und bezahlen kannst |
| Leuchtende Karte aus der Hand ziehen und **irgendwo** auf dem Tisch loslassen (nicht über der Hand) | spielen: Forge bekommt die passende Option, zahlt das Mana automatisch und legt die Karte hin (Länder in die Länder-Reihe, bleibende Karten aufs Spielfeld, Zauber auf den Friedhof). Solange du ziehst, wird die Karte zusätzlich **groß ganz oben in der Bildmitte** angezeigt. Ungültige Züge springen an ihren Platz zurück |
| Button „Passen“ bzw. Leertaste | Priorität abgeben |
| Auswahlfenster (Bildschirmmitte) | erscheint bei jeder allgemeinen Entscheidung von Forge (Mulligan, Startspieler, Modi, Kreaturtyp, Farbe, Kartenname, Ja/Nein, Zahl …). Die Karte, um die es geht, ist golden markiert. **Eine Option:** Knopf anklicken. **Mehrere:** Knöpfe an-/abwählen (golden = gewählt), dann „Fertig (n/…)“ – aktiv erst bei gültiger Anzahl. **Zahl:** Zahlenfeld, „OK“. Ab 13 Optionen gibt es ein **Suchfeld** (Treffer, die mit dem Suchtext beginnen, zuerst); angezeigt werden höchstens 40 Knöpfe – bei „Kartennamen nennen“ (alle über 34.000 Namen) also einfach den Namen eintippen und anklicken. „Abbrechen“ nur, wo Forge das erlaubt. Farb-Optionen sind in der Manafarbe eingefärbt |
| **Klick vs. Ziehen** | Kurz klicken (Maus bewegt sich weniger als 6 Pixel) = Klick; erst bei Bewegung wird die Karte gezogen |
| Klick auf eine leuchtende Karte auf dem Spielfeld | Fähigkeit aktivieren (z. B. Ausrüsten). Hat die Karte mehrere Fähigkeiten, erscheint ein Menü „Fähigkeit wählen“ (mit „Abbrechen“). Handkarten spielt man weiter durch Ziehen |
| **Ziel wählen** | Gültige Ziele leuchten (Karten) bzw. bekommen einen pulsierenden goldenen Ring (Spieler-Medaillons). Klick auf die Karte bzw. das Medaillon wählt das Ziel; bei nur einem Ziel geht es sofort weiter, bei mehreren erneuter Klick = abwählen, dann „Fertig (n/…)“. Gewählte Ziele sind golden markiert und bekommen einen goldenen **Pfeil** von der Quellkarte; ein blasser Pfeil zeigt auf das gültige Ziel unter dem Mauszeiger. Das Auswahlfenster steht dabei am rechten Bildrand (Liste aller Ziele als Ausweichweg, „Abbrechen“ nimmt den Zauber zurück) |
| **Mana bezahlen** | Nach dem Wirken (und nach Modus, X, Zielen) erscheint am rechten Rand „Mana bezahlen: {2}{R}“. Länder und andere Manaquellen, die jetzt Mana erzeugen können, leuchten: Klick = tappen, das Mana wird sofort eingesetzt und die offenen Kosten sinken (bei Quellen mit mehreren Manafähigkeiten erscheint ein Menü). Schwebendes Mana im Manapool wird automatisch zuerst verwendet. „Automatisch bezahlen“ überlässt den Rest Forge, „Abbrechen“ nimmt das Wirken zurück |
| **Kartenfächer** (Karten wählen) | Muss man Karten wählen – abwerfen, opfern, Bibliothek durchsuchen, Karten für einen Effekt, nach einem Mulligan Karten unter die Bibliothek –, erscheinen sie groß in der Bildmitte (Tisch abgedunkelt, auch Karten aus der Bibliothek). Klick wählt bzw. wählt ab (goldener Rahmen), „Fertig (n/…)“ ist erst bei gültiger Anzahl aktiv. Viele Karten: kleiner und scrollbar |
| **Hellsicht / Überwachen** | Kartenfächer mit zwei Reihen „Oben auf die Bibliothek“ und „Unter die Bibliothek“ bzw. „In den Friedhof“. Klick legt eine Karte ans Ende der anderen Reihe; die Reihenfolge oben gilt von oben nach unten. „Fertig“ |
| **Reihenfolge wählen** | Gleichzeitige eigene Trigger (bzw. Karten, die gleichzeitig in eine Zone wechseln): im Auswahlfenster in der gewünschten Reihenfolge anklicken (Knöpfe zeigen „1.“, „2.“ …), erneuter Klick nimmt eine Karte heraus; dann „Fertig“ |
| **Kosten wählen** | Muss als Kosten eine Karte geopfert, abgeworfen, getappt, exiliert oder auf die Hand zurückgegeben werden (z. B. „Opfere eine Kreatur:“), erscheint der Kartenfächer mit allen passenden Karten. Gibt es genau so viele passende Karten wie nötig, wird nicht gefragt |
| **Phyrexianisches Mana** | Vor dem Bezahlen je Symbol die Frage „mit 2 Leben bezahlen?“ („2 Leben zahlen“ / „Mit Mana zahlen“) |
| **„…, es sei denn, du zahlst“** | Kann man die Kosten bezahlen, fragt das Spiel „Zahlen“ / „Nicht zahlen“; bezahlt wird wie beim Wirken (Mana über das Bezahl-Fenster). Kann man nicht zahlen, wird nicht gefragt |
| **Schaden verteilen** | Bei Zaubern wie „… beliebig auf ein oder zwei Ziele aufgeteilt“ und bei Kampfschaden gegen mehrere Blocker bzw. mit Trampeln: Fenster mit einem Zahlenfeld je Ziel, vorbelegt (Kampf: tödlicher Schaden der Reihe nach, Rest aufs Trampel-Ziel), Anzeige „Verteilt: n / gesamt“; „OK“ erst, wenn die Summe stimmt. Beim Wirken bekommt jedes Ziel mindestens 1 |
| **Ziele ausgelöster Fähigkeiten** | Eigene Trigger mit Ziel (z. B. „Wenn … ins Spiel kommt, fügt einer Kreatur 4 Schaden zu“) fragen das Ziel wie beim Wirken (Klick auf Karte/Medaillon) |
| **Aufgedeckte Karten** | Deckt ein Spieler Karten auf (z. B. eine gesuchte Karte aus der Bibliothek), erscheinen sie im Kartenfächer mit Überschrift „Aufgedeckt – …“; „Fertig“ schließt die Anzeige. Mitteilungen wie „wählt Rot“ stehen im Spielprotokoll |
| **Kopien, Umleiten, Erschöpfen, Rekrutieren** | Kopiert ein Effekt einen Zauber und erlaubt neue Ziele (z. B. Reverberate), fragt das Spiel „Neue Ziele wählen für …?“; bei „Ja“ wie beim Wirken Ziel anklicken. Gegenzauber und Kopiereffekte zielen auf den Zauber im Stack (Karte in der Tischmitte anklicken). Beim Angriff fragt das Spiel nach Angreifern, die erschöpft (Exert) bzw. rekrutiert (Enlist) werden sollen: auf dem Tisch anklicken, „Fertig“ |
| **Convoke, Improvise, Delve** | Beim Wirken solcher Zauber fragt das Spiel vor dem Bezahlen: ungetappte Kreaturen (Convoke) bzw. Artefakte (Improvise) leuchten auf dem Tisch, jede gewählte bezahlt ein Mana (passende Farbe zuerst); bei Delve erscheinen die Friedhofskarten im Kartenfächer, jede exilierte Karte spart {1}. „Fertig“ mit 0 Karten = normal bezahlen |
| **Karten auf dem Tisch wählen** | Muss man Karten wählen, die alle offen auf dem Tisch liegen (z. B. eine Kreatur als Kosten opfern, „wähle eine bleibende Karte“), leuchten sie direkt auf dem Tisch: Klick wählt (goldene Markierung). Das Fenster am rechten Rand zeigt die Namen als Ausweichweg und „Fertig“ bei mehreren. Muss man einen Spieler wählen („Choose a player“), pulsieren die Medaillons wie bei Zielen |
| **Angreifen** (Angriffsschritt) | Kreaturen, die angreifen können, leuchten. Klick = angreifen (roter Pfeil zum Gegner, rote Markierung), erneuter Klick = nicht angreifen. Gibt es mehrere Angriffsziele (z. B. Planeswalker), danach Klick auf das gewünschte Ziel (Karte bzw. golden pulsierendes Medaillon) = der zuletzt gewählte Angreifer greift es an. Bestätigen mit „Angreifen (n)“ bzw. „Nicht angreifen“ im Fenster am rechten Rand. Ungültige Angriffe (z. B. eine Kreatur muss angreifen) werden mit Hinweis erneut gefragt |
| **Blocken** (Blockschritt) | Eigene Kreaturen, die blocken können, leuchten. Klick auf eine davon (goldene Markierung), dann Klick auf einen Angreifer = blocken (blauer Pfeil, blaue Markierung; ein blasser Pfeil zeigt vorher auf den Angreifer unter der Maus). Klick auf einen Blocker = Block aufheben. Mehrere Blocker auf denselben Angreifer sind möglich. Bestätigen mit „Blocken (n)“ bzw. „Nicht blocken“; ungültige Blocks werden mit Forges Begründung erneut gefragt |
| **Marken, Spielsteine, angelegte Karten, Exil** | Marken stehen als helle Plaketten über dem Bildfeld der Karte („+1/+1 ×2“, „Loyalität 4“), Spielsteine tragen die Plakette „Spielstein“. Auren und Ausrüstung liegen leicht versetzt unter ihrem Träger und wandern mit ihm. Exilierte Karten liegen offen auf dem Exil-Stapel neben dem Friedhof. Spieler-Marken (Gift, Energie, Erfahrung …) stehen am linken Bildrand unter dem Namen |
| **Kommandozone, doppelseitige und verdeckte Karten** | Commander, Embleme, Dungeons (Plakette „Raum: …“), The Ring, The Monarch, The Initiative u. a. liegen in der Kommandozone der Seite (Spalte am rechten Rand). Doppelseitige Karten drehen sich beim Transformieren sichtbar um (0,35 s, 180° um die Längsachse, `Card3D::flip_to`); die andere Seite steht als Plakette „↺ Name“. Verdeckte Karten (Morph, Manifest) zeigen das Rückseitenmuster mit ihren Werten (meist 2/2). Monarch, Initiative, Stadtsegen und Ring-Stufe stehen unter dem Spielernamen, Tag ☀ / Nacht ☾ neben der Rundenzahl |
| **Stack** | Zauber, die gewirkt werden, gleiten in die Stack-Zone in der Tischmitte; aktivierte und ausgelöste Fähigkeiten erscheinen dort als Karte ihrer Quelle. Der oberste Eintrag liegt rechts oben. Violette Pfeile zeigen die Ziele jedes Eintrags (auch auf andere Zauber im Stack, z. B. Gegenzauber). Die Liste am linken Bildrand nennt Quelle, Art und Text. Bei belegtem Stack ist der Hauptbutton orange |
| Kampf der KI | Angriffe der KI erscheinen als rote Pfeile (Angreifer → Ziel), Blocks als blaue Pfeile, solange der Kampf läuft |
| Rechtsklick auf Karte | Kamera-Nahaufnahme; erneut oder Esc = Übersicht |

**Kartenbild:** Name, Kostensymbole, Typzeile und Regeltext kommen von der Engine. Symbole im
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
Forge-KI springt nur ein, wenn eine Antwort ungültig ist. **Mana aus dem Pool:** Schwebendes Mana (z. B. aus Ritualen) wird nicht
mehr automatisch ausgegeben: Beim Bezahlen sind passende Symbole der Manapool-Anzeige golden
umrandet, Klick setzt eines davon ein („Automatisch bezahlen“ nutzt den Pool ebenfalls).
**Modi mit Wiederholung** („you may choose the same mode more than once“): jeder Klick zählt den
Modus erneut („2× …“), bei voller Auswahl nimmt ein Klick ihn wieder heraus. Fahrzeuge besetzen:
Kreaturen auf dem Tisch anklicken, bis die nötige Stärke erreicht ist, dann „Fertig“.

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
  ihres Trägers versetzt darunter (`Zone.set_attached`), auch auf der Gegnerseite. Marken und
  „Spielstein“ stehen als Plaketten über dem Bildfeld (`Card3D::set_badges`, Texte aus
  `game/counters.rs`); Stack-Einträge → Zone `stack_zone` (Zauber als ihre Karte, Fähigkeiten als Karte
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
