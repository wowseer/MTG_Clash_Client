# Anweisungen für Claude: Animationsdesign

Lies diese Datei, bevor du eine Animation baust oder änderst (Kartenbewegung, Effekte, HUD,
Kamera). Ziel: Jede Bewegung im Spiel fühlt sich gleich hochwertig an, ist lesbar und hält das
Spiel nie auf.

## Grundsätze

1. **Animation erklärt Spielzustand.** Jede Animation zeigt, *woher* etwas kommt und *wohin* es
   geht (Deck → Hand, Hand → Stack → Spielfeld, Spielfeld → Friedhof). Keine Karte erscheint oder
   verschwindet einfach, außer sie entsteht oder vergeht wirklich (Spielstein).
2. **Forge ist die Wahrheit, die Animation läuft hinterher.** Der Zustand wird sofort
   übernommen; Animationen stellen ihn nur dar. Eine Animation darf nie eine Antwort an Forge
   verzögern oder blockieren. Kommt ein neuer Zustand, während noch animiert wird: die laufende
   Bewegung von ihrer aktuellen Position aus zum neuen Ziel umlenken, nicht neu starten.
3. **Eingabe hat Vorrang.** Der Spieler kann während jeder Animation klicken. Was er anklickt,
   reagiert sofort (Hover, Greifen); laufende Zierde wird abgebrochen.
4. **Eine Sprache für alles.** Gleiche Bedeutung = gleiche Bewegung, gleiche Dauer, gleiche
   Kurve. Werte stehen zentral als Konstanten (siehe „Timing-Tabelle“), nie als Zahlen verstreut.
5. **Weich, mit Gewicht.** Karten sind leichte, aber physische Objekte: Sie heben ab, fliegen in
   einem flachen Bogen, landen mit einem kleinen Nachfedern. Kein lineares Gleiten, kein Teleport,
   kein Schweben ohne Grund.
6. **Lesbarkeit vor Show.** Wichtige Ereignisse (Schaden, Tod, Gegenzauber, Spielende) bekommen
   Nachdruck. Routine (Enttappen, Ziehen im eigenen Zug) bleibt kurz und ruhig. Gleichzeitige
   Bewegungen leicht staffeln (40–80 ms), damit das Auge folgen kann.
7. **Eigene Gestaltung.** Ähnliche Qualität wie moderne digitale Kartenspiele, aber eigene
   Bewegungen, Effekte und Klänge. Nichts aus anderen Spielen nachbauen.

## Timing-Tabelle (Startwerte, zentral als Konstanten pflegen)

