//! `ForgeTable`: spielt eine Forge-Partie auf dem bestehenden 3D-Tisch.
//!
//! - Baut aus Forges Spielzustand (`state`) echte [`Card3D`]-Karten in den Zonen der beiden
//!   [`PlayerSeat`]s auf und hält sie synchron (Forge-Karten-ID → Karte).
//! - Zeigt die Bibliothek als verdeckten Stapel in der Deck-Zone; gezogene Karten gleiten von
//!   dort auf die Hand.
//! - Hand → Tisch per Drag-and-Drop ([`TableController`]-Signal `card_dropped`) spielt die
//!   Karte über Forge; spielbare Karten leuchten.
//! - Liefert dem [`TurnHud`](crate::game::hud::TurnHud) seine Anzeige ([`HudView`]).
//!
//! Sitzplatz 0 (Forge) = Spieler unten (ferngesteuert), Sitzplatz 1 = Gegner oben (Forge-KI).

use std::collections::{HashMap, HashSet};

use godot::prelude::*;

use super::bridge::ForgeBridge;
use crate::game::cards::{CardDef, CardKind};
use crate::game::choice_panel::{ChoiceMode, ChoiceOption, HudChoice};
use crate::game::card_fan::{FanMode, HudFan};
use crate::game::decision_view;
use crate::game::hud::HudView;
use crate::rules::decision::{CardChoiceReason, PayAction};
use crate::rules::{Action, CardId, Choice, Decision, Request, RulesEngine, Target};
use crate::game::mana::{Cost, ManaType};
use crate::game::phases::Phase;
use crate::game::seat::PlayerSeat;
use crate::table::card3d::{Card3D, CardMarker, SELECT_GLOW};
use crate::table::target_arrows::{ArrowSpec, TargetArrows};
use crate::table::placeholder;
use crate::table::table_controller::TableController;
use crate::table::zone::Zone;

/// Forge-Sitzplatz des Spielers (unten).
const LOCAL_SEAT: i64 = 0;

/// Spielt eine Partie mit der Forge-Regel-Engine auf dem 3D-Tisch: Karten, Zonen, Kamera,
/// Drag-and-Drop und HUD sind dieselben wie im normalen Spiel, nur die Regeln kommen von Forge
/// (über [ForgeBridge]).
///
/// Forge-Sitzplatz 0 ist der Spieler (unten, von dir gesteuert), Sitzplatz 1 der Gegner
/// (oben, Forge-KI). Länder liegen in der Ressourcen-Reihe, alle anderen bleibenden Karten auf dem
/// Spielfeld, der Friedhof im Ablagestapel; die Gegnerhand wird verdeckt angezeigt.
///
/// Bedienung: Hat der Spieler Priorität, leuchten alle Handkarten, die er jetzt spielen kann.
/// Eine solche Karte aus der Hand auf den Tisch ziehen = spielen (Mana zahlt Forge automatisch).
/// Hauptbutton / Leertaste = passen. Mulligan und alle allgemeinen Entscheidungen (Optionen,
/// Modi, Ja/Nein, Zahlen, Farben, Kartennamen) im Auswahlfenster des HUD; die Karte, um die es
/// geht, wird dabei golden markiert. Ziele, Angreifer, Blocker und Manabezahlung wählt vorerst
/// die Forge-KI für dich.
#[derive(GodotClass)]
#[class(base=Node)]
pub struct ForgeTable {
    /// Verbindung zu Forge.
    #[export]
    bridge: Option<Gd<ForgeBridge>>,
    /// Tischsteuerung (Drag-and-Drop, Kamera). Sie kennt keine Regeln; dieser Knoten prüft
    /// die Züge über Forge.
    #[export]
    controller: Option<Gd<TableController>>,
    /// Sitzplatz des Spielers (unten) = Forge-Sitzplatz 0.
    #[export]
    player_seat: Option<Gd<PlayerSeat>>,
    /// Sitzplatz des Gegners (oben) = Forge-Sitzplatz 1.
    #[export]
    opponent_seat: Option<Gd<PlayerSeat>>,
    /// Deck des Spielers: Forge-`.dck`-Datei (`res://` erlaubt) oder Forge-Deckname.
    #[export]
    player_deck: GString,
    /// Deck des Gegners: Forge-`.dck`-Datei (`res://` erlaubt) oder Forge-Deckname.
    #[export]
    opponent_deck: GString,
    /// Zufallsstartwert der Partie (< 0 = zufällig).
    #[export]
    seed: i64,
    /// Brücke und Partie automatisch beim Start der Szene starten.
    #[export]
    autostart: bool,
    /// Mindestabstand (Sekunden) zwischen zwei Zustandsabfragen, während Forge Ereignisse sendet.
    #[export(range = (0.05, 2.0))]
    refresh_interval: f64,

    /// Forge-Karten-ID → Karte auf dem Tisch. Verdeckte Gegner-Handkarten: negative Schlüssel.
    cards: HashMap<i64, Gd<Card3D>>,
    /// Verdeckte Karten im Deck je Sitzplatz (Anzahl = Forges Bibliothek). Wird eine Karte
    /// gezogen, wandert die oberste davon auf die Hand.
    deck_cards: [Vec<Gd<Card3D>>; 2],
    /// Zuletzt angewandter Kartenstand je ID (zum Erkennen geänderter Werte).
    signatures: HashMap<i64, String>,
    /// Offene Entscheidung des Spielers (NeoForge-`Decision` mit Anzeigehilfen).
    pending: Option<Request>,
    /// Karte, um die es in der offenen Auswahl geht (golden markiert).
    marked: Option<Gd<Card3D>>,
    /// Aktuelle Auswahl im Auswahlfenster (Indizes; vom HUD jedes Frame gemeldet).
    selection: Vec<usize>,
    /// Als Ziel gewählte (golden markierte) Karten – zum Zurücksetzen der Markierung.
    selected_cards: Vec<Gd<Card3D>>,
    /// Menü nach Klick auf eine Karte mit mehreren Möglichkeiten (nur UI, keine
    /// Engine-Entscheidung): fertige Antworten (`Action` bzw. `Pay`) und ihre Texte.
    menu: Option<(Vec<Choice>, Vec<String>)>,
    /// Pfeil-Ebene über dem Tisch (Zielwahl, Kampf).
    arrows: Option<Gd<TargetArrows>>,
    /// Angriffsplan bei `DeclareAttackers`: (Angreifer, Ziel) in Klick-Reihenfolge.
    attack_plan: Vec<(CardId, Target)>,
    /// Blockplan bei `DeclareBlockers`: (Blocker, Angreifer).
    block_plan: Vec<(CardId, CardId)>,
    /// Zuletzt gewählte Kreatur im Kampf: Angreifer, dessen Ziel ein Klick auf ein anderes
    /// Angriffsziel ändert, bzw. Blocker, der auf seinen Angreifer wartet.
    combat_armed: Option<CardId>,
    /// Kartenbilder für den Kartenfächer der offenen Anfrage: Karten vom Tisch oder (für
    /// verdeckte Zonen) temporäre Karten aus den mitgeschickten Kartendaten. Temporäre Karten
    /// (zweiter Wert true) werden nach der Antwort freigegeben.
    fan_cards: Vec<(CardId, Gd<Card3D>, bool)>,
    /// Zuletzt empfangener Spielzustand.
    state: Option<VarDictionary>,
    /// Seit der letzten Zustandsabfrage kamen Ereignisse.
    dirty: bool,
    since_refresh: f64,
    winner: String,
    match_running: bool,

    base: Base<Node>,
}

