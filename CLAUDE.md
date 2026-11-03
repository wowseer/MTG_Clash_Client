# CLAUDE.md – MTG Clash

## Ziel

**Das komplette Backend von Forge in die 3D-UI bringen.** Forge (Java-Regel-Engine) ist die
einzige Regel-Engine des Spiels. Jede Entscheidung, die Forge einem Spieler abverlangt, soll der
Spieler in der 3D-Ansicht selbst treffen, und jeder Teil des Forge-Spielzustands soll auf dem
Tisch oder im HUD sichtbar sein. Am Ende entscheidet die Forge-KI nichts mehr für den
menschlichen Spieler. Sie steuert nur noch den Gegner.

Ein eigenes Regelwerk wird **nicht** wieder eingeführt: Das Spiel prüft keine Regeln selbst,
es zeigt Forges Zustand an und schickt Forge die Antworten des Spielers.

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
| `forge-bridge/src/clash/forgebridge/` | `RemotePlayerController` (Entscheidungen des Spielers → `request`), `StateJson` (Zustand → JSON), `ForgeBridge` (Befehle, Partie, Anfragen) |
| `forge-bridge/forge/` | lokale Kopie von Forge (Jar + `res/`), **nicht im Git** (GPL, Kartendaten von WotC) |
| `docs/Forge-Schnittstelle.md` | Protokoll (Befehle, Nachrichten, `state`-Aufbau). **Bei jeder Protokolländerung pflegen** |

Forge-Sitzplatz 0 = Spieler unten (ferngesteuert), Sitzplatz 1 = Gegner oben (Forge-KI).

## Stand

Ferngesteuert, also vom Spieler in der UI entschieden:
- [x] Priorität: Karte oder Fähigkeit spielen bzw. passen (`chooseSpellAbilityToPlay`): Karte aus der Hand auf den Tisch ziehen, Button/Leertaste = passen
- [x] Mulligan (`mulliganKeepHand`): Auswahlfenster im HUD

Angezeigt:
- [x] Hand, Spielfeld, Länder-Reihe, Friedhof, verdeckte Gegnerhand, getappt
- [x] Bibliothek als Deck-Stapel; Ziehen gleitet animiert vom Deck auf die Hand
- [x] Phase, Leben, Runde, Spieler am Zug, Spielende

## Fahrplan

Jeder Punkt ist ein eigener Schritt: Brücke (Java), Protokoll, Rust/UI, Doku, Test, Commit.
Die vollständige Liste mit Zuordnung zu Bedienmustern steht in `docs/Kartenabdeckung.md`.

**Entscheidungen des Spielers** (in `RemotePlayerController` überschreiben, als `request` an Godot):
1. Angreifer erklären (`declareAttackers`): Klick auf Kreaturen, Ziel Spieler/Planeswalker
2. Blocker erklären (`declareBlockers`): Blocker einem Angreifer zuweisen. Die Zielpfeile werden dafür neu gebaut; die alten wurden mit dem eigenen Regelwerk gelöscht
3. Ziele wählen (`chooseTargetsFor`, `chooseSingleEntityForEffect`, `chooseEntitiesForEffect`): gültige Ziele leuchten, Klick oder Pfeil
4. Manabezahlung (`payManaCost`, `chooseManaFromPool`): Länder antippen; mit Automatik-Button
5. Aktivierte Fähigkeiten bleibender Karten: Klick auf die Karte, bei mehreren Fähigkeiten ein Auswahlmenü
6. Auswahl aus Karten (`chooseCardsForEffect`, Abwerfen, Opfern, Scry/Surveil, Reihenfolge)
7. Modi, X-Kosten, Zahlen, Farben, Ja/Nein (`chooseModeForAbility`, `announceRequirements`, `chooseNumber`, `chooseColor`, `confirmAction`)
8. Kampfschaden verteilen (`assignCombatDamage`), optionale und zusätzliche Kosten

**Zustand, der noch fehlt:**
- Stack mit Zaubern und Fähigkeiten, Ziele als Pfeile
- Manapool (die Anzeige `ManaPoolView` gibt es schon, Forge liefert den Pool noch nicht)
- Exil, Marken (Counter), angelegte Karten (Auren, Ausrüstung), Spielsteine
- Regeltext von Forge auf dem Kartenbild; das Kartenmodell (`CardDef`/`Card3D`) auf Forges Kartendaten verschlanken (Helden/Willenskraft-Felder stammen aus dem alten Regelwerk)
- Spielprotokoll aus `forge_event`

**Später:** Deckauswahl, mehr als 2 Spieler, Formate.

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

**Partie ohne Fenster testen:** ein temporäres SceneTree-Skript (z. B. `godot/_probe.gd`, danach
wieder löschen), das `res://table.tscn` lädt, sich an `ForgeBridge.decision_requested` hängt und im
nächsten `_process` über `ForgeTable` antwortet (`hud_choose`, `pass_priority`). Nicht direkt im
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
- **Nach jeder Aufgabe committen** auf `main` (`git add . && git commit`).
- Rust-Build ohne Warnungen halten; toten Code löschen statt mit `allow` zu verstecken.
- Texte im Spiel zweisprachig: `tr!("Deutsch", "English")` / `trf!` aus `rust/src/i18n.rs`.
- Godot-Logik in Rust (gdext), kein GDScript im Spiel (nur temporäre Testskripte).
- Lizenz: `forge-bridge/` ist GPL-3.0 und läuft als getrennter Prozess; keinen Forge-Code in
  `rust/` übernehmen. `forge-bridge/forge/` nie committen.