| Ereignis | Dauer | Kurve | Bemerkung |
|---|---|---|---|
| Hover-Zoom an / aus | 0,12 s / 0,10 s | Expo-Out | sofort spürbar |
| Karte folgt beim Ziehen | exponentiell, Faktor ~20/s | – | schon vorhanden (`drag_follow_speed`) |
| Ziehen Deck → Hand | 0,45 s | Cubic-Out, Bogen 0,6 hoch | beim Abheben kurz umdrehen (Rückseite → Vorderseite) in der zweiten Hälfte |
| Austeilen der Starthand | je 0,35 s, gestaffelt 0,08 s | Cubic-Out | |
| Hand → Stack (Zauber gespielt) | 0,35 s | Back-Out (leichtes Überschwingen) | Karte wird größer, kurzer Lichtring |
| Stack → Spielfeld (bleibende Karte) | 0,40 s | Cubic-In-Out | Landung: 4 % Stauchung + Staubring, 0,15 s |
| Stack → Friedhof (Spontanzauber) | 0,45 s | Cubic-In | Karte zerfällt / löst sich auf, dann Friedhof |
| Tappen / Enttappen | 0,18 s | Quad-Out | Drehung um 90° bzw. projekteigene Neigung |
| Angriff erklären | 0,25 s | Back-Out | Karte rückt 0,3 nach vorn, Pfeil wächst aus der Karte |
| Pfeil erscheinen | 0,33 s (Blender: Frame 1 → 9 bei 24 fps) | weich (smoothstep) | wächst vom Start zum Ziel (`TargetArrows`, `GROW_TIME`); selbst gezogene Pfeile wachsen nicht |
| Mana bezahlen (`ManaFx`) | 80 Frames à 30 fps (2,7 s) | wie Blender (`mana_glow_generator.py`) | Land blitzt (F6), Komet startet (F14, `t^1,7`), Einschlag (F30), Ausklingen bis F80; Größe `UNIT` 0,5 je Blender-Einheit |
| Rahmen-Glow aufleuchten (`Card3D::flash`) | 1,6 s | weich | Verstärkung 1 → 4,6 (0,07 s) → 2,7 → 1,7 → 1, solange orange |
| Angreifer rückt vor | exponentiell (Faktor 10/s) | – | 0,22 nach vorn (Kartenmaß), höchstens 0,3 Welteinheiten (Lücke zwischen den Kreaturenreihen 0,8 = 2 × `FRONT_GAP` in `zone.rs`, etwa 0,6 Kachelhöhen wie im Video „Blocken“; es bleibt deutlich Abstand); einem Blocker gegenüber gar nicht; Angriffsspitze sichtbar |
| Schlag im Kampf (`Card3D::lunge`, Video „Blocken“) | 0,30 s hin / 0,38 s zurück | quadratisch / Cubic-Out | Angreifer läuft in den Blocker (95 % des Wegs) und landet über ihm (angehoben 0,6 bis zum Treffer, `sin(π/2·k)`), dabei bis 16° seitlich gekippt; Blocker stemmt sich nur 18 % dagegen; zum Spieler 85 %; Einschlag rot beim Treffer. Schlagender und Getroffener bleiben bis 0,15 s nach dem Treffer liegen und behalten ihre Form (`Card3D::hold`), erst dann ziehen sie z. B. zum Friedhof |
| Blocker zuweisen (Video „Blocken“) | 0,25 s Farbe / 1,6 s Aufleuchten | exponentiell (Faktor 14/s) | Rahmen des Blockers blendet von Blau zu Orange über (`GLOW_FADE_SPEED`) und blitzt auf (`Card3D::flash`); geblockter Angreifer und Blocker rutschen auf eigene Stapel |
| Blockschweif (Video „Blocken“, `Card3D::set_block_trail`) | ein/aus ≈ 0,25 s | exponentiell (Faktor 12/s, `TRAIL_FADE_SPEED`) | Bogen aus flackernden Lichtzungen über der Kante des Blockers zum Angreifer (fBm-Rauschen, steigt mit 1,7/s), in der Mitte am höchsten (`TRAIL_HEIGHT` 0,24); Pfeil Blocker → Angreifer orange |
| Block erklärt (Video „Blocken“, `Card3D::set_block_declared`) | ≈ 0,25 s | exponentiell (Rahmen 14/s, Spitze/Schweif 12/s) | Angreifer und Blocker werden gelb (`BLOCKED_GLOW`): Rahmen, Angriffsspitze, Schweif (ruhiger), Pfeil gelb ohne neu zu wachsen; bis zum Ende des Kampfes. Gilt sofort nach dem Bestätigen (`sent_blocks`) und für die Blocks der KI |
| Pfeil-Bogen auf dem Stack | wächst wie jeder Pfeil (0,33 s) | weich | Ziel liegt unter der Quelle (näher als `LOOP_DISTANCE`): Bogen steigt aus der vorderen Karte auf (Scheitel 1,6) und landet auf dem sichtbaren Rand der Karte dahinter |
| Hand-Bühne schieben (`Slider3D`) | Zonen-Tween 0,35 s | Cubic-Out | Karten gleiten dem Regler nach; Fächer 0,7 Abstand, bis ±Überbreite/2 |
| „Wähle eins“ | Bühne wie Mulligan (0,20 s auf / 0,15 s zu) | Cubic-Out | Kartenkopien gleiten aus der Fächermitte in den Fächer |
| Pfeil biegen | Feder 140 / Dämpfung 15 | leicht unterdämpft | Querbewegung eines Endes biegt den Pfeil (höchstens 0,3 der Länge), danach schwingt er gerade zurück |
| Pfeil Lichtwellen | laufend, 1,5 Einheiten/s | – | fBm-Rauschen entlang des Pfeils zur Spitze hin (Treiber `frame/24`) |
| Kampfschaden | 0,30 s Stoß + 0,2 s zurück | Expo-Out / Quad-In | Zahl springt aus dem Ziel, Treffer-Blitz |
| Kreatur stirbt | 0,50 s | Quad-In | entsättigen, zerfallen, dann zum Friedhof |
| Spielstein entsteht / vergeht | 0,30 s | Back-Out / Quad-In | aus Licht aufbauen / in Partikel zerfallen |
| Doppelseitige Karte umdrehen | 0,35 s | Cubic-In-Out | 180°-Drehung um die Längsachse, Mitte 0,25 anheben; umgesetzt: `Card3D::flip_to` (`FLIP_DURATION`, `FLIP_LIFT`), auch beim Aufdecken verdeckter Karten |
| Lebenspunkte ändern | 0,6 s | – | Edelstein blitzt (schon vorhanden), Zahl zählt hoch/runter |
| Phasenwechsel | 0,25 s | Cubic-Out | Zeitstrahl (vorhanden) |
| Auswahlfenster / Kartenfächer öffnen | 0,20 s | Cubic-Out | Hintergrund abdunkeln 0,15 s |
| 3D-Knopf erscheinen / verschwinden | 0,20 s / 0,15 s | Cubic-Out / linear | wächst aus der Mitte (`Button3D`, `APPEAR_TIME`, `DISMISS_TIME`) |
| 3D-Knopf Hover / Klick | exponentiell (≈0,12 s) / 0,12 s | – / Sinus | +5 % Größe, Leuchten +45 %; Klick drückt 7 % ein |
| 3D-Knopf Oberfläche | 8 s Schleife (Blender: 4 s; halbes Tempo, ruhiger) | – | Shader nach den Blender-Treibern (`speed` 0,5): Rauschen kreist in z/w, Leuchten 0,85 + 0,15·sin(2φ); Zeitversatz je Knopf zufällig |
| Farbrad (`ManaWheel`) aufgehen / schließen | 0,20 s / 0,15 s | Cubic-Out / linear | wächst aus der Mitte der Karte, folgt ihr; Segment-Hover exponentiell (Faktor 18/s), +60 % heller |
| Planeswalker-Ansicht (`LoyaltyView`) aufgehen / schließen | 0,20 s / 0,15 s | Cubic-Out / linear | Karte wächst von 90 % und blendet ein, Tisch dunkelt auf 55 % ab; Feld-Hover exponentiell (18/s), 3 % größer, goldener Rand |
| Auswahlliste (`SearchList`) erscheinen | 0,20 s | Cubic-Out | wächst von 96 % und blendet ein (mit der Bühne); Zeilen-Hover exponentiell (18/s), Auswahl golden |
| Dungeon-Ansicht (`DungeonView`) aufgehen / schließen | 0,25 s / 0,15 s | Cubic-Out / linear | Karten wachsen von 92 %, Tisch dunkelt auf 62 % ab; wählbare Räume pulsieren blau (1,2 s), Hover golden (18/s) |
| Loyalitäts-Schild blitzt bei Änderung | 0,6 s | Sinus-Puls | springt um bis zu 45 % auf und leuchtet hell, klingt linear ab |
| Exil-Strudel öffnen / schließen | 0,30 s / 0,30 s | Back-Out / Quad-In | nur offen, solange Karten im Exil liegen; Wirbel dreht laufend (`TIME`), Größe 1,45 |
| Bühne (Showcase) öffnen / schließen | 0,15 s | linear | Abdunkeln und Texte ein-/ausblenden; Karten gleiten mit `move_duration` herein und zurück |
| Kamera Fokus / Übersicht | exponentiell, `smoothing` | – | vorhanden (`TableCamera`) |

