//! TurnManager: treibt die Phasen-State-Machine an.
//!
//! - Startet die Partie nach `start_delay` Sekunden (Zeit fürs Austeilen).
//! - Nicht wartende Phasen schalten nach ihrer Verzögerung automatisch weiter.
//! - Wartende Phasen (Mensch handelt) schalten mit `next_phase()` weiter
//!   (Button „Nächste Phase“ im HUD oder Leertaste).
//! - Der TableController fragt `can_drag`, `resolve_drop` und `on_card_clicked`.
//! - Am Ende jeder Phase verfällt das Mana in den Pools.

use godot::prelude::*;

use super::game_state::GameState;
use super::phases::{self, Phase, PhaseState};
use super::cards::CardKind;
use super::mana::{Cost, ManaType};
use super::timing::{check_sorcery_timing, PlayError};
use crate::table::card3d::CardMarker;
use super::seat::PlayerSeat;
use super::stack::{StackItem, StackManager};
use crate::table::card3d::{Card3D, ATTACK_GLOW, BLOCK_GLOW, READY_GLOW};
use crate::table::target_arrows::ArrowSpec;

/// Länge des Angriffspfeils (vom Angreifer nach vorn, Einheiten).
const ATTACK_ARROW_LENGTH: f32 = 2.4;
use crate::table::zone::Zone;

/// Steuert die Partie: Rundenablauf als State Machine (Enttappen → Auffrischung → Ziehen →
/// Hauptphase 1 → Angreifer bestimmen → Blocker zuweisen → Schadensberechnung →
/// Hauptphase 2 → Ende), die Prioritätsschleife des Stacks und alle Spielregeln
/// (Timing je Kartentyp, Mana, Kampf, Auren, zustandsbasierte Aktionen, KI).
/// Schnittstelle für [TableController], [TurnHud] und eigene Skripte.
/// Sitzplätze werden als Zahl angegeben: 0 = Spieler (unten), 1 = Gegner (oben).
#[derive(GodotClass)]
#[class(base=Node)]
pub struct TurnManager {
    /// Sitzplatz des menschlichen Spielers (unten).
    #[export]
    player_seat: Option<Gd<PlayerSeat>>,
    /// Sitzplatz des Gegners (oben, normalerweise KI).
    #[export]
    opponent_seat: Option<Gd<PlayerSeat>>,
    /// Karten, die in der Zieh-Phase gezogen werden.
    #[export]
    draw_count: i32,
    /// Wartezeit vor dem ersten Zug (Austeilen der Starthände).
    #[export]
    start_delay: f32,
    /// Faktor für alle automatischen Phasen-Verzögerungen.
    #[export]
    phase_delay_scale: f32,
    /// Stack mit Prioritätsschleife für Zauber und Fähigkeiten.
    #[export]
    stack_manager: Option<Gd<StackManager>>,
    /// Zone, in der Zauberkarten liegen, solange sie auf dem Stack sind.
    #[export]
    stack_zone: Option<Gd<Zone>>,
    /// Bedenkzeit der KI, bevor sie passt oder antwortet (Sekunden).
    #[export]
    priority_delay: f32,
    /// Passt der Mensch automatisch, wenn er keinen spielbaren Sofortzauber hat?
    /// Standard aus: Der Stack läuft, bis der Mensch „Weiter“ drückt.
    #[export]
    auto_pass_human: bool,

    game: Option<GameState>,
    state: Option<Box<dyn PhaseState>>,
    timer: f32,
    priority_timer: f32,
    last_stack_size: i64,
    /// KI-Hauptphase fortsetzen, sobald der Stack wieder leer ist.
    ai_continue: bool,

    base: Base<Node>,
}

#[godot_api]
impl INode for TurnManager {
    fn init(base: Base<Node>) -> Self {
        Self {
            player_seat: None,
            opponent_seat: None,
            draw_count: 1,
            start_delay: 1.8,
            phase_delay_scale: 1.0,
            game: None,
            state: None,
            stack_manager: None,
            stack_zone: None,
            priority_delay: 0.9,
            auto_pass_human: false,
            timer: 0.0,
            priority_timer: 0.0,
            last_stack_size: 0,
            ai_continue: false,
            base,
        }
    }

