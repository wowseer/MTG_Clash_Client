//! Rundenablauf als State Machine: jede Phase ist ein eigener Zustand.
//!
//! ```text
//! Untap → Upkeep → Draw → Main1 → Attackers ─┬→ Blockers → Damage ─┬→ Main2 → End ─┐
//!   ↑                                        └──(keine Angreifer)──┘                │
//!   └──────────────────── nächster Spieler ──────────────────────────────────────────┘
//! ```
//! Phasen, in denen ein Mensch handelt (`waits_for_input`), warten auf den
//! Button „Nächste Phase“. Alle anderen laufen nach `auto_delay` Sekunden weiter.

use godot::prelude::*;

use super::game_state::GameState;
use crate::table::card3d::Card3D;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    Untap,
    Upkeep,
    Draw,
    Main1,
    Attackers,
    Blockers,
    Damage,
    Main2,
    End,
    GameOver,
}

impl Phase {
    /// Phasen in Anzeigereihenfolge (ohne GameOver).
    pub const ORDER: [Phase; 9] = [
        Phase::Untap,
        Phase::Upkeep,
        Phase::Draw,
        Phase::Main1,
        Phase::Attackers,
        Phase::Blockers,
        Phase::Damage,
        Phase::Main2,
        Phase::End,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Phase::Untap => tr!("Enttappen", "Untap"),
            Phase::Upkeep => tr!("Auffrischung", "Upkeep"),
            Phase::Draw => tr!("Ziehen", "Draw"),
            Phase::Main1 => tr!("Hauptphase 1", "Main phase 1"),
            Phase::Attackers => tr!("Angreifer bestimmen", "Declare attackers"),
            Phase::Blockers => tr!("Blocker zuweisen", "Declare blockers"),
            Phase::Damage => tr!("Schadensberechnung", "Combat damage"),
            Phase::Main2 => tr!("Hauptphase 2", "Main phase 2"),
            Phase::End => tr!("Ende", "End"),
            Phase::GameOver => tr!("Spielende", "Game over"),
        }
    }

    pub fn index(self) -> i64 {
        Self::ORDER.iter().position(|p| *p == self).map_or(-1, |i| i as i64)
    }

    pub fn is_main(self) -> bool {
        matches!(self, Phase::Main1 | Phase::Main2)
    }
}

/// Ein Zustand der Runden-State-Machine.
pub trait PhaseState {
    fn phase(&self) -> Phase;
    /// Wird beim Betreten der Phase aufgerufen.
    fn enter(&mut self, _g: &mut GameState) {}
    /// Wird beim Verlassen der Phase aufgerufen.
    fn exit(&mut self, _g: &mut GameState) {}
    /// Wartet die Phase auf den Button eines menschlichen Spielers?
    fn waits_for_input(&self, _g: &GameState) -> bool {
        false
    }
    /// Verzögerung, bevor eine nicht wartende Phase automatisch weiterschaltet.
    fn auto_delay(&self, _g: &GameState) -> f32 {
        0.6
    }
    /// Klick auf eine Karte; true = Klick wurde verbraucht (kein Drag).
    fn on_card_clicked(&mut self, _g: &mut GameState, _card: Gd<Card3D>) -> bool {
        false
    }
    /// Hinweistext für das HUD.
    fn hint(&self, _g: &GameState) -> String {
        String::new()
    }
    /// Folgezustand.
    fn next(&self, g: &GameState) -> Phase;
}

pub fn create(phase: Phase) -> Box<dyn PhaseState> {
    match phase {
        Phase::Untap => Box::new(UntapPhase),
        Phase::Upkeep => Box::new(UpkeepPhase),
        Phase::Draw => Box::new(DrawPhase),
        Phase::Main1 => Box::new(MainPhase { second: false }),
        Phase::Attackers => Box::new(AttackersPhase),
        Phase::Blockers => Box::new(BlockersPhase),
        Phase::Damage => Box::new(DamagePhase),
        Phase::Main2 => Box::new(MainPhase { second: true }),
        Phase::End => Box::new(EndPhase),
        Phase::GameOver => Box::new(GameOverPhase),
    }
}

