# Kartenabdeckung – was der Client für alle Karten können muss

> **Protokoll v3:** Die UI spricht NeoForges Entscheidungsmodell (`Decision`/`Choice`). Die Muster
> unten entsprechen diesen Varianten: M1 `Priority`, M2 `ChooseTargets`, M3 `ChooseCards`,
> M4 `Scry`/`OrderTriggers`, M5 `PayMana`, M6 `DeclareAttackers`, M7 `DeclareBlockers`,
> M8 `AssignCombatDamage`, M9 `ChooseModes`/`ChooseType`/`ChooseOption`, M10 `Confirm`/
> `OptionalTrigger`/`Mulligan`, M11 `ChooseX`/`ChooseNumber`, M12 `ChooseColor`/`NameCard`.
> Planechase, Contraptions, Ante u. Ä. sind laut NeoForge-Umfang ausgeschlossen (keine UI dafür).

Forge kennt die Regeln jeder Karte. Damit **jede** Karte spielbar ist, muss der Client zwei Dinge
können:

1. **Jede Entscheidung** annehmen, die Forge dem Spieler stellt. Das sind die Methoden von
   `forge.game.player.PlayerController`, in Forge 2.0.15 insgesamt 113.
2. **Jeden Teil des Spielzustands** anzeigen, den diese Entscheidungen voraussetzen (Stack,
   Exil, Marken, angelegte Karten …).

Die gute Nachricht: Die 113 Methoden lassen sich auf **13 Bedienmuster** zurückführen. Baut der
Client diese Muster einmal allgemein, deckt er praktisch alle Karten ab. Eine eigene UI pro Karte
oder Mechanik ist nicht nötig.

---

## 1. Entscheidungen → Bedienmuster

Jede Zeile wird in der Brücke als eine `request`-Art umgesetzt (`RemotePlayerController`
überschreibt die Methode und ruft `ForgeBridge.request(kind, …)`), in Rust als ein Muster.

