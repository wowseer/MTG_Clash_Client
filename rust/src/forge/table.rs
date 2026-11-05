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

use std::collections::{HashMap, HashSet, VecDeque};

use godot::prelude::*;

use super::bridge::ForgeBridge;
use crate::game::cards::{CardDef, CardKind};
use crate::game::choice_panel::{ChoiceMode, ChoiceOption, HudChoice};
use crate::game::decision_view;
use crate::game::counters;
use crate::game::hud::HudView;
use crate::rules::decision::{CardChoiceReason, PayAction};
use crate::rules::{Action, CardId, Choice, Color as ManaColor, Decision, PlayerId, Request, RulesEngine, Target};
use crate::table::mana_wheel::{ManaWheel, WheelSegment};
use crate::table::exile_portal::{attach_exile_portal, set_exile_portal_open};
use crate::game::debug_panel::DebugPanel;
use godot::classes::Os;
use crate::game::phases::Phase;
use crate::game::seat::PlayerSeat;
use crate::table::button3d::{Button3D, ButtonStyle, COMPACT_BODY};
use crate::table::showcase::Showcase;
use crate::table::card3d::{Card3D, CardMarker, CARD_HEIGHT, CARD_WIDTH, LUNGE_OUT, PLAYABLE_GLOW, TILE_HEIGHT};
use crate::table::card_inspector::{CardInspector, InspectorEntry};
use crate::table::fx::{mana_color, ManaFx};
use godot::classes::Label3D;
use crate::table::target_arrows::{ArrowColor, ArrowSpec, TargetArrows, LOOP_DISTANCE};
use crate::table::placeholder;
use crate::table::table_controller::TableController;
use crate::table::zone::Zone;

/// Forge-Sitzplatz des Spielers (unten).
const LOCAL_SEAT: i64 = 0;
/// Schlüssel der Karten für Fähigkeiten auf dem Stack: `ABILITY_KEY_BASE - Stack-ID`
/// (verdeckte Gegner-Handkarten liegen zwischen −1 und dieser Grenze).
const ABILITY_KEY_BASE: i64 = -1_000_000;
/// Beschriftung des Fertig-Knopfes beim Unter-die-Bibliothek-Legen.
fn pick_label(n: u32, min: u32, max: u32) -> String {
    if min == max {
        trf!("Fertig ({n}/{max})", "Done ({n}/{max})")
    } else {
        trf!("Fertig ({n}/{min}–{max})", "Done ({n}/{min}–{max})")
    }
}