    fn process(&mut self, delta: f64) {
        let dt = delta as f32;
        if self.game.is_none() {
            self.start_delay -= dt;
            if self.start_delay <= 0.0 {
                self.start_game();
            }
            return;
        }
        // Zustandsbasierte Aktionen (Auren ohne Ziel, tote Kreaturen, globale Boni)
        if self.phase() != Some(Phase::GameOver) {
            if let Some(g) = self.game.as_mut() {
                g.check_state_based_actions();
            }
        }
        if self.update_stack(dt) {
            return; // Phase pausiert, solange etwas auf dem Stack liegt
        }
        let (Some(game), Some(state)) = (self.game.as_ref(), self.state.as_ref()) else {
            return;
        };
        if state.phase() == Phase::GameOver || state.waits_for_input(game) {
            return;
        }
        self.timer -= dt;
        if self.timer <= 0.0 {
            self.advance();
        }
    }
}

#[godot_api]
impl TurnManager {
    /// Neue Phase betreten (Index in Phase::ORDER, -1 = Spielende).
    #[signal]
    fn phase_changed(index: i64, title: GString);

    /// Die Partie ist entschieden; `winner` ist der Name des Gewinners.
    #[signal]
    fn game_over(winner: GString);

    /// Startet die Partie (der Spieler beginnt).
    #[func]
    pub fn start_game(&mut self) {
        let (Some(p), Some(o)) = (self.player_seat.clone(), self.opponent_seat.clone()) else {
            godot_error!("TurnManager: player_seat und opponent_seat müssen gesetzt sein");
            return;
        };
        let resolver = Callable::from_object_method(&self.to_gd(), "resolve_stack_item");
        self.game = Some(GameState::new(
            p,
            o,
            self.draw_count,
            self.stack_manager.clone(),
            self.stack_zone.clone(),
            resolver,
        ));
        self.enter(Phase::Untap);
    }

    /// Button „Nächste Phase“: nur wirksam, wenn die Phase auf einen Menschen wartet.
    #[func]
    pub fn next_phase(&mut self) {
        if !self.is_waiting_for_input() {
            return;
        }
        if self.stack_is_busy() {
            // Bei vollem Stack heißt der Button "Passen"
            let seat = self.priority_seat();
            self.pass_priority(seat);
        } else {
            self.advance();
        }
    }

    /// Wird vom StackManager beim Auflösen eines Items aufgerufen.
    #[func]
    pub fn resolve_stack_item(&mut self, item: Gd<StackItem>) {
        if let Some(g) = self.game.as_mut() {
            g.resolve_item(&item);
        }
    }

    /// Alle Elemente auf dem Stack, unterstes zuerst (für die Stack-Anzeige).
    #[func]
    pub fn get_stack_items(&self) -> Array<Gd<StackItem>> {
        self.stack_manager.as_ref().map(|s| s.bind().get_items()).unwrap_or_default()
    }

    /// Stack-Einträge als Text, oberster zuerst.
    #[func]
    pub fn get_stack_lines(&self) -> PackedStringArray {
        self.game
            .as_ref()
            .map(|g| g.stack_lines().iter().map(|l| GString::from(l.as_str())).collect())
            .unwrap_or_default()
    }

    /// Spieler mit Priorität bei vollem Stack (-1 = Stack leer).
    #[func]
    pub fn get_priority_seat(&self) -> i64 {
        if self.stack_is_busy() { self.priority_seat() as i64 } else { -1 }
    }

    /// Beschriftung für den HUD-Button.
    #[func]
    pub fn get_button_text(&self) -> GString {
        if self.stack_is_busy() {
            tr!("Weiter (passen)  »", "Continue (pass)  »").into()
        } else {
            tr!("Nächste Phase  »", "Next phase  »").into()
        }
    }

    /// Wartet das Spiel auf eine Eingabe des Menschen (Button „Nächste Phase“ bzw. „Passen“)?
    #[func]
    pub fn is_waiting_for_input(&self) -> bool {
        if self.stack_is_busy() {
            let seat = self.priority_seat();
            return self.game.as_ref().is_some_and(|g| g.is_human(seat));
        }
        match (&self.game, &self.state) {
            (Some(g), Some(s)) => s.waits_for_input(g),
            _ => false,
        }
    }

    /// Index der aktuellen Phase in [method get_phase_titles] (−1 = noch nicht gestartet oder Spielende).
    #[func]
    pub fn get_phase_index(&self) -> i64 {
        self.phase().map_or(-1, Phase::index)
    }

    /// Name der aktuellen Phase.
    #[func]
    pub fn get_phase_title(&self) -> GString {
        self.phase().map_or(tr!("Austeilen …", "Dealing …"), Phase::title).into()
    }