Faustregel: Routine ≤ 0,3 s, wichtige Ereignisse 0,3–0,6 s, nie über 0,8 s ohne Grund.
Gegnerzüge dürfen etwas langsamer sein als eigene, damit man sie verfolgen kann.

## Technik im Projekt

- **Bewegung von Karten:** `Card3D::move_to(target, delay)` (Tween auf `global_transform`).
  Erweiterungen (Bogen, Drehung, Nachfedern) dort zentral einbauen, nicht pro Aufrufer. Für Bögen
  `tween_method` mit eigener Interpolation statt `tween_property` verwenden.
- **Neu-Ausrichtung in Zonen:** `Zone::layout_with_delay` legt Ziele fest; die Zone entscheidet
  das Ziel, `Card3D` entscheidet, *wie* es dorthin geht.
- **Hover/Neigung:** im `process` exponentiell geglättet (`tilt_speed`), nicht per Tween.
- **Unterbrechen:** laufenden Tween immer `kill()`en und vom aktuellen Transform aus neu
  starten (siehe `Card3D::move_to`). Nie zwei Tweens auf dieselbe Eigenschaft.
- **Effekte** (Licht, Funken, Zerfall): `GPUParticles3D` oder Shader auf der Karte (Parameter
  über `ShaderMaterial` animieren). Partikel nach Ablauf freigeben (`one_shot` + `finished`).