// ------------------------------------------------------------------ 1. Enttappen

struct UntapPhase;

impl PhaseState for UntapPhase {
    fn phase(&self) -> Phase {
        Phase::Untap
    }
    fn enter(&mut self, g: &mut GameState) {
        let seat = g.active;
        g.log(trf!("— Runde {}: {} ist am Zug —", "— Turn {}: {} to play —", g.turn, g.name(seat)));
        g.untap(seat);
    }
    fn auto_delay(&self, _g: &GameState) -> f32 {
        0.7
    }
    fn next(&self, _g: &GameState) -> Phase {
        Phase::Upkeep
    }
}

// ------------------------------------------------------------------ 2. Auffrischung

struct UpkeepPhase;

impl PhaseState for UpkeepPhase {
    fn phase(&self) -> Phase {
        Phase::Upkeep
    }
    fn enter(&mut self, g: &mut GameState) {
        g.trigger_upkeep(g.active);
    }
    fn next(&self, _g: &GameState) -> Phase {
        Phase::Draw
    }
}

// ------------------------------------------------------------------ 3. Ziehen

struct DrawPhase;

impl PhaseState for DrawPhase {
    fn phase(&self) -> Phase {
        Phase::Draw
    }
    fn enter(&mut self, g: &mut GameState) {
        // Der Startspieler zieht in seiner allerersten Runde nicht.
        if g.turn == 1 {
            return;
        }
        g.draw(g.active, g.draw_count);
    }
    fn auto_delay(&self, _g: &GameState) -> f32 {
        0.8
    }
    fn next(&self, _g: &GameState) -> Phase {
        Phase::Main1
    }
}

// ------------------------------------------------------------------ 4./6. Hauptphasen

struct MainPhase {
    second: bool,
}

impl PhaseState for MainPhase {
    fn phase(&self) -> Phase {
        if self.second { Phase::Main2 } else { Phase::Main1 }
    }
    fn enter(&mut self, g: &mut GameState) {
        if !g.is_human(g.active) {
            let phase = self.phase();
            g.ai_play_cards(g.active, phase);
        }
    }
    fn waits_for_input(&self, g: &GameState) -> bool {
        g.is_human(g.active)
    }
    fn auto_delay(&self, _g: &GameState) -> f32 {
        1.4
    }
    fn hint(&self, g: &GameState) -> String {
        if g.is_human(g.active) {
            tr!("Karten aus der Hand auf deine Tischhälfte ziehen. Klick auf Ressource = Mana, auf Held = Fähigkeit, auf Ausrüstung und dann Kreatur = anlegen.", "Drag cards from your hand onto your half of the table. Click a resource = mana, a hero = ability, equipment and then a creature = equip.").into()
        } else {
            trf!("{} spielt Karten aus …", "{} is playing cards …", g.name(g.active))
        }
    }
    fn next(&self, _g: &GameState) -> Phase {
        if self.second { Phase::End } else { Phase::Attackers }
    }
}

// ------------------------------------------------------------------ 5a. Angreifer bestimmen

struct AttackersPhase;