#[godot_api]
impl INode for ForgeTable {
    fn init(base: Base<Node>) -> Self {
        Self {
            bridge: None,
            controller: None,
            player_seat: None,
            opponent_seat: None,
            player_deck: "res://../forge-bridge/testdecks/spieler.dck".into(),
            opponent_deck: "res://../forge-bridge/testdecks/gegner.dck".into(),
            seed: -1,
            autostart: true,
            refresh_interval: 0.25,
            cards: HashMap::new(),
            deck_cards: [Vec::new(), Vec::new()],
            signatures: HashMap::new(),
            pending: None,
            marked: None,
            selection: Vec::new(),
            selected_cards: Vec::new(),
            menu: None,
            arrows: None,
            attack_plan: Vec::new(),
            block_plan: Vec::new(),
            combat_armed: None,
            fan_cards: Vec::new(),
            state: None,
            dirty: false,
            since_refresh: 0.0,
            winner: String::new(),
            match_running: false,
            base,
        }
    }

    fn ready(&mut self) {
        let this = self.to_gd();
        let cb = |name: &str| Callable::from_object_method(&this, name);
        if let Some(mut bridge) = self.bridge.clone() {
            bridge.connect("bridge_ready", &cb("on_bridge_ready"));
            bridge.connect("match_started", &cb("on_state"));
            bridge.connect("state_received", &cb("on_state"));
            bridge.connect("forge_event", &cb("on_forge_event"));
            bridge.connect("decision_requested", &cb("on_decision_requested"));
            bridge.connect("game_over", &cb("on_game_over"));
            bridge.connect("bridge_error", &cb("on_bridge_error"));
            if self.autostart && !bridge.bind().is_running() {
                bridge.bind_mut().start_bridge();
            }
        } else {
            godot_warn!("ForgeTable: keine ForgeBridge gesetzt");
        }
        if let Some(mut controller) = self.controller.clone() {
            controller.connect("card_dropped", &cb("on_card_dropped"));
            controller.connect("card_clicked", &cb("on_card_clicked"));
            // zurückgestellt: der Tisch baut in diesem Moment noch seine Kinder auf
            let arrows = TargetArrows::new_alloc();
            controller.call_deferred("add_child", &[arrows.to_variant()]);
            self.arrows = Some(arrows);
        }
    }

    fn process(&mut self, delta: f64) {
        self.update_arrows();
        self.since_refresh += delta;
        if self.dirty && self.match_running && self.since_refresh >= self.refresh_interval {
            self.dirty = false;
            self.since_refresh = 0.0;
            if let Some(mut b) = self.bridge.clone() {
                b.request_state(LOCAL_SEAT as u8);
            }
        }
    }
}

#[godot_api]
impl ForgeTable {
    /// Startet die Partie mit [member player_deck] gegen [member opponent_deck]
    /// (Brücke muss bereit sein). Gibt false zurück, wenn das nicht ging.
    #[func]
    pub fn start_match(&mut self) -> bool {
        let names = PackedStringArray::from(&[self.seat_name(0), self.seat_name(1)]);
        let decks = PackedStringArray::from(&[self.player_deck.clone(), self.opponent_deck.clone()]);
        let Some(bridge) = self.bridge.as_mut() else { return false };
        let ok = bridge.bind_mut().start_match(decks, PackedInt32Array::from(&[LOCAL_SEAT as i32]), names, self.seed);
        if ok {
            self.winner.clear();
            self.match_running = true;
        }
        ok
    }

    /// Passt (Hauptbutton / Leertaste): beantwortet eine offene `Priority`-Entscheidung mit
    /// `Action::Pass`.
    #[func]
    pub fn pass_priority(&mut self) {
        if matches!(self.pending.as_ref().map(|r| &r.decision), Some(Decision::Priority { .. })) {
            self.answer(Choice::Action(Action::Pass));
        }
    }

    /// Eine Option wurde auf dem Tisch gewählt (Klick auf ein gültiges Ziel – Karte oder
    /// Medaillon): `index` = Option im Auswahlfenster. Das HUD behandelt das wie einen Klick auf
    /// diese Option (zurückgestellt verbunden).
    #[signal]
    fn choice_option_clicked(index: i64);

    /// Eingabe aus dem Auswahlfenster des HUD für die offene Entscheidung: eine Option = Index,
    /// mehrere = Array von Indizes, Ja/Nein und Mulligan = bool (true = erste Option / behalten),
    /// Zahl = int, Abbrechen = null. Wird in die passende NeoForge-`Choice` übersetzt; unpassende
    /// Eingaben werden ignoriert. Ist das Fähigkeiten-Menü offen, wählt die Eingabe eine Aktion
    /// daraus (null = Menü schließen).
    #[func]
    pub fn submit_choice(&mut self, choice: Variant) {
        if let Some((choices, _)) = self.menu.take() {
            let picked = choice.try_to::<i64>().ok().and_then(|i| choices.get(usize::try_from(i).ok()?).cloned());
            if let Some(c) = picked {
                self.answer(c);
            }
            return;
        }
        let Some(req) = self.pending.as_ref() else { return };
        // Kampf: der Knopf bestätigt den Plan, der auf dem Tisch gebaut wurde.
        // Mana: Knopf = automatisch bezahlen, Abbrechen = Wirken zurücknehmen
        match &req.decision {
            Decision::PayMana { .. } => {
                let pay = if choice.is_nil() { PayAction::Cancel } else { PayAction::AutoPay };
                self.answer(Choice::Pay(pay));
                return;
            }
            Decision::DeclareAttackers { .. } if choice.booleanize() => {
                let plan = self.attack_plan.clone();
                self.answer(Choice::Attackers(plan));
                return;
            }
            Decision::DeclareBlockers { .. } if choice.booleanize() => {
                let plan = self.block_plan.clone();
                self.answer(Choice::Blocks(plan));
                return;
            }
            _ => {}
        }
        match decision_view::to_choice(&req.decision, &choice) {
            Some(c) => self.answer(c),
            None => {
                godot_warn!("ForgeTable: Eingabe {choice} passt nicht zu {:?}", req.decision);
            }
        }
    }

    /// Ein Spieler-Medaillon wurde angeklickt (`seat` = Sitzplatz): ist der Spieler ein gültiges
    /// Ziel der offenen Zielwahl, wird er gewählt bzw. abgewählt.
    #[func]
    pub fn player_clicked(&mut self, seat: i64) {
        let Ok(seat) = u8::try_from(seat) else { return };
        if self.retarget_attacker(Target::Player(seat)) {
            return;
        }
        if let Some(i) = self.candidate_index(Target::Player(seat)) {
            self.signals().choice_option_clicked().emit(i as i64);
        }
    }

    /// Antwort aus dem Kartenfächer: `first` = gewählte Karten (`ChooseCards`, `MulliganBottom`)
    /// bzw. Karten oben auf der Bibliothek (`Scry`, von oben nach unten), `second` = Karten unter die
    /// Bibliothek bzw. in den Friedhof.
    #[func]
    pub fn submit_fan(&mut self, first: PackedInt64Array, second: PackedInt64Array) {
        let ids = |a: &PackedInt64Array| a.as_slice().iter().filter_map(|i| u32::try_from(*i).ok()).collect::<Vec<_>>();
        let choice = match self.pending.as_ref().map(|r| &r.decision) {
            Some(Decision::ChooseCards { .. }) | Some(Decision::MulliganBottom { .. }) => Choice::Cards(ids(&first)),
            Some(Decision::Scry { .. }) => Choice::Arrange { top: ids(&first), bottom: ids(&second) },
            _ => return,
        };
        self.answer(choice);
    }

    /// Aktuelle Auswahl im Auswahlfenster (vom HUD jedes Frame gemeldet): gewählte Ziele werden
    /// auf dem Tisch golden markiert und bekommen einen Pfeil.
    #[func]
    pub fn set_choice_selection(&mut self, selection: PackedInt64Array) {
        let sel: Vec<usize> = selection.as_slice().iter().filter_map(|i| usize::try_from(*i).ok()).collect();
        if sel != self.selection {
            self.selection = sel;
            self.update_glow();
        }
    }