| # | Bedienmuster in 3D | Forge-Methoden (Auswahl) | Stand |
|---|---|---|---|
| M1 | **Aktion wählen**: spielbare Karten leuchten, Karte ziehen oder anklicken; mehrere Fähigkeiten → Fähigkeiten-Menü an der Karte | `chooseSpellAbilityToPlay`, `getAbilityToPlay`, `chooseSaToActivateFromOpeningHand`, `playChosenSpellAbility`, `playSpellAbilityNoStack`, `playTrigger`, `playSaFromPlayEffect` | teilweise (nur aus der Hand) |
| M2 | **Ziel(e) wählen**: gültige Ziele leuchten (Karten, Spieler-Medaillons, Stack-Einträge), Pfeil von der Quelle zum Ziel, Min/Max-Anzeige, „Fertig“ | `chooseTargetsFor`, `chooseNewTargetsFor`, `chooseTarget`, `chooseSingleEntityForEffect`, `chooseEntitiesForEffect`, `chooseSingleSpellForEffect`, `chooseSpellAbilitiesForEffect` | **teilweise:** `chooseTargetsFor` beim Spielen (Liste im Auswahlfenster, Optionen mit `card_id`/`player`); Klick auf dem Tisch und Pfeile fehlen noch |
| M3 | **Karten aus einer Menge wählen**: liegen die Karten sichtbar auf dem Tisch, direkt anklicken; sonst (Bibliothek, Hand des Gegners, versteckte Zonen) ein **Kartenfächer** als Overlay mit Mehrfachauswahl und Min/Max | `chooseCardsForEffect`, `chooseCardsForEffectMultiple`, `chooseCardsForZoneChange`, `chooseSingleCardForZoneChange`, `chooseCardsToDiscardFrom`, `chooseCardsToDiscardToMaximumHandSize`, `chooseCardsToDiscardUnlessType`, `chooseCardsToRevealFromHand`, `choosePermanentsToSacrifice`, `choosePermanentsToDestroy`, `chooseCardsForCost`, `chooseCardsToDelve`, `chooseCardsForConvokeOrImprovise`, `chooseCardsForSplice`, `chooseStartingHand` | offen |
| M4 | **Reihenfolge festlegen**: Kartenfächer, Karten per Drag umsortieren; Scry/Surveil mit zwei Ablagen („oben“ / „unten“ bzw. „Friedhof“) | `arrangeForScry`, `arrangeForSurveil`, `orderMoveToZoneList`, `orderBlockers`, `orderBlocker`, `orderAttackers`, `orderSimultaneousSa`, `orderAndPlaySimultaneousSa`, `orderCosts`, `willPutCardOnTop` | offen |
| M5 | **Mana bezahlen**: offene Kosten als Symbole über der Karte, Länder/Manaquellen leuchten, antippen bezahlt; Pool-Mana anklicken; „Automatisch bezahlen“; „Abbrechen“ | `payManaCost`, `chooseManaFromPool`, `applyManaToCost`, `specifyManaCombo`, `confirmPayment`, `payCostToPreventEffect`, `payCombatCost`, `payCostDuringRoll`, `choosePlayerToAssistPayment`, `helpPayForAssistSpell` | offen (Forge-KI bezahlt) |
| M6 | **Angreifer erklären**: Kreaturen anklicken, Pfeil zum Angriffsziel (Spieler, Planeswalker, Schlacht); Erschöpfen/Rekrutieren als Zusatzoption an der Karte | `declareAttackers`, `exertAttackers`, `enlistAttackers` | offen |
| M7 | **Blocker erklären**: Pfeil vom Blocker auf den Angreifer, mehrere Blocker pro Angreifer, Mehrfachblock | `declareBlockers` | offen |
| M8 | **Schaden verteilen**: Zahlen-Schieber je Empfänger, Pflichtmenge (tödlicher Schaden) vorbelegt | `assignCombatDamage`, `divideShield` | offen |
| M9 | **Option wählen** (Liste/Knöpfe im Auswahlfenster): Modi, Ersatzeffekte, statische Fähigkeiten, Kartenseite, Marken-Typ, Schlüsselwort, Kreaturtyp, Schutz-Art, Zufalls- und Würfelentscheidungen | `chooseModeForAbility`, `chooseSingleReplacementEffect`, `chooseSingleStaticAbility`, `chooseSingleCardFace`, `chooseSingleCardState`, `chooseCounterType`, `chooseKeywordForPump`, `chooseSomeType`, `chooseProtectionType`, `chooseSector`, `chooseSprocket`, `chooseFlipResult`, `chooseDiceToReroll`, `chooseRollToIgnore`, `chooseRollToModify`, `chooseRollToSwap`, `chooseRollSwapValue`, `choosePDRollToIgnore`, `chooseContraptionsToCrank`, `chooseCardsPile`, `vote`, `chooseStartingPlayer` | teilweise (nur Mulligan) |
| M10 | **Ja/Nein bestätigen**: Fenster mit Kartenvorschau der Quelle | `confirmAction`, `confirmTrigger`, `confirmReplacementEffect`, `confirmStaticApplication`, `confirmBidAction`, `chooseBinary`, `mulliganKeepHand` | **fertig** außer `confirmBidAction`; `willPutCardOnTop`, `chooseFlipResult` ebenfalls fertig |
| M11 | **Zahl wählen**: Zahlenrad bzw. Schieber mit Min/Max (X-Kosten, „beliebig viele“) | `announceRequirements`, `chooseNumber`, `chooseNumberForCostReduction`, `chooseNumberForKeywordCost`, `chooseOptionalCosts` (Mehrfachauswahl) | **fertig**; X-Kosten beim Zaubern legt aber noch die KI fest (kommt mit M5) |
| M12 | **Farbe / Kartenname wählen**: fünf Farbknöpfe (+ farblos); Kartenname per Suchfeld mit Vorschlägen aus Forges Kartendatenbank | `chooseColor`, `chooseColors`, `chooseColorAllowColorless`, `chooseCardName` | **fertig** (Kartenname: Liste bei ≤ 60 erlaubten Karten, sonst Texteingabe) |
| M13 | **Information zeigen** (keine Antwort nötig, nur „OK“): aufgedeckte Karten als Fächer, Werte, Würfelergebnis | `reveal`, `revealAnte`, `revealAISkipCards`, `revealUnsupported`, `notifyOfValue` | offen |

Kein eigenes Muster brauchen: `tuckCardsViaMulligan` (= M3), `sideboard` und
`chooseCardsYouWonToAddToDeck` (Deck-Editor außerhalb der Partie), `autoPassCancel`,
`awaitNextInput`, `cancelAwaitNextInput` (interne Steuerung der Brücke).

**Rückfall:** Solange ein Muster fehlt, entscheidet die Forge-KI (`PlayerControllerAi`). Das ist
der aktuelle Zustand für M2–M8 und M13; M1 ist teilweise, M9–M12 sind umgesetzt (Anfrage
`choice`, siehe `docs/Forge-Schnittstelle.md`). Mehrfachwahl desselben Modus (z. B. „Wähle drei,
derselbe Modus darf mehrfach gewählt werden“) ist noch nicht möglich.

---

## 2. Spielzustand, den der Client zeigen muss

| Bereich | Was | Darstellung in 3D |
|---|---|---|
| Zonen | Bibliothek, Hand, Spielfeld, Friedhof, **Exil**, **Stack**, **Kommandozone** (Embleme, Commander, Dungeons, Ringe), Sideboard (nur Deck-Editor) | eigene `Zone` je Bereich; Friedhof/Exil aufklappbar als Fächer; Stack groß in der Bildmitte |
| Karte | Name, Manakosten, Typzeile, **Regeltext**, Stärke/Widerstand (aktuell vs. aufgedruckt), Loyalität, Verteidigung (Schlachten), Farbe | Kartenbild aus Forge-Daten; geänderte Werte farbig |
| Status | getappt, **verdeckt** (Morph/Manifest), **umgedreht/transformiert** (doppelseitig, Flip), **phasiert**, **Spielsteine**, **Kopien**, Beschwörungskrankheit, angreifend/blockend | Drehen, Rückseite, Umdrehen-Animation, halbtransparent, Spielstein-Rahmen |
| Marken | +1/+1, −1/−1, Loyalität, Gift, Energie, Erfahrung, Zeit, Lore … (auf Karten **und** Spielern) | Plaketten an der Karte bzw. am Medaillon |
| Verbindungen | Auren/Ausrüstung **angelegt** an, Ziele von Stack-Einträgen, „exiliert mit“, verbundene Karten, Kontrollwechsel | Karten überlappend unter dem Träger; Pfeile für Ziele |
| Spieler | Leben, Gift, Manapool, Handgröße, Bibliotheksgröße, Monarch/Initiative/Stadtsegen, Tag/Nacht | Medaillon, Plaketten, Mana-Symbole |
| Spielfluss | Phase/Schritt, aktiver Spieler, Priorität, Kampf (wer greift wen an, wer blockt wen), Spielprotokoll | Zeitstrahl, Kampfpfeile, Protokoll-Leiste |