    /// Namen aller Phasen in ihrer Reihenfolge.
    #[func]
    pub fn get_phase_titles(&self) -> PackedStringArray {
        Phase::ORDER.iter().map(|p| GString::from(p.title())).collect()
    }

    /// Hinweistext für das HUD: was der Spieler gerade tun kann.
    #[func]
    pub fn get_hint(&self) -> GString {
        if self.stack_is_busy() {
            let seat = self.priority_seat();
            let text = match &self.game {
                Some(g) if g.is_human(seat) => {
                    tr!("Du hast Priorität: Sofortzauber spielen oder „Weiter“ drücken. Das oberste Element wird erst aufgelöst, wenn beide Spieler weiter gedrückt haben.", "You have priority: play an instant or press “Continue”. The top item only resolves once both players have continued.").to_string()
                }
                Some(g) => trf!("{} hat Priorität …", "{} has priority …", g.name(seat)),
                None => String::new(),
            };
            return GString::from(text.as_str());
        }
        match (&self.game, &self.state) {
            (Some(g), Some(s)) => GString::from(s.hint(g).as_str()),
            _ => tr!("Karten werden ausgeteilt …", "Dealing cards …").into(),
        }
    }

    /// Die letzten Protokolleinträge, ältester zuerst.
    #[func]
    pub fn get_log(&self) -> PackedStringArray {
        self.game
            .as_ref()
            .map(|g| g.log.iter().map(|l| GString::from(l.as_str())).collect())
            .unwrap_or_default()
    }

    /// Rundennummer (jeder Zug eines Spielers zählt als eine Runde).
    #[func]
    pub fn get_turn(&self) -> i64 {
        self.game.as_ref().map_or(0, |g| g.turn as i64)
    }

    /// 0 = Spieler, 1 = Gegner, -1 = noch nicht gestartet.
    #[func]
    pub fn get_active_seat(&self) -> i64 {
        self.game.as_ref().map_or(-1, |g| g.active as i64)
    }

    /// Name des Sitzplatzes `seat` (0 = Spieler, 1 = Gegner).
    #[func]
    pub fn get_seat_name(&self, seat: i64) -> GString {
        self.seat(seat).map_or(GString::new(), |s| GString::from(crate::i18n::name(&s.bind().display_name.to_string()).as_str()))
    }

    /// Lebenspunkte von `seat` (0 = Spieler, 1 = Gegner).
    #[func]
    pub fn get_life(&self, seat: i64) -> i64 {
        self.seat(seat).map_or(0, |s| s.bind().life as i64)
    }

    /// Ressourcen-Karten: (ungetappt, gesamt).
    #[func]
    pub fn get_resource_counts(&self, seat: i64) -> Vector2i {
        let Some(g) = &self.game else {
            return Vector2i::ZERO;
        };
        let seat = seat.clamp(0, 1) as usize;
        let total = GameState::cards_in(g.resource_zone(seat)).len();
        Vector2i::new(g.untapped_resources(seat).len() as i32, total as i32)
    }

    /// Manapool als BBCode (farbig) für das HUD, z. B. "[color=#e64d33]Rot 2[/color]".
    #[func]
    pub fn get_pool_bbcode(&self, seat: i64) -> GString {
        let Some(pool) = self.seat(seat).and_then(|s| s.bind().mana_pool.clone()) else {
            return GString::new();
        };
        if self.seat(seat).is_some_and(|s| s.bind().infinite_mana) {
            return GString::from(tr!("[color=#b48cff]unendlich (Debug)[/color]", "[color=#b48cff]infinite (debug)[/color]"));
        }
        let mana = pool.bind().mana();
        let parts: Vec<String> = ManaType::ALL
            .into_iter()
            .filter(|t| mana.get(*t) > 0)
            .map(|t| format!("[color=#{}]{} {}[/color]", t.color().to_html_without_alpha(), t.name(), mana.get(t)))
            .collect();
        let text = if parts.is_empty() { "–".to_string() } else { parts.join("  ") };
        GString::from(text.as_str())
    }

    /// Name des Gewinners (leer, solange die Partie läuft).
    #[func]
    pub fn get_winner_name(&self) -> GString {
        match self.game.as_ref().and_then(|g| g.winner.map(|w| g.name(w))) {
            Some(n) => GString::from(n.as_str()),
            None => GString::new(),
        }
    }

    // -------------------------------------------------------- Schnittstelle zum TableController