    /// Karte auf dem Tisch zur Forge-Karten-ID (null, wenn nicht sichtbar).
    #[func]
    pub fn get_card(&self, forge_id: i64) -> Option<Gd<Card3D>> {
        self.cards.get(&forge_id).cloned()
    }

    /// Zuletzt von Forge empfangener Spielzustand (Aufbau siehe `docs/Forge-Schnittstelle.md`).
    #[func]
    pub fn get_state(&self) -> VarDictionary {
        self.state.clone().unwrap_or_default()
    }

    /// Hat der Spieler gerade eine offene Entscheidung? Liefert den Namen der NeoForge-`Decision`
    /// (z. B. "Priority", "Mulligan", "ChooseTargets", "ChooseOption") oder einen leeren Text.
    #[func]
    pub fn get_pending_kind(&self) -> GString {
        GString::from(self.pending_kind().unwrap_or_default().as_str())
    }

    /// Brücke bereit → Partie starten (bei [member autostart]).
    #[func]
    fn on_bridge_ready(&mut self, _protocol: i64) {
        if self.autostart {
            self.start_match();
        }
    }

    /// Neuer Spielzustand von Forge (Spielstart oder Abfrage): Tisch abgleichen.
    #[func]
    fn on_state(&mut self, state: VarDictionary) {
        self.apply_state(state);
    }

    /// Forge-Ereignis: Zustand bald neu abfragen (gebündelt, siehe [member refresh_interval]).
    #[func]
    fn on_forge_event(&mut self, _event: VarDictionary) {
        self.dirty = true;
    }

    /// Entscheidung des Spielers nötig (Protokoll v3): Zustand übernehmen, Quellkarte markieren,
    /// spielbare Karten leuchten lassen. Unbekannte Entscheidungen werden mit `Cancel`
    /// beantwortet (die Engine entscheidet dann selbst).
    #[func]
    fn on_decision_requested(&mut self, request: VarDictionary) {
        if let Some(state) = request.get("state").and_then(|v| v.try_to::<VarDictionary>().ok()) {
            self.apply_state(state);
        }
        let Some(req) = Request::from_var(&request) else {
            godot_warn!("ForgeTable: unbekannte Entscheidung {}", request.get("decision").unwrap_or_default());
            let id = get_int(&request, "id");
            if let Some(mut b) = self.bridge.clone() {
                b.submit(id, &Choice::Cancel);
            }
            return;
        };
        // Karte, um die es geht, golden markieren
        let source = req.source.and_then(|id| self.cards.get(&(id as i64)).cloned());
        self.set_marked(source);
        self.prepare_fan(&req);
        self.attack_plan.clear();
        self.block_plan.clear();
        self.combat_armed = None;
        self.pending = Some(req);
        self.update_glow();
    }

    /// Partie vorbei: Sieger merken (HUD zeigt ihn an).
    #[func]
    fn on_game_over(&mut self, result: VarDictionary) {
        self.match_running = false;
        self.pending = None;
        self.update_glow();
        self.winner = result.get("winner").filter(|v| !v.is_nil()).map(|v| v.to_string()).unwrap_or_else(|| {
            tr!("Niemand", "Nobody").to_string()
        });
        if let Some(b) = self.bridge.as_mut() {
            b.bind_mut().request_state(LOCAL_SEAT);
        }
    }

    /// Fehler der Brücke: als Warnung in die Godot-Konsole schreiben.
    #[func]
    fn on_bridge_error(&mut self, code: GString, message: GString) {
        godot_warn!("ForgeTable: Forge-Fehler {code}: {message}");
    }

    /// Karte angeklickt: bei der Zielwahl = Ziel wählen/abwählen; mit Priorität = Aktion der
    /// Karte (nicht aus der Hand – die spielt man durch Ziehen): eine Aktion sofort, mehrere über
    /// das Fähigkeiten-Menü.
    #[func]
    fn on_card_clicked(&mut self, card: Gd<Card3D>) {
        let Some(id) = self.forge_id(&card) else { return };
        let Ok(card_id) = u32::try_from(id) else { return };
        if self.combat_click(card_id) {
            self.update_glow();
            return;
        }
        if let Some(i) = self.candidate_index(Target::Card(card_id)) {
            self.signals().choice_option_clicked().emit(i as i64);
            return;
        }
        let in_hand = self.zones(0).hand.is_some_and(|h| card.bind().get_zone().as_ref() == Some(&h));
        let Some(req) = self.pending.as_ref() else { return };
        let mine: Vec<(Choice, String)> = match &req.decision {
            // Mana bezahlen: Manafähigkeiten dieser Karte
            Decision::PayMana { options, .. } => options
                .iter()
                .enumerate()
                .filter(|(_, o)| matches!(o, PayAction::ActivateMana(c, _) if *c == card_id))
                .map(|(i, o)| (Choice::Pay(*o), req.label(i)))
                .collect(),
            // Priorität: Aktionen dieser Karte (nicht aus der Hand – die spielt man durch Ziehen)
            Decision::Priority { actions, .. } if !in_hand => actions
                .iter()
                .enumerate()
                .filter(|(_, a)| a.card() == Some(card_id))
                .map(|(i, a)| (Choice::Action(*a), req.label(i)))
                .collect(),
            _ => return,
        };
        match mine.len() {
            0 => {}
            1 => self.answer(mine[0].0.clone()),
            _ => self.menu = Some(mine.into_iter().unzip()),
        }
    }

    /// Karte wurde auf dem Tisch abgelegt: Hat sie eine Aktion in der offenen `Priority`-
    /// Entscheidung (aus der Hand: Land spielen / wirken; auf dem Spielfeld: Fähigkeit
    /// aktivieren), wird diese gewählt. Aus der Hand zurück auf die Hand = abbrechen. Alles andere
    /// wird auf den Engine-Stand zurückgesetzt.
    #[func]
    fn on_card_dropped(&mut self, card: Gd<Card3D>, from_zone: Gd<Zone>, to_zone: Gd<Zone>) {
        let hand = self.zones(0).hand;
        let back_to_hand = hand.as_ref() == Some(&from_zone) && hand.as_ref() == Some(&to_zone);
        let id = self.forge_id(&card);
        if !back_to_hand {
            if let Some(action) = id.and_then(|id| self.action_for_card(id)) {
                self.answer(Choice::Action(action));
                return;
            }
        }
        // Kein gültiger Zug: Karte zurück an ihren Forge-Platz
        if let Some(state) = self.state.clone() {
            self.apply_state(state);
        }
    }
}

/// Zonen eines Sitzplatzes.
struct SeatZones {
    hand: Option<Gd<Zone>>,
    deck: Option<Gd<Zone>>,
    battlefield: Option<Gd<Zone>>,
    discard: Option<Gd<Zone>>,
    resources: Option<Gd<Zone>>,
}