Dafür muss `StateJson` (Brücke) erweitert werden: Stack, Kommandozone, Regeltext, Marken,
`attached_to`, Kampfzuordnung, Manapool, Spielerzähler, Vorder-/Rückseite. Jede Erweiterung
erhöht ggf. die Protokollversion und wird in `docs/Forge-Schnittstelle.md` beschrieben.

---

## 3. Was der 3D-Client technisch können muss

| Fähigkeit | Wofür | Heute |
|---|---|---|
| **Kartenbild aus Forge-Daten** mit Regeltext, Schriftgrößen-Anpassung bei langem Text, Symbol-Ersetzung (`{T}`, `{2}{R}` → eigene Symbole) | jede Karte lesbar | nur Name/Typ/Kosten/Werte |
| **Auswahlmodus** (Zustandsautomat im `ForgeTable`): genau eine offene Anfrage bestimmt, was klickbar ist, was leuchtet und was Buttons tun | alle Muster M1–M13 | Priorität, Mulligan, `choice` (Quellkarte golden markiert) |
| **Leuchten mit Bedeutung**: spielbar, gültiges Ziel, ausgewählt, Angreifer, Blocker (verschiedene Farben) | M1–M7 | `set_ready_glow`, `set_marker` vorhanden |
| **Pfeile** (neu bauen): Quelle → Ziel, Angreifer → Ziel, Blocker → Angreifer, Stack-Eintrag → Ziele | M2, M6, M7, Stack | gelöscht, wird neu gebaut |
| **Kartenfächer-Overlay** (Karten groß in Bildschirmmitte, scrollbar, Mehrfachauswahl, Umsortieren per Drag, Min/Max-Zähler, „Fertig“) | M3, M4, M13, Friedhof/Exil ansehen | fehlt |
| **Auswahlfenster** für Listen, Ja/Nein, Zahlen, Farben, Kartennamen mit Suche | M9–M12 | **fertig** (`ChoicePanel`) |
| **Kosten-Anzeige** über der Karte, die bezahlt wird, plus Manapool zum Anklicken | M5 | Manapool-Anzeige vorhanden |
| **Stack-Anzeige** mit Quelle, Text und Zielen | Antworten, Trigger | fehlt |
| **Kartenverwandlungen**: umdrehen (Rückseite/Transform), verdeckt, Spielstein erscheinen/verschwinden, Kopie | doppelseitige Karten, Morph, Token | fehlt |
| **Angelegte Karten** unter dem Träger, Marken-Plaketten | Auren, Ausrüstung, Counter | Darstellung teilweise im alten Code, für Forge neu anbinden |
| **Variable Zonen** (Exil, Kommandozone, beliebig viele Spielsteine, Platzmangel → Stapeln gleicher Karten mit Anzahl) | große Spielfelder, Token-Decks | Grundlayout vorhanden |
| **Spielprotokoll** aus `forge_event` | nachvollziehen, was passiert | fehlt |
| **Rücknahme/Abbruch**: Anfrage abbrechen, solange Forge das erlaubt (Zauber zurück auf die Hand) | Fehlklicks | fehlt |
| **Zeitlimits/Autopass-Einstellungen** (z. B. bis Ende des Zuges passen) | Spielfluss | fehlt |

---

## 4. Empfohlene Reihenfolge

1. ~~Auswahlmodus-Gerüst im `ForgeTable` + allgemeines Auswahlfenster (M9, M10, M11, M12)~~ – erledigt
2. Ziele (M2) inkl. neuer Pfeile, Stack-Anzeige mit Zielen
3. Angreifer/Blocker (M6, M7), Schaden verteilen (M8)
4. Mana bezahlen (M5), aktivierte Fähigkeiten auf dem Spielfeld (M1 vollständig)
5. Kartenfächer (M3, M4, M13), Friedhof/Exil ansehen
6. Zustand vervollständigen: Regeltext, Marken, angelegte Karten, doppelseitige Karten, Kommandozone, Protokoll

**Messen statt raten:** Forge enthält Testpuzzles (`res/puzzle/`). Ein Testlauf, der alle Decks aus
`res/` gegen die KI spielt und protokolliert, wie oft der KI-Rückfall pro Methode greift, zeigt,
welches Muster als Nächstes am meisten bringt.