- **HUD:** `create_tween()` auf dem Control; Positionen über `position`/`modulate`/`scale`.
  HUD-Animationen laufen unabhängig von der 3D-Szene.
- **Bildrate:** Alles zeitbasiert (`delta`, Tween-Dauer), nie pro Frame feste Schritte.
- **Reihenfolge von Ereignissen:** Forge schickt `forge_event`s (z. B. `GameEventCardChangeZone`,
  `GameEventCardDamaged`, `GameEventSpellResolved`). Für Effekte, die über die reine
  Zonen-Bewegung hinausgehen, diese Ereignisse in eine **Animations-Warteschlange** im
  `ForgeTable` legen und nacheinander abspielen; der Zustandsabgleich bleibt davon unabhängig.

## Vorlage für eine neue Animation

Bevor du eine Animation baust, fülle diese Vorlage aus (im Commit-Text oder in
`docs/DOCUMENTATION.md`):

```
Animation: <Name, z. B. „Kreatur stirbt“>
Auslöser:   <Forge-Ereignis oder Zustandsänderung, z. B. GameEventCardDestroyed / Karte verlässt Spielfeld → Friedhof>
Bedeutung:  <was der Spieler daraus verstehen soll>
Ablauf:     1. <Schritt, Dauer, Kurve>
            2. …
Gesamtdauer: <s>   Blockiert Eingabe: nein
Unterbrechung: <was passiert bei neuem Zustand / Klick währenddessen>
Gestaffelt mit: <andere gleichzeitige Animationen, Versatz>
Umsetzung:  <Card3D::…, Zone::…, Shader/Partikel, HUD>
Konstanten: <Name = Wert in der zentralen Tabelle>
Test:       <wie geprüft: headless Ablauf + Sichtprüfung durch den User>
```

## Prüfen

- Logik (richtiger Start/Ziel, keine hängenden Karten, kein doppelter Tween) ohne Fenster mit
  einem Testskript prüfen.
- Sichtprüfung macht der User. Fenster-Renders (`--write-movie`) öffnen sich auf seinem Desktop
  und bekommen seine echten Mausklicks: nur nach Absprache, nie `warp_mouse`.
- Nach jeder Animation: Dauer und Kurve in die Timing-Tabelle oben eintragen, Doku pflegen,
  committen.