impl ForgeTable {
    /// Anzeige für das HUD.
    pub fn hud_view(&self) -> HudView {
        let st = self.state.as_ref();
        let players = st.and_then(|s| get_array(s, "players")).unwrap_or_default();
        let player = |seat: usize| players.get(seat).and_then(|v| v.try_to::<VarDictionary>().ok());
        let life = |seat: usize| player(seat).map_or(0, |p| get_int(&p, "life"));
        let phase = st.map(|s| get_str(s, "phase")).unwrap_or_default();
        let my_priority = matches!(self.pending.as_ref().map(|r| &r.decision), Some(Decision::Priority { .. }));

        let menu_choice = self.menu.as_ref().zip(self.pending.as_ref()).map(|((_, labels), req)| HudChoice {
            key: -req.id,
            title: if matches!(req.decision, Decision::PayMana { .. }) {
                tr!("Manafähigkeit wählen", "Choose a mana ability").into()
            } else {
                tr!("Fähigkeit wählen", "Choose an ability").into()
            },
            detail: String::new(),
            mode: ChoiceMode::Single,
            options: labels.iter().map(|l| ChoiceOption { text: l.clone(), color: None }).collect(),
            cancelable: true,
            side: false,
            numbered: false,
        });
        let combat_choice = self.pending.as_ref().and_then(|req| self.combat_choice(req));
        let choice = menu_choice.or(combat_choice).or_else(|| self.pending.as_ref().and_then(|req| {
            let name_of = |i: usize| match &req.decision {
                Decision::ChooseTargets { candidates, .. } => match candidates.get(i) {
                    Some(Target::Player(p)) => self.seat_name(*p as usize).to_string(),
                    Some(Target::Card(c)) => self
                        .cards
                        .get(&(*c as i64))
                        .map(|card| card.bind().card_name().to_string())
                        .unwrap_or_else(|| format!("#{c}")),
                    None => String::new(),
                },
                _ => String::new(),
            };
            decision_view::hud_choice(req, &name_of)
        }));
        let seat_state = |selected_only: bool| -> [bool; 2] {
            let mut out = [false; 2];
            match self.pending.as_ref().map(|r| &r.decision) {
                Some(Decision::ChooseTargets { candidates, .. }) => {
                    for (i, t) in candidates.iter().enumerate() {
                        if let Target::Player(p) = t {
                            if (*p as usize) < 2 && (!selected_only || self.selection.contains(&i)) {
                                out[*p as usize] = true;
                            }
                        }
                    }
                }
                // Angriff: Spieler als Ziel des zuletzt gewählten Angreifers wählbar (nur wenn es
                // mehrere Angriffsziele gibt) bzw. gewählt
                Some(Decision::DeclareAttackers { defenders, .. }) if defenders.len() > 1 => {
                    for t in defenders {
                        if let Target::Player(p) = t {
                            let chosen = self.attack_plan.iter().any(|(_, d)| d == t);
                            if (*p as usize) < 2 && (if selected_only { chosen } else { self.combat_armed.is_some() }) {
                                out[*p as usize] = true;
                            }
                        }
                    }
                }
                _ => {}
            }
            out
        };
        let button_text = if my_priority {
            tr!("Passen  »", "Pass  »")
        } else if choice.is_some() {
            tr!("Auswahl treffen …", "Make a choice …")
        } else if !self.winner.is_empty() {
            tr!("Spielende", "Game over")
        } else if self.match_running {
            tr!("Gegner ist am Zug …", "Opponent is acting …")
        } else {
            tr!("Warte auf Forge …", "Waiting for Forge …")
        };
        HudView {
            phase_titles: Phase::ORDER.iter().map(|p| GString::from(p.title())).collect(),
            phase_index: phase_index(&phase),
            active_seat: st.map_or(-1, |s| get_int(s, "active_player")),
            names: [self.seat_name(0), self.seat_name(1)],
            lives: [life(0), life(1)],
            turn: st.map_or(0, |s| get_int(s, "turn")),
            winner: self.winner.clone(),
            button_enabled: my_priority,
            button_text: button_text.into(),
            stack_busy: false,
            player_pool: player(0)
                .and_then(|p| get_array(&p, "mana_pool"))
                .map(|a| a.iter_shared().map(|v| v.try_to::<i64>().unwrap_or(0)).collect())
                .unwrap_or_default(),
            choice,
            fan: self.pending.as_ref().and_then(|req| self.hud_fan(req)),
            targetable_seats: seat_state(false),
            selected_seats: seat_state(true),
        }
    }

    fn seat_name(&self, seat: usize) -> GString {
        let s = if seat == 0 { &self.player_seat } else { &self.opponent_seat };
        s.as_ref()
            .map(|s| s.bind().display_name.clone())
            .unwrap_or_else(|| if seat == 0 { "Spieler".into() } else { "Gegner".into() })
    }

    fn zones(&self, seat: usize) -> SeatZones {
        let s = if seat == 0 { &self.player_seat } else { &self.opponent_seat };
        match s {
            Some(s) => {
                let s = s.bind();
                SeatZones {
                    hand: s.hand.clone(),
                    deck: s.deck.clone(),
                    battlefield: s.battlefield.clone(),
                    discard: s.discard.clone(),
                    resources: s.resource_zone.clone(),
                }
            }
            None => SeatZones { hand: None, deck: None, battlefield: None, discard: None, resources: None },
        }
    }

    fn pending_kind(&self) -> Option<String> {
        self.pending.as_ref().map(|r| decision_name(&r.decision).to_string())
    }

    /// Antwortet über die Engine-Schnittstelle auf die offene Entscheidung.
    fn answer(&mut self, choice: Choice) {
        let Some(req) = self.pending.take() else { return };
        self.menu = None;
        self.selection.clear();
        self.free_fan_cards();
        self.attack_plan.clear();
        self.block_plan.clear();
        self.combat_armed = None;
        self.set_marked(None);
        self.update_glow();
        if let Some(mut b) = self.bridge.clone() {
            b.submit(req.id, &choice);
        }
    }

    /// Aktion der offenen `Priority`-Entscheidung für die Karte `card_id`: Land spielen bzw.
    /// wirken zuerst, sonst die erste Aktion der Karte (z. B. Fähigkeit aktivieren).
    fn action_for_card(&self, card_id: i64) -> Option<Action> {
        let Some(Decision::Priority { actions, .. }) = self.pending.as_ref().map(|r| &r.decision) else {
            return None;
        };
        let mine = || actions.iter().copied().filter(|a| a.card().map(i64::from) == Some(card_id));
        mine().find(|a| matches!(a, Action::PlayLand(_) | Action::Cast(_))).or_else(|| mine().next())
    }

    /// Markiert `card` golden (Quelle der offenen Auswahl); hebt die alte Markierung auf.
    fn set_marked(&mut self, card: Option<Gd<Card3D>>) {
        if let Some(mut old) = self.marked.take().filter(|c| c.is_instance_valid()) {
            old.bind_mut().set_marker(CardMarker::None);
        }
        if let Some(mut c) = card.clone() {
            c.bind_mut().set_marker(CardMarker::Selected);
        }
        self.marked = card;
    }

    fn forge_id(&self, card: &Gd<Card3D>) -> Option<i64> {
        self.cards.iter().find(|(_, c)| *c == card).map(|(id, _)| *id)
    }

