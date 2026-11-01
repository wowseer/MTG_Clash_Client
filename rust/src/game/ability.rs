//! Kartenfähigkeiten.

use godot::prelude::*;

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
            Self::None => "Kreatur",
            Self::Regeneration => "Kreatur – Heiler",
            Self::Firebreath => "Kreatur – Feuer",
            Self::Rage => "Kreatur – Raufbold",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::None => "",
            Self::Regeneration => "Auffrischung: Du erhältst 1 Leben.",
            Self::Firebreath => "Auffrischung: 1 Schaden am Gegner.",
            Self::Rage => "Greift sie an, erhält sie +1 Angriff bis Rundenende.",
        }
    }
}
