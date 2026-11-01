//! Timing-Regeln: wann darf welcher Kartentyp gespielt bzw. aktiviert werden?
//!
//! Reine Logik ohne Godot-Objekte (per `cargo test` geprüft). Der TurnManager baut
//! aus dem Spielzustand einen [`PlayContext`] und fragt [`check_play`].
//!
//! | Typ          | Wer / wann                                   | Stack  | Mana |
//! |--------------|----------------------------------------------|--------|------|
//! | Ressource    | eigener Zug, Hauptphase, max. 1 pro Runde    | leer   | –    |
//! | Sofortzauber | jederzeit (auch im gegnerischen Zug)         | egal   | ja   |
//! | Hexerei      | eigener Zug, Hauptphase                      | leer   | ja   |
//! | Kreatur      | eigener Zug, Hauptphase                      | leer   | ja   |
//! | Held         | eigener Zug, Hauptphase                      | leer   | ja   |
//! | Ausrüstung   | eigener Zug, Hauptphase                      | leer   | ja   |
//! | Artefakt     | eigener Zug, Hauptphase                      | leer   | ja   |
//! | Verzauberung | eigener Zug, Hauptphase                      | leer   | ja   |
//! | Aura         | eigener Zug, Hauptphase, braucht Kreatur-Ziel| leer   | ja   |
//!
//! Alle Karten außer Ressourcen gehen beim Ausspielen auf den Stack; bleibende
//! Karten kommen erst beim Auflösen aufs Spielfeld.
//!
//! Aktivierungen (Heldenfähigkeit, Ausrüstung anlegen) gelten wie Hexereien:
//! eigener Zug, Hauptphase, leerer Stack; Held max. 1× pro Runde.

use super::cards::CardKind;
use super::phases::Phase;

/// Momentaufnahme des Spielzustands aus Sicht eines Spielers.
#[derive(Clone, Copy, Debug)]
pub struct PlayContext {
    /// Ist der Spieler am Zug?
    pub own_turn: bool,
    pub phase: Phase,
    pub stack_empty: bool,
    /// Wurde in dieser Runde schon eine Ressource gelegt?
    pub resource_played: bool,
    /// Reicht Pool + ungetappte Ressourcen für die Kosten?
    pub can_afford: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlayError {
    GameOver,
    NotYourTurn,
    WrongPhase,
    StackNotEmpty,
    ResourceLimit,
    NotEnoughMana,
    AlreadyActivated,
    NotEnoughWillpower,
    NoTarget,
    NotInHand,
}

impl PlayError {
    pub fn message(self) -> &'static str {
        match self {
            PlayError::GameOver => "Die Partie ist beendet.",
            PlayError::NotYourTurn => "Nur im eigenen Zug.",
            PlayError::WrongPhase => "Nur in einer Hauptphase.",
            PlayError::StackNotEmpty => "Nur wenn der Stack leer ist.",
            PlayError::ResourceLimit => "Nur eine Ressourcen-Karte pro Runde.",
            PlayError::NotEnoughMana => "Nicht genug Mana.",
            PlayError::AlreadyActivated => "Dieser Held war in dieser Runde schon aktiv.",
            PlayError::NotEnoughWillpower => "Nicht genug Willenskraft.",
            PlayError::NoTarget => "Kein gültiges Ziel.",
            PlayError::NotInHand => "Die Karte ist nicht auf deiner Hand.",
        }
    }
}

/// "Hexerei-Timing": eigener Zug, Hauptphase, leerer Stack.
pub fn check_sorcery_timing(ctx: &PlayContext) -> Result<(), PlayError> {
    if ctx.phase == Phase::GameOver {
        return Err(PlayError::GameOver);
    }
    if !ctx.own_turn {
        return Err(PlayError::NotYourTurn);
    }
    if !ctx.phase.is_main() {
        return Err(PlayError::WrongPhase);
    }
    if !ctx.stack_empty {
        return Err(PlayError::StackNotEmpty);
    }
    Ok(())
}

/// Darf eine Karte dieses Typs jetzt aus der Hand gespielt werden?
pub fn check_play(kind: CardKind, ctx: &PlayContext) -> Result<(), PlayError> {
    match kind {
        CardKind::Instant => {
            if ctx.phase == Phase::GameOver {
                return Err(PlayError::GameOver);
            }
        }
        CardKind::Resource => {
            check_sorcery_timing(ctx)?;
            if ctx.resource_played {
                return Err(PlayError::ResourceLimit);
            }
            return Ok(()); // Ressourcen kosten nichts
        }
        CardKind::Sorcery
        | CardKind::Creature
        | CardKind::Hero
        | CardKind::Equipment
        | CardKind::Artifact
        | CardKind::Enchantment
        | CardKind::Aura => {
            check_sorcery_timing(ctx)?;
        }
    }
    if !ctx.can_afford {
        return Err(PlayError::NotEnoughMana);
    }
    Ok(())
}