    /// Leuchten: mit Priorität die Karten, die eine Aktion haben; bei der Zielwahl alle gültigen
    /// Ziele (gewählte zusätzlich golden markiert).
    fn update_glow(&mut self) {
        let (glow, chosen): (HashSet<i64>, HashSet<i64>) = match self.pending.as_ref().map(|r| &r.decision) {
            Some(Decision::Priority { actions, .. }) => {
                (actions.iter().filter_map(|a| a.card()).map(i64::from).collect(), HashSet::new())
            }
            Some(Decision::ChooseTargets { candidates, .. }) => {
                let card = |t: &Target| match t {
                    Target::Card(c) => Some(i64::from(*c)),
                    Target::Player(_) => None,
                };
                (
                    candidates.iter().filter_map(card).collect(),
                    self.selection.iter().filter_map(|i| candidates.get(*i)).filter_map(card).collect(),
                )
            }
            Some(Decision::DeclareAttackers { candidates, defenders, .. }) => {
                let mut glow: HashSet<i64> = candidates.iter().map(|c| i64::from(*c)).collect();
                // Mit gewähltem Angreifer: Planeswalker/Schlachten als Ziel wählbar
                if self.combat_armed.is_some() && defenders.len() > 1 {
                    glow.extend(defenders.iter().filter_map(|t| match t {
                        Target::Card(c) => Some(i64::from(*c)),
                        Target::Player(_) => None,
                    }));
                }
                (glow, HashSet::new())
            }
            Some(Decision::PayMana { options, .. }) => (
                options
                    .iter()
                    .filter_map(|o| match o {
                        PayAction::ActivateMana(c, _) => Some(i64::from(*c)),
                        _ => None,
                    })
                    .collect(),
                HashSet::new(),
            ),
            Some(Decision::DeclareBlockers { candidates, attackers, .. }) => {
                let assigned: HashSet<CardId> = self.block_plan.iter().map(|(b, _)| *b).collect();
                let mut glow: HashSet<i64> =
                    candidates.iter().filter(|c| !assigned.contains(c)).map(|c| i64::from(*c)).collect();
                if self.combat_armed.is_some() {
                    glow.extend(attackers.iter().map(|c| i64::from(*c)));
                }
                (glow, HashSet::new())
            }
            _ => (HashSet::new(), HashSet::new()),
        };
        for (id, card) in self.cards.iter_mut() {
            card.bind_mut().set_ready_glow(glow.contains(id));
        }
        for mut old in std::mem::take(&mut self.selected_cards) {
            if old.is_instance_valid() && Some(&old) != self.marked.as_ref() {
                old.bind_mut().set_marker(CardMarker::None);
            }
        }
        // Markierungen: gewählte Ziele (gold), Angreifer (rot), Blocker (blau), gewählte Kreatur (gold)
        let mut marks: Vec<(i64, CardMarker)> = chosen.into_iter().map(|id| (id, CardMarker::Selected)).collect();
        marks.extend(self.attack_plan.iter().map(|(c, _)| (i64::from(*c), CardMarker::Attacking)));
        marks.extend(self.block_plan.iter().map(|(b, _)| (i64::from(*b), CardMarker::Blocking)));
        if let (Some(armed), Some(Decision::DeclareBlockers { .. })) = (self.combat_armed, self.pending.as_ref().map(|r| &r.decision)) {
            marks.push((i64::from(armed), CardMarker::Selected));
        }
        for (id, marker) in marks {
            if let Some(mut card) = self.cards.get(&id).cloned() {
                card.bind_mut().set_marker(marker);
                self.selected_cards.push(card);
            }
        }
    }

    /// Klick auf eine Karte während `DeclareAttackers` / `DeclareBlockers`. Gibt true zurück, wenn
    /// der Klick dort verarbeitet wurde.
    ///
    /// Angriff: Klick auf einen Kandidaten = angreifen (Ziel: erster Gegner) bzw. nicht mehr
    /// angreifen; Klick auf einen Planeswalker/eine Schlacht = neues Ziel des zuletzt gewählten
    /// Angreifers. Block: Klick auf eigene Kreatur = als Blocker wählen (bzw. ihre Blocks
    /// aufheben), danach Klick auf einen Angreifer = blocken.
    fn combat_click(&mut self, card: CardId) -> bool {
        let Some(decision) = self.pending.as_ref().map(|r| r.decision.clone()) else { return false };
        match decision {
            Decision::DeclareAttackers { candidates, defenders, .. } => {
                if candidates.contains(&card) {
                    if let Some(pos) = self.attack_plan.iter().position(|(c, _)| *c == card) {
                        self.attack_plan.remove(pos);
                        self.combat_armed = None;
                    } else {
                        let default = defenders
                            .iter()
                            .copied()
                            .find(|t| matches!(t, Target::Player(_)))
                            .or_else(|| defenders.first().copied());
                        if let Some(def) = default {
                            self.attack_plan.push((card, def));
                            self.combat_armed = Some(card);
                        }
                    }
                    return true;
                }
                self.retarget_attacker(Target::Card(card))
            }
            Decision::DeclareBlockers { candidates, attackers, .. } => {
                if candidates.contains(&card) {
                    if self.block_plan.iter().any(|(b, _)| *b == card) {
                        self.block_plan.retain(|(b, _)| *b != card);
                        self.combat_armed = None;
                    } else if self.combat_armed == Some(card) {
                        self.combat_armed = None;
                    } else {
                        self.combat_armed = Some(card);
                    }
                    return true;
                }
                if attackers.contains(&card) {
                    if let Some(blocker) = self.combat_armed.take() {
                        self.block_plan.push((blocker, card));
                    }
                    return true;
                }
                false
            }
            _ => false,
        }
    }

    /// Angriff: macht `target` (wenn es ein Angriffsziel ist) zum Ziel des zuletzt gewählten
    /// Angreifers. Gibt true zurück, wenn das passiert ist.
    fn retarget_attacker(&mut self, target: Target) -> bool {
        let Some(Decision::DeclareAttackers { defenders, .. }) = self.pending.as_ref().map(|r| &r.decision) else {
            return false;
        };
        if !defenders.contains(&target) {
            return false;
        }
        let Some(armed) = self.combat_armed else { return false };
        if let Some(entry) = self.attack_plan.iter_mut().find(|(c, _)| *c == armed) {
            entry.1 = target;
            return true;
        }
        false
    }

    /// Auswahlfenster für den Kampf: Erklärung und Bestätigungsknopf (die Auswahl passiert auf
    /// dem Tisch).
    fn combat_choice(&self, req: &Request) -> Option<HudChoice> {
        let (title, detail, label) = match &req.decision {
            Decision::DeclareAttackers { defenders, .. } => {
                let n = self.attack_plan.len();
                let retarget = if defenders.len() > 1 {
                    tr!(" Danach Klick auf ein anderes Ziel (Spieler, Planeswalker) = dieser Angreifer greift es an.", " Then click another target (player, planeswalker) to attack it instead.")
                } else {
                    ""
                };
                (
                    tr!("Angreifer wählen", "Declare attackers").to_string(),
                    format!(
                        "{}{}",
                        tr!("Klick auf eine leuchtende Kreatur = angreifen, erneut = nicht angreifen.", "Click a glowing creature to attack, again to cancel."),
                        retarget
                    ),
                    if n == 0 { tr!("Nicht angreifen", "No attack").to_string() } else { trf!("Angreifen ({n})", "Attack ({n})") },
                )
            }
            Decision::PayMana { remaining, .. } => {
                return Some(HudChoice {
                    key: req.id,
                    title: trf!("Mana bezahlen: {remaining}", "Pay mana: {remaining}"),
                    detail: tr!(
                        "Klick auf ein leuchtendes Land bzw. eine Manaquelle = Mana erzeugen und einsetzen.",
                        "Click a glowing land or mana source to tap it for mana."
                    )
                    .into(),
                    mode: ChoiceMode::Done { label: tr!("Automatisch bezahlen", "Auto pay").into() },
                    options: Vec::new(),
                    cancelable: true,
                    side: true,
                    numbered: false,
                });
            }
            Decision::DeclareBlockers { .. } => {
                let n = self.block_plan.len();
                (
                    tr!("Blocker wählen", "Declare blockers").to_string(),
                    tr!(
                        "Klick auf eigene Kreatur, dann auf einen Angreifer = blocken. Klick auf einen Blocker = Block aufheben.",
                        "Click your creature, then an attacker to block. Click a blocker to undo."
                    )
                    .to_string(),
                    if n == 0 { tr!("Nicht blocken", "No blocks").to_string() } else { trf!("Blocken ({n})", "Block ({n})") },
                )
            }
            _ => return None,
        };
        // Fragetext der Engine (z. B. Hinweis nach ungültiger Erklärung) hat Vorrang
        let title = if req.prompt.is_empty() || req.prompt.starts_with("Angreifer wählen") || req.prompt.starts_with("Blocker wählen") {
            title
        } else {
            req.prompt.clone()
        };
        Some(HudChoice { key: req.id, title, detail, mode: ChoiceMode::Done { label }, options: Vec::new(), cancelable: false, side: true, numbered: false })
    }