    /// Darf der Mensch diese Karte greifen? (eigene Hand / eigenes Spielfeld)
    #[func]
    pub fn can_drag(&self, card: Gd<Card3D>, zone: Gd<Zone>) -> bool {
        let _ = card;
        let Some(g) = &self.game else {
            return false;
        };
        match g.seat_of_zone(&zone) {
            Some(seat) => {
                g.is_human(seat) && (Some(&zone) == g.hand(seat).as_ref() || Some(&zone) == g.battlefield(seat).as_ref())
            }
            None => false,
        }
    }

    // -------------------------------------------------------- Timing-Regeln (Kartentypen)

    /// Darf `seat` (0 = Spieler, 1 = Gegner) diese Handkarte jetzt spielen?
    /// Prüft Zug, Phase, Stack und Mana je nach Kartentyp (siehe `timing.rs`).
    #[func]
    pub fn can_play_card(&self, card: Gd<Card3D>, seat: i64) -> bool {
        self.play_error(&card, seat).is_none()
    }

    /// Grund, warum die Karte nicht gespielt werden darf ("" = erlaubt).
    #[func]
    pub fn get_play_error(&self, card: Gd<Card3D>, seat: i64) -> GString {
        GString::from(self.play_error(&card, seat).unwrap_or_default().as_str())
    }

    /// Wird vor dem Ablegen gefragt und liefert die tatsächliche Zielzone
    /// (null = nicht erlaubt, Karte gleitet zurück).
    /// - Umsortieren innerhalb einer Zone: immer erlaubt.
    /// - Hand -> eigene Tischzone: Timing-Regeln je Kartentyp; Ressourcen landen in
    ///   der Reihe, Zauber werden aufgelöst und landen auf dem Ablagestapel.
    #[func]
    pub fn resolve_drop(&mut self, card: Gd<Card3D>, from: Gd<Zone>, to: Gd<Zone>) -> Option<Gd<Zone>> {
        self.drop_impl(card, from, to, None)
    }

    /// Wie `resolve_drop`, mit Ablagepunkt (Welt) als Wunschplatz für bleibende Karten.
    #[func]
    pub fn resolve_drop_at(&mut self, card: Gd<Card3D>, from: Gd<Zone>, to: Gd<Zone>, point: Vector3) -> Option<Gd<Zone>> {
        self.drop_impl(card, from, to, Some(point))
    }

    /// Kann von dieser Karte ein Zielpfeil gezogen werden? (Zielwahl: eigene ungetappte Kreatur
    /// beim Blocken, eigene Ausrüstung in der eigenen Hauptphase.)
    #[func]
    pub fn can_aim_from(&self, card: Gd<Card3D>) -> bool {
        let phase = self.phase().unwrap_or(Phase::Untap);
        let Some(g) = self.game.as_ref() else { return false };
        let c = card.bind();
        // Ressourcen erzeugen nie Pfeile (Klick = Mana)
        if c.is_resource() {
            return false;
        }
        let defender = g.defender();
        if phase == Phase::Blockers && g.is_human(defender) && !self.stack_is_busy() {
            return g.is_on_battlefield(&card, defender) && c.is_creature() && !c.is_tapped();
        }
        let Some(owner) = g.seat_of_card(&card) else { return false };
        c.kind() == CardKind::Equipment
            && g.is_human(owner)
            && g.is_on_battlefield(&card, owner)
            && check_sorcery_timing(&g.play_context(owner, phase, &Cost::default())).is_ok()
    }

    /// Zielpfeil von `source` auf `target` losgelassen: Blocker zuweisen bzw. Ausrüstung anlegen.
    /// true = ausgeführt (bei ungültigem Ziel steht der Grund im Protokoll).
    #[func]
    pub fn aim_at(&mut self, source: Gd<Card3D>, mut target: Gd<Card3D>) -> bool {
        if !self.can_aim_from(source.clone()) {
            return false;
        }
        let phase = self.phase().unwrap_or(Phase::Untap);
        let Some(g) = self.game.as_mut() else { return false };
        if phase == Phase::Blockers {
            return g.assign_block(source, target);
        }
        let Some(owner) = g.seat_of_card(&source) else { return false };
        let mut eq = source;
        g.clear_pending();
        if !(g.is_on_battlefield(&target, owner) && target.bind().is_creature()) {
            return false;
        }
        if let Err(e) = g.equip(owner, &mut eq, &mut target, phase) {
            g.log(trf!("Anlegen nicht möglich: {}", "Cannot equip: {}", e.message()));
        }
        true
    }

