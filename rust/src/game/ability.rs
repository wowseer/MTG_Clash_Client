//! Kartenfähigkeiten.

use godot::prelude::*;

/// Fähigkeit einer Kreatur. Als Zahl: 0 = keine, 1 = Heiler, 2 = Feuer, 3 = Raufbold.
#[derive(GodotConvert, Var, Export, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[godot(via = i64)]
pub enum CardAbility {
    #[default]
    None,
    /// Auffrischung: Besitzer erhält 1 Leben.
    Regeneration,
    /// Auffrischung: 1 Schaden am Gegner.
    Firebreath,
    /// Beim Angriff +1 Angriff bis Rundenende (temporärer Effekt).
    Rage,
}

impl CardAbility {
    pub fn title(self) -> &'static str {
        match self {
            Self::None => tr!("Kreatur", "Creature"),
            Self::Regeneration => tr!("Kreatur – Heiler", "Creature – Healer"),
            Self::Firebreath => tr!("Kreatur – Feuer", "Creature – Fire"),
            Self::Rage => tr!("Kreatur – Raufbold", "Creature – Brawler"),
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::None => "",
            Self::Regeneration => tr!("Auffrischung: Du erhältst 1 Leben.", "Upkeep: You gain 1 life."),
            Self::Firebreath => tr!("Auffrischung: 1 Schaden am Gegner.", "Upkeep: 1 damage to the opponent."),
            Self::Rage => tr!("Greift sie an, erhält sie +1 Angriff bis Rundenende.", "When it attacks, it gets +1 attack until end of turn."),
        }
    }
}