    /// Karten-IDs, die der Kartenfächer für `decision` zeigt (leer = kein Fächer).
    fn fan_ids(decision: &Decision) -> Vec<CardId> {
        match decision {
            Decision::ChooseCards { options, .. } => options.clone(),
            Decision::MulliganBottom { hand, .. } => hand.clone(),
            Decision::Scry { cards, .. } => cards.clone(),
            _ => Vec::new(),
        }
    }

    /// Stellt die Kartenbilder für den Kartenfächer bereit (Tischkarte oder temporäre Karte aus
    /// den mitgeschickten Kartendaten).
    fn prepare_fan(&mut self, req: &Request) {
        self.free_fan_cards();
        for id in Self::fan_ids(&req.decision) {
            if let Some(card) = self.cards.get(&i64::from(id)).cloned() {
                self.fan_cards.push((id, card, false));
                continue;
            }
            let data = req.cards.iter().find(|c| get_int(c, "id") == i64::from(id));
            let def = data.map(def_from_forge).unwrap_or_else(|| CardDef::base("?", CardKind::Creature, Cost::default()));
            let mut card = Card3D::new_alloc();
            {
                let mut c = card.bind_mut();
                c.set_face_texture(Some(placeholder::card_face(&def)));
                c.apply_def(&def);
            }
            self.fan_cards.push((id, card, true));
        }
    }

    fn free_fan_cards(&mut self) {
        for (_, card, temporary) in self.fan_cards.drain(..) {
            if temporary && card.is_instance_valid() {
                card.free();
            }
        }
    }

    /// Inhalt des Kartenfächers für die offene Anfrage.
    fn hud_fan(&self, req: &Request) -> Option<HudFan> {
        if self.fan_cards.is_empty() {
            return None;
        }
        let cards = self.fan_cards.iter().map(|(id, c, _)| (*id, c.clone())).collect();
        let (title, mode) = match &req.decision {
            Decision::ChooseCards { reason, min, max, .. } => {
                let fallback = match reason {
                    CardChoiceReason::Discard | CardChoiceReason::DiscardCost => tr!("Karten abwerfen", "Discard cards"),
                    CardChoiceReason::DiscardToHandSize => tr!("Auf Handkartenlimit abwerfen", "Discard to hand size"),
                    CardChoiceReason::Sacrifice | CardChoiceReason::SacrificeCost => tr!("Bleibende Karten opfern", "Sacrifice permanents"),
                    CardChoiceReason::Search => tr!("Bibliothek durchsuchen", "Search your library"),
                    _ => tr!("Karten wählen", "Choose cards"),
                };
                let title = if req.prompt.is_empty() { fallback.to_string() } else { req.prompt.clone() };
                (title, FanMode::Pick { min: *min as usize, max: *max as usize })
            }
            Decision::MulliganBottom { count, .. } => (
                trf!("{count} Karte(n) unter die Bibliothek legen", "Put {count} card(s) on the bottom of your library"),
                FanMode::Pick { min: *count as usize, max: *count as usize },
            ),
            Decision::Scry { graveyard, .. } => (
                if *graveyard {
                    tr!("Überwachen: Klick legt eine Karte in die andere Reihe", "Surveil: click a card to move it to the other row").into()
                } else {
                    tr!("Hellsicht: Klick legt eine Karte in die andere Reihe", "Scry: click a card to move it to the other row").into()
                },
                FanMode::Arrange { graveyard: *graveyard },
            ),
            _ => return None,
        };
        Some(HudFan { key: req.id, title, detail: req.source_name.clone(), mode, cards, cancelable: req.cancelable })
    }

    /// Index des Kandidaten `target` in der offenen Zielwahl.
    fn candidate_index(&self, target: Target) -> Option<usize> {
        match self.pending.as_ref().map(|r| &r.decision) {
            Some(Decision::ChooseTargets { candidates, .. }) => candidates.iter().position(|t| *t == target),
            _ => None,
        }
    }

    /// Position eines Ziels auf dem Tisch: Karte bzw. Medaillon-Anker des Sitzplatzes.
    fn target_position(&self, target: Target) -> Option<Vector3> {
        match target {
            Target::Card(c) => self.cards.get(&i64::from(c)).filter(|c| c.is_instance_valid()).map(|c| c.get_global_position()),
            Target::Player(p) => {
                let seat = if p == 0 { self.player_seat.as_ref() } else { self.opponent_seat.as_ref() }?;
                let anchor = seat.get_node_or_null("HeroAnchor").and_then(|n| n.try_cast::<Node3D>().ok());
                Some(anchor.map_or(seat.get_global_position(), |a| a.get_global_position()))
            }
        }
    }

    /// Kampfpfeile: während der eigenen Erklärung der Plan (Angreifer rot → Ziel, Blocker blau →
    /// Angreifer, gewählter Blocker gold → Angreifer unter der Maus), sonst der laufende Kampf aus
    /// dem Engine-Zustand (`combat`).
    fn combat_arrows(&self) -> Vec<ArrowSpec> {
        const RED: Color = Color::from_rgb(1.0, 0.32, 0.25);
        const BLUE: Color = Color::from_rgb(0.35, 0.62, 1.0);
        let pos = |c: CardId| self.target_position(Target::Card(c));
        let mut arrows = Vec::new();
        let mut push = |from: Option<Vector3>, to: Option<Vector3>, color: Color| {
            if let (Some(from), Some(to)) = (from, to) {
                let dir = (to - from).normalized_or_zero();
                arrows.push(ArrowSpec { from: from + dir * 0.3, to, color });
            }
        };
        match self.pending.as_ref().map(|r| &r.decision) {
            Some(Decision::DeclareAttackers { .. }) => {
                for (c, t) in &self.attack_plan {
                    push(pos(*c), self.target_position(*t), RED);
                }
            }
            Some(Decision::DeclareBlockers { attackers, .. }) => {
                for (b, a) in &self.block_plan {
                    push(pos(*b), pos(*a), BLUE);
                }
                if let Some(armed) = self.combat_armed {
                    let hovered = self
                        .controller
                        .as_ref()
                        .and_then(|c| c.bind().get_hovered_card())
                        .and_then(|h| self.forge_id(&h))
                        .and_then(|id| u32::try_from(id).ok())
                        .filter(|id| attackers.contains(id));
                    if let Some(a) = hovered {
                        push(pos(armed), pos(a), Color::from_rgba(SELECT_GLOW.r, SELECT_GLOW.g, SELECT_GLOW.b, 0.5));
                    }
                }
            }
            _ => {
                // Laufender Kampf aus dem Zustand
                let combat = self.state.as_ref().and_then(|s| s.get("combat")).and_then(|v| v.try_to::<VarDictionary>().ok());
                if let Some(combat) = combat {
                    for a in get_array(&combat, "attackers").unwrap_or_default().iter_shared() {
                        let Ok(a) = a.try_to::<VarDictionary>() else { continue };
                        let attacker = u32::try_from(get_int(&a, "card")).ok();
                        let target = a.get("target").and_then(|t| Target::from_var(&t));
                        push(attacker.and_then(pos), target.and_then(|t| self.target_position(t)), RED);
                    }
                    for pair in get_array(&combat, "blocks").unwrap_or_default().iter_shared() {
                        let Ok(pair) = pair.try_to::<VarArray>() else { continue };
                        let id = |i: usize| pair.get(i).and_then(|v| v.try_to::<i64>().ok()).and_then(|v| u32::try_from(v).ok());
                        push(id(0).and_then(pos), id(1).and_then(pos), BLUE);
                    }
                }
            }
        }
        arrows
    }