/// Startspieler-Knöpfe: Breite, Abstand untereinander (in Bildschirm-Hochrichtung) und Mitte der
/// Knopfspalte (über der Tischmitte, zum Spieler hin).
const STARTING_BUTTON_WIDTH: f32 = 4.6;
const STARTING_BUTTON_SPACING: f32 = 1.15;
const STARTING_CHOICE_CENTER: Vector3 = Vector3::new(0.0, 1.6, 0.9);
/// Gemerkte bzw. im HUD gezeigte Zeilen des Spielprotokolls.
/// Farbe der Pfeile vom Stack-Eintrag zu seinen Zielen.

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
    /// Stack-Zone (Art 4) in der Tischmitte: Zauber liegen dort als ihre Karte, aktivierte und
    /// ausgelöste Fähigkeiten als Karte ihrer Quelle; der oberste Eintrag liegt rechts oben.
    /// Pfeile zeigen die Ziele jedes Eintrags. Leer = Stack nur in der Liste im HUD.
    #[export]
    stack_zone: Option<Gd<Zone>>,
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
    /// Offene Entscheidung des Spielers (`Decision` mit Anzeigehilfen).
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
    /// 3D-Bedienobjekte der offenen Entscheidung (z. B. Startspieler-Knöpfe) und ihre Frage als
    /// 3D-Text; werden bei der Antwort ausgeblendet.
    choice_buttons: Vec<Gd<Button3D>>,
    choice_prompt: Option<Gd<Label3D>>,
    /// ✕-Knopf an der Karte, die gerade gewirkt bzw. deren Fähigkeit aktiviert wird (Wirken
    /// abbrechen, siehe [`Self::cast_cancel`]).
    cancel_badge: Option<Gd<Button3D>>,
    /// Offenes Farbrad: Rad, Antwort je Segment, freiwillig (Manafähigkeit eines Landes mit
    /// Priorität – schließen ohne Antwort möglich) oder Antwort auf eine Frage der Engine.
    wheel: Option<(Gd<ManaWheel>, Vec<Choice>, bool)>,
    /// Friedhof bzw. Exil wird gerade auf der Bühne durchgesehen (keine Entscheidung).
    viewer_open: bool,
    /// Kartenkopien der Friedhof-/Exil-Ansicht mit ihrer Forge-ID (Klick = Aktion der Karte, z. B.
    /// Rückblende oder aus dem Exil wirken).
    viewer_cards: Vec<(Gd<Card3D>, CardId)>,
    /// Bühne vor der Kamera (z. B. Starthand beim Mulligan); schließt mit der Antwort.
    showcase: Option<Gd<Showcase>>,
    /// Karten, die nach einem Mulligan unter die Bibliothek sollen (Klick-Reihenfolge).
    /// Auswahl auf der Hand-Bühne (Mulligan unter die Bibliothek, Abwerfen …) und ihr
    /// „Fertig“-Knopf.
    hand_pick: Vec<CardId>,
    hand_pick_button: Option<Gd<Button3D>>,
    /// Zahlen-Bühne (X wählen, Zahl wählen): gewählter Wert und der Bestätigen-Knopf (zeigt
    /// den Wert).
    x_value: u32,
    x_pay_button: Option<Gd<Button3D>>,
    /// Bühnen-Knöpfe: Antwort je Knopf (Index = Argument von `on_stage_choice`).
    stage_choices: Vec<Choice>,
    /// Modus-Bühne: Kartenkopie je Modus (Modusnummer der Engine) und die Wahl.
    mode_cards: Vec<(Gd<Card3D>, usize)>,
    mode_pick: Vec<usize>,
    /// Optionale Kosten mit mehreren Möglichkeiten: an/aus je Option.
    option_toggles: Vec<bool>,
    /// Hellsicht/Überwachen bzw. Kartenwahl aus verdeckten Zonen: Kartenkopien auf der Bühne mit
    /// ihrer Forge-ID, die Ablagen „unten“/„oben“ (Hellsicht) und die Wahl.
    pick_cards: Vec<(Gd<Card3D>, CardId)>,
    scry_zones: Option<(Gd<Zone>, Gd<Zone>)>,
    pick_chosen: Vec<CardId>,
    /// Reihenfolge-Bühne: Kartenkopie je Eintrag (Index) und die vergebene Reihenfolge.
    order_cards: Vec<(Gd<Card3D>, usize)>,
    order_pick: Vec<usize>,
    /// „Sofort beenden“: in dieser Runde automatisch passen (Runde laut Zustand; `None` = aus).
    auto_pass_turn: Option<i64>,
    /// Pfeile, die der Spieler selbst gezogen hat (wachsen nicht noch einmal).
    drawn_arrows: HashSet<u64>,
    /// „Wähle eins“: Kartenkopien auf der Bühne und die Aktion, für die sie stehen.
    action_options: Vec<(Gd<Card3D>, Action)>,
    /// Zuletzt getappte Karten (Forge-ID, Zeitpunkt in s) – Quelle des nächsten Manas.
    recent_taps: VecDeque<(i64, f64)>,
    /// Laufende Mana-Effekte, die noch ein Ziel (Karte auf dem Stack) bekommen können, mit Sitz.
    mana_fx: Vec<(Gd<ManaFx>, i64)>,
    /// Schläge im Kampf, die schon gespielt wurden (Runde, Phase, Quelle).
    lunged: HashSet<(i64, String, i64)>,
    /// Stack-Einträge des letzten Zustands (neue leuchten beim Aktivieren auf).
    known_stack: HashSet<i64>,
    /// Laufzeit (s) für die Zeitstempel oben.
    clock: f64,
    /// Kartenansicht beim Hovern und die Karte, die sie gerade zeigt.
    inspector: Option<Gd<CardInspector>>,
    inspected: Option<InstanceId>,
    /// Von Skripten erzwungene Kartenansicht (statt der Karte unter der Maus).
    inspect_override: Option<Gd<Card3D>>,
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
            stack_zone: None,
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
            choice_buttons: Vec::new(),
            choice_prompt: None,
            cancel_badge: None,
            wheel: None,
            viewer_open: false,
            viewer_cards: Vec::new(),
            showcase: None,
            hand_pick: Vec::new(),
            hand_pick_button: None,
            x_value: 0,
            x_pay_button: None,
            stage_choices: Vec::new(),
            mode_cards: Vec::new(),
            mode_pick: Vec::new(),
            option_toggles: Vec::new(),
            pick_cards: Vec::new(),
            scry_zones: None,
            pick_chosen: Vec::new(),
            order_cards: Vec::new(),
            order_pick: Vec::new(),
            auto_pass_turn: None,
            drawn_arrows: HashSet::new(),
            action_options: Vec::new(),
            recent_taps: VecDeque::new(),
            mana_fx: Vec::new(),
            lunged: HashSet::new(),
            known_stack: HashSet::new(),
            clock: 0.0,
            inspector: None,
            inspected: None,
            inspect_override: None,
            state: None,
            dirty: false,
            since_refresh: 0.0,
            winner: String::new(),
            match_running: false,
            base,
        }
    }

    fn ready(&mut self) {
        placeholder::open_printed();
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
            // Debug-Fenster (F10), nur in Debug-Builds
            if Os::singleton().is_debug_build() {
                let mut panel = DebugPanel::new_alloc();
                panel.set("bridge", &bridge.to_variant());
                self.base_mut().call_deferred("add_child", &[panel.to_variant()]);
            }
            if self.autostart && !bridge.bind().is_running() {
                bridge.bind_mut().start_bridge();
            }
        } else {
            godot_warn!("ForgeTable: keine ForgeBridge gesetzt");
        }
        if let Some(mut controller) = self.controller.clone() {
            controller.connect("card_dropped", &cb("on_card_dropped"));
            controller.connect("card_clicked", &cb("on_card_clicked"));
            controller.connect("aim_released", &cb("on_aim_released"));
            controller.connect("cancel_pressed", &cb("cancel_cast"));
            controller.connect("zone_clicked", &cb("on_zone_clicked"));
            // zurückgestellt: der Tisch baut in diesem Moment noch seine Kinder auf
            let arrows = TargetArrows::new_alloc();
            controller.call_deferred("add_child", &[arrows.to_variant()]);
            self.arrows = Some(arrows);
            let inspector = CardInspector::new_alloc();
            controller.call_deferred("add_child", &[inspector.to_variant()]);
            self.inspector = Some(inspector);
        }
    }

    fn exit_tree(&mut self) {
        // geladene Kartenbilder freigeben (sonst meldet Godot beim Beenden übrige Texturen)
        placeholder::clear_printed();
    }

    fn process(&mut self, delta: f64) {
        self.clock += delta;
        // Mana-Effekte ohne Ziel: sobald eine Karte auf dem Stack liegt, fliegt das Mana dorthin
        self.mana_fx.retain(|(fx, _)| fx.is_instance_valid());
        for i in 0..self.mana_fx.len() {
            let (fx, seat) = self.mana_fx[i].clone();
            if !fx.bind().has_target() {
                if let Some(top) = self.stack_top_card_of(seat) {
                    let mut fx = fx;
                    fx.bind_mut().set_target(Some(top));
                }
            }
        }
        self.update_inspector();
        self.flash_new_stack();
        self.update_combat_look();
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
        if self.viewer_open {
            self.close_zone_viewer();
            return;
        }
        match self.pending.as_ref().map(|r| &r.decision) {
            Some(Decision::Priority { .. }) => self.answer(Choice::Action(Action::Pass)),
            // Kampf: der Phasen-Knopf bestätigt den Plan, der auf dem Tisch gebaut wurde
            Some(Decision::DeclareAttackers { .. }) => {
                let plan = self.attack_plan.clone();
                self.answer(Choice::Attackers(plan));
            }
            Some(Decision::DeclareBlockers { .. }) => {
                let plan = self.block_plan.clone();
                self.answer(Choice::Blocks(plan));
            }
            // Zielwahl: genug Ziele = bestätigen, sonst Wirken abbrechen (wo Forge das erlaubt)
            Some(Decision::ChooseTargets { candidates, min, .. }) => {
                if self.selection.len() >= *min as usize {
                    let targets = self.selection.iter().filter_map(|i| candidates.get(*i).copied()).collect();
                    self.answer(Choice::Targets(targets));
                } else if self.pending.as_ref().is_some_and(|r| r.cancelable) {
                    self.answer(Choice::Cancel);
                }
            }
            // sonst beim Wirken/Aktivieren: der Knopf heißt „Abbrechen“ und nimmt es zurück
            _ => {
                if let Some(c) = self.cast_cancel() {
                    self.answer(c);
                }
            }
        }
    }

    /// Wirken bzw. Aktivieren abbrechen (✕ an der Karte, Escape, Karte zurück auf die Hand
    /// ziehen): beantwortet die offene Frage mit Abbrechen, wenn die Engine das erlaubt und die
    /// Frage zum Wirken einer eigenen Karte bzw. Aktivieren einer eigenen Fähigkeit gehört.
    #[func]
    pub fn cancel_cast(&mut self) {
        if self.viewer_open {
            self.close_zone_viewer();
            return;
        }
        if self.wheel.as_ref().is_some_and(|(_, _, v)| *v) {
            self.close_wheel();
            return;
        }
        if self.stage_covers() {
            return; // die Bühne hat ihre eigenen Knöpfe
        }
        if let Some(c) = self.cast_cancel() {
            self.answer(c);
        }
    }

    /// Ist gerade eine Bühne offen (und nicht schon geschlossen)?
    fn stage_open_now(&self) -> bool {
        self.showcase.as_ref().is_some_and(|s| s.is_instance_valid() && s.bind().is_covering())
    }

    /// Friedhof bzw. Exil, in dem `card` liegt: (Sitzplatz, true = Exil).
    fn pile_of(&self, card: &Gd<Card3D>) -> Option<(usize, bool)> {
        let zone = card.bind().get_zone()?;
        (0..2).find_map(|seat| {
            let z = self.zones(seat);
            if z.discard.as_ref() == Some(&zone) {
                Some((seat, false))
            } else if z.exile.as_ref() == Some(&zone) {
                Some((seat, true))
            } else {
                None
            }
        })
    }

    /// Friedhof bzw. Exil von `seat` durchsehen (CR 404.2, 406.3: beide Zonen sind öffentlich):
    /// dieselbe Cover-Flow-Bühne wie beim Durchsuchen der Bibliothek, jüngste Karte zuerst,
    /// Titel „Friedhof (n)“ bzw. „Exil (n)“ mit dem Namen des Besitzers, „Schlachtfeld ansehen“,
    /// orange „Fertig“ (auch Leertaste / Escape). Die offene Entscheidung bleibt bestehen.
    fn show_zone_viewer(&mut self, seat: usize, exile: bool) {
        let key = if exile { "exile" } else { "graveyard" };
        let cards: Vec<VarDictionary> = self
            .state
            .as_ref()
            .and_then(|s| get_array(s, "players"))
            .and_then(|ps| ps.get(seat))
            .and_then(|p| p.try_to::<VarDictionary>().ok())
            .and_then(|p| get_array(&p, key))
            .map(|a| a.iter_shared().filter_map(|v| v.try_to::<VarDictionary>().ok()).collect())
            .unwrap_or_default();
        if cards.is_empty() {
            return;
        }
        let n = cards.len();
        let title = if exile { trf!("Exil ({n})", "Exile ({n})") } else { trf!("Friedhof ({n})", "Graveyard ({n})") };
        let owner = self.seat_name(seat).to_string();
        let Some(mut showcase) = self.open_stage(&title, &owner) else { return };
        let mut copies: Vec<(Gd<Card3D>, String)> = Vec::new();
        let mut viewer_cards = Vec::new();
        for d in cards.iter().rev() {
            let mut copy = Self::stage_card(d, None);
            let id = u32::try_from(get_int(d, "id")).unwrap_or(0);
            // spielbar (z. B. Rückblende, aus dem Exil wirken): leuchtet, Klick führt es aus
            if !self.card_actions(id).is_empty() {
                copy.bind_mut().set_ready_color(Some(PLAYABLE_GLOW));
            }
            viewer_cards.push((copy.clone(), id));
            copies.push((copy, String::new()));
        }
        showcase.bind_mut().show_options(copies);
        showcase.bind_mut().enable_coverflow(LIB_STEP, LIB_SCALE);
        showcase.bind_mut().enable_peek();
        let mut done = Button3D::create(ButtonStyle::Orange, tr!("Fertig", "Done"), Showcase::button_width());
        done.connect("pressed", &Callable::from_object_method(&self.to_gd(), "close_zone_viewer"));
        showcase.bind_mut().add_button(done);
        self.viewer_open = true;
        self.viewer_cards = viewer_cards;
    }

    /// Anklickbare Zonen-Fläche angeklickt (vom [TableController]): Exil-Strudel → Exil-Ansicht.
    #[func]
    fn on_zone_clicked(&mut self, zone: Gd<Zone>) {
        if self.stage_open_now() {
            return;
        }
        if let Some(seat) = (0..2).find(|s| self.zones(*s).exile.as_ref() == Some(&zone)) {
            self.show_zone_viewer(seat, true);
        }
    }

    /// Friedhof-/Exil-Ansicht schließen; die offene Entscheidung zeigt wieder ihre Bedienung.
    #[func]
    fn close_zone_viewer(&mut self) {
        if !self.viewer_open {
            return;
        }
        self.viewer_open = false;
        self.clear_choice_objects();
        self.show_cancel_badge();
    }

    /// Farbrad für eine Frage der Engine, wenn sie eine Manafarbe verlangt: `ChooseColor`
    /// (z. B. „ein Mana beliebiger Farbe“, CR 106.1) oder `ChooseType` nur mit Grundland-Typen
    /// (CR 305.6: Ebene → Weiß, Insel → Blau, Sumpf → Schwarz, Gebirge → Rot, Wald → Grün).
    /// Je Segment Manabuchstabe und Antwort, Reihenfolge WUBRG.
    fn wheel_entries(decision: &Decision) -> Option<Vec<(char, Choice)>> {
        let mut entries: Vec<(char, Choice)> = match decision {
            Decision::ChooseColor { options, .. } if !options.is_empty() => options
                .iter()
                .map(|c| {
                    let letter = match c {
                        ManaColor::White => 'W',
                        ManaColor::Blue => 'U',
                        ManaColor::Black => 'B',
                        ManaColor::Red => 'R',
                        ManaColor::Green => 'G',
                    };
                    (letter, Choice::Color(*c))
                })
                .collect(),
            Decision::ChooseType { options, .. } if !options.is_empty() => options
                .iter()
                .enumerate()
                .map(|(i, t)| {
                    let letter = match t.as_str() {
                        "Plains" => 'W',
                        "Island" => 'U',
                        "Swamp" => 'B',
                        "Mountain" => 'R',
                        "Forest" => 'G',
                        _ => return None,
                    };
                    Some((letter, Choice::Number(i as u32)))
                })
                .collect::<Option<Vec<_>>>()?,
            _ => return None,
        };
        entries.sort_by_key(|(l, _)| WHEEL_ORDER.find(*l).unwrap_or(9));
        Some(entries)
    }

    /// Farbrad für Manafähigkeiten einer Karte: jede Fähigkeit erzeugt genau eine Manaart
    /// (Text nach dem Doppelpunkt, z. B. „{T}: Add {G}.“), alle verschieden; sonst `None`.
    fn mana_wheel_entries(mine: &[(Choice, String)]) -> Option<Vec<(char, Choice)>> {
        let mana = |c: &Choice| matches!(c, Choice::Action(Action::ActivateMana(..)) | Choice::Pay(PayAction::ActivateMana(..)));
        if mine.len() < 2 || !mine.iter().all(|(c, _)| mana(c)) {
            return None;
        }
        let mut entries = Vec::new();
        for (choice, label) in mine {
            let effect = label.rsplit(':').next().unwrap_or(label);
            let mut letters: Vec<char> = effect
                .split('{')
                .skip(1)
                .filter_map(|s| s.split('}').next())
                .filter_map(|sym| (sym.len() == 1).then(|| sym.chars().next()).flatten())
                .filter(|c| WHEEL_ORDER.contains(*c))
                .collect();
            letters.dedup();
            if letters.len() != 1 || entries.iter().any(|(l, _)| *l == letters[0]) {
                return None;
            }
            entries.push((letters[0], choice.clone()));
        }
        entries.sort_by_key(|(l, _)| WHEEL_ORDER.find(*l).unwrap_or(9));
        Some(entries)
    }

    /// Hinweis über dem Farbrad: „(Quelle) Wähle eine Farbe.“ bzw. „… einen Grundland-Typ.“
    fn wheel_prompt(req: &Request, decision: &Decision) -> String {
        let what = match decision {
            Decision::ChooseType { .. } => tr!("Wähle einen Grundland-Typ.", "Choose a basic land type."),
            _ => tr!("Wähle eine Farbe.", "Choose a color."),
        };
        if req.source_name.is_empty() { what.to_string() } else { format!("({}) {what}", req.source_name) }
    }

    /// Farbrad öffnen (an der Bildschirmstelle der Karte `source`, sonst in der Bildmitte, mit
    /// ihrem Bild in der Mitte). `voluntary` = ohne offene Frage (schließen ohne Antwort).
    fn open_wheel(&mut self, entries: Vec<(char, Choice)>, source: Option<CardId>, prompt: &str, voluntary: bool) {
        self.close_wheel();
        let Some(mut parent) = self.controller.clone() else { return };
        let card = source.and_then(|id| self.cards.get(&i64::from(id))).filter(|c| c.is_instance_valid()).cloned();
        let viewport = self.base().get_viewport();
        let screen = viewport.as_ref().map_or(Vector2::new(1280.0, 720.0), |v| v.get_visible_rect().size);
        // Mitte der Karte, wie sie gerade auf dem Bildschirm erscheint (auch angehoben)
        let anchor = card
            .as_ref()
            .and_then(|c| {
                let quad = c.bind().screen_quad();
                (quad.len() == 4).then(|| quad.as_slice().iter().fold(Vector2::ZERO, |a, p| a + *p) * 0.25)
            })
            .unwrap_or(screen * 0.5);
        let art = card.as_ref().and_then(|c| c.get("face_texture").try_to::<Gd<godot::classes::Texture2D>>().ok());
        let segments = entries.iter().filter_map(|(l, _)| WheelSegment::for_letter(*l)).collect();
        let mut wheel = ManaWheel::create(segments, anchor, card.clone(), art, prompt);
        wheel.connect("picked", &Callable::from_object_method(&self.to_gd(), "on_wheel_picked"));
        parent.add_child(&wheel);
        self.wheel = Some((wheel, entries.into_iter().map(|(_, c)| c).collect(), voluntary));
    }

    /// Farbrad schließen (ohne Antwort).
    fn close_wheel(&mut self) {
        if let Some((mut w, _, _)) = self.wheel.take() {
            if w.is_instance_valid() {
                w.bind_mut().close();
            }
        }
    }

    /// Segment `index` des Farbrads angeklickt: die zugehörige Antwort an Forge.
    #[func]
    fn on_wheel_picked(&mut self, index: i64) {
        let choice = self.wheel.as_ref().and_then(|(_, c, _)| c.get(usize::try_from(index).ok()?).cloned());
        self.close_wheel();
        if let Some(c) = choice {
            self.answer(c);
        }
    }

    /// Kann das laufende Wirken / Aktivieren abgebrochen werden? Gibt die Abbrechen-Antwort der
    /// offenen Frage zurück (`PayMana`: `Pay(Cancel)`, sonst `Cancel`), wenn die Engine sie
    /// erlaubt und die Quelle der Frage eine eigene Karte ist, die gerade gewirkt wird (Eintrag
    /// „Casting“ auf dem Stack, CR 601.2) oder deren Fähigkeit gerade aktiviert wird (CR 602.2:
    /// eigene Karte auf dem Tisch, noch kein Stack-Eintrag von ihr).
    fn cast_cancel(&self) -> Option<Choice> {
        let req = self.pending.as_ref()?;
        let choice = match &req.decision {
            Decision::Priority { .. } | Decision::Mulligan { .. } | Decision::ChooseStartingPlayer { .. } => return None,
            Decision::PayMana { options, .. } => options.contains(&PayAction::Cancel).then_some(Choice::Pay(PayAction::Cancel))?,
            _ if req.cancelable => Choice::Cancel,
            _ => return None,
        };
        let src = i64::from(req.source?);
        let items = self.stack_items();
        let casting = items
            .iter()
            .any(|i| get_str(i, "kind") == "Casting" && get_int(i, "source") == src && get_int(i, "controller") == LOCAL_SEAT);
        let mine = self.zones(LOCAL_SEAT as usize);
        let on_my_table = self
            .cards
            .get(&src)
            .and_then(|c| c.bind().get_zone())
            .is_some_and(|z| Some(&z) == mine.battlefield.as_ref() || Some(&z) == mine.resources.as_ref());
        let activating = on_my_table && !items.iter().any(|i| get_int(i, "source") == src);
        (casting || activating).then_some(choice)
    }

    /// ✕-Knopf an die Karte heften, deren Wirken bzw. Aktivieren gerade läuft (oben rechts an
    /// der Karte, zur Kamera gedreht; folgt ihr). Nur ohne abdeckende Bühne.
    fn show_cancel_badge(&mut self) {
        if self.cancel_badge.is_some() || self.stage_covers() || self.cast_cancel().is_none() {
            return;
        }
        let Some(src) = self.pending.as_ref().and_then(|r| r.source) else { return };
        let Some(mut card) = self.cards.get(&i64::from(src)).filter(|c| c.is_instance_valid()).cloned() else { return };
        let scale = card.get_global_transform().basis.col_a().length().max(0.01);
        let height = if card.bind().is_tile() { TILE_HEIGHT } else { CARD_HEIGHT };
        let mut b = Button3D::create_shaped(ButtonStyle::Grey, "✕", CANCEL_BADGE_WIDTH / scale, COMPACT_BODY);
        b.set("face_camera", &true.to_variant());
        b.set_position(Vector3::new(CARD_WIDTH * 0.5, CANCEL_BADGE_LIFT / scale, -height * 0.5));
        b.connect("pressed", &Callable::from_object_method(&self.to_gd(), "cancel_cast"));
        card.add_child(&b);
        self.cancel_badge = Some(b);
    }

    /// Knopf „»»“ (Sofort beenden, nur im eigenen Zug): bis zum Ende dieser Runde automatisch
    /// passen; es wird ohne Angreifer weitergegangen. Andere Entscheidungen (Blocker, Auswahl, Ziele) fragt die
    /// UI weiter. Legt ein anderer Spieler etwas auf den Stack, endet das automatische Passen,
    /// damit man antworten kann.
    #[func]
    pub fn end_turn(&mut self) {
        // nur im eigenen Zug (im Zug des Gegners ist der Knopf ausgegraut)
        if self.state.as_ref().is_none_or(|s| get_int(s, "active_player") != 0) {
            return;
        }
        let turn = self.state.as_ref().map_or(0, |s| get_int(s, "turn"));
        self.auto_pass_turn = Some(turn);
        if let Some(choice) = self.auto_pass_choice() {
            self.answer(choice);
        }
    }

    /// Ist „Sofort beenden“ gerade aktiv?
    #[func]
    pub fn is_ending_turn(&self) -> bool {
        self.auto_pass_turn.is_some()
    }

    /// Eine Option wurde auf dem Tisch gewählt (Klick auf ein gültiges Ziel – Karte oder
    /// Medaillon): `index` = Option im Auswahlfenster. Das HUD behandelt das wie einen Klick auf
    /// diese Option (zurückgestellt verbunden).
    #[signal]
    fn choice_option_clicked(index: i64);

    /// Eingabe aus dem Auswahlfenster des HUD für die offene Entscheidung: eine Option = Index,
    /// mehrere = Array von Indizes, Ja/Nein und Mulligan = bool (true = erste Option / behalten),
    /// Zahl = int, Abbrechen = null. Wird in die passende `Choice` übersetzt; unpassende
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
        // Tisch-Modus von ChooseCards: Indizes der Liste → Karten
        if let Some(options) = self.table_pick(&req.decision) {
            if choice.is_nil() {
                self.answer(Choice::Cancel);
                return;
            }
            let indices: Vec<i64> = match choice.try_to::<VarArray>() {
                Ok(arr) => arr.iter_shared().filter_map(|v| v.try_to::<i64>().ok()).collect(),
                Err(_) => choice.try_to::<i64>().ok().into_iter().collect(),
            };
            let cards: Option<Vec<CardId>> =
                indices.iter().map(|i| usize::try_from(*i).ok().and_then(|i| options.get(i).copied())).collect();
            if let Some(cards) = cards {
                self.answer(Choice::Cards(cards));
            }
            return;
        }
        match decision_view::to_choice(&req.decision, &choice) {
            Some(c) => self.answer(c),
            None => {
                godot_warn!("ForgeTable: Eingabe {choice} passt nicht zu {:?}", req.decision);
            }
        }
    }

    /// Ein Mana-Symbol der Manapool-Anzeige wurde angeklickt (`mana` = Manaart 0 = Weiß … 5 =
    /// Farblos): beim Bezahlen dieses schwebende Mana einsetzen (`PayAction::UsePool`), sonst
    /// nichts.
    #[func]
    pub fn pool_clicked(&mut self, mana: i64) {
        let want = pool_color(mana);
        let Some(Decision::PayMana { options, .. }) = self.pending.as_ref().map(|r| &r.decision) else { return };
        if let Some(pay) = options.iter().find(|o| matches!(o, PayAction::UsePool(c) if Some(*c) == want)) {
            let pay = *pay;
            self.answer(Choice::Pay(pay));
        }
    }

    /// „Fertig“ auf der Hand-Bühne: gewählte Karten an die Engine.
    #[func]
    fn on_hand_pick_done(&mut self) {
        let Some((options, min, max)) = self.pending.as_ref().and_then(|r| self.hand_choice(&r.decision)) else {
            return;
        };
        let n = self.hand_pick.len() as u32;
        if n < min || n > max {
            return;
        }
        for id in &options {
            if let Some(mut c) = self.cards.get(&i64::from(*id)).cloned() {
                c.bind_mut().set_marker(CardMarker::None);
            }
        }
        let pick = std::mem::take(&mut self.hand_pick);
        self.answer(Choice::Cards(pick));
    }

    /// Knopf `index` einer 3D-Bühne (Ja/Nein, Kicker/Normal …): die hinterlegte Antwort an Forge.
    #[func]
    fn on_stage_choice(&mut self, index: i64) {
        let Some(choice) = usize::try_from(index).ok().and_then(|i| self.stage_choices.get(i).cloned()) else { return };
        self.answer(choice);
    }

    /// Modus-Bühne: „Fertig“ (mehrere Modi) – gewählte Modi an Forge.
    #[func]
    fn on_modes_done(&mut self) {
        if matches!(self.pending.as_ref().map(|r| &r.decision), Some(Decision::ChooseOption { .. })) {
            // Karten-Optionen: die gewählte Möglichkeit (Index) an Forge
            if let Some(i) = self.mode_pick.first().copied() {
                self.mode_pick.clear();
                self.answer(Choice::Number(i as u32));
            }
            return;
        }
        let Some(Decision::ChooseModes { min, max, .. }) = self.pending.as_ref().map(|r| r.decision.clone()) else { return };
        let n = self.mode_pick.len() as u32;
        if n >= min && n <= max {
            let pick = std::mem::take(&mut self.mode_pick);
            self.answer(Choice::Modes(pick));
        }
    }

    /// Modus-Bühne: Wahl zurücksetzen (bei wiederholbaren Modi).
    #[func]
    fn on_modes_reset(&mut self) {
        self.mode_pick.clear();
        self.refresh_mode_cards();
    }

    /// Optionale Kosten mit mehreren Möglichkeiten: Option `index` an/aus.
    #[func]
    fn on_option_toggle(&mut self, index: i64) {
        let Some(Decision::ChooseOptions { options, .. }) = self.pending.as_ref().map(|r| r.decision.clone()) else { return };
        let Some(i) = usize::try_from(index).ok().filter(|i| *i < self.option_toggles.len()) else { return };
        self.option_toggles[i] = !self.option_toggles[i];
        let on = self.option_toggles[i];
        let label = options.get(i).map(|o| o.text.clone()).unwrap_or_default();
        for mut b in self.stage_buttons() {
            if b.bind().get_text().to_string().trim_start_matches("✓ ") == label {
                b.bind_mut().set_label_text(GString::from(if on { format!("✓ {label}") } else { label.clone() }.as_str()));
            }
        }
    }

    /// Optionale Kosten mit mehreren Möglichkeiten: „Fertig“ – eingeschaltete Optionen an Forge.
    #[func]
    fn on_options_done(&mut self) {
        let pick: Vec<usize> = self.option_toggles.iter().enumerate().filter(|(_, on)| **on).map(|(i, _)| i).collect();
        self.answer(Choice::Modes(pick));
    }

    /// X-Knöpfe: Wert um `delta` ändern (−5, −1, +1, +5), begrenzt auf 0 … Maximum.
    #[func]
    fn on_x_step(&mut self, delta: i64) {
        let Some((min, max, _)) = self.pending.as_ref().and_then(|r| Self::number_range(&r.decision)) else { return };
        self.x_value = (i64::from(self.x_value) + delta).clamp(i64::from(min), i64::from(max)) as u32;
        self.refresh_x_buttons();
    }

    /// „Bezahlen X = n“ bzw. „Bestätigen: n“: Wert an Forge.
    #[func]
    fn on_x_pay(&mut self) {
        if self.pending.as_ref().and_then(|r| Self::number_range(&r.decision)).is_some() {
            let x = self.x_value;
            self.answer(Choice::Number(x));
        }
    }

    /// „Abbrechen“ auf der Zahlen-Bühne: Wirken zurücknehmen (nur wo Forge es erlaubt).
    #[func]
    fn on_x_cancel(&mut self) {
        if self.pending.as_ref().is_some_and(|r| r.cancelable && Self::number_range(&r.decision).is_some()) {
            self.answer(Choice::Cancel);
        }
    }

    /// Hellsicht/Überwachen: „Fertig“ – Karten im Feld „oben“ (die rechte Karte zuoberst) und
    /// „unten“ bzw. Friedhof an Forge.
    #[func]
    fn on_scry_done(&mut self) {
        let Some((bottom, top)) = self.scry_zones.clone() else { return };
        let ids = |z: &Gd<Zone>| -> Vec<CardId> {
            z.bind().get_cards().iter_shared().filter_map(|c| self.pick_cards.iter().find(|(k, _)| *k == c).map(|(_, id)| *id)).collect()
        };
        let mut top_ids = ids(&top);
        top_ids.reverse();
        let bottom_ids = ids(&bottom);
        // die Karten fliegen beim Schließen zum Bibliotheksstapel (bzw. zum Friedhof)
        let graveyard = matches!(self.pending.as_ref().map(|r| &r.decision), Some(Decision::Scry { graveyard: true, .. }));
        let target = if graveyard && top_ids.is_empty() {
            self.zones(LOCAL_SEAT as usize).discard.map(|z| z.get_global_transform())
        } else {
            self.deck_cards[LOCAL_SEAT as usize].last().filter(|c| c.is_instance_valid()).map(|c| c.get_global_transform())
                .or_else(|| self.zones(LOCAL_SEAT as usize).deck.map(|z| z.get_global_transform()))
        };
        if let (Some(t), Some(mut sc)) = (target, self.showcase.clone().filter(|s| s.is_instance_valid())) {
            sc.bind_mut().set_fly_target(t);
        }
        self.answer(Choice::Arrange { top: top_ids, bottom: bottom_ids });
    }

    /// Reihenfolge-Bühne: „Fertig“ – vergebene Reihenfolge an Forge.
    #[func]
    fn on_order_done(&mut self) {
        let Some(Decision::OrderTriggers { descriptions, .. }) = self.pending.as_ref().map(|r| r.decision.clone()) else { return };
        if self.order_pick.len() == descriptions.len() {
            let order = std::mem::take(&mut self.order_pick);
            self.answer(Choice::Order(order));
        }
    }

    /// Reihenfolge-Bühne: alle Nummern löschen.
    #[func]
    fn on_order_reset(&mut self) {
        self.order_pick.clear();
        self.refresh_order_cards();
    }

    /// Kartenwahl auf der Bühne: „Fertig“ (mehrere Karten) bzw. „Ablehnen“ (keine).
    #[func]
    fn on_pick_done(&mut self, decline: bool) {
        let Some(Decision::ChooseCards { min, max, .. }) = self.pending.as_ref().map(|r| r.decision.clone()) else { return };
        let pick = if decline { Vec::new() } else { self.pick_chosen.clone() };
        let n = pick.len() as u32;
        if n >= min && n <= max {
            self.answer(Choice::Cards(pick));
        }
    }

    /// Kartenansicht für `card` zeigen wie beim Hovern (für Skripte und Tests; null = wieder der
    /// Karte unter der Maus folgen).
    #[func]
    pub fn inspect_card(&mut self, card: Option<Gd<Card3D>>) {
        self.inspect_override = card;
    }

    /// „Abbrechen“ auf der Hand-Bühne (nur wo Forge das erlaubt).
    #[func]
    fn on_hand_pick_cancel(&mut self) {
        self.hand_pick.clear();
        self.answer(Choice::Cancel);
    }

    /// Ein Mulligan-Knopf wurde gedrückt: `keep` = Starthand behalten, sonst Mulligan.
    #[func]
    fn on_mulligan_pressed(&mut self, keep: bool) {
        if matches!(self.pending.as_ref().map(|r| &r.decision), Some(Decision::Mulligan { .. })) {
            self.answer(if keep { Choice::Keep } else { Choice::Mulligan });
        }
    }

    /// Ein Startspieler-Knopf wurde gedrückt (`index` = Eintrag in `players` der Entscheidung
    /// `ChooseStartingPlayer`).
    #[func]
    fn on_starting_player_pressed(&mut self, index: i64) {
        let Some(Decision::ChooseStartingPlayer { players, .. }) = self.pending.as_ref().map(|r| &r.decision) else {
            return;
        };
        if let Ok(i) = usize::try_from(index) {
            if i < players.len() {
                self.answer(Choice::Number(i as u32));
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
            self.pick_candidate(i);
        }
    }


    /// Aktuelle Auswahl im Auswahlfenster (vom HUD jedes Frame gemeldet): gewählte Ziele werden
    /// auf dem Tisch golden markiert und bekommen einen Pfeil.
    #[func]
    pub fn set_choice_selection(&mut self, selection: PackedInt64Array) {
        // Zielwahl läuft ohne Auswahlfenster: die Auswahl verwaltet der ForgeTable selbst
        if matches!(self.pending.as_ref().map(|r| &r.decision), Some(Decision::ChooseTargets { .. })) {
            return;
        }
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

    /// Hat der Spieler gerade eine offene Entscheidung? Liefert den Namen der `Decision`
    /// (z. B. "Priority", "Mulligan", "ChooseTargets", "ChooseOption") oder einen leeren Text.
    #[func]
    pub fn get_pending_kind(&self) -> GString {
        GString::from(self.pending_kind().unwrap_or_default().as_str())
    }

    /// Brücke bereit → Partie starten (bei [member autostart]).
    #[func]
    fn on_bridge_ready(&mut self, _protocol: i64) {
        let hosted = self.bridge.as_ref().is_some_and(|b| b.bind().is_hosted());
        if hosted {
            // Eingebettet (NeoForge): die Partie startet der Gastgeber
            self.winner.clear();
            self.match_running = true;
        } else if self.autostart {
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
    fn on_forge_event(&mut self, event: VarDictionary) {
        self.dirty = true;
        match get_str(&event, "event").as_str() {
            // Land/Quelle getappt: leuchtet orange auf, ist Quelle des nächsten Manas
            "GameEventCardTapped" if event.get("tapped").is_some_and(|v| v.booleanize()) => {
                let id = get_int(&event, "card");
                if let Some(mut c) = self.cards.get(&id).cloned() {
                    c.bind_mut().flash();
                }
                self.recent_taps.push_back((id, self.clock));
                while self.recent_taps.len() > 16 {
                    self.recent_taps.pop_front();
                }
            }
            "GameEventManaPool" if get_str(&event, "mode") == "Added" => self.spawn_mana_fx(&event),
            "GameEventPlayerDamaged" if event.get("combat").is_some_and(|v| v.booleanize()) => {
                let seat = get_int(&event, "player");
                let point = u8::try_from(seat).ok().and_then(|s| self.hero_anchor(s));
                self.combat_hit(&event, None, point, 0.85);
            }
            "GameEventCardDamaged" if get_str(&event, "phase").starts_with("COMBAT") => {
                let target = self.cards.get(&get_int(&event, "card")).cloned();
                let point = target.as_ref().map(|c| c.get_global_position());
                self.combat_hit(&event, target, point, 0.5);
            }
            _ => {}
        }
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
        self.attack_plan.clear();
        self.block_plan.clear();
        self.combat_armed = None;
        let starting = matches!(req.decision, Decision::ChooseStartingPlayer { .. }).then(|| req.clone());
        let mulligan = matches!(req.decision, Decision::Mulligan { .. }).then(|| req.clone());
        let hand_pick = self.hand_choice(&req.decision).is_some().then(|| req.clone());
        let choose_x = Self::number_range(&req.decision).is_some().then(|| req.clone());
        let player_pick = Self::player_pick(&req.decision).is_some().then(|| req.clone());
        let stage_3d = Self::stage_kind(&req.decision).map(|k| (k, req.clone()));
        let library_pick = (matches!(req.decision, Decision::ChooseCards { .. })
            && self.table_pick(&req.decision).is_none()
            && self.hand_choice(&req.decision).is_none())
        .then(|| req.clone());
        let auto_pay = self.auto_pay(&req.decision);
        self.pending = Some(req);
        if auto_pay {
            self.answer(Choice::Pay(PayAction::AutoPay));
            return;
        }
        if let Some(choice) = self.auto_pass_choice() {
            self.answer(choice);
            return;
        }
        // Genau ein mögliches Ziel und genau eins verlangt: keine echte Wahl – sofort nehmen
        // (der Pfeil erscheint dann am Stack-Eintrag)
        if let Some(Decision::ChooseTargets { candidates, min: 1, max: 1, .. }) = self.pending.as_ref().map(|r| &r.decision) {
            if candidates.len() == 1 {
                let only = candidates[0];
                self.answer(Choice::Targets(vec![only]));
                return;
            }
        }
        if let Some(req) = starting {
            self.show_starting_player(&req);
        }
        if let Some(req) = mulligan {
            self.show_mulligan(&req);
        }
        if let Some(req) = hand_pick {
            self.show_hand_pick(&req);
        }
        if let Some(req) = player_pick {
            self.show_player_pick(&req);
        }
        if let Some(req) = choose_x {
            self.show_choose_x(&req);
        }
        if let Some((kind, req)) = stage_3d {
            match kind {
                StageKind::Modes => self.show_modes(&req),
                StageKind::Binary => self.show_binary(&req),
                StageKind::OptionalCost => self.show_optional_cost(&req),
                StageKind::Scry => self.show_scry(&req),
                StageKind::Order => self.show_order(&req),
                StageKind::CardOptions => self.show_card_options(&req),
            }
        }
        if let Some(req) = library_pick {
            self.show_library_pick(&req);
        }
        if let Some(req) = self.pending.clone() {
            if let Some(entries) = Self::wheel_entries(&req.decision) {
                let prompt = Self::wheel_prompt(&req, &req.decision);
                self.open_wheel(entries, req.source, &prompt, false);
            }
        }
        self.show_cancel_badge();
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
        // Kopie in der Friedhof-/Exil-Ansicht: hat die Karte eine Aktion, Ansicht schließen und
        // wie ein Klick auf die Karte selbst behandeln
        if let Some(id) = self.viewer_cards.iter().find(|(c, _)| *c == card).map(|(_, id)| *id) {
            if !self.card_actions(id).is_empty() {
                self.close_zone_viewer();
                self.card_action_click(id, None);
            }
            return;
        }
        // freiwilliges Farbrad offen: Klick daneben schließt es
        if self.wheel.as_ref().is_some_and(|(_, _, v)| *v) {
            self.close_wheel();
        }
        if let Some(id) = self.pick_cards.iter().find(|(c, _)| *c == card).map(|(_, id)| *id) {
            if self.scry_zones.is_none() {
                self.library_click(id);
            } else {
                self.scry_click(card);
            }
            return;
        }
        if let Some(i) = self.order_cards.iter().find(|(c, _)| *c == card).map(|(_, i)| *i) {
            self.order_click(i);
            return;
        }
        if let Some(mode) = self.mode_cards.iter().find(|(c, _)| *c == card).map(|(_, m)| *m) {
            self.mode_click(mode);
            return;
        }
        if let Some(action) = self.action_options.iter().find(|(c, _)| *c == card).map(|(_, a)| *a) {
            self.answer(Choice::Action(action));
            return;
        }
        let Some(id) = self.forge_id(&card) else { return };
        let Ok(card_id) = u32::try_from(id) else { return };
        if self.hand_pick_click(card_id) {
            return;
        }
        if self.combat_click(card_id) {
            self.update_glow();
            return;
        }
        if let Some(i) = self.candidate_index(Target::Card(card_id)) {
            self.pick_candidate(i);
            return;
        }
        // Friedhof / Exil: ohne eigene Aktion der Karte (z. B. Rückblende) durchsehen
        if let Some((seat, exile)) = self.pile_of(&card) {
            if self.card_actions(card_id).is_empty() && !self.stage_open_now() {
                self.show_zone_viewer(seat, exile);
                return;
            }
        }
        self.card_action_click(card_id, Some(card));
    }

    /// Aktionen der Karte `card_id` ausführen (Klick auf eine Karte außerhalb der Hand bzw. auf
    /// ihre Kopie in der Friedhof-/Exil-Ansicht): eine Aktion sofort, mehrere über „Wähle eins“,
    /// Manafähigkeiten über das Farbrad bzw. Menü.
    fn card_action_click(&mut self, card_id: CardId, card: Option<Gd<Card3D>>) {
        let card = card.or_else(|| self.cards.get(&i64::from(card_id)).cloned());
        let in_hand = card.as_ref().is_some_and(|c| self.zones(0).hand.is_some_and(|h| c.bind().get_zone().as_ref() == Some(&h)));
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
        let all_actions = mine.iter().all(|(c, _)| matches!(c, Choice::Action(_)));
        match mine.len() {
            0 => {}
            1 => self.answer(mine[0].0.clone()),
            // mehrere Fähigkeiten: „Wähle eins“ auf der 3D-Bühne (Manafähigkeiten: Menü)
            // mehrere Manafähigkeiten mit je einer Farbe: Farbrad an der Karte
            _ if Self::mana_wheel_entries(&mine).is_some() => {
                let entries = Self::mana_wheel_entries(&mine).unwrap_or_default();
                let name = card.as_ref().map(|c| c.bind().card_name().to_string()).unwrap_or_default();
                let prompt = format!("({name}) {}", tr!("Wähle das Mana.", "Choose the mana."));
                let voluntary = !matches!(self.pending.as_ref().map(|r| &r.decision), Some(Decision::PayMana { .. }));
                self.open_wheel(entries, Some(card_id), &prompt, voluntary);
            }
            _ if all_actions => self.show_action_choice(card_id),
            _ => self.menu = Some(mine.into_iter().unzip()),
        }
    }

    /// Knopf „Abbrechen“ bei „Wähle eins“: Bühne schließen, die Priorität bleibt offen.
    #[func]
    fn on_action_choice_cancel(&mut self) {
        self.clear_choice_objects();
    }

    /// Pfeil losgelassen (vom [TableController]): Angreifer → Spieler/Planeswalker = angreifen,
    /// Blocker → Angreifer = blocken, Quelle der Zielwahl → gültiges Ziel = Ziel wählen. Ins Leere
    /// losgelassen nimmt den Angriff bzw. Block dieser Kreatur zurück.
    #[func]
    fn on_aim_released(&mut self, card: Gd<Card3D>, target: Option<Gd<Card3D>>) {
        let Some(id) = self.forge_id(&card).and_then(|i| u32::try_from(i).ok()) else { return };
        // die Karte, die gerade gewirkt wird, zurück auf die eigene Hand gezogen: abbrechen
        if target.is_none() && self.pending.as_ref().and_then(|r| r.source) == Some(id) && self.cast_cancel().is_some() {
            let hand = self.zones(LOCAL_SEAT as usize).hand;
            let over = self.controller.as_ref().and_then(|c| c.bind().zone_under_mouse());
            if hand.is_some() && over == hand {
                self.cancel_cast();
                return;
            }
        }
        let target = self.aim_target(target);
        let Some((kind, valid)) = self.aim_kind(id) else { return };
        let mut target = target.filter(|t| valid.contains(t));
        // nur ein Angriffsziel: Ziehen heißt einfach angreifen
        if kind == AimKind::Attack && valid.len() == 1 && target.is_none() {
            target = valid.first().copied();
        }
        match kind {
            AimKind::Attack => {
                self.attack_plan.retain(|(c, _)| *c != id);
                if let Some(t) = target {
                    self.attack_plan.push((id, t));
                    self.drawn_arrows.insert(arrow_key(ArrowColor::Orange as u64, i64::from(id), t));
                }
                self.combat_armed = None;
            }
            AimKind::Block => {
                self.block_plan.retain(|(b, _)| *b != id);
                if let Some(Target::Card(a)) = target {
                    self.block_plan.push((id, a));
                    self.drawn_arrows.insert(arrow_key(ArrowColor::Blue as u64, i64::from(id), Target::Card(a)));
                }
                self.combat_armed = None;
            }
            AimKind::Target => {
                if let Some(t) = target {
                    if let Some(i) = self.candidate_index(t) {
                        self.drawn_arrows.insert(arrow_key(20, i64::from(id), t));
                        self.pick_candidate(i);
                    }
                }
            }
        }
        self.update_glow();
    }

    /// Karte wurde auf dem Tisch abgelegt: Hat sie eine Aktion in der offenen `Priority`-
    /// Entscheidung (aus der Hand: Land spielen / wirken; auf dem Spielfeld: Fähigkeit
    /// aktivieren), wird diese gewählt. Aus der Hand zurück auf die Hand = abbrechen. Alles andere
    /// wird auf den Engine-Stand zurückgesetzt.
    #[func]
    fn on_card_dropped(&mut self, card: Gd<Card3D>, from_zone: Gd<Zone>, to_zone: Gd<Zone>) {
        // Hellsicht: Karten werden zwischen „unten“ und „oben“ verschoben – nur Anzeige
        if let Some((bottom, top)) = self.scry_zones.as_ref() {
            if [bottom, top].contains(&&to_zone) || [bottom, top].contains(&&from_zone) {
                return;
            }
        }
        let hand = self.zones(0).hand;
        let back_to_hand = hand.as_ref() == Some(&from_zone) && hand.as_ref() == Some(&to_zone);
        let id = self.forge_id(&card);
        if !back_to_hand {
            // mehrere Spielweisen (z. B. Wirken oder Cycling): „Wähle eins“; die Karte geht
            // solange zurück an ihren Platz
            if let Some(cid) = id.and_then(|i| u32::try_from(i).ok()).filter(|c| self.card_actions(*c).len() > 1) {
                if let Some(state) = self.state.clone() {
                    self.apply_state(state);
                }
                self.show_action_choice(cid);
                return;
            }
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
    exile: Option<Gd<Zone>>,
    command: Option<Gd<Zone>>,
    resources: Option<Gd<Zone>>,
}

impl ForgeTable {
    /// Startspieler wählen (CR 103.1) in 3D: je Sitzplatz ein Knopf über der Tischmitte,
    /// untereinander – der eigene orange und oben, die der anderen blau –, darüber die Frage als
    /// 3D-Text. „Untereinander“ gilt auf dem Bildschirm (Hochachse der Kamera).
    fn show_starting_player(&mut self, req: &Request) {
        let Decision::ChooseStartingPlayer { players, player } = &req.decision else { return };
        self.clear_choice_objects();
        let Some(mut parent) = self.controller.clone().map(|c| c.upcast::<Node>()) else { return };
        // Bildschirm-Hochrichtung in der Welt (Kamera schaut schräg auf den Tisch)
        let up = self
            .base()
            .get_viewport()
            .and_then(|v| v.get_camera_3d())
            .map_or(Vector3::UP, |c| c.get_global_transform().basis.col_b().normalized());
        // eigener Sitz zuerst (oben), dann die anderen in Sitzreihenfolge
        let mut order: Vec<usize> = (0..players.len()).collect();
        order.sort_by_key(|i| players[*i] != *player);
        let n = players.len().max(1) as f32;
        for (row, i) in order.into_iter().enumerate() {
            let seat = &players[i];
            let own = seat == player;
            let style = if own { ButtonStyle::Orange } else { ButtonStyle::Blue };
            let name = Some(req.label(i)).filter(|l| !l.is_empty()).unwrap_or_else(|| self.seat_name(*seat as usize).to_string());
            let text = if own { tr!("Ich beginne", "I go first").to_string() } else { trf!("{name} beginnt", "{name} goes first") };
            let mut button = Button3D::create(style, &text, STARTING_BUTTON_WIDTH);
            let offset = ((n - 1.0) * 0.5 - row as f32) * STARTING_BUTTON_SPACING;
            parent.add_child(&button);
            button.set_position(STARTING_CHOICE_CENTER + up * offset);
            let cb = Callable::from_object_method(&self.to_gd(), "on_starting_player_pressed").bind(&[(i as i64).to_variant()]);
            button.connect("pressed", &cb);
            self.choice_buttons.push(button);
        }
        let mut prompt = Label3D::new_alloc();
        prompt.set_text(tr!("Wer beginnt?", "Who goes first?"));
        prompt.set_billboard_mode(godot::classes::base_material_3d::BillboardMode::ENABLED);
        prompt.set_font_size(96);
        prompt.set_pixel_size(0.0055);
        prompt.set_outline_size(16);
        prompt.set_modulate(Color::from_rgb(0.98, 0.84, 0.5));
        prompt.set_outline_modulate(Color::from_rgba(0.05, 0.03, 0.02, 0.9));
        prompt.set_draw_flag(godot::classes::label_3d::DrawFlags::SHADED, false);
        parent.add_child(&prompt);
        prompt.set_position(STARTING_CHOICE_CENTER + up * ((n - 1.0) * 0.5 * STARTING_BUTTON_SPACING + 0.85));
        self.choice_prompt = Some(prompt);
    }

    /// Spieler wählen für einen Effekt ohne Ziel (`purpose == "Choose"`, z. B. „wähle einen
    /// Spieler“, CR 115.1), bei der nur Spieler zur Wahl stehen und genau einer gewählt wird:
    /// `(Sitzplätze, darf leer bleiben)`. Echte Ziele (CR 115, z. B. Lightning Bolt „any target“,
    /// „target player“) wählt man immer nur mit dem Pfeil – keine Knöpfe.
    fn player_pick(decision: &Decision) -> Option<(Vec<PlayerId>, bool)> {
        let Decision::ChooseTargets { candidates, min, max, purpose, .. } = decision else { return None };
        if purpose != "Choose" || *max != 1 || candidates.len() < 2 {
            return None;
        }
        let seats: Vec<PlayerId> = candidates
            .iter()
            .map(|t| match t {
                Target::Player(p) => Some(*p),
                _ => None,
            })
            .collect::<Option<_>>()?;
        Some((seats, *min == 0))
    }

    /// Spieler wählen (nur Effekte ohne Ziel, siehe `player_pick`) wie die Startspieler-Wahl, ohne Karte: über dem Tisch je Gegner ein
    /// **oranger** Knopf mit seinem Namen (oben), darunter der **blaue** Knopf für den eigenen
    /// Spieler („Ich selbst“); darf man keinen wählen, zuletzt grau „Keinen Spieler“. Darüber
    /// „(Karte) Wähle einen Spieler.“. Medaillons und Pfeil funktionieren weiter; Abbrechen über den Phasen-Knopf.
    fn show_player_pick(&mut self, req: &Request) {
        let Some((seats, optional)) = Self::player_pick(&req.decision) else { return };
        self.clear_choice_objects();
        let Some(mut parent) = self.controller.clone().map(|c| c.upcast::<Node>()) else { return };
        let up = self
            .base()
            .get_viewport()
            .and_then(|v| v.get_camera_3d())
            .map_or(Vector3::UP, |c| c.get_global_transform().basis.col_b().normalized());
        // Gegner zuerst (oben, orange), dann der eigene Spieler (blau)
        let me = LOCAL_SEAT as PlayerId;
        let mut rows: Vec<(ButtonStyle, String, Choice)> = seats
            .iter()
            .filter(|s| **s != me)
            .map(|s| (ButtonStyle::Orange, self.seat_name(*s as usize).to_string(), Choice::Targets(vec![Target::Player(*s)])))
            .collect();
        if seats.contains(&me) {
            rows.push((ButtonStyle::Blue, tr!("Ich selbst", "Myself").to_string(), Choice::Targets(vec![Target::Player(me)])));
        }
        if optional {
            rows.push((ButtonStyle::Grey, tr!("Keinen Spieler", "No player").to_string(), Choice::Targets(Vec::new())));
        }
        let n = rows.len().max(1) as f32;
        for (row, (style, text, choice)) in rows.into_iter().enumerate() {
            let i = self.stage_choices.len() as i64;
            self.stage_choices.push(choice);
            let mut button = Button3D::create(style, &text, STARTING_BUTTON_WIDTH);
            let offset = ((n - 1.0) * 0.5 - row as f32) * STARTING_BUTTON_SPACING;
            parent.add_child(&button);
            button.set_position(STARTING_CHOICE_CENTER + up * offset);
            button.connect("pressed", &Callable::from_object_method(&self.to_gd(), "on_stage_choice").bind(&[i.to_variant()]));
            self.choice_buttons.push(button);
        }
        // Forges Beschreibung ist englisch („Select target player“): eigener Text mit Quelle
        let what = tr!("Wähle einen Spieler.", "Choose a player.");
        let text = if req.source_name.is_empty() { what.to_string() } else { format!("({}) {what}", req.source_name) };
        let mut prompt = Label3D::new_alloc();
        prompt.set_text(&text);
        prompt.set_billboard_mode(godot::classes::base_material_3d::BillboardMode::ENABLED);
        prompt.set_font_size(96);
        prompt.set_pixel_size(0.0045);
        prompt.set_outline_size(16);
        prompt.set_modulate(Color::from_rgb(0.98, 0.84, 0.5));
        prompt.set_outline_modulate(Color::from_rgba(0.05, 0.03, 0.02, 0.9));
        prompt.set_draw_flag(godot::classes::label_3d::DrawFlags::SHADED, false);
        parent.add_child(&prompt);
        prompt.set_position(STARTING_CHOICE_CENTER + up * ((n - 1.0) * 0.5 * STARTING_BUTTON_SPACING + 0.85));
        self.choice_prompt = Some(prompt);
    }

    /// Mulligan (CR 103.5) in 3D: die Starthand groß auf der Bühne vor der Kamera (Tisch
    /// abgedunkelt), darüber wer beginnt und die Frage, darunter „Mulligan“ (blau, links) und
    /// „Behalten“ (orange, rechts), ggf. der Hinweis, wie viele Karten danach unter die Bibliothek
    /// gehen.
    fn show_mulligan(&mut self, req: &Request) {
        let Decision::Mulligan { mulligans_taken, first_player, .. } = &req.decision else { return };
        let taken = *mulligans_taken;
        let first = first_player.map_or(-1, i64::from);
        self.clear_choice_objects();
        let Some(mut parent) = self.controller.clone().map(|c| c.upcast::<Node>()) else { return };
        let mut showcase = Showcase::new_alloc();
        parent.add_child(&showcase);
        showcase.bind_mut().open();

        let title = match first {
            s if s == LOCAL_SEAT => tr!("Du beginnst", "You go first").to_string(),
            s if s >= 0 => {
                let name = self.seat_name(s as usize);
                trf!("{name} beginnt", "{name} goes first")
            }
            _ => tr!("Starthand", "Opening hand").to_string(),
        };
        let subtitle = if taken == 0 {
            tr!("Starthand behalten oder Mulligan?", "Keep your opening hand or mulligan?").to_string()
        } else {
            trf!("Mulligan {taken}: behalten oder noch einmal?", "Mulligan {taken}: keep or mulligan again?")
        };
        // London-Mulligan (Forge): neue 7 Karten, sofort danach so viele unter die Bibliothek,
        // wie man schon Mulligans genommen hat
        let next = taken + 1;
        let note = trf!(
            "Mulligan: 7 neue Karten, danach {next} unter die Bibliothek.",
            "Mulligan: draw 7 new cards, then put {next} on the bottom of your library."
        );
        showcase.bind_mut().set_texts(GString::from(title.as_str()), GString::from(subtitle.as_str()), GString::from(note.as_str()));

        let hand: Vec<Gd<Card3D>> = self
            .zones(LOCAL_SEAT as usize)
            .hand
            .map(|h| h.bind().get_cards().iter_shared().collect())
            .unwrap_or_default();
        showcase.bind_mut().show_cards(&hand);

        let width = Showcase::button_width();
        for (style, text, keep) in [
            (ButtonStyle::Blue, tr!("Mulligan", "Mulligan"), false),
            (ButtonStyle::Orange, tr!("Behalten", "Keep"), true),
        ] {
            let mut button = Button3D::create(style, text, width);
            let cb = Callable::from_object_method(&self.to_gd(), "on_mulligan_pressed").bind(&[keep.to_variant()]);
            button.connect("pressed", &cb);
            showcase.bind_mut().add_button(button);
        }
        self.showcase = Some(showcase);
    }

    /// Welche 3D-Bühne zeigt diese Entscheidung (statt eines 2D-Fensters)?
    fn stage_kind(decision: &Decision) -> Option<StageKind> {
        match decision {
            Decision::ChooseModes { .. } => Some(StageKind::Modes),
            Decision::Confirm { .. }
            | Decision::OptionalTrigger { .. }
            | Decision::PayUnlessCost { .. }
            | Decision::PayLifeForPhyrexian { .. } => Some(StageKind::Binary),
            Decision::ChooseOption { options, .. } if options.len() >= 2 && options.iter().all(|o| o.card.is_some()) => {
                Some(StageKind::CardOptions)
            }
            Decision::ChooseOption { options, .. } if options.len() == 2 => Some(StageKind::Binary),
            Decision::ChooseOptions { purpose, .. } if purpose == "OptionalCost" => Some(StageKind::OptionalCost),
            Decision::Scry { .. } => Some(StageKind::Scry),
            Decision::OrderTriggers { .. } => Some(StageKind::Order),
            _ => None,
        }
    }

    /// Kartendaten der Karte, um die es geht (Quelle der Anfrage; auch aus Hand oder Stack).
    fn source_data(&self, req: &Request) -> Option<VarDictionary> {
        let id = i64::from(req.source?);
        self.card_data(id).or_else(|| req.cards.iter().find(|c| get_int(c, "id") == id).cloned())
    }

    /// Kartenkopie für eine Bühne aus Kartendaten (mit verändertem Regeltext `text`, falls gesetzt).
    fn stage_card(data: &VarDictionary, text: Option<&str>) -> Gd<Card3D> {
        let mut def = CardDef::from_state(data);
        if let Some(t) = text {
            // eigener Text (ein Modus, ein Effekt): echtes Kartenbild, aber nur dieser Text auf
            // einem Feld über dem gedruckten Textfeld (wie im Video „Spree und Stack“)
            def.text = t.to_string();
            def.custom_text = true;
        }
        let mut copy = Card3D::new_alloc();
        {
            let mut c = copy.bind_mut();
            c.set_face_texture(Some(placeholder::card_face(&def)));
            c.apply_def(&def);
        }
        copy
    }

    /// Leere Bühne mit Titel und Untertitel öffnen.
    fn open_stage(&mut self, title: &str, subtitle: &str) -> Option<Gd<Showcase>> {
        self.clear_choice_objects();
        let mut parent = self.controller.clone()?.upcast::<Node>();
        let mut showcase = Showcase::new_alloc();
        parent.add_child(&showcase);
        showcase.bind_mut().open();
        showcase.bind_mut().set_texts(GString::from(title), GString::from(subtitle), GString::new());
        self.showcase = Some(showcase.clone());
        Some(showcase)
    }

    /// Knopf auf der Bühne, der beim Drücken `choice` an Forge schickt.
    fn stage_button(&mut self, showcase: &mut Gd<Showcase>, style: ButtonStyle, text: &str, width: f32, choice: Choice) {
        let i = self.stage_choices.len() as i64;
        self.stage_choices.push(choice);
        let mut b = Button3D::create(style, text, width);
        let cb = Callable::from_object_method(&self.to_gd(), "on_stage_choice").bind(&[i.to_variant()]);
        b.connect("pressed", &cb);
        showcase.bind_mut().add_button(b);
    }

    /// Alle Knöpfe der offenen Bühne.
    fn stage_buttons(&self) -> Vec<Gd<Button3D>> {
        self.showcase
            .as_ref()
            .filter(|s| s.is_instance_valid())
            .map(|s| s.get_children().iter_shared().filter_map(|c| c.try_cast::<Button3D>().ok()).collect())
            .unwrap_or_default()
    }

    /// Modus wählen (CR 700.2): je Modus eine Kopie der Karte, auf der nur dieser Modus steht.
    /// Ein Modus: Klick wählt sofort. Mehrere: Klick wählt (orange) bzw. hebt auf, „Fertig (n/…)“;
    /// wiederholbare Modi zählen je Klick hoch („×2“), „Zurücksetzen“ leert die Wahl.
    /// Modi mit eigenen Zusatzkosten (Spree CR 702.172, Tiered; Video „Spree und Stack“): Titel =
    /// Schlüsselwort, darunter „Wähle eine oder mehrere Zusatzkosten.“, über jeder Kopie „+ {Kosten}“,
    /// orange „Bezahlen {Grundkosten + gewählte}“ (erst bei gültiger Wahl aktiv) und „Abbrechen“.
    fn show_modes(&mut self, req: &Request) {
        let Decision::ChooseModes { modes, min, max, can_repeat, costs, keyword, .. } = &req.decision else { return };
        let (min, max, can_repeat) = (*min, *max, *can_repeat);
        let with_costs = Self::mode_costs(&req.decision);
        let data = self.source_data(req).unwrap_or_default();
        let title = if with_costs && !keyword.is_empty() {
            keyword.clone()
        } else if with_costs {
            tr!("Zusatzkosten wählen", "Choose additional costs").to_string()
        } else if min == max && max == 1 {
            tr!("Wähle einen Modus", "Choose one").to_string()
        } else if min == max {
            trf!("Wähle {max} Modi", "Choose {max}")
        } else {
            trf!("Wähle {min}–{max} Modi", "Choose {min}–{max}")
        };
        let subtitle = if !with_costs {
            req.source_name.clone()
        } else if max == 1 {
            tr!("Wähle eine der Zusatzkosten.", "Choose one additional cost.").to_string()
        } else {
            tr!("Wähle eine oder mehrere Zusatzkosten.", "Choose one or more additional costs.").to_string()
        };
        let Some(mut showcase) = self.open_stage(&title, &subtitle) else { return };
        let mut cards = Vec::new();
        for (n, (index, text)) in modes.iter().enumerate() {
            let mut copy = Self::stage_card(&data, Some(text));
            copy.bind_mut().set_ready_color(Some(PLAYABLE_GLOW));
            // Modus-Kosten über der Kopie („+ {3}“), sonst „Modus n“
            let caption = match costs.get(n).filter(|c| with_costs && !c.is_empty()) {
                Some(c) => format!("+ {c}"),
                None => trf!("Modus {}", "Mode {}", n + 1),
            };
            cards.push((copy.clone(), caption));
            self.mode_cards.push((copy, *index));
        }
        // vor dem Einordnen: die Reihe rückt nach links, wenn der Zauber am Stack steht
        self.stage_spell_at_stack(&mut showcase, req, &data);
        showcase.bind_mut().show_options(cards);
        showcase.bind_mut().enable_scroll();
        if with_costs {
            // wie im Video „Spree und Stack“: orange „Bezahlen {Gesamtkosten}“, daneben „Abbrechen“
            let mut pay = Button3D::create(ButtonStyle::Orange, &self.mode_pay_label(), Showcase::button_width() * 1.2);
            pay.bind_mut().set_enabled(min == 0);
            let cb = Callable::from_object_method(&self.to_gd(), "on_modes_done");
            pay.connect("pressed", &cb);
            showcase.bind_mut().add_button(pay);
            if req.cancelable {
                self.stage_button(&mut showcase, ButtonStyle::Blue, tr!("Abbrechen", "Cancel"), Showcase::button_width(), Choice::Cancel);
            }
            return;
        }
        if req.cancelable {
            self.stage_button(&mut showcase, ButtonStyle::Blue, tr!("Abbrechen", "Cancel"), Showcase::button_width(), Choice::Cancel);
        }
        if max > 1 {
            if can_repeat {
                let mut reset = Button3D::create(ButtonStyle::Blue, tr!("Zurücksetzen", "Reset"), Showcase::button_width());
                let cb = Callable::from_object_method(&self.to_gd(), "on_modes_reset");
                reset.connect("pressed", &cb);
                showcase.bind_mut().add_button(reset);
            }
            let mut done = Button3D::create(ButtonStyle::Orange, &pick_label(0, min, max), Showcase::button_width() * 1.2);
            done.bind_mut().set_enabled(min == 0);
            let cb = Callable::from_object_method(&self.to_gd(), "on_modes_done");
            done.connect("pressed", &cb);
            showcase.bind_mut().add_button(done);
        }
    }

    /// Klick auf eine Modus-Karte.
    fn mode_click(&mut self, mode: usize) {
        let Some(decision) = self.pending.as_ref().map(|r| r.decision.clone()) else { return };
        if matches!(decision, Decision::ChooseOption { .. }) {
            // Karten-Optionen: Klick wählt genau eine (erneuter Klick hebt auf)
            let again = self.mode_pick.contains(&mode);
            self.mode_pick.clear();
            if !again {
                self.mode_pick.push(mode);
            }
            self.refresh_mode_cards();
            return;
        }
        let Decision::ChooseModes { max, can_repeat, .. } = decision else { return };
        let with_costs = Self::mode_costs(&decision);
        if max <= 1 && !with_costs {
            self.answer(Choice::Modes(vec![mode]));
            return;
        }
        if max <= 1 {
            // eine Zusatzkosten (Tiered): Klick tauscht die Wahl, bezahlt wird mit dem Knopf
            let again = self.mode_pick.contains(&mode);
            self.mode_pick.clear();
            if !again {
                self.mode_pick.push(mode);
            }
            self.refresh_mode_cards();
            return;
        }
        let picked = self.mode_pick.iter().filter(|m| **m == mode).count();
        if can_repeat {
            if self.mode_pick.len() < max as usize {
                self.mode_pick.push(mode);
            }
        } else if picked > 0 {
            self.mode_pick.retain(|m| *m != mode);
        } else if self.mode_pick.len() < max as usize {
            self.mode_pick.push(mode);
        }
        self.refresh_mode_cards();
    }

    /// Modus-Karten markieren (orange = gewählt), Zähler im Fertig-Knopf.
    fn refresh_mode_cards(&mut self) {
        for (card, mode) in self.mode_cards.iter_mut() {
            let n = self.mode_pick.iter().filter(|m| *m == mode).count();
            card.bind_mut().set_marker(if n > 0 { CardMarker::Selected } else { CardMarker::None });
            card.bind_mut().set_badges(&if n > 1 { format!("×{n}") } else { String::new() });
        }
        if matches!(self.pending.as_ref().map(|r| &r.decision), Some(Decision::ChooseOption { .. })) {
            let chosen = !self.mode_pick.is_empty();
            for mut b in self.stage_buttons() {
                if b.bind().get_text().to_string() == tr!("Anwenden", "Apply") {
                    b.bind_mut().set_enabled(chosen);
                }
            }
            return;
        }
        let Some(Decision::ChooseModes { min, max, .. }) = self.pending.as_ref().map(|r| r.decision.clone()) else { return };
        let n = self.mode_pick.len() as u32;
        let with_costs = self.pending.as_ref().is_some_and(|r| Self::mode_costs(&r.decision));
        let pay_label = self.mode_pay_label();
        for mut b in self.stage_buttons() {
            let text = b.bind().get_text().to_string();
            if with_costs && text.starts_with(tr!("Bezahlen", "Pay")) {
                b.bind_mut().set_label_text(GString::from(pay_label.as_str()));
                b.bind_mut().set_enabled(n >= min && n <= max);
            } else if !with_costs && text.starts_with(tr!("Fertig", "Done")) {
                b.bind_mut().set_label_text(GString::from(pick_label(n, min, max).as_str()));
                b.bind_mut().set_enabled(n >= min && n <= max);
            }
        }
    }

    /// Zauber, der gerade gewirkt wird, während einer Bühne am Stack zeigen (CR 601.2a: die Karte
    /// kommt zuerst auf den Stack, dann werden Modi gewählt; Video „Spree und Stack“): eine helle
    /// Kopie der ganzen Karte auf der Bühne an der Bildschirmstelle des Stack-Platzes (rechts),
    /// das Original (auf der Hand bzw. am Stack) ist so lange ausgeblendet. Nur für Karten, die
    /// nicht auf dem Spielfeld liegen (Fähigkeiten bleiben bei ihrer Karte).
    fn stage_spell_at_stack(&mut self, showcase: &mut Gd<Showcase>, req: &Request, data: &VarDictionary) {
        let Some(id) = req.source.map(i64::from) else { return };
        let Some(original) = self.cards.get(&id).cloned() else { return };
        let zone = original.bind().get_zone();
        let on_field = (0..2).any(|seat| {
            let z = self.zones(seat);
            [z.battlefield, z.resources].iter().flatten().any(|b| zone.as_ref() == Some(b))
        });
        if on_field {
            return;
        }
        let (Some(stack), Some(cam)) = (self.stack_zone.clone(), self.base().get_viewport().and_then(|v| v.get_camera_3d())) else {
            return;
        };
        // Bildschirmstelle des obersten Stack-Platzes → Punkt in der Bühnenebene
        let screen = cam.unproject_position(stack.get_global_transform().origin);
        let depth = (showcase.get_global_position() - cam.get_global_position()).length();
        let world = cam.project_position(screen, depth);
        let local = showcase.get_global_transform().affine_inverse() * world;
        let face_up = Basis::from_axis_angle(Vector3::RIGHT, std::f32::consts::FRAC_PI_2);
        let copy = Self::stage_card(data, None);
        showcase.bind_mut().add_prop_card(copy, Transform3D::new(face_up.scaled(Vector3::splat(STACK_STAGE_SCALE)), local));
        showcase.bind_mut().hide_while_open(original);
        // die Auswahl rückt nach links, damit die Karte am Stack frei steht
        if let Some(mut fan) = showcase.bind().fan_zone() {
            let p = fan.get_position();
            fan.set_position(p + Vector3::new(-STACK_STAGE_ROW_SHIFT, 0.0, 0.0));
        }
    }

    /// Eine von mehreren Möglichkeiten, die je zu einer Karte gehören, wie die Spree-Bühne: je
    /// Möglichkeit eine Kopie ihrer Karte mit nur dem Text dieser Möglichkeit (z. B. der
    /// Ersatzeffekt), Klick wählt (orange) bzw. hebt auf, orange „Anwenden“ erst mit Wahl aktiv.
    /// Ersatzeffekte (CR 616.1): Titel „Ersatzeffekt wählen“, darunter „Welcher Ersatzeffekt gilt
    /// zuerst?“; über der Kopie der Kartenname, falls mehrere Möglichkeiten dieselbe Karte haben.
    fn show_card_options(&mut self, req: &Request) {
        let Decision::ChooseOption { options, purpose, prompt, .. } = &req.decision else { return };
        let (title, subtitle) = match purpose.as_str() {
            "ReplacementEffect" => (
                tr!("Ersatzeffekt wählen", "Choose a replacement effect").to_string(),
                tr!("Welcher Ersatzeffekt gilt zuerst?", "Which replacement effect applies first?").to_string(),
            ),
            _ => (if prompt.is_empty() { tr!("Wähle eins", "Choose one").to_string() } else { prompt.clone() }, String::new()),
        };
        let Some(mut showcase) = self.open_stage(&title, &subtitle) else { return };
        let mut cards = Vec::new();
        for (i, o) in options.iter().enumerate() {
            let Some(id) = o.card else { continue };
            let data = req
                .cards
                .iter()
                .find(|c| get_int(c, "id") == i64::from(id))
                .cloned()
                .or_else(|| self.card_data(i64::from(id)))
                .unwrap_or_default();
            let mut copy = Self::stage_card(&data, Some(&o.text));
            copy.bind_mut().set_ready_color(Some(PLAYABLE_GLOW));
            // gleiche Karte mehrfach: Nummer darüber, sonst keine Beschriftung (der Name steht auf der Karte)
            let same = options.iter().filter(|p| p.card == o.card).count() > 1;
            let caption = if same { trf!("Effekt {}", "Effect {}", i + 1) } else { String::new() };
            cards.push((copy.clone(), caption));
            self.mode_cards.push((copy, i));
        }
        let few = cards.len() <= 3;
        showcase.bind_mut().show_options(cards);
        if few {
            // wenige Möglichkeiten: weiter auseinander, damit sie sich im Bogen nicht berühren
            showcase.bind_mut().enable_scroll_with(CARD_OPTION_STEP, CARD_OPTION_RADIUS);
        } else {
            showcase.bind_mut().enable_scroll();
        }
        let mut apply = Button3D::create(ButtonStyle::Orange, tr!("Anwenden", "Apply"), Showcase::button_width() * 1.2);
        apply.bind_mut().set_enabled(false);
        let cb = Callable::from_object_method(&self.to_gd(), "on_modes_done");
        apply.connect("pressed", &cb);
        showcase.bind_mut().add_button(apply);
        if req.cancelable {
            self.stage_button(&mut showcase, ButtonStyle::Blue, tr!("Abbrechen", "Cancel"), Showcase::button_width(), Choice::Cancel);
        }
    }

    /// Hat die Moduswahl eigene Zusatzkosten je Modus (Spree CR 702.172, Tiered)?
    fn mode_costs(decision: &Decision) -> bool {
        matches!(decision, Decision::ChooseModes { costs, .. } if costs.iter().any(|c| !c.is_empty()))
    }

    /// „Bezahlen {Gesamtkosten}“: Grundkosten plus die Kosten der gewählten Modi (nur Anzeige).
    fn mode_pay_label(&self) -> String {
        let Some(Decision::ChooseModes { modes, costs, base_cost, .. }) = self.pending.as_ref().map(|r| &r.decision) else {
            return String::new();
        };
        let mut parts: Vec<&str> = vec![base_cost.as_str()];
        for m in &self.mode_pick {
            if let Some(c) = modes.iter().position(|(i, _)| i == m).and_then(|n| costs.get(n)) {
                parts.push(c.as_str());
            }
        }
        let total = crate::game::cards::add_costs(&parts);
        if total.is_empty() {
            tr!("Bezahlen", "Pay").to_string()
        } else {
            format!("{} {total}", tr!("Bezahlen", "Pay"))
        }
    }

    /// Ja/Nein-Fragen und Entweder-oder (optionale Fähigkeit, „… es sei denn, du zahlst“,
    /// phyrexianisches Mana, zwei Optionen): zwei Knöpfe untereinander – oben orange (erste
    /// Antwort, z. B. „Ja“), darunter blau; die Quellkarte groß darüber.
    fn show_binary(&mut self, req: &Request) {
        let name_of = |t: Target| format!("{t:?}");
        let Some(hud) = decision_view::hud_choice(req, &name_of) else { return };
        if hud.options.len() < 2 {
            return;
        }
        let confirm_like = !matches!(req.decision, Decision::ChooseOption { .. });
        let data = self.source_data(req);
        let Some(mut showcase) = self.open_stage(&hud.title, &hud.detail) else { return };
        if let Some(data) = data {
            showcase.bind_mut().show_options(vec![(Self::stage_card(&data, None), String::new())]);
        }
        showcase.bind_mut().set_focus_layout();
        let width = Showcase::focus_button_width();
        for (i, (style, opt)) in [ButtonStyle::Orange, ButtonStyle::Blue].into_iter().zip(hud.options.iter()).enumerate() {
            let raw = if confirm_like { (i == 0).to_variant() } else { (i as i64).to_variant() };
            if let Some(choice) = decision_view::to_choice(&req.decision, &raw) {
                self.stage_button(&mut showcase, style, &opt.text, width, choice);
            }
        }
        if hud.cancelable {
            self.stage_button(&mut showcase, ButtonStyle::Blue, tr!("Abbrechen", "Cancel"), width * 0.8, Choice::Cancel);
        }
    }

    /// Optionale Kosten (Kicker, Buyback, Entwinden …): die Karte groß und hell im Vordergrund,
    /// darüber der Name der Kosten (z. B. „Kicker“), darunter untereinander orange
    /// „Kicker – {R}“ und grau „Normal“.
    /// Mehrere optionale Kosten: je Kosten ein Knopf zum An-/Abschalten (✓) und „Fertig“.
    fn show_optional_cost(&mut self, req: &Request) {
        let Decision::ChooseOptions { options, .. } = &req.decision else { return };
        let first = options.first().map(|o| o.text.clone()).unwrap_or_default();
        let title = if options.len() == 1 {
            first.split([' ', '–', '-']).next().unwrap_or("").to_string()
        } else {
            tr!("Optionale Kosten", "Optional costs").to_string()
        };
        let data = self.source_data(req);
        let Some(mut showcase) = self.open_stage(&title, &req.source_name) else { return };
        if let Some(data) = data {
            showcase.bind_mut().show_options(vec![(Self::stage_card(&data, None), String::new())]);
        }
        showcase.bind_mut().set_focus_layout();
        let width = Showcase::focus_button_width();
        if options.len() == 1 {
            self.stage_button(&mut showcase, ButtonStyle::Orange, &first, width, Choice::Modes(vec![0]));
            self.stage_button(&mut showcase, ButtonStyle::Grey, tr!("Normal", "Normal"), width, Choice::Modes(Vec::new()));
        } else {
            self.option_toggles = vec![false; options.len()];
            for (i, o) in options.iter().enumerate() {
                let mut b = Button3D::create(ButtonStyle::Orange, &o.text, width);
                let cb = Callable::from_object_method(&self.to_gd(), "on_option_toggle").bind(&[(i as i64).to_variant()]);
                b.connect("pressed", &cb);
                showcase.bind_mut().add_button(b);
            }
            let mut done = Button3D::create(ButtonStyle::Blue, tr!("Fertig", "Done"), width);
            let cb = Callable::from_object_method(&self.to_gd(), "on_options_done");
            done.connect("pressed", &cb);
            showcase.bind_mut().add_button(done);
        }
        if req.cancelable {
            self.stage_button(&mut showcase, ButtonStyle::Blue, tr!("Abbrechen", "Cancel"), width * 0.8, Choice::Cancel);
        }
    }

    /// Kartenkopien für `ids` aus den Kartendaten der Anfrage (bzw. des Zustands).
    fn copies_for(&self, req: &Request, ids: &[CardId]) -> Vec<(Gd<Card3D>, CardId)> {
        ids.iter()
            .map(|id| {
                let data = req
                    .cards
                    .iter()
                    .find(|c| get_int(c, "id") == i64::from(*id))
                    .cloned()
                    .or_else(|| self.card_data(i64::from(*id)))
                    .unwrap_or_default();
                let mut copy = Self::stage_card(&data, None);
                copy.bind_mut().set_ready_color(Some(PLAYABLE_GLOW));
                (copy, *id)
            })
            .collect()
    }

    /// Reihenfolge festlegen (CR 603.3b: eigene gleichzeitige Trigger; auch Karten beim
    /// Zonenwechsel, Kosten) wie die Kartenwahl-Bühne: je Eintrag eine Karte nebeneinander
    /// (Trigger: Kopie der Quelle mit dem Text dieser Fähigkeit), Klick vergibt die nächste Nummer
    /// (1, 2, …; groß auf der Karte, orange), erneuter Klick nimmt sie zurück. „Fertig“, sobald
    /// alle nummeriert sind; „Zurücksetzen“ löscht alle Nummern.
    fn show_order(&mut self, req: &Request) {
        let Decision::OrderTriggers { descriptions, sources, .. } = &req.decision else { return };
        let title = tr!("Reihenfolge festlegen", "Choose the order");
        let sub = if req.prompt.is_empty() { tr!("Klicke die Karten der Reihe nach an.", "Click the cards in order.").to_string() } else { req.prompt.clone() };
        let Some(mut showcase) = self.open_stage(title, &sub) else { return };
        let mut copies = Vec::new();
        for (i, desc) in descriptions.iter().enumerate() {
            let src = sources.get(i).copied().flatten();
            let data = src.and_then(|id| {
                req.cards.iter().find(|c| get_int(c, "id") == i64::from(id)).cloned().or_else(|| self.card_data(i64::from(id)))
            });
            let mut copy = match data {
                Some(d) => {
                    let name = get_str(&d, "name");
                    // Trigger: „Quelle: Fähigkeitstext“ → nur der Fähigkeitstext auf der Karte
                    let text = desc.strip_prefix(&format!("{name}: ")).map(str::to_string);
                    Self::stage_card(&d, text.as_deref())
                }
                None => {
                    let mut def = CardDef::named(desc);
                    def.text = desc.clone();
                    let mut c = Card3D::new_alloc();
                    {
                        let mut b = c.bind_mut();
                        b.set_face_texture(Some(placeholder::card_face(&def)));
                        b.apply_def(&def);
                    }
                    c
                }
            };
            copy.bind_mut().set_ready_color(Some(PLAYABLE_GLOW));
            copies.push(copy.clone());
            self.order_cards.push((copy, i));
        }
        let fan = showcase.bind().fan_zone();
        if let Some(zone) = fan {
            showcase.bind_mut().add_owned_cards(zone, copies);
        }
        showcase.bind_mut().enable_scroll();
        let me = self.to_gd();
        let mut reset = Button3D::create(ButtonStyle::Blue, tr!("Zurücksetzen", "Reset"), Showcase::button_width());
        reset.connect("pressed", &Callable::from_object_method(&me, "on_order_reset"));
        showcase.bind_mut().add_button(reset);
        let n = descriptions.len() as u32;
        let mut done = Button3D::create(ButtonStyle::Orange, &pick_label(0, n, n), Showcase::button_width() * 1.2);
        done.bind_mut().set_enabled(false);
        done.connect("pressed", &Callable::from_object_method(&me, "on_order_done"));
        showcase.bind_mut().add_button(done);
    }

    /// Klick auf eine Karte der Reihenfolge-Bühne: nächste Nummer vergeben bzw. zurücknehmen.
    fn order_click(&mut self, index: usize) {
        if let Some(pos) = self.order_pick.iter().position(|i| *i == index) {
            self.order_pick.remove(pos);
        } else {
            self.order_pick.push(index);
        }
        self.refresh_order_cards();
    }

    /// Nummern auf den Karten (groß, orange markiert) und „Fertig (n/N)“ aktualisieren.
    fn refresh_order_cards(&mut self) {
        let Some(Decision::OrderTriggers { descriptions, .. }) = self.pending.as_ref().map(|r| r.decision.clone()) else { return };
        for (card, i) in self.order_cards.iter_mut() {
            let pos = self.order_pick.iter().position(|p| p == i);
            card.bind_mut().set_marker(if pos.is_some() { CardMarker::Selected } else { CardMarker::None });
            card.bind_mut().set_badges(&pos.map_or(String::new(), |p| (p + 1).to_string()));
        }
        let (n, total) = (self.order_pick.len() as u32, descriptions.len() as u32);
        for mut b in self.stage_buttons() {
            if b.bind().get_text().to_string().starts_with(tr!("Fertig", "Done")) {
                b.bind_mut().set_label_text(GString::from(pick_label(n, total, total).as_str()));
                b.bind_mut().set_enabled(n == total);
            }
        }
    }

    /// Hellsicht / Überwachen (CR 701.18, 701.25) wie im Video des Users (`Scry.mp4`): Titel und
    /// Anleitung, ein dunkler Kasten; darin liegt die eigene Bibliothek als verdeckter Stapel.
    /// Die aufgedeckten Karten liegen zuerst rechts **auf** dem Stapel („OBEN“, die oberste
    /// rechts); zieht man eine nach links, rutscht sie **unter** den Stapel und schaut links
    /// hervor („UNTEN“). Beim Überwachen liegt links statt dessen der Friedhof (neben dem Stapel).
    /// Ein Klick auf eine Karte wechselt ebenfalls die Seite. „Fertig“ schickt die Anordnung an
    /// Forge; die Karten fliegen dabei zum Bibliotheksstapel auf dem Tisch.
    fn show_scry(&mut self, req: &Request) {
        let Decision::Scry { cards, graveyard, player } = &req.decision else { return };
        let (title, sub, left) = if *graveyard {
            (tr!("Überwachen", "Surveil"), tr!("Ziehe die Karten oben auf deine Bibliothek oder in den Friedhof.", "Drag cards to the top of your library or into your graveyard."), tr!("FRIEDHOF", "GRAVEYARD"))
        } else {
            (tr!("Hellsicht", "Scry"), tr!("Ziehe die Karten nach oben oder unten in deine Bibliothek.", "Drag cards to the top or bottom of your library."), tr!("UNTEN", "BOTTOM"))
        };
        let Some(mut showcase) = self.open_stage(title, sub) else { return };
        showcase.bind_mut().add_frame(SCRY_FRAME_CENTER, SCRY_FRAME_SIZE);
        let (c, size) = (SCRY_FRAME_CENTER, SCRY_FRAME_SIZE);
        // verdeckter Bibliotheksstapel in der Kastenmitte (leicht versetzte Rückseiten)
        let library = self.deck_cards.get(*player as usize).map_or(0, |d| d.len());
        let face_down = Basis::from_axis_angle(Vector3::RIGHT, std::f32::consts::FRAC_PI_2)
            * Basis::from_axis_angle(Vector3::BACK, std::f32::consts::PI);
        for i in 0..library.min(SCRY_PILE_SHOWN) {
            let mut card = Card3D::new_alloc();
            {
                let mut b = card.bind_mut();
                b.set_back_texture(Some(placeholder::card_back()));
                let hidden = CardDef::named("?");
                b.set_face_texture(Some(placeholder::card_face(&hidden)));
                b.apply_def(&hidden);
            }
            let k = i as f32;
            let pos = Vector3::new(c.x + SCRY_PILE_X - k * 0.012, c.y + k * 0.008, k * 0.004);
            showcase.bind_mut().add_prop_card(card, Transform3D::new(face_down.scaled(Vector3::splat(SCRY_CARD_SCALE)), pos));
        }
        // Ablagen: links unter dem Stapel (bzw. Friedhof daneben), rechts auf dem Stapel
        let left_z = if *graveyard { SCRY_TOP_Z } else { SCRY_BOTTOM_Z };
        let left_x = if *graveyard { SCRY_GRAVEYARD_X } else { SCRY_BOTTOM_X };
        let drop_h = size.y - 0.1;
        let split = c.x + SCRY_SPLIT_X;
        let left_w = split - (c.x - size.x * 0.5);
        let right_w = (c.x + size.x * 0.5) - split;
        let bottom = showcase.bind_mut().add_zone("ScryBottom", Vector3::new(c.x + left_x, c.y, left_z), Vector3::new(left_w, 0.1, drop_h), 30.0, SCRY_STEP, 170.0, true);
        let top = showcase.bind_mut().add_zone("ScryTop", Vector3::new(c.x + SCRY_TOP_X, c.y, SCRY_TOP_Z), Vector3::new(right_w, 0.1, drop_h), 30.0, SCRY_STEP, 170.0, true);
        for mut z in [bottom.clone(), top.clone()] {
            z.set("card_scale", &SCRY_CARD_SCALE.to_variant());
        }
        let y = c.y - size.y * 0.5 - 0.09;
        showcase.bind_mut().add_caption(left, Vector2::new(c.x - size.x * 0.5, y), false);
        showcase.bind_mut().add_caption(tr!("OBEN", "TOP"), Vector2::new(c.x + size.x * 0.5, y), true);
        self.pick_cards = self.copies_for(req, cards);
        let copies: Vec<Gd<Card3D>> = self.pick_cards.iter().map(|(c, _)| c.clone()).collect();
        showcase.bind_mut().add_owned_cards(top.clone(), copies);
        self.scry_zones = Some((bottom, top));
        let mut done = Button3D::create(ButtonStyle::Orange, tr!("Fertig", "Done"), Showcase::button_width());
        let cb = Callable::from_object_method(&self.to_gd(), "on_scry_done");
        done.connect("pressed", &cb);
        showcase.bind_mut().add_button(done);
        showcase.bind_mut().enable_peek();
    }

    /// Klick auf eine Karte der Hellsicht-Bühne: auf die andere Seite (oben ↔ unten/Friedhof).
    fn scry_click(&mut self, card: Gd<Card3D>) {
        let Some((mut bottom, mut top)) = self.scry_zones.clone() else { return };
        let in_top = card.bind().get_zone().as_ref() == Some(&top);
        let (from, to) = if in_top { (&mut top, &mut bottom) } else { (&mut bottom, &mut top) };
        from.bind_mut().remove_card(card.clone());
        to.bind_mut().add_card(card, -1, 0.0);
    }

    /// Karten aus verdeckten Zonen wählen (Bibliothek durchsuchen, Friedhof, aufgedeckte Karten …)
    /// wie im Video des Users: Beim Durchsuchen der Bibliothek leuchtet zuerst der eigene
    /// Bibliotheksstapel auf. Titel („Wähle bis zu einer“ …) mit Quelle und Text, die Karten groß
    /// als Cover-Flow (blau = wählbar; die Karte unter der Maus kommt groß und gerade nach vorn),
    /// Schieberegler bei vielen Karten, oben rechts „Schlachtfeld ansehen“. Eine Karte: Klick
    /// wählt sofort; mehrere: Klick wählt (orange), „Fertig (n/…)“. Darf man keine nehmen, steht
    /// blau „Ablehnen“ unten rechts. Nur ansehen (max 0): „Fertig“.
    fn show_library_pick(&mut self, req: &Request) {
        let Decision::ChooseCards { options, min, max, reason, player } = &req.decision else { return };
        let (min, max) = (*min, *max);
        if *reason == CardChoiceReason::Search {
            let seat = *player as usize;
            if let Some(mut top) = self.deck_cards.get(seat).and_then(|d| d.last()).cloned().filter(|c| c.is_instance_valid()) {
                top.bind_mut().flash();
            }
        }
        let title = match (min, max) {
            (_, 0) => if req.prompt.is_empty() { tr!("Aufgedeckt", "Revealed").to_string() } else { req.prompt.clone() },
            (0, 1) => tr!("Wähle bis zu einer Karte", "Choose up to one").to_string(),
            (1, 1) => tr!("Wähle eine Karte", "Choose one card").to_string(),
            (a, b) if a == b => trf!("Wähle {b} Karten", "Choose {b} cards"),
            (0, b) => trf!("Wähle bis zu {b} Karten", "Choose up to {b} cards"),
            (a, b) => trf!("Wähle {a}–{b} Karten", "Choose {a}–{b} cards"),
        };
        let sub = match (req.source_name.is_empty(), max == 0 || req.prompt.is_empty()) {
            (false, false) => format!("({}) {}", req.source_name, req.prompt),
            (false, true) => format!("({})", req.source_name),
            (true, false) => req.prompt.clone(),
            (true, true) => String::new(),
        };
        let Some(mut showcase) = self.open_stage(&title, &sub) else { return };
        self.pick_cards = self.copies_for(req, options);
        let copies: Vec<Gd<Card3D>> = self.pick_cards.iter().map(|(c, _)| c.clone()).collect();
        let fan = showcase.bind().fan_zone();
        if let Some(zone) = fan {
            showcase.bind_mut().add_owned_cards(zone, copies);
        }
        showcase.bind_mut().enable_coverflow(LIB_STEP, LIB_SCALE);
        showcase.bind_mut().enable_peek();
        let me = self.to_gd();
        let mut button = |style: ButtonStyle, text: &str, pos: Option<Vector2>, decline: bool| {
            let mut b = Button3D::create(style, text, Showcase::button_width());
            let cb = Callable::from_object_method(&me, "on_pick_done").bind(&[decline.to_variant()]);
            b.connect("pressed", &cb);
            match pos {
                Some(p) => showcase.bind_mut().add_button_at(b, p),
                None => showcase.bind_mut().add_button(b),
            }
        };
        if max == 0 {
            button(ButtonStyle::Orange, tr!("Fertig", "Done"), None, true);
        } else if max > 1 {
            button(ButtonStyle::Orange, &pick_label(0, min, max), None, false);
        }
        if min == 0 && max > 0 {
            button(ButtonStyle::Blue, tr!("Ablehnen", "Decline"), Some(LIB_DECLINE_POS), true);
        }
        if req.cancelable {
            self.stage_button(&mut showcase, ButtonStyle::Grey, tr!("Abbrechen", "Cancel"), Showcase::button_width(), Choice::Cancel);
        }
        self.refresh_pick_buttons();
    }

    /// Klick auf eine Karte der Kartenwahl-Bühne.
    fn library_click(&mut self, id: CardId) {
        let Some(Decision::ChooseCards { max, .. }) = self.pending.as_ref().map(|r| r.decision.clone()) else { return };
        if max == 0 {
            return;
        }
        if max == 1 {
            self.answer(Choice::Cards(vec![id]));
            return;
        }
        if let Some(pos) = self.pick_chosen.iter().position(|c| *c == id) {
            self.pick_chosen.remove(pos);
        } else if self.pick_chosen.len() < max as usize {
            self.pick_chosen.push(id);
        }
        for (card, cid) in self.pick_cards.iter_mut() {
            let on = self.pick_chosen.contains(cid);
            card.bind_mut().set_marker(if on { CardMarker::Selected } else { CardMarker::None });
        }
        self.refresh_pick_buttons();
    }

    /// „Fertig (n/…)“ der Kartenwahl-Bühne aktualisieren.
    fn refresh_pick_buttons(&mut self) {
        let Some(Decision::ChooseCards { min, max, .. }) = self.pending.as_ref().map(|r| r.decision.clone()) else { return };
        if max <= 1 {
            return;
        }
        let n = self.pick_chosen.len() as u32;
        for mut b in self.stage_buttons() {
            if b.bind().get_text().to_string().starts_with(tr!("Fertig", "Done")) {
                b.bind_mut().set_label_text(GString::from(pick_label(n, min, max).as_str()));
                b.bind_mut().set_enabled(n >= min && n <= max);
            }
        }
    }

    /// Zahlen-Frage der Engine: `(kleinster, größter Wert, X-Kosten?)` für `ChooseX` (X-Kosten,
    /// CR 107.3) und `ChooseNumber` (z. B. „wie viele Marken“, „um wie viel verringern“).
    fn number_range(decision: &Decision) -> Option<(u32, u32, bool)> {
        match decision {
            Decision::ChooseX { max, .. } => Some((0, *max, true)),
            Decision::ChooseNumber { min, max, .. } => Some((*min, (*max).max(*min), false)),
            _ => None,
        }
    }

    /// Zahlen-Bühne für X-Kosten und jede andere Zahlen-Frage (wie im Video des Users zu X):
    /// die Karte groß, hell und lesbar (orange leuchtend), darunter die Knopfreihe
    /// „−5“, „−“, orange „Bezahlen X = n“ (bzw. „Bestätigen: n“), „+“, „+5“, darunter grau
    /// „Abbrechen“ (wo erlaubt). „−“/„+“ sind nur aktiv, wo sich der Wert ändern kann; der Start
    /// ist der kleinste erlaubte Wert. Liegt die Karte auf dem Stack (sie wird gerade gewirkt),
    /// steht die Bühne dort ohne Abdunkeln; sonst vor der Kamera wie „Kicker“ (Tisch abgedunkelt).
    /// Titel: „Wähle einen Wert für X.“ bzw. der Fragetext der Engine. Kein Fenster.
    fn show_choose_x(&mut self, req: &Request) {
        let Some((min, max, is_x)) = Self::number_range(&req.decision) else { return };
        self.x_value = min;
        let data = self.source_data(req);
        // eigener Fragetext der Karte (z. B. „How much life?“) bleibt, das allgemeine „Choose a
        // number“ der Engine wird übersetzt und nennt den Bereich
        let prompt = match &req.decision {
            Decision::ChooseNumber { prompt, .. } if !prompt.is_empty() => prompt.clone(),
            _ => req.prompt.clone(),
        };
        let title = if is_x {
            tr!("Wähle einen Wert für X.", "Select a value for X.").to_string()
        } else if prompt.is_empty() || prompt.to_lowercase().contains("choose a number") {
            trf!("Wähle eine Zahl von {min} bis {max}.", "Choose a number from {min} to {max}.")
        } else {
            prompt
        };
        let Some(mut showcase) = self.open_stage(&title, "") else { return };
        let source = req.source.and_then(|id| self.cards.get(&i64::from(id)).cloned());
        let on_stack = source.as_ref().zip(self.stack_zone.as_ref()).is_some_and(|(c, z)| c.bind().get_zone().as_ref() == Some(z));
        if on_stack {
            // an ihrem Platz am Stack (rechts), kleiner, ohne Abdunkeln
            if let Some(zone) = self.stack_zone.as_ref() {
                let at = zone.get_global_transform() * X_STAGE_LOCAL;
                showcase.bind_mut().open_at(at, X_STAGE_SCALE, X_STAGE_LIFT);
            }
            // die leuchtende Originalkarte auf dem Stack dahinter ausblenden, bis die Bühne weg ist
            if let Some(card) = source {
                showcase.bind_mut().hide_while_open(card);
            }
        }
        // die Karte groß, hell und lesbar in der Mitte
        if let Some(data) = data {
            let mut copy = Self::stage_card(&data, None);
            copy.bind_mut().set_marker(CardMarker::Selected);
            showcase.bind_mut().show_options(vec![(copy, String::new())]);
        }
        showcase.bind_mut().set_focus_layout();
        // darunter die Knopfreihe −5 / − / Bestätigen / + / +5, darunter Abbrechen
        let me = self.to_gd();
        let mut add = |button: Gd<Button3D>, pos: Vector2, method: &str, arg: Option<i64>| {
            let mut button = button;
            let mut cb = Callable::from_object_method(&me, method);
            if let Some(a) = arg {
                cb = cb.bind(&[a.to_variant()]);
            }
            button.connect("pressed", &cb);
            showcase.bind_mut().add_button_at(button.clone(), pos);
            button
        };
        for (text, delta, x) in [("−5", -5i64, -X_OUTER), ("−", -1, -X_INNER), ("+", 1, X_INNER), ("+5", 5, X_OUTER)] {
            add(Button3D::create_shaped(ButtonStyle::Blue, text, X_SMALL_WIDTH, COMPACT_BODY), Vector2::new(x, X_ROW_Y), "on_x_step", Some(delta));
        }
        let pay = add(Button3D::create(ButtonStyle::Orange, &number_label(min, is_x), X_PAY_WIDTH), Vector2::new(0.0, X_ROW_Y), "on_x_pay", None);
        if req.cancelable {
            add(Button3D::create(ButtonStyle::Grey, tr!("Abbrechen", "Cancel"), X_PAY_WIDTH * 0.75), Vector2::new(0.0, X_CANCEL_Y), "on_x_cancel", None);
        }
        self.x_pay_button = Some(pay);
        self.refresh_x_buttons();
    }

    /// Bestätigen-Knopf zeigt den Wert; „−“ bzw. „+“ nur, wo sich der Wert ändern kann.
    fn refresh_x_buttons(&mut self) {
        let Some((min, max, is_x)) = self.pending.as_ref().and_then(|r| Self::number_range(&r.decision)) else { return };
        let x = self.x_value;
        if let Some(mut b) = self.x_pay_button.as_ref().filter(|b| b.is_instance_valid()).cloned() {
            b.bind_mut().set_label_text(GString::from(number_label(x, is_x).as_str()));
        }
        for mut b in self.stage_buttons() {
            let text = b.bind().get_text().to_string();
            let can = match text.as_str() {
                "−5" | "−" => x > min,
                "+" | "+5" => x < max,
                _ => true,
            };
            b.bind_mut().set_enabled(can);
        }
    }

    /// Karten aus der eigenen Hand wählen – nach einem Mulligan unter die Bibliothek legen,
    /// abwerfen (auch als Kosten oder wegen Handgröße), aufdecken …: `(Optionen, min, max)`, wenn
    /// alle wählbaren Karten auf der eigenen Hand liegen, sonst `None`.
    fn hand_choice(&self, decision: &Decision) -> Option<(Vec<CardId>, u32, u32)> {
        match decision {
            Decision::MulliganBottom { hand, count, .. } => Some((hand.clone(), *count, *count)),
            Decision::ChooseCards { options, min, max, .. } => {
                let hand = self.zones(LOCAL_SEAT as usize).hand?;
                // Handkarten, auch wenn sie gerade auf der Bühne liegen
                let staged = |c: &Gd<Card3D>| self.showcase.as_ref().is_some_and(|s| s.is_instance_valid() && s.bind().holds(c.clone()));
                let in_hand = |id: &CardId| {
                    self.cards
                        .get(&i64::from(*id))
                        .is_some_and(|c| staged(c) || c.bind().get_zone().is_some_and(|z| z == hand))
                };
                (!options.is_empty() && options.iter().all(in_hand)).then(|| (options.clone(), *min, *max))
            }
            _ => None,
        }
    }

    /// Hand-Bühne (wie der Mulligan): die wählbaren Handkarten groß aufgefächert vor der Kamera,
    /// Titel nach dem Grund, Klick wählt (orange) bzw. hebt die Wahl auf, „Fertig (n/…)“ erst bei
    /// gültiger Anzahl aktiv, „Abbrechen“ wo Forge das erlaubt.
    fn show_hand_pick(&mut self, req: &Request) {
        let Some((options, min, max)) = self.hand_choice(&req.decision) else { return };
        self.clear_choice_objects();
        self.hand_pick.clear();
        let Some(mut parent) = self.controller.clone().map(|c| c.upcast::<Node>()) else { return };
        let mut showcase = Showcase::new_alloc();
        parent.add_child(&showcase);
        showcase.bind_mut().open();
        let (title, subtitle) = match &req.decision {
            Decision::MulliganBottom { count, .. } => (
                trf!("{count} Karte(n) unter die Bibliothek", "Put {count} card(s) on the bottom"),
                tr!("Klicke die Karten an, die unter deine Bibliothek kommen.", "Click the cards to put on the bottom of your library.").to_string(),
            ),
            Decision::ChooseCards { reason, .. } => {
                let count = if min == max { max.to_string() } else { format!("{min}–{max}") };
                let title = match reason {
                    CardChoiceReason::Discard | CardChoiceReason::DiscardCost | CardChoiceReason::DiscardToHandSize => {
                        trf!("{count} Karte(n) abwerfen", "Discard {count} card(s)")
                    }
                    _ if !req.prompt.is_empty() => req.prompt.clone(),
                    _ => trf!("{count} Karte(n) wählen", "Choose {count} card(s)"),
                };
                let sub = if req.source_name.is_empty() {
                    tr!("Klicke die Karten an.", "Click the cards.").to_string()
                } else {
                    format!("{} – {}", req.source_name, tr!("Klicke die Karten an.", "Click the cards."))
                };
                (title, sub)
            }
            _ => return,
        };
        showcase.bind_mut().set_texts(GString::from(title.as_str()), GString::from(subtitle.as_str()), GString::new());
        let cards: Vec<Gd<Card3D>> = options.iter().filter_map(|id| self.cards.get(&i64::from(*id)).cloned()).collect();
        for c in &cards {
            let mut c = c.clone();
            c.bind_mut().set_ready_glow(true);
        }
        showcase.bind_mut().show_cards(&cards);
        // Karten nebeneinander, bei vielen Karten mit Schieberegler
        showcase.bind_mut().enable_scroll();
        if req.cancelable {
            let mut cancel = Button3D::create(ButtonStyle::Blue, tr!("Abbrechen", "Cancel"), Showcase::button_width());
            let cb = Callable::from_object_method(&self.to_gd(), "on_hand_pick_cancel");
            cancel.connect("pressed", &cb);
            showcase.bind_mut().add_button(cancel);
        }
        let mut button = Button3D::create(ButtonStyle::Orange, &pick_label(0, min, max), Showcase::button_width() * 1.2);
        button.bind_mut().set_enabled(min == 0);
        let cb = Callable::from_object_method(&self.to_gd(), "on_hand_pick_done");
        button.connect("pressed", &cb);
        showcase.bind_mut().add_button(button.clone());
        self.hand_pick_button = Some(button);
        self.showcase = Some(showcase);
    }

    /// Klick auf eine Karte auf der Hand-Bühne: wählen (orange) bzw. Wahl aufheben, Knopf
    /// aktualisieren; ist schon das Maximum gewählt und nur eine erlaubt, wird getauscht.
    /// true = behandelt.
    fn hand_pick_click(&mut self, card: CardId) -> bool {
        let Some((options, min, max)) = self.pending.as_ref().and_then(|r| self.hand_choice(&r.decision)) else {
            return false;
        };
        if !options.contains(&card) {
            return true;
        }
        if let Some(pos) = self.hand_pick.iter().position(|c| *c == card) {
            self.hand_pick.remove(pos);
        } else if self.hand_pick.len() < max as usize {
            self.hand_pick.push(card);
        } else if max == 1 {
            self.hand_pick = vec![card];
        }
        for id in &options {
            if let Some(mut c) = self.cards.get(&i64::from(*id)).cloned() {
                let marker = if self.hand_pick.contains(id) { CardMarker::Selected } else { CardMarker::None };
                c.bind_mut().set_marker(marker);
            }
        }
        let n = self.hand_pick.len() as u32;
        if let Some(mut b) = self.hand_pick_button.as_ref().filter(|b| b.is_instance_valid()).cloned() {
            b.bind_mut().set_label_text(GString::from(pick_label(n, min, max).as_str()));
            b.bind_mut().set_enabled(n >= min && n <= max);
        }
        true
    }

    /// 3D-Bedienobjekte der letzten Entscheidung ausblenden und freigeben.
    fn clear_choice_objects(&mut self) {
        self.viewer_open = false;
        self.viewer_cards.clear();
        self.action_options.clear();
        self.stage_choices.clear();
        self.mode_cards.clear();
        self.mode_pick.clear();
        self.option_toggles.clear();
        self.pick_cards.clear();
        self.scry_zones = None;
        self.pick_chosen.clear();
        self.order_cards.clear();
        self.order_pick.clear();
        if let Some(mut s) = self.showcase.take() {
            if s.is_instance_valid() {
                s.bind_mut().close();
            }
        }
        for mut b in self.choice_buttons.drain(..) {
            if b.is_instance_valid() {
                b.bind_mut().dismiss();
            }
        }
        if let Some(mut p) = self.choice_prompt.take() {
            if p.is_instance_valid() {
                p.queue_free();
            }
        }
        if let Some(mut b) = self.cancel_badge.take().filter(|b| b.is_instance_valid()) {
            b.bind_mut().dismiss();
        }
        self.close_wheel();
    }

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
            repeat: false,
        });
        let combat_choice = self
            .pending
            .as_ref()
            .and_then(|req| self.table_pick_choice(req));
        let choice = menu_choice.or(combat_choice).or_else(|| self.pending.as_ref().filter(|r| Self::stage_kind(&r.decision).is_none() && self.wheel.is_none() && Self::number_range(&r.decision).is_none()).and_then(|req| {
            // Anzeigename eines Ziels: Spieler-Name, Karte vom Tisch oder aus den Kartendaten
            let name_of = |t: Target| match t {
                Target::Player(p) => self.seat_name(p as usize).to_string(),
                Target::Card(c) => self
                    .cards
                    .get(&i64::from(c))
                    .map(|card| card.bind().card_name().to_string())
                    .or_else(|| req.cards.iter().find(|d| get_int(d, "id") == i64::from(c)).map(|d| get_str(d, "name")))
                    .unwrap_or_else(|| format!("#{c}")),
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
        let combat = self.combat_button();
        let button_text = if let Some((label, _)) = combat.as_ref() {
            label.clone()
        } else if my_priority {
            tr!("Passen  »", "Pass  »").to_string()
        } else if choice.is_some() || self.wheel.is_some() || self.pending.as_ref().is_some_and(|r| Self::number_range(&r.decision).is_some()) {
            tr!("Auswahl treffen …", "Make a choice …").to_string()
        } else if !self.winner.is_empty() {
            tr!("Spielende", "Game over").to_string()
        } else if self.match_running {
            tr!("Gegner ist am Zug …", "Opponent is acting …").to_string()
        } else {
            tr!("Warte auf Forge …", "Waiting for Forge …").to_string()
        };
        HudView {
            phase_titles: Phase::ORDER.iter().map(|p| GString::from(p.title())).collect(),
            phase_index: phase_index(&phase),
            active_seat: st.map_or(-1, |s| get_int(s, "active_player")),
            names: [self.seat_name(0), self.seat_name(1)],
            lives: [life(0), life(1)],
            counters: [0, 1].map(|seat| self.seat_flags(seat, player(seat).as_ref())),
            day: st.and_then(|s| s.get("day")).and_then(|v| v.try_to::<bool>().ok()),
            turn: st.map_or(0, |s| get_int(s, "turn")),
            winner: self.winner.clone(),
            button_enabled: my_priority || combat.is_some(),
            button_text: button_text.as_str().into(),
            button_hint: combat.map(|(_, hint)| hint).unwrap_or_default(),
            stage_open: self.showcase.is_some() || !self.choice_buttons.is_empty(),
            stage_covers: self.stage_covers(),
            end_turn_enabled: self.match_running
                && self.winner.is_empty()
                && self.auto_pass_turn.is_none()
                && st.is_some_and(|s| get_int(s, "active_player") == 0),
            pool_clickable: (0..6)
                .map(|m| match self.pending.as_ref().map(|r| &r.decision) {
                    Some(Decision::PayMana { options, .. }) => {
                        options.iter().any(|o| matches!(o, PayAction::UsePool(c) if Some(*c) == pool_color(m)))
                    }
                    _ => false,
                })
                .collect(),
            player_pool: player(0)
                .and_then(|p| get_array(&p, "mana_pool"))
                .map(|a| a.iter_shared().map(|v| v.try_to::<i64>().unwrap_or(0)).collect())
                .unwrap_or_default(),
            choice,
            targetable_seats: seat_state(false),
            selected_seats: seat_state(true),
        }
    }

    /// Zeile unter dem Spielernamen: Monarch, Initiative, Stadtsegen, Ring-Stufe und Marken.
    fn seat_flags(&self, seat: usize, player: Option<&VarDictionary>) -> String {
        let st = self.state.as_ref();
        let game_flag = |key: &str| st.is_some_and(|s| s.get(key).and_then(|v| v.try_to::<i64>().ok()) == Some(seat as i64));
        let mut parts: Vec<String> = Vec::new();
        if game_flag("monarch") {
            parts.push(tr!("♛ Monarch", "♛ Monarch").to_string());
        }
        if game_flag("initiative") {
            parts.push(tr!("Initiative", "Initiative").to_string());
        }
        if let Some(p) = player {
            if get_bool(p, "blessing") {
                parts.push(tr!("Stadtsegen", "City's blessing").to_string());
            }
            let ring = get_int(p, "ring_level");
            if ring > 0 {
                parts.push(trf!("Ring {ring}", "Ring {ring}"));
            }
            let counters = counters::player_counters(&counter_list(p));
            if !counters.is_empty() {
                parts.push(counters);
            }
        }
        parts.join(" · ")
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
                    exile: s.exile.clone(),
                    command: s.command.clone(),
                    resources: s.resource_zone.clone(),
                }
            }
            None => SeatZones { hand: None, deck: None, battlefield: None, discard: None, exile: None, command: None, resources: None },
        }
    }

    fn pending_kind(&self) -> Option<String> {
        self.pending.as_ref().map(|r| decision_name(&r.decision).to_string())
    }

    /// Antwortet über die Engine-Schnittstelle auf die offene Entscheidung.
    fn answer(&mut self, choice: Choice) {
        let Some(req) = self.pending.take() else { return };
        self.viewer_open = false;
        self.clear_choice_objects();
        self.menu = None;
        self.selection.clear();
        self.attack_plan.clear();
        self.block_plan.clear();
        self.combat_armed = None;
        self.drawn_arrows.clear();
        self.set_marked(None);
        self.update_glow();
        if let Some(mut b) = self.bridge.clone() {
            b.submit(req.id, &choice);
        }
    }

    /// Aktion der offenen `Priority`-Entscheidung für die Karte `card_id`: Land spielen bzw.
    /// wirken zuerst, sonst die erste Aktion der Karte (z. B. Fähigkeit aktivieren).
    /// Aktionen der offenen Priorität für Karte `card` (mit Beschriftung), ohne Manafähigkeiten.
    fn card_actions(&self, card: CardId) -> Vec<(Action, String)> {
        let Some(req) = self.pending.as_ref() else { return Vec::new() };
        let Decision::Priority { actions, .. } = &req.decision else { return Vec::new() };
        actions
            .iter()
            .enumerate()
            .filter(|(_, a)| a.card() == Some(card) && !matches!(a, Action::ActivateMana(..) | Action::UndoMana(..)))
            .map(|(i, a)| (*a, req.label(i)))
            .collect()
    }

    /// „Wähle eins“ (wie im Video des Users): Bühne mit je einer Kopie der Karte pro Spielweise
    /// (Wirken, Cycling, Fähigkeiten …), Beschriftung darüber, blaues Leuchten, Hover vergrößert;
    /// Klick wählt die Aktion, „Abbrechen“ schließt die Bühne (Priorität bleibt offen).
    fn show_action_choice(&mut self, card: CardId) {
        let options = self.card_actions(card);
        if options.len() < 2 {
            return;
        }
        let Some(data) = self.card_data(i64::from(card)) else { return };
        self.clear_choice_objects();
        let Some(mut parent) = self.controller.clone().map(|c| c.upcast::<Node>()) else { return };
        let mut showcase = Showcase::new_alloc();
        parent.add_child(&showcase);
        showcase.bind_mut().open();
        showcase.bind_mut().set_texts(
            tr!("Wähle eins", "Choose one").into(),
            tr!("Klicke eine Option an.", "Click an option to select it.").into(),
            GString::new(),
        );
        let def = CardDef::from_state(&data);
        let mut cards = Vec::new();
        for (action, label) in options {
            let mut copy = Card3D::new_alloc();
            {
                let mut c = copy.bind_mut();
                c.set_face_texture(Some(placeholder::card_face(&def)));
                c.apply_def(&def);
                c.set_ready_color(Some(PLAYABLE_GLOW));
            }
            cards.push((copy.clone(), action_caption(action, &label)));
            self.action_options.push((copy, action));
        }
        showcase.bind_mut().show_options(cards);
        let mut cancel = Button3D::create(ButtonStyle::Orange, tr!("Abbrechen", "Cancel"), Showcase::button_width());
        let cb = Callable::from_object_method(&self.to_gd(), "on_action_choice_cancel");
        cancel.connect("pressed", &cb);
        showcase.bind_mut().add_button(cancel);
        self.showcase = Some(showcase);
    }

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
                // Manafähigkeiten (Vorab-Tappen) leuchten nicht, sonst glühte jedes Land
                let playable = actions.iter().filter(|a| !matches!(a, Action::ActivateMana(..) | Action::UndoMana(..)));
                (playable.filter_map(|a| a.card()).map(i64::from).collect(), HashSet::new())
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
            // Wahl aus der eigenen Hand (Hand-Bühne): alle Optionen leuchten blau
            Some(d) if self.hand_choice(d).is_some() => {
                let (options, _, _) = self.hand_choice(d).unwrap_or_default();
                (options.iter().map(|c| i64::from(*c)).collect(), HashSet::new())
            }
            Some(d @ Decision::ChooseCards { .. }) if self.table_pick(d).is_some() => {
                let options = self.table_pick(d).unwrap_or_default();
                (
                    options.iter().map(|c| i64::from(*c)).collect(),
                    self.selection.iter().filter_map(|i| options.get(*i)).map(|c| i64::from(*c)).collect(),
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

    /// Kampf und Zielwahl: Text des Phasen-Knopfs (bestätigt den Plan bzw. die Ziele auf dem
    /// Tisch, bei zu wenigen Zielen „Abbrechen“) und Hinweis (Engine-Hinweis zum Kampf bzw. was die
    /// Karte als Ziel verlangt; leer = keiner). `None` = weder Kampf noch Zielwahl offen.
    fn combat_button(&self) -> Option<(String, String)> {
        let req = self.pending.as_ref()?;
        let (label, standard) = match &req.decision {
            Decision::DeclareAttackers { .. } => {
                let n = self.attack_plan.len();
                (
                    if n == 0 { tr!("Angreifen  »", "Attack  »").to_string() } else { trf!("Angreifen ({n})  »", "Attack ({n})  »") },
                    "Angreifer wählen",
                )
            }
            Decision::DeclareBlockers { .. } => {
                let n = self.block_plan.len();
                (
                    if n == 0 { tr!("Blocken  »", "Block  »").to_string() } else { trf!("Blocken ({n})  »", "Block ({n})  »") },
                    "Blocker wählen",
                )
            }
            // Zielwahl: Hinweis = was die Karte als Ziel verlangt
            Decision::ChooseTargets { min, description, .. } => {
                let n = self.selection.len();
                let label = if n >= *min as usize && n > 0 {
                    trf!("Ziele bestätigen ({n})  »", "Confirm targets ({n})  »")
                } else if *min == 0 {
                    tr!("Ohne Ziel weiter  »", "No target  »").to_string()
                } else if req.cancelable {
                    tr!("Abbrechen", "Cancel").to_string()
                } else {
                    tr!("Ziel wählen …", "Choose a target …").to_string()
                };
                let what = if description.is_empty() { req.prompt.clone() } else { description.clone() };
                return Some((label, what));
            }
            // jede andere abbrechbare Frage beim Wirken/Aktivieren: Knopf „Abbrechen“
            _ => return self.cast_cancel().map(|_| (tr!("Abbrechen", "Cancel").to_string(), String::new())),
        };
        let hint = if req.prompt.is_empty() || req.prompt.starts_with(standard) { String::new() } else { req.prompt.clone() };
        Some((label, hint))
    }

    /// Kandidat `index` angeklickt bzw. per Pfeil gewählt. Zielwahl (`ChooseTargets`): wählen bzw.
    /// abwählen; sind so viele Ziele gewählt, wie die Karte höchstens erlaubt, geht die Antwort
    /// sofort an Forge. Kartenwahl auf dem Tisch: weiter an das Auswahlfenster.
    fn pick_candidate(&mut self, index: usize) {
        let Some(Decision::ChooseTargets { candidates, max, .. }) = self.pending.as_ref().map(|r| r.decision.clone()) else {
            self.signals().choice_option_clicked().emit(index as i64);
            return;
        };
        if let Some(pos) = self.selection.iter().position(|i| *i == index) {
            self.selection.remove(pos);
        } else {
            self.selection.push(index);
        }
        if self.selection.len() >= max.max(1) as usize {
            let targets = self.selection.iter().filter_map(|i| candidates.get(*i).copied()).collect();
            self.answer(Choice::Targets(targets));
            return;
        }
        self.update_glow();
    }

    /// Index des Kandidaten `target` in der offenen Zielwahl.
    fn candidate_index(&self, target: Target) -> Option<usize> {
        match self.pending.as_ref().map(|r| &r.decision) {
            Some(Decision::ChooseTargets { candidates, .. }) => candidates.iter().position(|t| *t == target),
            Some(d @ Decision::ChooseCards { .. }) => match target {
                Target::Card(c) => self.table_pick(d)?.iter().position(|o| *o == c),
                Target::Player(_) => None,
            },
            _ => None,
        }
    }

    /// `ChooseCards`, deren Karten alle offen auf dem Tisch liegen (Spielfeld, Länder-Reihe,
    /// Kommandozone, Stack): werden auf dem Tisch angeklickt statt im Kartenfächer gewählt.
    /// Liefert die wählbaren Karten (sonst None = Kartenfächer).
    fn table_pick(&self, decision: &Decision) -> Option<Vec<CardId>> {
        let Decision::ChooseCards { options, .. } = decision else { return None };
        let mut table_zones: Vec<Gd<Zone>> = Vec::new();
        for seat in 0..2 {
            let z = self.zones(seat);
            table_zones.extend([z.battlefield, z.resources, z.command].into_iter().flatten());
        }
        table_zones.extend(self.stack_zone.clone());
        let on_table = |id: &CardId| {
            self.cards
                .get(&i64::from(*id))
                .and_then(|c| c.bind().get_zone())
                .is_some_and(|z| table_zones.contains(&z))
        };
        (!options.is_empty() && options.iter().all(on_table)).then(|| options.clone())
    }

    /// Auswahlfenster am rechten Rand für `ChooseCards` im Tisch-Modus: Kartennamen als Liste
    /// (Ausweichweg zum Klick), „Fertig“ bei Mehrfachwahl.
    fn table_pick_choice(&self, req: &Request) -> Option<HudChoice> {
        let options = self.table_pick(&req.decision)?;
        let Decision::ChooseCards { min, max, .. } = &req.decision else { return None };
        let name = |id: &CardId| {
            self.cards.get(&i64::from(*id)).map(|c| c.bind().card_name().to_string()).unwrap_or_else(|| format!("#{id}"))
        };
        let title = if req.prompt.is_empty() { tr!("Karten wählen", "Choose cards").to_string() } else { req.prompt.clone() };
        Some(HudChoice {
            key: req.id,
            title,
            detail: tr!("Karte auf dem Tisch anklicken", "Click a card on the table").into(),
            mode: if *min == 1 && *max == 1 {
                ChoiceMode::Single
            } else {
                ChoiceMode::Multi { min: *min as usize, max: *max as usize }
            },
            options: options.iter().map(|id| ChoiceOption { text: name(id), color: None }).collect(),
            cancelable: req.cancelable,
            side: true,
            numbered: false,
            repeat: false,
        })
    }

    /// Medaillon-Anker eines Sitzplatzes (Welt).
    fn hero_anchor(&self, seat: u8) -> Option<Vector3> {
        let seat = if seat == 0 { self.player_seat.as_ref() } else { self.opponent_seat.as_ref() }?;
        let anchor = seat.get_node_or_null("HeroAnchor").and_then(|n| n.try_cast::<Node3D>().ok());
        Some(anchor.map_or(seat.get_global_position(), |a| a.get_global_position()))
    }

    /// Position eines Ziels auf dem Tisch: Karte bzw. Punkt vor dem Medaillon-Anker des
    /// Sitzplatzes (zur Tischmitte hin).
    fn target_position(&self, target: Target) -> Option<Vector3> {
        match target {
            Target::Card(c) => self.cards.get(&i64::from(c)).filter(|c| c.is_instance_valid()).map(|c| c.get_global_position()),
            Target::Player(p) => {
                let p = self.hero_anchor(p)?;
                // etwas zur Tischmitte hin: Pfeilspitzen bleiben vor dem Tischrand sichtbar
                let center = self.controller.as_ref().map_or(Vector3::ZERO, |c| c.get_global_position());
                let inward = Vector3::new(center.x - p.x, 0.0, center.z - p.z).normalized_or_zero();
                Some(p + inward * PLAYER_TARGET_INSET)
            }
        }
    }

    /// Kampfpfeile: während der eigenen Erklärung der Plan (Angreifer rot → Ziel, Blocker blau →
    /// Angreifer, gewählter Blocker gold → Angreifer unter der Maus), sonst der laufende Kampf aus
    /// dem Engine-Zustand (`combat`).
    fn combat_arrows(&self) -> Vec<ArrowSpec> {
        let pos = |c: CardId| self.target_position(Target::Card(c));
        let mut arrows = Vec::new();
        let aiming = self.live_arrow().map(|(id, _)| id);
        let mut push = |from: CardId, to: Target, color: ArrowColor, alpha: f32| {
            if aiming == Some(from) {
                return; // wird gerade neu gezogen
            }
            if let (Some(a), Some(b)) = (pos(from), self.target_position(to)) {
                let key = arrow_key(color as u64, i64::from(from), to);
                arrows.push(arrow(key, a, b, color, alpha, !self.drawn_arrows.contains(&key)));
            }
        };
        match self.pending.as_ref().map(|r| &r.decision) {
            // Pfeil nur, wenn es mehr als ein Angriffsziel gibt (Planeswalker, Schlacht): dann zeigt
            // er, wen der Angreifer angreift. Sonst reicht die Kampfansicht (Karte rückt vor).
            Some(Decision::DeclareAttackers { defenders, .. }) if defenders.len() > 1 => {
                for (c, t) in &self.attack_plan {
                    push(*c, *t, ArrowColor::Orange, 1.0);
                }
            }
            Some(Decision::DeclareAttackers { .. }) => {}
            Some(Decision::DeclareBlockers { attackers, .. }) => {
                for (b, a) in &self.block_plan {
                    push(*b, Target::Card(*a), ArrowColor::Blue, 1.0);
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
                        push(armed, Target::Card(a), ArrowColor::Gold, 0.5);
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
                        // Angriff auf einen Spieler: Kampfansicht genügt; auf Planeswalker/Schlacht: Pfeil
                        if let (Some(c), Some(t @ Target::Card(_))) = (attacker, target) {
                            push(c, t, ArrowColor::Orange, 1.0);
                        }
                    }
                    for pair in get_array(&combat, "blocks").unwrap_or_default().iter_shared() {
                        let Ok(pair) = pair.try_to::<VarArray>() else { continue };
                        let id = |i: usize| pair.get(i).and_then(|v| v.try_to::<i64>().ok()).and_then(|v| u32::try_from(v).ok());
                        if let (Some(b), Some(a)) = (id(0), id(1)) {
                            push(b, Target::Card(a), ArrowColor::Blue, 1.0);
                        }
                    }
                }
            }
        }
        arrows
    }

    /// Mana-Effekt (Assets „Mana Glow Effekte“): vom zuletzt getappten Land zur Karte, die gerade
    /// bezahlt wird (oberste Karte auf dem Stack, sonst zum Medaillon des Spielers), in der Farbe
    /// des erzeugten Manas.
    fn spawn_mana_fx(&mut self, event: &VarDictionary) {
        let Some((id, at)) = self.recent_taps.pop_back() else { return };
        if self.clock - at > 1.5 {
            return;
        }
        let Some(source) = self.cards.get(&id).filter(|c| c.is_instance_valid()).cloned() else { return };
        let colors = get_str(event, "colors");
        let letter = colors.chars().next().map(|c| c.to_string()).unwrap_or_else(|| "C".into());
        let seat = get_int(event, "player");
        // gleichzeitig getappte Länder starten leicht versetzt
        let delay = 0.08 * self.mana_fx.iter().filter(|(fx, _)| fx.is_instance_valid()).count().min(6) as f32;
        let target = self.stack_top_card_of(seat);
        // ohne Zauber im Wirken (Mana vorab getappt): zum Manapool des Spielers, nicht zum Medaillon
        let fallback = u8::try_from(seat).ok().and_then(|s| self.mana_node(s).or_else(|| self.hero_node(s)));
        let fx = ManaFx::mana(source.upcast(), target, fallback, mana_color(&letter), delay);
        if let Some(mut parent) = self.controller.clone() {
            parent.add_child(&fx);
        }
        self.mana_fx.push((fx, seat));
    }

    /// Kartenansicht beim Hovern (wie `Fähigkeiten.png`): statt die Karte selbst zur Mitte zu
    /// holen, zeigt der [CardInspector] eine helle Kopie am Bildschirmrand gegenüber der Maus,
    /// daneben Fähigkeiten (Schlüsselwörter mit Erklärung der Engine) und Veränderungen
    /// (Marken, Stärke/Widerstand, Schaden, angelegte Karten). Nicht bei Handkarten (die zeigen
    /// sich selbst groß), nicht beim Ziehen und Zielen und nicht bei verdeckten Karten.
    fn update_inspector(&mut self) {
        let Some(controller) = self.controller.clone() else { return };
        let Some(mut inspector) = self.inspector.as_ref().filter(|i| i.is_instance_valid()).cloned() else { return };
        let busy = controller.bind().get_dragged_card().is_some() || controller.bind().get_aiming_card().is_some();
        let hovered = self.inspect_override.as_ref().filter(|c| c.is_instance_valid()).cloned().or_else(|| {
            if busy { None } else { controller.bind().get_hovered_card().filter(|c| c.is_instance_valid()) }
        });
        let id = hovered.as_ref().map(|c| c.instance_id());
        if id == self.inspected {
            return;
        }
        self.inspected = id;
        let Some(card) = hovered else {
            inspector.bind_mut().hide_card();
            return;
        };
        let def = card.bind().def().clone();
        // Handkarten (eigene Hand, Fächer der Bühnen) zeigen sich beim Hovern selbst groß
        let in_hand = card.bind().get_zone().is_some_and(|z| z.bind().is_hand());
        if def.name == "?" || def.name.is_empty() || (in_hand && self.inspect_override.is_none()) {
            inspector.bind_mut().hide_card();
            return;
        }
        let data = self.forge_id(&card).and_then(|fid| self.card_data(fid));
        let mut abilities = Vec::new();
        let mut buffs = Vec::new();
        if let Some(data) = data.as_ref() {
            for k in get_array(data, "keywords").unwrap_or_default().iter_shared() {
                let Ok(k) = k.try_to::<VarDictionary>() else { continue };
                abilities.push(InspectorEntry { title: get_str(&k, "title"), text: get_str(&k, "reminder") });
            }
            if let (Some((p, t)), Some((bp, bt))) = (def.stats, def.base_stats) {
                if (p, t) != (bp, bt) {
                    buffs.push(InspectorEntry {
                        title: tr!("Stärke/Widerstand", "Power/toughness").into(),
                        text: trf!("{p}/{t} (aufgedruckt {bp}/{bt})", "{p}/{t} (printed {bp}/{bt})"),
                    });
                }
            }
            if def.damage > 0 {
                let d = def.damage;
                buffs.push(InspectorEntry { title: tr!("Schaden", "Damage").into(), text: d.to_string() });
            }
            for line in card_badges(data, &[]).lines().filter(|l| !l.is_empty()) {
                buffs.push(InspectorEntry { title: line.to_string(), text: String::new() });
            }
            // angelegte Karten (Auren, Ausrüstung)
            let fid = get_int(data, "id");
            let players = self.state.as_ref().and_then(|s| get_array(s, "players")).unwrap_or_default();
            for p in players.iter_shared().filter_map(|p| p.try_to::<VarDictionary>().ok()) {
                for c in get_array(&p, "battlefield").unwrap_or_default().iter_shared().filter_map(|c| c.try_to::<VarDictionary>().ok()) {
                    if get_int(&c, "attached_to") == fid && data.contains_key("id") {
                        buffs.push(InspectorEntry { title: tr!("Angelegt", "Attached").into(), text: get_str(&c, "name") });
                    }
                }
            }
        }
        // Seite gegenüber der Karte: Karte rechts im Bild → Ansicht links und umgekehrt
        let right_side = self
            .base()
            .get_viewport()
            .and_then(|v| v.get_camera_3d().map(|cam| (cam, v.get_visible_rect().size)))
            .map(|(cam, size)| cam.unproject_position(card.get_global_position()).x < size.x * 0.5)
            .unwrap_or(false);
        inspector.bind_mut().show(&def, &abilities, &buffs, right_side);
    }

    /// Neue Stack-Einträge: gewirkte Karte bzw. Quelle der Fähigkeit leuchtet orange auf.
    fn flash_new_stack(&mut self) {
        let items = self.stack_items();
        let ids: HashSet<i64> = items.iter().map(|i| get_int(i, "id")).collect();
        if ids == self.known_stack {
            return;
        }
        for item in items.iter().filter(|i| !self.known_stack.contains(&get_int(i, "id"))) {
            let card_key = item
                .get("card")
                .and_then(|v| v.try_to::<VarDictionary>().ok())
                .map_or(ABILITY_KEY_BASE - get_int(item, "id"), |c| get_int(&c, "id"));
            let source = get_int(item, "source");
            for key in [card_key, source] {
                if let Some(mut c) = self.cards.get(&key).filter(|c| c.is_instance_valid()).cloned() {
                    c.bind_mut().flash();
                }
            }
        }
        self.known_stack = ids;
    }

    /// Kampfansicht: geplante (eigene Erklärung) und erklärte Angreifer (Zustand `combat`) rücken
    /// vor und tragen die Angriffsspitze; alle anderen nicht.
    fn update_combat_look(&mut self) {
        let mut attacking: HashSet<i64> = self.attack_plan.iter().map(|(c, _)| i64::from(*c)).collect();
        if let Some(combat) = self.state.as_ref().and_then(|s| s.get("combat")).and_then(|v| v.try_to::<VarDictionary>().ok()) {
            for a in get_array(&combat, "attackers").unwrap_or_default().iter_shared() {
                if let Ok(a) = a.try_to::<VarDictionary>() {
                    attacking.insert(get_int(&a, "card"));
                }
            }
        }
        for (id, card) in self.cards.iter_mut() {
            let on = attacking.contains(id);
            if card.bind().is_attacking() != on {
                card.bind_mut().set_attacking(on);
            }
        }
    }

    /// Oberste Karte auf dem Stack, die Sitzplatz `seat` kontrolliert (wird gerade gewirkt bzw.
    /// zuletzt gewirkt / aktiviert).
    fn stack_top_card_of(&self, seat: i64) -> Option<Gd<Node3D>> {
        self.stack_items().iter().rev().find(|item| get_int(item, "controller") == seat).and_then(|item| {
            let key = item
                .get("card")
                .and_then(|v| v.try_to::<VarDictionary>().ok())
                .map_or(ABILITY_KEY_BASE - get_int(item, "id"), |c| get_int(&c, "id"));
            self.cards.get(&key).filter(|c| c.is_instance_valid()).map(|c| c.clone().upcast())
        })
    }

    /// Medaillon-Anker eines Sitzplatzes als Knoten.
    /// Anker der Manapool-Anzeige eines Sitzplatzes (`ManaAnchor`), falls vorhanden.
    fn mana_node(&self, seat: u8) -> Option<Gd<Node3D>> {
        let seat = if seat == 0 { self.player_seat.as_ref() } else { self.opponent_seat.as_ref() }?;
        seat.get_node_or_null("ManaAnchor").and_then(|n| n.try_cast::<Node3D>().ok())
    }

    fn hero_node(&self, seat: u8) -> Option<Gd<Node3D>> {
        let seat = if seat == 0 { self.player_seat.as_ref() } else { self.opponent_seat.as_ref() }?;
        seat.get_node_or_null("HeroAnchor").and_then(|n| n.try_cast::<Node3D>().ok())
    }

    /// Kampfschaden: die Quelle schlägt zum Ziel (Spieler-Medaillon bzw. Kreatur/Planeswalker),
    /// beim Treffer roter Einschlag (Assets „Mana Glow Effekte“, Farbe Rot). Je Quelle und
    /// Schadensschritt einmal.
    fn combat_hit(&mut self, event: &VarDictionary, target: Option<Gd<Card3D>>, point: Option<Vector3>, reach: f32) {
        let source = get_int(event, "source");
        let key = (get_int(event, "turn"), get_str(event, "phase"), source);
        if !self.lunged.insert(key) {
            return;
        }
        let (Some(mut card), Some(point)) = (self.cards.get(&source).cloned(), point) else { return };
        card.bind_mut().lunge(point, reach, 0.0);
        let fx = ManaFx::impact(target.map(|t| t.upcast()), point, mana_color("R"), LUNGE_OUT);
        if let Some(mut parent) = self.controller.clone() {
            parent.add_child(&fx);
        }
    }

    /// Pfeilfarbe einer Karte aus der Farbpalette des Pfeil-Assets: einfarbig = ihre Farbe,
    /// mehrfarbig = Gold, farblos bzw. unbekannt = Grau.
    fn arrow_color_of(&self, card: i64) -> ArrowColor {
        let colors = self.card_data(card).map(|c| get_str(&c, "colors")).unwrap_or_default();
        match colors.as_str() {
            "W" => ArrowColor::Gold,
            "U" => ArrowColor::Blue,
            "B" => ArrowColor::Black,
            "R" => ArrowColor::Red,
            "G" => ArrowColor::Green,
            "" => ArrowColor::Gray,
            _ => ArrowColor::Gold,
        }
    }

    /// Darf von Karte `card` jetzt ein Pfeil gezogen werden? Art und gültige Ziele.
    fn aim_kind(&self, card: CardId) -> Option<(AimKind, Vec<Target>)> {
        let req = self.pending.as_ref()?;
        match &req.decision {
            Decision::DeclareAttackers { candidates, defenders, .. } if candidates.contains(&card) => {
                Some((AimKind::Attack, defenders.clone()))
            }
            Decision::DeclareBlockers { candidates, attackers, .. } if candidates.contains(&card) => {
                Some((AimKind::Block, attackers.iter().map(|a| Target::Card(*a)).collect()))
            }
            Decision::ChooseTargets { candidates, .. } if req.source == Some(card) => {
                Some((AimKind::Target, candidates.clone()))
            }
            _ => None,
        }
    }

    /// Worauf der gezogene Pfeil zeigt: Karte unter der Maus, sonst der Spieler, dessen
    /// Medaillon am nächsten am Mauszeiger liegt (höchstens [`AIM_PLAYER_RADIUS`] Pixel).
    fn aim_target(&self, hovered: Option<Gd<Card3D>>) -> Option<Target> {
        if let Some(id) = hovered.and_then(|h| self.forge_id(&h)).and_then(|i| u32::try_from(i).ok()) {
            return Some(Target::Card(id));
        }
        let viewport = self.base().get_viewport()?;
        let cam = viewport.get_camera_3d()?;
        let mouse = viewport.get_mouse_position();
        // nächstes Medaillon im Umkreis
        (0u8..2)
            .filter_map(|seat| {
                let p = self.hero_anchor(seat)?;
                let d = (!cam.is_position_behind(p)).then(|| cam.unproject_position(p).distance_to(mouse))?;
                (d < AIM_PLAYER_RADIUS).then_some((Target::Player(seat), d))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(t, _)| t)
    }

    /// Pfeil, den der Spieler gerade zieht: von der Karte zur Maus bzw. zum gültigen Ziel
    /// darunter (rastet ein). Bei der Zielwahl hängt er ohne Drücken an der Quelle (Zauber auf dem
    /// Stack bzw. Karte der Fähigkeit). `None` = kein Pfeil.
    fn live_arrow(&self) -> Option<(CardId, ArrowSpec)> {
        let controller = self.controller.as_ref()?;
        // Zielwahl: der Pfeil hängt ohne Drücken an der Quelle und folgt der Maus
        let target_source = match self.pending.as_ref() {
            Some(Request { decision: Decision::ChooseTargets { .. }, source: Some(s), .. }) => {
                self.cards.get(&i64::from(*s)).filter(|c| c.is_instance_valid()).cloned()
            }
            _ => None,
        };
        let card = controller.bind().get_aiming_card().or(target_source)?;
        let id = u32::try_from(self.forge_id(&card)?).ok()?;
        let (kind, valid) = self.aim_kind(id)?;
        if kind == AimKind::Attack && valid.len() <= 1 {
            return None; // nur ein Angriffsziel: kein Pfeil, die Karte rückt beim Loslassen vor
        }
        let hovered = controller.bind().get_hovered_card().filter(|h| *h != card);
        let target = self.aim_target(hovered).filter(|t| valid.contains(t));
        let (to, alpha) = match target.and_then(|t| self.target_position(t)) {
            Some(p) => (p, 1.0),
            None => (controller.bind().get_aim_point(), 0.7),
        };
        let color = match kind {
            AimKind::Attack => ArrowColor::Orange,
            AimKind::Block => ArrowColor::Blue,
            AimKind::Target => self.arrow_color_of(i64::from(id)),
        };
        let from = card.get_global_position();
        // fester Schlüssel während des Ziehens: die Biegung läuft ohne Sprung weiter
        let key = arrow_key(99, i64::from(id), Target::Player(u8::MAX));
        let mut spec = arrow(key, from, to, color, alpha, false);
        if target.is_none() {
            spec.to = to; // frei an der Maus: bis zum Mauspunkt
        }
        Some((id, spec))
    }

    /// Pfeile vom Stack-Eintrag (seiner Karte in der Stack-Zone) zu allen seinen Zielen.
    fn stack_arrows(&self) -> Vec<ArrowSpec> {
        let mut arrows = Vec::new();
        for item in self.stack_items() {
            let key = item
                .get("card")
                .and_then(|v| v.try_to::<VarDictionary>().ok())
                .map_or(ABILITY_KEY_BASE - get_int(&item, "id"), |c| get_int(&c, "id"));
            let Some(from) = self.cards.get(&key).filter(|c| c.is_instance_valid()).map(|c| c.get_global_position()) else {
                continue;
            };
            for t in stack_targets(&item) {
                if let Some(to) = self.target_position(t) {
                    let color = self.arrow_color_of(get_int(&item, "source"));
                    arrows.push(arrow(arrow_key(10, key, t), from, to, color, 1.0, true));
                }
            }
        }
        arrows
    }

    /// Mana wird immer automatisch bezahlt (auch mit vorab getapptem Mana im Pool, das Forge dabei
    /// zuerst verwendet) – es gibt kein Fenster zum Bezahlen von Hand.
    fn auto_pay(&mut self, decision: &Decision) -> bool {
        matches!(decision, Decision::PayMana { options, .. } if options.contains(&PayAction::AutoPay))
    }

    /// Antwort für „Sofort beenden“ auf die offene Entscheidung (`None` = selbst entscheiden).
    /// Beendet das automatische Passen bei neuer Runde oder fremdem Stack-Eintrag.
    fn auto_pass_choice(&mut self) -> Option<Choice> {
        let since = self.auto_pass_turn?;
        let turn = self.state.as_ref().map_or(0, |s| get_int(s, "turn"));
        let foreign = self.stack_items().iter().any(|item| get_int(item, "controller") != 0);
        if turn != since || foreign {
            self.auto_pass_turn = None;
            return None;
        }
        match self.pending.as_ref().map(|r| &r.decision) {
            Some(Decision::Priority { .. }) => Some(Choice::Action(Action::Pass)),
            Some(Decision::DeclareAttackers { .. }) => Some(Choice::Attackers(Vec::new())),
            _ => None,
        }
    }

    /// Einträge des Stacks aus dem Zustand (unterster zuerst, oberster zuletzt).
    fn stack_items(&self) -> Vec<VarDictionary> {
        self.state
            .as_ref()
            .and_then(|s| get_array(s, "stack"))
            .map(|a| a.iter_shared().filter_map(|v| v.try_to::<VarDictionary>().ok()).collect())
            .unwrap_or_default()
    }

    /// Kartendaten der Quelle eines Stack-Eintrags (aus allen offenen Zonen; `None` = unbekannt).
    fn card_data(&self, id: i64) -> Option<VarDictionary> {
        let players = self.state.as_ref().and_then(|s| get_array(s, "players")).unwrap_or_default();
        players
            .iter_shared()
            .filter_map(|p| p.try_to::<VarDictionary>().ok())
            .find_map(|p| {
                ["battlefield", "graveyard", "exile", "hand"].iter().find_map(|zone| {
                    get_array(&p, zone)?
                        .iter_shared()
                        .filter_map(|c| c.try_to::<VarDictionary>().ok())
                        .find(|c| get_int(c, "id") == id)
                })
            })
            .or_else(|| {
                // Zauber auf dem Stack (auch „wird gewirkt“)
                self.stack_items()
                    .iter()
                    .filter_map(|item| item.get("card").and_then(|v| v.try_to::<VarDictionary>().ok()))
                    .find(|c| get_int(c, "id") == id)
            })
    }

    /// Kartendaten für die Karte einer Fähigkeit auf dem Stack: Name und Typ der Quelle, keine
    /// Kosten, dazu `source` und `text` des Eintrags.
    fn ability_card(&self, item: &VarDictionary) -> VarDictionary {
        let source = get_int(item, "source");
        let mut d = VarDictionary::new();
        match self.card_data(source) {
            Some(src) => {
                d.set("name", get_str(&src, "name"));
                d.set("type", get_str(&src, "type"));
            }
            None => {
                let name = self.cards.get(&source).filter(|c| c.is_instance_valid()).map(|c| c.bind().card_name().to_string());
                d.set("name", name.unwrap_or_else(|| tr!("Fähigkeit", "Ability").to_string()));
            }
        }
        d.set("source", source);
        d.set("text", get_str(item, "text"));
        d
    }

    /// Alle Pfeile: Kampf, Ziele auf dem Stack (violett), Zielwahl: Quelle → gewählte Ziele (gold),
    /// und der Pfeil, den der Spieler gerade zeichnet ([`Self::live_arrow`]).
    /// Deckt eine offene Bühne gerade den Tisch ab (siehe [method Showcase.is_covering])?
    fn stage_covers(&self) -> bool {
        self.showcase.as_ref().is_some_and(|s| s.is_instance_valid() && s.bind().is_covering())
    }

    fn update_arrows(&mut self) {
        let Some(mut layer) = self.arrows.clone() else { return };
        // Bühne deckt den Tisch ab: keine Pfeile quer über Titel und Karten
        if self.stage_covers() {
            layer.bind_mut().set_arrows(&[]);
            return;
        }
        let mut specs = self.combat_arrows();
        specs.extend(self.stack_arrows());
        if let Some((_, live)) = self.live_arrow() {
            specs.push(live);
        }
        if let Some(req) = self.pending.as_ref() {
            if let Decision::ChooseTargets { candidates, .. } = &req.decision {
                let from = req
                    .source
                    .and_then(|s| self.cards.get(&i64::from(s)))
                    .filter(|c| c.is_instance_valid())
                    .map(|c| c.get_global_position());
                if let Some(from) = from {
                    let targets: Vec<(Target, f32)> =
                        self.selection.iter().filter_map(|i| candidates.get(*i)).map(|t| (*t, 1.0)).collect();
                    let source = req.source.map_or(-1, i64::from);
                    for (t, alpha) in targets {
                        if let Some(to) = self.target_position(t) {
                            let key = arrow_key(20, source, t);
                            let color = self.arrow_color_of(source);
                            specs.push(arrow(key, from, to, color, alpha, !self.drawn_arrows.contains(&key)));
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
        // Exil-Strudel: offen, solange Karten im Exil liegen, sonst ist die Zone zu
        for (seat, p) in players.iter().enumerate().take(2) {
            if let Some(z) = self.zones(seat).exile {
                let mut node = z.upcast::<Node3D>();
                attach_exile_portal(&mut node);
                let open = get_array(p, "exile").is_some_and(|a| !a.is_empty());
                set_exile_portal_open(&mut node, open);
            }
        }
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
            for (key, zone) in [("graveyard", &z.discard), ("exile", &z.exile), ("command", &z.command)] {
                if let Some(zone) = zone {
                    for c in list(key) {
                        wanted.push((get_int(&c, "id"), Some(c), zone.clone(), seat));
                    }
                }
            }
        }
        // Angelegte Karten (CR 301.5, 303.4) liegen in der Zone ihres Trägers – auch auf der
        // Seite des Gegners (Aura auf seiner Kreatur); mehrere Durchläufe für Ketten
        for _ in 0..4 {
            let zone_of: HashMap<i64, Gd<Zone>> = wanted.iter().map(|w| (w.0, w.2.clone())).collect();
            for w in wanted.iter_mut() {
                let host = w.1.as_ref().map(|d| get_int(d, "attached_to")).filter(|h| *h > 0);
                if let Some(zone) = host.and_then(|h| zone_of.get(&h)) {
                    w.2 = zone.clone();
                }
            }
        }
        let attached: Vec<(i64, i64)> = wanted
            .iter()
            .filter_map(|w| w.1.as_ref().map(|d| (w.0, get_int(d, "attached_to"))))
            .collect();

        // Stack (CR 405): Zauber als ihre Karte, Fähigkeiten als Karte ihrer Quelle;
        // unterster Eintrag zuerst, damit der oberste in der Zone oben liegt
        if let Some(stack_zone) = self.stack_zone.clone() {
            for item in self.stack_items() {
                let seat = get_int(&item, "controller").clamp(0, 1) as usize;
                let card = match item.get("card").and_then(|v| v.try_to::<VarDictionary>().ok()) {
                    Some(c) => (get_int(&c, "id"), c),
                    None => (ABILITY_KEY_BASE - get_int(&item, "id"), self.ability_card(&item)),
                };
                wanted.push((card.0, Some(card.1), stack_zone.clone(), seat));
            }
        }

        // Nicht mehr sichtbare Karten entfernen (Bibliothek, Exil …). Verdeckte Gegner-Handkarten
        // werden aufgehoben: Taucht beim Gegner eine neue Karte auf (ausgespielt), nimmt sie deren
        // Platz ein und gleitet so sichtbar aus seiner Hand.
        let keep: HashSet<i64> = wanted.iter().map(|w| w.0).collect();
        let gone: Vec<i64> = self.cards.keys().copied().filter(|k| !keep.contains(k)).collect();
        let mut retap_zones: Vec<Gd<Zone>> = Vec::new();
        let mut from_hidden_hand: [Vec<Gd<Card3D>>; 2] = [Vec::new(), Vec::new()];
        for key in gone {
            self.signatures.remove(&key);
            let Some(card) = self.cards.remove(&key) else { continue };
            if is_hidden_hand_key(key) {
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

        // Ringträger (CR 701.52) aller Spieler, für die Plaketten
        let ring_bearers: Vec<i64> = players.iter().map(|p| get_int(p, "ring_bearer")).filter(|id| *id > 0).collect();
        for (key, data, mut zone, seat) in wanted {
            let card = match self.cards.get(&key) {
                Some(c) => c.clone(),
                None => {
                    let to_hand = self.zones(seat).hand.as_ref() == Some(&zone);
                    // Neue Handkarte: oberste Deckkarte ziehen; Fähigkeit auf dem Stack: auf
                    // ihrer Quelle erscheinen; sonst aus der verdeckten Hand (Gegner spielt aus);
                    // sonst neu über dem Deck erzeugen (z. B. Spielsteine)
                    let c = if key <= ABILITY_KEY_BASE {
                        let mut c = self.spawn_card(seat);
                        let source = data.as_ref().map(|d| get_int(d, "source")).and_then(|s| self.cards.get(&s));
                        if let Some(src) = source.filter(|s| s.is_instance_valid()) {
                            c.bind_mut().snap_to(src.get_global_transform());
                        }
                        c
                    } else {
                        if to_hand { self.deck_cards[seat].pop() } else { None }
                            .or_else(|| from_hidden_hand[seat].pop())
                            .unwrap_or_else(|| self.spawn_card(seat))
                    };
                    self.cards.insert(key, c.clone());
                    c
                }
            };
            let mut card = card;
            if let Some(data) = &data {
                // Werte nur bei Änderung neu setzen (Kartenbild wird dabei neu gezeichnet)
                // Bild nur neu zeichnen, wenn sich Rahmen oder Kosten ändern; Beschriftung bei
                // jeder Änderung der Kartendaten (Text, Werte, Schaden …)
                let def = CardDef::from_state(data);
                let sig = format!("{def:?}");
                if self.signatures.get(&key) != Some(&sig) {
                    let frame = |d: &CardDef| (d.kind, d.cost, d.produces, d.colors.clone(), d.face_down, d.face, d.image.clone());
                    let known = self.signatures.contains_key(&key);
                    let mut c = card.bind_mut();
                    // Seitenwechsel (Transformieren, Umdrehen, Aufdecken; CR 711, 708.8): animiert
                    let turned = known && (c.def().face != def.face || c.def().face_down != def.face_down);
                    if turned {
                        c.flip_to(&def, placeholder::card_face(&def));
                    } else {
                        if !known || frame(c.def()) != frame(&def) {
                            c.set_face_texture(Some(placeholder::card_face(&def)));
                        }
                        c.apply_def(&def);
                    }
                    drop(c);
                    self.signatures.insert(key, sig);
                }
                card.bind_mut().set_badges(&card_badges(data, &ring_bearers));
                card.bind_mut().set_counters(&counter_list(data));
                let tapped = get_bool(data, "tapped");
                if card.bind().is_tapped() != tapped {
                    card.bind_mut().set_tapped_state(tapped);
                    // Drehung setzt die Zone beim Anordnen: Zone neu anordnen (z. B. Länder-Reihe)
                    if let Some(z) = card.bind().get_zone() {
                        if !retap_zones.contains(&z) {
                            retap_zones.push(z);
                        }
                    }
                }
            }
            if card.bind().is_dragging() {
                continue;
            }
            let current = card.bind().get_zone();
            // Karten auf der Bühne (z. B. Starthand beim Mulligan) bleiben dort, bis sie schließt
            if self.showcase.as_ref().is_some_and(|s| s.is_instance_valid() && s.bind().holds(card.clone())) {
                continue;
            }
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
        for mut z in retap_zones {
            if z.is_instance_valid() {
                z.bind_mut().layout();
            }
        }
        for card in from_hidden_hand.into_iter().flatten() {
            discard_card(card);
        }
        // Verbindungen setzen bzw. lösen (Träger in derselben Zone)
        for (key, host) in attached {
            let Some(card) = self.cards.get(&key).cloned() else { continue };
            let Some(mut zone) = card.bind().get_zone() else { continue };
            let host = self.cards.get(&host).cloned().filter(|_| host > 0);
            zone.bind_mut().set_attached(card, host);
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
            let hidden = CardDef::named("?");
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

/// Name der Entscheidung (für [method ForgeTable.get_pending_kind]).
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
        Decision::ChooseStartingPlayer { .. } => "ChooseStartingPlayer",
        Decision::DivideAmount { .. } => "DivideAmount",
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

/// Plaketten einer Karte: Spielstein, Commander, Ringträger, Dungeon-Raum, andere Seite, Marken.
fn card_badges(data: &VarDictionary, ring_bearers: &[i64]) -> String {
    let mut lines: Vec<String> = Vec::new();
    if get_bool(data, "commander") {
        lines.push(tr!("Commander", "Commander").to_string());
    }
    if ring_bearers.contains(&get_int(data, "id")) {
        lines.push(tr!("Ringträger", "Ring-bearer").to_string());
    }
    let room = get_str(data, "room");
    if !room.is_empty() {
        lines.push(trf!("Raum: {room}", "Room: {room}"));
    }
    if let Some(other) = data.get("other_face").and_then(|v| v.try_to::<VarDictionary>().ok()) {
        lines.push(format!("↺ {}", get_str(&other, "name")));
    }
    let rest = counters::card_badges(get_bool(data, "token"));
    if !rest.is_empty() {
        lines.push(rest);
    }
    lines.join("\n")
}

/// Manaart der Manapool-Anzeige (0 = Weiß … 4 = Grün, 5 = Farblos) → Farbe für
/// `PayAction::UsePool` (`Some(None)` = farblos, `None` = ungültig).
fn pool_color(mana: i64) -> Option<Option<crate::rules::Color>> {
    use crate::rules::Color;
    Some(match mana {
        0 => Some(Color::White),
        1 => Some(Color::Blue),
        2 => Some(Color::Black),
        3 => Some(Color::Red),
        4 => Some(Color::Green),
        5 => None,
        _ => return None,
    })
}

/// Schlüssel einer verdeckten Gegner-Handkarte (nicht: Fähigkeit auf dem Stack)?
fn is_hidden_hand_key(key: i64) -> bool {
    key < 0 && key > ABILITY_KEY_BASE
}

/// Alle Ziele eines Stack-Eintrags (`targets` = Liste je Fähigkeit der Kette), ohne Doppelte.
fn stack_targets(item: &VarDictionary) -> Vec<Target> {
    let mut out = Vec::new();
    for part in get_array(item, "targets").unwrap_or_default().iter_shared() {
        let Ok(part) = part.try_to::<VarArray>() else { continue };
        for t in part.iter_shared().filter_map(|t| Target::from_var(&t)) {
            if !out.contains(&t) {
                out.push(t);
            }
        }
    }
    out
}

/// Marken einer Karte bzw. eines Spielers (`counters` = Name → Anzahl), nach Namen sortiert.
fn counter_list(d: &VarDictionary) -> Vec<(String, u32)> {
    let Some(map) = d.get("counters").and_then(|v| v.try_to::<VarDictionary>().ok()) else {
        return Vec::new();
    };
    let mut out: Vec<(String, u32)> = map
        .iter_shared()
        .map(|(k, v)| (k.to_string(), v.try_to::<i64>().unwrap_or(0).max(0) as u32))
        .collect();
    out.sort();
    out
}

fn is_land(card: &VarDictionary) -> bool {
    CardKind::from_type_line(&get_str(card, "type")) == CardKind::Land
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
    fn maps_forge_phases() {
        assert_eq!(phase_index("MAIN1"), 3);
        assert_eq!(phase_index("COMBAT_DECLARE_BLOCKERS"), 5);
        assert_eq!(phase_index("CLEANUP"), 8);
        assert_eq!(phase_index("???"), -1);
    }

    /// Echte Ziele (Lightning Bolt ohne Kreaturen: nur Spieler) nur per Pfeil, Knöpfe je Spieler
    /// nur bei „wähle einen Spieler“ ohne Ziel.
    #[test]
    fn player_buttons_only_for_non_target_choices() {
        let choose = |purpose: &str| Decision::ChooseTargets {
            player: 0,
            source: 5,
            description: String::new(),
            candidates: vec![Target::Player(0), Target::Player(1)],
            min: 1,
            max: 1,
            purpose: purpose.to_string(),
        };
        assert_eq!(ForgeTable::player_pick(&choose("")), None);
        assert_eq!(ForgeTable::player_pick(&choose("Choose")), Some((vec![0, 1], false)));
    }
}

/// Pfeil von `from` nach `to`; beginnt ein Stück vor der Quellkarte und endet kurz vor dem
/// Ziel (die Spitze bleibt sichtbar, auch vor Medaillon und Tischrand).
fn arrow(key: u64, from: Vector3, to: Vector3, color: ArrowColor, alpha: f32, grow: bool) -> ArrowSpec {
    // übereinanderliegende Karten: Bogen, den baut TargetArrows (Enden nicht verschieben)
    if Vector3::new(to.x - from.x, 0.0, to.z - from.z).length() < LOOP_DISTANCE {
        return ArrowSpec { key, from, to, color, alpha, grow };
    }
    let dir = (to - from).normalized_or_zero();
    ArrowSpec { key, from: from + dir * 0.3, to: to - dir * 0.35, color, alpha, grow }
}

/// Art eines Pfeils, den der Spieler zieht.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AimKind {
    Attack,
    Block,
    Target,
}

/// Abstand (Pixel) zwischen Mauszeiger und Medaillon, ab dem ein gezogener Pfeil auf den Spieler
/// einrastet.
const AIM_PLAYER_RADIUS: f32 = 90.0;

/// Pfeile auf Spieler enden so weit vor dem Medaillon-Anker zur Tischmitte hin.
const PLAYER_TARGET_INSET: f32 = 1.0;

/// Schlüssel eines Pfeils über Frames hinweg: Art, Quelle, Ziel.
fn arrow_key(kind: u64, source: i64, target: Target) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    kind.hash(&mut h);
    source.hash(&mut h);
    match target {
        Target::Card(c) => (0u8, c).hash(&mut h),
        Target::Player(p) => (1u8, u32::from(p)).hash(&mut h),
    }
    h.finish()
}

/// Beschriftung einer Spielweise auf der „Wähle eins“-Bühne: Wirken / Land spielen, sonst der
/// Anfang des Fähigkeitstexts der Engine (vor Erklärungen in Klammern, höchstens 28 Zeichen).
fn action_caption(action: Action, label: &str) -> String {
    match action {
        Action::Cast(_) => return tr!("Wirken", "Cast").to_string(),
        Action::PlayLand(_) => return tr!("Land spielen", "Play land").to_string(),
        _ => {}
    }
    let short = label.split(" (").next().unwrap_or(label).trim();
    if short.chars().count() > 28 {
        format!("{}…", short.chars().take(27).collect::<String>())
    } else {
        short.to_string()
    }
}

/// Beschriftung des Bezahlen-Knopfs beim X wählen.
/// Beschriftung des Bestätigen-Knopfs der Zahlen-Bühne: „Bezahlen X = n“ bzw. „Bestätigen: n“.
fn number_label(x: u32, is_x: bool) -> String {
    if is_x {
        trf!("Bezahlen  X = {x}", "Pay  X = {x}")
    } else {
        trf!("Bestätigen:  {x}", "Confirm:  {x}")
    }
}

/// X-Bühne am Stack: Mitte (lokal in der Stack-Zone, etwas unter der obersten Karte und ein
/// Stück nach links, damit „+5“ nicht über den rechten Bildrand ragt), Größe der Bühne in der
/// Szene und Abstand zur Kamera hin (vor den Stack-Karten).
/// Karten-Optionen-Bühne mit bis zu drei Möglichkeiten: Kartenabstand und Bogenradius.
const CARD_OPTION_STEP: f32 = 0.82;
const CARD_OPTION_RADIUS: f32 = 60.0;
/// Größe der Kopie des gewirkten Zaubers am Stack-Platz einer Bühne (`stage_spell_at_stack`).
const STACK_STAGE_SCALE: f32 = 1.1;
/// So weit (Bühnen-Einheiten) rückt die Auswahlreihe nach links, wenn der Zauber am Stack steht.
const STACK_STAGE_ROW_SHIFT: f32 = 0.5;
const X_STAGE_LOCAL: Vector3 = Vector3::new(-0.45, 0.2, 0.15);
const X_STAGE_SCALE: f32 = 2.8;
const X_STAGE_LIFT: f32 = 2.0;
/// X-Bühne (Koordinaten der Bühne, Karte groß darüber): Höhe der Knopfreihe, Abstände der
/// Knöpfe von der Mitte (mit sichtbarer Lücke), Breiten, Höhe von „Abbrechen“.
const X_ROW_Y: f32 = -0.52;
const X_INNER: f32 = 0.82;
const X_OUTER: f32 = 1.22;
const X_SMALL_WIDTH: f32 = 0.6;
const X_PAY_WIDTH: f32 = 1.35;
const X_CANCEL_Y: f32 = -0.84;

/// Welche 3D-Bühne eine Entscheidung bekommt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StageKind {
    /// Modus wählen: Kartenkopie je Modus.
    Modes,
    /// Ja/Nein bzw. zwei Antworten: zwei Knöpfe untereinander.
    Binary,
    /// Optionale Kosten (Kicker …): Karte groß, Knöpfe „Kicker“ / „Normal“.
    OptionalCost,
    /// Hellsicht/Überwachen: Kasten mit „UNTEN“/„OBEN“, Karten dorthin ziehen.
    Scry,
    /// Reihenfolge (Trigger, Karten): Karten nebeneinander, Klick vergibt 1, 2, …
    Order,
    /// Eine von mehreren Möglichkeiten, die je zu einer Karte gehören (Ersatzeffekte …):
    /// Kartenkopie je Möglichkeit, Klick wählt, „Anwenden“.
    CardOptions,
}

/// Hellsicht-Bühne: Mitte und Größe des Kastens (Bühnen-Koordinaten).
const SCRY_FRAME_CENTER: Vector2 = Vector2::new(0.0, 0.08);
const SCRY_FRAME_SIZE: Vector2 = Vector2::new(2.6, 1.32);
/// Hellsicht-Bühne (relativ zur Kastenmitte): Kartengröße, Lage des verdeckten Stapels und wie
/// viele Rückseiten er zeigt, Mitte der Ablagen „unten“ (unter dem Stapel, dahinter), „oben“
/// (auf dem Stapel, davor) und – beim Überwachen – „Friedhof“ (neben dem Stapel), Grenze
/// zwischen den Ablagen und Kartenabstand darin.
const SCRY_CARD_SCALE: f32 = 1.2;
const SCRY_PILE_X: f32 = -0.15;
const SCRY_PILE_SHOWN: usize = 6;
const SCRY_BOTTOM_X: f32 = -0.62;
const SCRY_BOTTOM_Z: f32 = -0.12;
const SCRY_GRAVEYARD_X: f32 = -0.85;
const SCRY_TOP_X: f32 = 0.3;
const SCRY_TOP_Z: f32 = 0.08;
const SCRY_SPLIT_X: f32 = -0.2;
const SCRY_STEP: f32 = 0.42;
/// Kartenwahl-Bühne (Cover-Flow): Kartenabstand (überlappend), Kartengröße, Lage von „Ablehnen“.
const LIB_STEP: f32 = 0.3;
/// Reihenfolge der Segmente im Farbrad (im Uhrzeigersinn ab oben; farblos zuletzt).
const WHEEL_ORDER: &str = "WUBRGC";
/// ✕-Knopf „Wirken abbrechen“ an der Karte: Breite und Abstand vor der Karte (Welteinheiten).
const CANCEL_BADGE_WIDTH: f32 = 1.6;
const CANCEL_BADGE_LIFT: f32 = 0.15;
const LIB_SCALE: f32 = 1.15;
const LIB_DECLINE_POS: Vector2 = Vector2::new(1.75, -0.85);