    /// Darf diese Aura die Karte `target` verzaubern? (für die Zielmarkierung beim Ziehen)
    #[func]
    pub fn is_legal_aura_target(&self, aura: Gd<Card3D>, target: Gd<Card3D>) -> bool {
        self.game.as_ref().is_some_and(|g| g.legal_aura_target(&aura, &target))
    }

    /// Aura aus der Hand auf eine bleibende Karte ziehen. Liefert die Zielzone (Stack) oder null.
    #[func]
    pub fn resolve_aura_drop(&mut self, card: Gd<Card3D>, from: Gd<Zone>, target: Gd<Card3D>) -> Option<Gd<Zone>> {
        let phase = self.phase().unwrap_or(Phase::Untap);
        let g = self.game.as_mut()?;
        let seat = g.seat_of_zone(&from)?;
        if !g.is_human(seat) || Some(&from) != g.hand(seat).as_ref() || card.bind().kind() != CardKind::Aura {
            return None;
        }
        g.clear_pending();
        g.cast_aura(seat, &card, &target, phase)
    }
}

impl TurnManager {
    /// Karte, die gerade ein Ziel wählt (nach einem Klick): ausgewählter Blocker oder
    /// Ausrüstung, die angelegt werden soll. Von ihr führt ein Zielpfeil zum Mauszeiger.
    pub fn aim_source(&self) -> Option<Gd<Card3D>> {
        let g = self.game.as_ref()?;
        let selected = if self.phase() == Some(Phase::Blockers) { g.combat.selected.clone() } else { None };
        selected.or_else(|| g.pending_equip.clone()).filter(|c| c.is_instance_valid())
    }

    /// Dauerhafte Pfeile des Kampfs: jeder Angreifer zeigt nach vorn (Richtung Gegner),
    /// jeder Blocker auf seinen Angreifer.
    pub fn combat_arrows(&self) -> Vec<ArrowSpec> {
        let Some(g) = self.game.as_ref() else { return Vec::new() };
        let mut arrows = Vec::new();
        for a in g.combat.attackers.iter().filter(|c| c.is_instance_valid() && c.bind().is_creature()) {
            // Spieler (Sitz 0) greift nach -z an, der Gegner nach +z
            let forward = if g.seat_of_card(a) == Some(0) { Vector3::FORWARD } else { Vector3::BACK };
            let p = a.get_global_position();
            arrows.push(ArrowSpec { from: p + forward * 0.5, to: p + forward * ATTACK_ARROW_LENGTH, color: ATTACK_GLOW });
        }
        for (b, a) in g.combat.blocks.iter().filter(|(b, a)| b.is_instance_valid() && a.is_instance_valid()) {
            let (from, to) = (b.get_global_position(), a.get_global_position());
            let dir = (to - from).normalized_or_zero();
            arrows.push(ArrowSpec { from: from + dir * 0.35, to: to - dir * 0.3, color: BLOCK_GLOW });
        }
        arrows
    }

    /// Ist `target` ein gültiges Ziel für den Zielpfeil von `source`? Nur dann rastet der Pfeil
    /// auf der Karte ein (Ressourcen und andere ungültige Karten nie).
    pub fn is_valid_aim_target(&self, source: &Gd<Card3D>, target: &Gd<Card3D>) -> bool {
        let Some(g) = self.game.as_ref() else { return false };
        if source == target || target.bind().is_resource() {
            return false;
        }
        if self.phase() == Some(Phase::Blockers) && source.bind().is_creature() {
            return g.combat.attackers.contains(target);
        }
        let Some(owner) = g.seat_of_card(source) else { return false };
        source.bind().kind() == CardKind::Equipment
            && target.bind().is_creature()
            && g.is_on_battlefield(target, owner)
            && source.bind().equipped_to.as_ref() != Some(target)
    }