    /// Pfeile der Zielwahl: Quelle → gewählte Ziele (gold), Quelle → gültiges Ziel unter dem
    /// Mauszeiger (blasser, Vorschau).
    fn update_arrows(&mut self) {
        let Some(mut layer) = self.arrows.clone() else { return };
        let mut specs = self.combat_arrows();
        if let Some(req) = self.pending.as_ref() {
            if let Decision::ChooseTargets { candidates, .. } = &req.decision {
                let from = req
                    .source
                    .and_then(|s| self.cards.get(&i64::from(s)))
                    .filter(|c| c.is_instance_valid())
                    .map(|c| c.get_global_position());
                if let Some(from) = from {
                    let mut targets: Vec<(Target, Color)> =
                        self.selection.iter().filter_map(|i| candidates.get(*i)).map(|t| (*t, SELECT_GLOW)).collect();
                    let hovered = self.controller.as_ref().and_then(|c| c.bind().get_hovered_card());
                    let hovered_target = hovered
                        .and_then(|h| self.forge_id(&h))
                        .and_then(|id| u32::try_from(id).ok())
                        .map(Target::Card)
                        .filter(|t| candidates.contains(t) && !targets.iter().any(|(s, _)| s == t));
                    if let Some(t) = hovered_target {
                        targets.push((t, Color::from_rgba(SELECT_GLOW.r, SELECT_GLOW.g, SELECT_GLOW.b, 0.45)));
                    }
                    for (t, color) in targets {
                        if let Some(to) = self.target_position(t) {
                            let dir = (to - from).normalized_or_zero();
                            specs.push(ArrowSpec { from: from + dir * 0.3, to, color });
                        }
                    }
                }
            }
        }
        layer.bind_mut().set_arrows(&specs);
    }

    /// Gleicht den Tisch mit dem Forge-Zustand ab: Karten anlegen, verschieben, tappen, entfernen.
    fn apply_state(&mut self, state: VarDictionary) {
        let players: Vec<VarDictionary> = get_array(&state, "players")
            .map(|a| a.iter_shared().filter_map(|v| v.try_to::<VarDictionary>().ok()).collect())
            .unwrap_or_default();
        self.state = Some(state);

        // Soll-Zustand: (Schlüssel, Karte oder None = verdeckt, Zielzone, Besitzer)
        let mut wanted: Vec<(i64, Option<VarDictionary>, Gd<Zone>, usize)> = Vec::new();
        for (seat, p) in players.iter().enumerate().take(2) {
            let z = self.zones(seat);
            let list = |key: &str| -> Vec<VarDictionary> {
                get_array(p, key)
                    .map(|a| a.iter_shared().filter_map(|v| v.try_to::<VarDictionary>().ok()).collect())
                    .unwrap_or_default()
            };
            if let Some(hand) = &z.hand {
                if p.contains_key("hand") {
                    for c in list("hand") {
                        wanted.push((get_int(&c, "id"), Some(c), hand.clone(), seat));
                    }
                } else {
                    for i in 0..get_int(p, "hand_count") {
                        wanted.push((-(seat as i64 * 1000 + i + 1), None, hand.clone(), seat));
                    }
                }
            }
            for c in list("battlefield") {
                let zone = if is_land(&c) { z.resources.clone() } else { z.battlefield.clone() };
                if let Some(zone) = zone.or_else(|| z.battlefield.clone()) {
                    wanted.push((get_int(&c, "id"), Some(c), zone, seat));
                }
            }
            if let Some(discard) = &z.discard {
                for c in list("graveyard") {
                    wanted.push((get_int(&c, "id"), Some(c), discard.clone(), seat));
                }
            }
        }

        // Nicht mehr sichtbare Karten entfernen (Bibliothek, Exil …). Verdeckte Gegner-Handkarten
        // werden aufgehoben: Taucht beim Gegner eine neue Karte auf (ausgespielt), nimmt sie deren
        // Platz ein und gleitet so sichtbar aus seiner Hand.
        let keep: HashSet<i64> = wanted.iter().map(|w| w.0).collect();
        let gone: Vec<i64> = self.cards.keys().copied().filter(|k| !keep.contains(k)).collect();
        let mut from_hidden_hand: [Vec<Gd<Card3D>>; 2] = [Vec::new(), Vec::new()];
        for key in gone {
            self.signatures.remove(&key);
            let Some(card) = self.cards.remove(&key) else { continue };
            if key < 0 {
                let seat = card.bind().owner_seat.clamp(0, 1) as usize;
                from_hidden_hand[seat].push(card);
            } else {
                discard_card(card);
            }
        }

        // Deck auffüllen: Bibliothek + alle Karten, die diesmal neu auf die Hand kommen (die
        // werden gleich von oben gezogen).
        for (seat, p) in players.iter().enumerate().take(2) {
            let hand = self.zones(seat).hand;
            let drawn = wanted
                .iter()
                .filter(|w| w.3 == seat && Some(&w.2) == hand.as_ref() && !self.cards.contains_key(&w.0))
                .count();
            self.sync_deck(seat, get_int(p, "library").max(0) as usize + drawn);
        }

        for (key, data, mut zone, seat) in wanted {
            let card = match self.cards.get(&key) {
                Some(c) => c.clone(),
                None => {
                    let to_hand = self.zones(seat).hand.as_ref() == Some(&zone);
                    // Neue Handkarte: oberste Deckkarte ziehen; sonst aus der verdeckten Hand
                    // (Gegner spielt aus); sonst neu über dem Deck erzeugen (z. B. Spielsteine)
                    let c = if to_hand { self.deck_cards[seat].pop() } else { None }
                        .or_else(|| from_hidden_hand[seat].pop())
                        .unwrap_or_else(|| self.spawn_card(seat));
                    self.cards.insert(key, c.clone());
                    c
                }
            };
            let mut card = card;
            if let Some(data) = &data {
                // Werte nur bei Änderung neu setzen (Kartenbild wird dabei neu gezeichnet)
                let sig = format!(
                    "{}|{}|{}|{}|{}",
                    get_str(data, "name"),
                    get_str(data, "type"),
                    get_str(data, "cost"),
                    get_int(data, "power"),
                    get_int(data, "toughness")
                );
                if self.signatures.get(&key) != Some(&sig) {
                    let def = def_from_forge(data);
                    let mut c = card.bind_mut();
                    c.set_face_texture(Some(placeholder::card_face(&def)));
                    c.apply_def(&def);
                    drop(c);
                    self.signatures.insert(key, sig);
                }
                let tapped = get_bool(data, "tapped");
                if card.bind().is_tapped() != tapped {
                    card.bind_mut().set_tapped_state(tapped);
                }
            }
            if card.bind().is_dragging() {
                continue;
            }
            let current = card.bind().get_zone();
            if current.as_ref() != Some(&zone) {
                if let Some(mut old) = current {
                    old.bind_mut().remove_card(card.clone());
                }
                zone.bind_mut().add_card(card.clone(), -1, 0.0);
            } else if zone.bind().is_free_placement() {
                // Zurückgesetzter Drop auf freier Fläche: neu anordnen
                zone.bind_mut().layout();
            }
        }
        for card in from_hidden_hand.into_iter().flatten() {
            discard_card(card);
        }
        for (seat, p) in players.iter().enumerate().take(2) {
            self.sync_deck(seat, get_int(p, "library").max(0) as usize);
        }
        self.update_glow();
    }