impl PhaseState for AttackersPhase {
    fn phase(&self) -> Phase {
        Phase::Attackers
    }
    fn enter(&mut self, g: &mut GameState) {
        if !g.is_human(g.active) {
            g.ai_declare_attackers();
        }
    }
    fn exit(&mut self, g: &mut GameState) {
        g.commit_attackers();
    }
    fn waits_for_input(&self, g: &GameState) -> bool {
        g.is_human(g.active)
    }
    fn auto_delay(&self, _g: &GameState) -> f32 {
        1.2
    }
    fn on_card_clicked(&mut self, g: &mut GameState, card: Gd<Card3D>) -> bool {
        if !g.is_human(g.active) {
            return false;
        }
        if g.is_on_battlefield(&card, g.active) {
            g.toggle_attacker(card);
            return true;
        }
        false
    }
    fn hint(&self, g: &GameState) -> String {
        if g.is_human(g.active) {
            tr!("Klicke ungetappte Kreaturen an, die angreifen sollen.", "Click untapped creatures that should attack.").into()
        } else {
            trf!("{} wählt Angreifer …", "{} is choosing attackers …", g.name(g.active))
        }
    }
    fn next(&self, g: &GameState) -> Phase {
        if g.combat.attackers.is_empty() { Phase::Main2 } else { Phase::Blockers }
    }
}

// ------------------------------------------------------------------ 5b. Blocker zuweisen

struct BlockersPhase;

impl PhaseState for BlockersPhase {
    fn phase(&self) -> Phase {
        Phase::Blockers
    }
    fn enter(&mut self, g: &mut GameState) {
        if !g.is_human(g.defender()) {
            g.ai_declare_blockers();
        }
    }
    fn exit(&mut self, g: &mut GameState) {
        if let Some(mut sel) = g.combat.selected.take() {
            sel.bind_mut().set_marker(crate::table::card3d::CardMarker::None);
        }
    }
    fn waits_for_input(&self, g: &GameState) -> bool {
        g.is_human(g.defender())
    }
    fn auto_delay(&self, _g: &GameState) -> f32 {
        1.2
    }
    fn on_card_clicked(&mut self, g: &mut GameState, card: Gd<Card3D>) -> bool {
        if !g.is_human(g.defender()) {
            return false;
        }
        g.blocker_click(card)
    }
    fn hint(&self, g: &GameState) -> String {
        if g.is_human(g.defender()) {
            tr!("Klicke eine eigene Kreatur und dann den Angreifer, den sie blocken soll.", "Click one of your creatures, then the attacker it should block.").into()
        } else {
            trf!("{} weist Blocker zu …", "{} is assigning blockers …", g.name(g.defender()))
        }
    }
    fn next(&self, _g: &GameState) -> Phase {
        Phase::Damage
    }
}

// ------------------------------------------------------------------ 5c. Schadensberechnung

struct DamagePhase;

impl PhaseState for DamagePhase {
    fn phase(&self) -> Phase {
        Phase::Damage
    }
    fn enter(&mut self, g: &mut GameState) {
        g.resolve_combat();
    }
    fn exit(&mut self, g: &mut GameState) {
        g.clear_combat();
    }
    fn auto_delay(&self, _g: &GameState) -> f32 {
        1.5
    }
    fn next(&self, _g: &GameState) -> Phase {
        Phase::Main2
    }
}

// ------------------------------------------------------------------ 7. Ende

struct EndPhase;

impl PhaseState for EndPhase {
    fn phase(&self) -> Phase {
        Phase::End
    }
    fn enter(&mut self, g: &mut GameState) {
        g.cleanup();
    }
    fn exit(&mut self, g: &mut GameState) {
        // Aktiven Spieler wechseln
        g.active = g.defender();
        g.turn += 1;
    }
    fn next(&self, _g: &GameState) -> Phase {
        Phase::Untap
    }
}

// ------------------------------------------------------------------ Spielende

struct GameOverPhase;

impl PhaseState for GameOverPhase {
    fn phase(&self) -> Phase {
        Phase::GameOver
    }
    fn auto_delay(&self, _g: &GameState) -> f32 {
        f32::INFINITY
    }
    fn hint(&self, g: &GameState) -> String {
        match g.winner {
            Some(w) => trf!("{} gewinnt die Partie!", "{} wins the game!", g.name(w)),
            None => tr!("Spielende", "Game over").into(),
        }
    }
    fn next(&self, _g: &GameState) -> Phase {
        Phase::GameOver
    }
}