    /// Setzt das dezente Leuchten „kann handeln“ auf allen Karten der Spielfelder: in der
    /// Angreifer-Phase des Menschen goldener Rand für Kreaturen, die angreifen können, in seiner
    /// Blocker-Phase blauer Rand für mögliche Blocker. Sonst leuchtet nichts.
    pub fn refresh_ready_glow(&self) {
        let Some(g) = self.game.as_ref() else { return };
        let phase = self.phase();
        let busy = self.stack_is_busy();
        let ready: Vec<Gd<Card3D>> = match phase {
            Some(Phase::Attackers) if g.is_human(g.active) && !busy => g
                .untapped_creatures(g.active)
                .into_iter()
                .filter(|c| c.bind().get_current_attack() > 0 && !g.combat.attackers.contains(c))
                .collect::<Vec<_>>(),
            Some(Phase::Blockers) if g.is_human(g.defender()) && !busy => g
                .untapped_creatures(g.defender())
                .into_iter()
                .filter(|c| !g.combat.blocks.iter().any(|(b, _)| b == c) && g.combat.selected.as_ref() != Some(c))
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        };
        // Angreifer-Phase: Gold (kann angreifen); Blocker-Phase: Blau (kann blocken)
        let color = if phase == Some(Phase::Blockers) { BLOCK_GLOW } else { READY_GLOW };
        for seat in 0..2 {
            for mut card in GameState::cards_in(g.battlefield(seat)) {
                let on = ready.contains(&card);
                card.bind_mut().set_ready_color(on.then_some(color));
            }
        }
    }

    fn drop_impl(&mut self, card: Gd<Card3D>, from: Gd<Zone>, to: Gd<Zone>, point: Option<Vector3>) -> Option<Gd<Zone>> {
        if from == to {
            return Some(to);
        }
        let phase = self.phase().unwrap_or(Phase::Untap);
        let g = self.game.as_mut()?;
        let seat = g.seat_of_zone(&from)?;
        let own_target = g.seat_of_zone(&to) == Some(seat);
        if !g.is_human(seat) || Some(&from) != g.hand(seat).as_ref() || !own_target {
            return None;
        }
        g.clear_pending();
        g.play_from_hand_at(seat, &card, phase, point)
    }
}

#[godot_api(secondary)]
impl TurnManager {

    /// Klick auf eine Karte. true = Klick verbraucht (kein Drag starten).
    /// - eigene Ressource in der Reihe: tappen (+1 Mana), jederzeit; erneuter Klick auf eine so
    ///   getappte Ressource nimmt den Tapp zurück, solange ihr Mana unbenutzt im Pool liegt
    /// - eigener Held: Fähigkeitenauswahl im HUD öffnen
    /// - eigene Ausrüstung: auswählen, danach eigene Kreatur anklicken = anlegen
    /// - sonst entscheidet die Phase (z. B. Kampfauswahl)
    #[func]
    pub fn on_card_clicked(&mut self, mut card: Gd<Card3D>) -> bool {
        let phase = self.phase().unwrap_or(Phase::Untap);
        let (Some(g), Some(s)) = (self.game.as_mut(), self.state.as_mut()) else {
            return false;
        };
        let zone = card.bind().get_zone();
        let in_row = |seat: usize| zone.is_some() && zone == g.resource_zone(seat);
        if card.bind().is_resource() && (in_row(0) || in_row(1)) {
            if (0..2).any(|seat| in_row(seat) && g.is_human(seat)) && phase != Phase::GameOver {
                // Getappt und Mana noch unbenutzt im Pool: Tapp zurücknehmen
                if card.bind().is_tapped() {
                    g.untap_unused_mana(&mut card);
                } else {
                    g.tap_for_mana_by_hand(&mut card);
                }
            }
            return true;
        }

        let Some(owner) = g.seat_of_card(&card) else {
            return false;
        };
        let on_own_field = g.is_human(owner) && g.is_on_battlefield(&card, owner);
        let kind = card.bind().kind();

        // Ausrüstung anlegen: zweiter Klick auf eine eigene Kreatur
        if let Some(mut eq) = g.pending_equip.clone() {
            if on_own_field && kind == CardKind::Creature {
                if let Err(e) = g.equip(owner, &mut eq, &mut card, phase) {
                    g.log(trf!("Anlegen nicht möglich: {}", "Cannot equip: {}", e.message()));
                }
                g.clear_pending();
                return true;
            }
            g.clear_pending();
            if eq == card {
                return true; // erneuter Klick = Auswahl aufheben
            }
        }
        if on_own_field && kind == CardKind::Equipment {
            let ctx = g.play_context(owner, phase, &Cost::default());
            match check_sorcery_timing(&ctx) {
                Ok(()) => {
                    card.bind_mut().set_marker(CardMarker::Selected);
                    g.pending_equip = Some(card);
                }
                Err(e) => g.log(trf!("Anlegen nicht möglich: {}", "Cannot equip: {}", e.message())),
            }
            return true;
        }
        if on_own_field && kind == CardKind::Hero {
            g.clear_pending();
            g.pending_hero = Some(card);
            return true;
        }
        if on_own_field && kind == CardKind::Artifact {
            if card.bind().activated.is_some() {
                if let Err(e) = g.activate_artifact(owner, &mut card, phase) {
                    g.log(trf!("Fähigkeit nicht möglich: {}", "Ability not possible: {}", e.message()));
                }
                return true;
            }
            return false; // Artefakt ohne Fähigkeit: verschieben erlaubt
        }
        if matches!(kind, CardKind::Hero | CardKind::Equipment | CardKind::Aura) {
            return true; // nicht verschieben (Auren hängen an ihrer Kreatur)
        }
        s.on_card_clicked(g, card)
    }