/// Darf eine Aura auf dieses Ziel gewirkt werden?
pub fn check_aura(ctx: &PlayContext, target_is_creature_on_field: bool) -> Result<(), PlayError> {
    check_play(CardKind::Aura, ctx)?;
    if !target_is_creature_on_field {
        return Err(PlayError::NoTarget);
    }
    Ok(())
}

/// Darf eine Heldenfähigkeit aktiviert werden?
pub fn check_hero_activation(ctx: &PlayContext, used_this_turn: bool, willpower: i32, delta: i32) -> Result<(), PlayError> {
    check_sorcery_timing(ctx)?;
    if used_this_turn {
        return Err(PlayError::AlreadyActivated);
    }
    if willpower + delta < 0 {
        return Err(PlayError::NotEnoughWillpower);
    }
    Ok(())
}

/// Darf eine Ausrüstung angelegt werden?
pub fn check_equip(ctx: &PlayContext, has_target: bool) -> Result<(), PlayError> {
    check_sorcery_timing(ctx)?;
    if !has_target {
        return Err(PlayError::NoTarget);
    }
    if !ctx.can_afford {
        return Err(PlayError::NotEnoughMana);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(own_turn: bool, phase: Phase) -> PlayContext {
        PlayContext { own_turn, phase, stack_empty: true, resource_played: false, can_afford: true }
    }

    #[test]
    fn instant_any_time() {
        for phase in [Phase::Upkeep, Phase::Attackers, Phase::Blockers, Phase::End, Phase::Main1] {
            assert_eq!(check_play(CardKind::Instant, &ctx(false, phase)), Ok(()));
        }
        let mut c = ctx(false, Phase::Blockers);
        c.stack_empty = false;
        assert_eq!(check_play(CardKind::Instant, &c), Ok(()));
    }

    #[test]
    fn sorcery_speed_types_need_own_main_and_empty_stack() {
        for kind in [
            CardKind::Sorcery,
            CardKind::Creature,
            CardKind::Hero,
            CardKind::Equipment,
            CardKind::Artifact,
            CardKind::Enchantment,
            CardKind::Aura,
        ] {
            assert_eq!(check_play(kind, &ctx(true, Phase::Main1)), Ok(()));
            assert_eq!(check_play(kind, &ctx(true, Phase::Main2)), Ok(()));
            assert_eq!(check_play(kind, &ctx(false, Phase::Main1)), Err(PlayError::NotYourTurn));
            assert_eq!(check_play(kind, &ctx(true, Phase::Attackers)), Err(PlayError::WrongPhase));
            let mut c = ctx(true, Phase::Main1);
            c.stack_empty = false;
            assert_eq!(check_play(kind, &c), Err(PlayError::StackNotEmpty));
        }
    }

    #[test]
    fn mana_is_checked() {
        let mut c = ctx(true, Phase::Main1);
        c.can_afford = false;
        assert_eq!(check_play(CardKind::Creature, &c), Err(PlayError::NotEnoughMana));
        assert_eq!(check_play(CardKind::Instant, &c), Err(PlayError::NotEnoughMana));
        // Ressourcen kosten nichts
        assert_eq!(check_play(CardKind::Resource, &c), Ok(()));
    }

    #[test]
    fn one_resource_per_turn() {
        let mut c = ctx(true, Phase::Main2);
        c.resource_played = true;
        assert_eq!(check_play(CardKind::Resource, &c), Err(PlayError::ResourceLimit));
    }

    #[test]
    fn nothing_after_game_over() {
        assert_eq!(check_play(CardKind::Instant, &ctx(true, Phase::GameOver)), Err(PlayError::GameOver));
        assert_eq!(check_play(CardKind::Creature, &ctx(true, Phase::GameOver)), Err(PlayError::GameOver));
    }

    #[test]
    fn hero_activation_rules() {
        let c = ctx(true, Phase::Main1);
        assert_eq!(check_hero_activation(&c, false, 3, -3), Ok(()));
        assert_eq!(check_hero_activation(&c, false, 2, -3), Err(PlayError::NotEnoughWillpower));
        assert_eq!(check_hero_activation(&c, true, 3, 1), Err(PlayError::AlreadyActivated));
        assert_eq!(check_hero_activation(&ctx(false, Phase::Main1), false, 3, 1), Err(PlayError::NotYourTurn));
    }

    #[test]
    fn aura_needs_creature_target() {
        let c = ctx(true, Phase::Main1);
        assert_eq!(check_aura(&c, true), Ok(()));
        assert_eq!(check_aura(&c, false), Err(PlayError::NoTarget));
        assert_eq!(check_aura(&ctx(false, Phase::Main1), true), Err(PlayError::NotYourTurn));
    }

    #[test]
    fn equip_rules() {
        let c = ctx(true, Phase::Main1);
        assert_eq!(check_equip(&c, true), Ok(()));
        assert_eq!(check_equip(&c, false), Err(PlayError::NoTarget));
        assert_eq!(check_equip(&ctx(true, Phase::Blockers), true), Err(PlayError::WrongPhase));
    }
}
