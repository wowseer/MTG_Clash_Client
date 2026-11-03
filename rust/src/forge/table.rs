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
use crate::game::hud::{HudChoice, HudView};
use crate::game::mana::{Cost, ManaType};
use crate::game::phases::Phase;
use crate::game::seat::PlayerSeat;
use crate::table::card3d::Card3D;
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
/// Hauptbutton / Leertaste = passen. Mulligan im Auswahlfenster. Ziele, Angreifer und Blocker
/// wählt vorerst die Forge-KI für dich.
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
    /// Offene Entscheidung (priority / mulligan) des Spielers.
    pending: Option<VarDictionary>,
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
            player_deck: "res://../forge-bridge/testdecks/rot.dck".into(),
            opponent_deck: "res://../forge-bridge/testdecks/gruen.dck".into(),
            seed: -1,
            autostart: true,
            refresh_interval: 0.25,
            cards: HashMap::new(),
            deck_cards: [Vec::new(), Vec::new()],
            signatures: HashMap::new(),
            pending: None,
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
        }
    }

    fn process(&mut self, delta: f64) {
        self.since_refresh += delta;
        if self.dirty && self.match_running && self.since_refresh >= self.refresh_interval {
            self.dirty = false;
            self.since_refresh = 0.0;
            if let Some(b) = self.bridge.as_mut() {
                b.bind_mut().request_state(LOCAL_SEAT);
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

    /// Passt (Hauptbutton / Leertaste): beantwortet eine offene Prioritätsanfrage mit -1.
    #[func]
    pub fn pass_priority(&mut self) {
        if self.pending_kind().as_deref() == Some("priority") {
            self.answer((-1i64).to_variant());
        }
    }

    /// Auswahl im HUD-Fenster: beim Mulligan 0 = behalten, 1 = Mulligan.
    #[func]
    pub fn hud_choose(&mut self, index: i64) {
        if self.pending_kind().as_deref() == Some("mulligan") && index >= 0 {
            self.answer((index == 0).to_variant());
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

    /// Hat der Spieler gerade eine offene Entscheidung? Liefert deren Art ("priority",
    /// "mulligan") oder einen leeren Text.
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

    /// Entscheidung des Spielers nötig: Zustand übernehmen, spielbare Karten leuchten lassen.
    #[func]
    fn on_decision_requested(&mut self, request: VarDictionary) {
        if let Some(state) = request.get("state").and_then(|v| v.try_to::<VarDictionary>().ok()) {
            self.apply_state(state);
        }
        self.pending = Some(request);
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

    /// Karte wurde auf dem Tisch abgelegt: aus der Hand gespielt → passende Option an Forge;
    /// alles andere wird auf den Forge-Stand zurückgesetzt.
    #[func]
    fn on_card_dropped(&mut self, card: Gd<Card3D>, from_zone: Gd<Zone>, to_zone: Gd<Zone>) {
        let hand = self.zones(0).hand;
        let from_hand = hand.as_ref() == Some(&from_zone);
        let to_hand = hand.as_ref() == Some(&to_zone);
        let id = self.forge_id(&card);
        if from_hand && !to_hand {
            if let Some(index) = id.and_then(|id| self.option_for_card(id)) {
                self.answer(index.to_variant());
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
        let kind = self.pending_kind();
        let my_priority = kind.as_deref() == Some("priority");

        let choice = (kind.as_deref() == Some("mulligan")).then(|| HudChoice {
            title: tr!("Starthand behalten?", "Keep your opening hand?").to_string(),
            options: vec![
                (tr!("Behalten", "Keep").to_string(), true),
                (tr!("Mulligan", "Mulligan").to_string(), true),
            ],
            cancelable: false,
        });
        let button_text = if my_priority {
            tr!("Passen  »", "Pass  »")
        } else if choice.is_some() {
            tr!("Starthand wählen …", "Choose your opening hand …")
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
            player_pool: PackedInt64Array::new(),
            choice,
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
        self.pending.as_ref().map(|p| get_str(p, "kind"))
    }

    fn answer(&mut self, choice: Variant) {
        let Some(req) = self.pending.take() else { return };
        let id = get_int(&req, "id");
        self.update_glow();
        if let Some(b) = self.bridge.as_mut() {
            b.bind_mut().answer(id, choice);
        }
    }

    /// Option der offenen Prioritätsanfrage, die die Karte `card_id` spielt (Land/Zauber zuerst).
    fn option_for_card(&self, card_id: i64) -> Option<i64> {
        let req = self.pending.as_ref().filter(|_| self.pending_kind().as_deref() == Some("priority"))?;
        let options = get_array(req, "options")?;
        let mut best = None;
        for o in options.iter_shared() {
            let Ok(o) = o.try_to::<VarDictionary>() else { continue };
            if get_int(&o, "card_id") != card_id {
                continue;
            }
            let index = get_int(&o, "index");
            if get_bool(&o, "land") || get_bool(&o, "spell") {
                return Some(index);
            }
            best.get_or_insert(index);
        }
        best
    }

    fn forge_id(&self, card: &Gd<Card3D>) -> Option<i64> {
        self.cards.iter().find(|(_, c)| *c == card).map(|(id, _)| *id)
    }

    /// Leuchten: Karten, die der Spieler gerade spielen kann.
    fn update_glow(&mut self) {
        let playable: HashSet<i64> = match (&self.pending, self.pending_kind().as_deref()) {
            (Some(req), Some("priority")) => get_array(req, "options")
                .map(|opts| {
                    opts.iter_shared()
                        .filter_map(|o| o.try_to::<VarDictionary>().ok())
                        .map(|o| get_int(&o, "card_id"))
                        .collect()
                })
                .unwrap_or_default(),
            _ => HashSet::new(),
        };
        for (id, card) in self.cards.iter_mut() {
            card.bind_mut().set_ready_glow(playable.contains(id));
        }
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