    // -------------------------------------------------------- Heldenfähigkeiten (HUD)

    /// Name des Helden mit offener Fähigkeitenauswahl ("" = keine).
    #[func]
    pub fn get_hero_choice_name(&self) -> GString {
        self.game
            .as_ref()
            .and_then(|g| g.pending_hero.as_ref())
            .map_or(GString::new(), |h| GString::from(h.bind().display_name().as_str()))
    }

    /// Fähigkeiten des gewählten Helden als Text; nicht verfügbare mit Grund.
    #[func]
    pub fn get_hero_choices(&self) -> PackedStringArray {
        let phase = self.phase().unwrap_or(Phase::Untap);
        let Some(g) = &self.game else {
            return PackedStringArray::new();
        };
        let Some(hero) = &g.pending_hero else {
            return PackedStringArray::new();
        };
        let seat = g.seat_of_card(hero).unwrap_or(0);
        let abilities = hero.bind().hero_abilities.clone();
        abilities
            .iter()
            .enumerate()
            .map(|(i, a)| {
                let text = match g.check_hero(seat, hero, i, phase) {
                    Ok(_) => a.description(),
                    Err(e) => format!("{}  ({})", a.description(), e.message()),
                };
                GString::from(text.as_str())
            })
            .collect()
    }

    /// Ist Fähigkeit `index` des gewählten Helden gerade aktivierbar?
    #[func]
    pub fn is_hero_choice_available(&self, index: i64) -> bool {
        let phase = self.phase().unwrap_or(Phase::Untap);
        let Some(g) = &self.game else {
            return false;
        };
        let Some(hero) = &g.pending_hero else {
            return false;
        };
        let seat = g.seat_of_card(hero).unwrap_or(0);
        g.check_hero(seat, hero, index.max(0) as usize, phase).is_ok()
    }

    /// Aktiviert Fähigkeit `index` des gewählten Helden. true = ausgeführt.
    #[func]
    pub fn activate_hero_ability(&mut self, index: i64) -> bool {
        let phase = self.phase().unwrap_or(Phase::Untap);
        let Some(g) = self.game.as_mut() else {
            return false;
        };
        let Some(mut hero) = g.pending_hero.take() else {
            return false;
        };
        let Some(seat) = g.seat_of_card(&hero) else {
            return false;
        };
        match g.activate_hero(seat, &mut hero, index.max(0) as usize, phase) {
            Ok(()) => true,
            Err(e) => {
                g.log(trf!("Fähigkeit nicht möglich: {}", "Ability not possible: {}", e.message()));
                false
            }
        }
    }

    /// Schließt das Auswahlfenster für Heldenfähigkeiten ohne Aktivierung.
    #[func]
    pub fn cancel_hero_choice(&mut self) {
        if let Some(g) = self.game.as_mut() {
            g.pending_hero = None;
        }
    }
}

impl TurnManager {
    fn play_error(&self, card: &Gd<Card3D>, seat: i64) -> Option<String> {
        let phase = self.phase().unwrap_or(Phase::Untap);
        let g = self.game.as_ref()?;
        let seat = seat.clamp(0, 1) as usize;
        // Liegt die Karte in einer Zone, muss es die eigene Hand sein
        // (während des Ziehens hat sie keine Zone).
        let zone = card.bind().get_zone();
        if zone.is_some() && zone != g.hand(seat) {
            return Some(PlayError::NotInHand.message().to_string());
        }
        g.check_card(seat, card, phase).err().map(|e| e.message().to_string())
    }

    fn stack_is_busy(&self) -> bool {
        self.stack_manager.as_ref().is_some_and(|s| !s.bind().is_empty())
    }

    fn priority_seat(&self) -> usize {
        self.stack_manager.as_ref().map_or(0, |s| s.bind().get_priority_player().clamp(0, 1) as usize)
    }