    /// Bringt den verdeckten Stapel in der Deck-Zone von `seat` auf `count` Karten (neue Karten
    /// unten einschieben, überzählige von oben entfernen).
    fn sync_deck(&mut self, seat: usize, count: usize) {
        let Some(mut deck) = self.zones(seat).deck else { return };
        while self.deck_cards[seat].len() < count {
            let card = self.spawn_card(seat);
            deck.bind_mut().add_card(card.clone(), 0, 0.0);
            self.deck_cards[seat].insert(0, card);
        }
        while self.deck_cards[seat].len() > count {
            if let Some(card) = self.deck_cards[seat].pop() {
                discard_card(card);
            }
        }
    }

    /// Neue Karte über dem Deck ihres Besitzers erzeugen (gleitet dann in die Zielzone).
    fn spawn_card(&mut self, seat: usize) -> Gd<Card3D> {
        let mut card = Card3D::new_alloc();
        {
            let mut c = card.bind_mut();
            c.owner_seat = seat as i64;
            c.set_back_texture(Some(placeholder::card_back()));
            let hidden = CardDef::base("?", CardKind::Creature, Cost::default());
            c.set_face_texture(Some(placeholder::card_face(&hidden)));
            c.apply_def(&hidden);
        }
        match self.controller.clone() {
            Some(mut ctrl) => ctrl.add_child(&card),
            None => self.base_mut().add_child(&card),
        }
        let z = self.zones(seat);
        if let Some(start) = z.deck.or(z.hand) {
            card.bind_mut().snap_to(start.get_global_transform());
        }
        card
    }
}

/// Karte aus ihrer Zone nehmen und löschen.
fn discard_card(mut card: Gd<Card3D>) {
    let zone = card.bind().get_zone();
    if let Some(mut zone) = zone {
        zone.bind_mut().remove_card(card.clone());
    }
    card.queue_free();
}

/// Name der NeoForge-Entscheidung (für [method ForgeTable.get_pending_kind]).
fn decision_name(d: &Decision) -> &'static str {
    match d {
        Decision::ChooseModes { .. } => "ChooseModes",
        Decision::Mulligan { .. } => "Mulligan",
        Decision::MulliganBottom { .. } => "MulliganBottom",
        Decision::Priority { .. } => "Priority",
        Decision::ChooseTargets { .. } => "ChooseTargets",
        Decision::ChooseX { .. } => "ChooseX",
        Decision::PayMana { .. } => "PayMana",
        Decision::PayLifeForPhyrexian { .. } => "PayLifeForPhyrexian",
        Decision::ChooseColor { .. } => "ChooseColor",
        Decision::NameCard { .. } => "NameCard",
        Decision::ChooseType { .. } => "ChooseType",
        Decision::DeclareAttackers { .. } => "DeclareAttackers",
        Decision::DeclareBlockers { .. } => "DeclareBlockers",
        Decision::AssignCombatDamage { .. } => "AssignCombatDamage",
        Decision::ChooseCards { .. } => "ChooseCards",
        Decision::OrderTriggers { .. } => "OrderTriggers",
        Decision::Scry { .. } => "Scry",
        Decision::PayUnlessCost { .. } => "PayUnlessCost",
        Decision::OptionalTrigger { .. } => "OptionalTrigger",
        Decision::Confirm { .. } => "Confirm",
        Decision::ChooseOption { .. } => "ChooseOption",
        Decision::ChooseOptions { .. } => "ChooseOptions",
        Decision::ChooseNumber { .. } => "ChooseNumber",
    }
}

/// Forge-Phase (`PhaseType`-Name) → Index in [`Phase::ORDER`].
fn phase_index(phase: &str) -> i64 {
    match phase {
        "UNTAP" => 0,
        "UPKEEP" => 1,
        "DRAW" => 2,
        "MAIN1" => 3,
        "COMBAT_BEGIN" | "COMBAT_DECLARE_ATTACKERS" => 4,
        "COMBAT_DECLARE_BLOCKERS" => 5,
        "COMBAT_FIRST_STRIKE_DAMAGE" | "COMBAT_DAMAGE" | "COMBAT_END" => 6,
        "MAIN2" => 7,
        "END_OF_TURN" | "CLEANUP" => 8,
        _ => -1,
    }
}

fn is_land(card: &VarDictionary) -> bool {
    get_str(card, "type").contains("Land")
}

/// Forge-Karte → Kartendefinition für Bild und Anzeige (Regeln macht Forge).
fn def_from_forge(card: &VarDictionary) -> CardDef {
    let ty = get_str(card, "type");
    let kind = if ty.contains("Land") {
        CardKind::Resource
    } else if ty.contains("Creature") {
        CardKind::Creature
    } else if ty.contains("Instant") {
        CardKind::Instant
    } else if ty.contains("Sorcery") {
        CardKind::Sorcery
    } else if ty.contains("Aura") {
        CardKind::Aura
    } else if ty.contains("Equipment") {
        CardKind::Equipment
    } else if ty.contains("Enchantment") {
        CardKind::Enchantment
    } else {
        CardKind::Artifact
    };
    let mut def = CardDef::base(get_str(card, "name"), kind, forge_cost(&get_str(card, "cost")));
    def.attack = get_int(card, "power") as i32;
    def.defense = get_int(card, "toughness") as i32;
    if kind == CardKind::Resource {
        def.produces = [
            ("Plains", ManaType::White),
            ("Island", ManaType::Blue),
            ("Swamp", ManaType::Black),
            ("Mountain", ManaType::Red),
            ("Forest", ManaType::Green),
        ]
        .into_iter()
        .find(|(n, _)| ty.contains(n))
        .map_or(ManaType::Colorless, |(_, t)| t);
    }
    def
}

/// Forge-Manakosten (z. B. "2 R R", "{1}{G}", "X U") → [`Cost`]. Unbekannte Symbole
/// (X, hybride Kosten …) werden ignoriert.
fn forge_cost(text: &str) -> Cost {
    let mut cost = Cost::default();
    for token in text.split(|c: char| c.is_whitespace() || c == '{' || c == '}').filter(|t| !t.is_empty()) {
        if let Ok(n) = token.parse::<u32>() {
            cost.generic += n;
            continue;
        }
        for ch in token.chars() {
            let t = match ch {
                'W' => ManaType::White,
                'U' => ManaType::Blue,
                'B' => ManaType::Black,
                'R' => ManaType::Red,
                'G' => ManaType::Green,
                'C' => ManaType::Colorless,
                _ => continue,
            };
            cost.specific.add(t, 1);
        }
    }
    cost
}

fn get_str(d: &VarDictionary, key: &str) -> String {
    d.get(key).filter(|v| !v.is_nil()).map(|v| v.to_string()).unwrap_or_default()
}

fn get_int(d: &VarDictionary, key: &str) -> i64 {
    d.get(key).and_then(|v| v.try_to::<i64>().ok()).unwrap_or(0)
}

fn get_bool(d: &VarDictionary, key: &str) -> bool {
    d.get(key).and_then(|v| v.try_to::<bool>().ok()).unwrap_or(false)
}

fn get_array(d: &VarDictionary, key: &str) -> Option<VarArray> {
    d.get(key).and_then(|v| v.try_to::<VarArray>().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_forge_costs() {
        let c = forge_cost("2 R R");
        assert_eq!(c.generic, 2);
        assert_eq!(c.specific.get(ManaType::Red), 2);
        let c = forge_cost("{1}{G}");
        assert_eq!(c.generic, 1);
        assert_eq!(c.specific.get(ManaType::Green), 1);
        assert_eq!(forge_cost("no cost").total(), 0);
    }

    #[test]
    fn maps_forge_phases() {
        assert_eq!(phase_index("MAIN1"), 3);
        assert_eq!(phase_index("COMBAT_DECLARE_BLOCKERS"), 5);
        assert_eq!(phase_index("CLEANUP"), 8);
        assert_eq!(phase_index("???"), -1);
    }
}