    /// Passen über den StackManager. Löst ggf. das oberste Item auf; die Callable
    /// ruft dabei `resolve_stack_item` auf diesem Node auf (Re-Entrancy über base_mut).
    fn pass_priority(&mut self, seat: usize) {
        let Some(mut sm) = self.stack_manager.clone() else {
            return;
        };
        let _guard = self.base_mut();
        sm.bind_mut().pass_priority(seat as i64);
    }

    /// Prioritätsschleife. true = Stack ist belegt, die Phase pausiert.
    fn update_stack(&mut self, dt: f32) -> bool {
        let phase = self.phase().unwrap_or(Phase::Untap);
        let size = self.stack_manager.as_ref().map_or(0, |s| s.bind().size());
        if size != self.last_stack_size {
            // Neues Item oder Auflösung: kurze Bedenkzeit
            if size == 0 && self.last_stack_size > 0 {
                let ai_turn = self.game.as_ref().is_some_and(|g| !g.is_human(g.active));
                self.ai_continue = ai_turn && phase.is_main();
            }
            self.last_stack_size = size;
            self.priority_timer = self.priority_delay;
        }

        // Spielende während der Auflösung
        if self.game.as_ref().is_some_and(|g| g.winner.is_some()) && phase != Phase::GameOver {
            if let Some(mut sm) = self.stack_manager.clone() {
                sm.bind_mut().clear();
            }
            self.advance();
            return true;
        }

        if size == 0 {
            // KI-Hauptphase fortsetzen (nächste Stack-Aktion), sobald der Stack leer ist
            if self.ai_continue {
                self.ai_continue = false;
                if let Some(g) = self.game.as_mut() {
                    let seat = g.active;
                    if !g.is_human(seat) && phase.is_main() && g.ai_play_cards(seat, phase) {
                        return true;
                    }
                }
                self.timer = self.timer.max(0.8 * self.phase_delay_scale);
            }
            return false;
        }

        self.priority_timer -= dt;
        if self.priority_timer > 0.0 {
            return true;
        }
        self.priority_timer = self.priority_delay;
        let seat = self.priority_seat();
        let Some(g) = self.game.as_mut() else {
            return true;
        };
        if g.is_human(seat) {
            // Der Mensch passt nur per Button „Weiter“ – außer `auto_pass_human` ist an
            // und er hat keinen spielbaren Sofortzauber.
            let can_answer = GameState::cards_in(g.hand(seat))
                .iter()
                .any(|c| c.bind().kind() == CardKind::Instant && g.check_card(seat, c, phase).is_ok());
            if self.auto_pass_human && !can_answer {
                self.pass_priority(seat);
            }
        } else if !g.ai_respond(seat, phase) {
            self.pass_priority(seat);
        }
        true
    }

    fn phase(&self) -> Option<Phase> {
        self.state.as_ref().map(|s| s.phase())
    }

    fn seat(&self, seat: i64) -> Option<Gd<PlayerSeat>> {
        match seat {
            0 => self.player_seat.clone(),
            1 => self.opponent_seat.clone(),
            _ => None,
        }
    }

    fn advance(&mut self) {
        let (Some(game), Some(state)) = (self.game.as_mut(), self.state.as_mut()) else {
            return;
        };
        // Mana verfällt erst am Ende des Zuges (nicht nach jeder Phase)
        let end_of_turn = state.phase() == Phase::End;
        state.exit(game);
        if end_of_turn {
            game.empty_pools();
        }
        game.clear_pending();
        let next = if game.winner.is_some() { Phase::GameOver } else { state.next(game) };
        self.enter(next);
    }

    fn enter(&mut self, phase: Phase) {
        let Some(game) = self.game.as_mut() else {
            return;
        };
        // Neue Phase: aktiver Spieler erhält Priorität
        if let Some(mut sm) = self.stack_manager.clone() {
            sm.bind_mut().set_active_player(game.active as i64);
        }
        let mut state = phases::create(phase);
        state.enter(game);
        self.timer = state.auto_delay(game) * self.phase_delay_scale;
        let winner = game.winner.map(|w| game.name(w));
        self.state = Some(state);

        self.signals().phase_changed().emit(phase.index(), &GString::from(phase.title()));
        if phase == Phase::GameOver {
            if let Some(w) = winner {
                self.signals().game_over().emit(&GString::from(w.as_str()));
            }
        } else if self.game.as_ref().is_some_and(|g| g.winner.is_some()) {
            // Sieg während der Phase (z. B. Schaden) -> sofort zum Spielende.
            self.advance();
        }
    }
}
