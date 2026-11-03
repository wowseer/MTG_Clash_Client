//! Kartentypen, Effekte und Demo-Kartenpool (Platzhalter, bis echte Kartendaten existieren).

use godot::prelude::*;

use super::ability::CardAbility;
use super::mana::{Cost, ManaType};

/// Kartentyp. Bestimmt, wann eine Karte gespielt werden darf (siehe `timing.rs`)
/// und wohin sie beim Ausspielen geht.
#[derive(GodotConvert, Var, Export, Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
#[godot(via = i64)]
pub enum CardKind {
    /// Bleibende Einheit mit Angriff/Verteidigung; greift an und blockt.
    #[default]
    Creature,
    /// Liegt in der Ressourcen-Reihe; Tappen erzeugt 1 Mana der Farbe `produces`.
    Resource,
    /// Sofortzauber: jederzeit spielbar, auch im gegnerischen Zug.
    Instant,
    /// Hexerei: nur in der eigenen Hauptphase bei leerem Stack.
    Sorcery,
    /// Held: bleibend, mit Willenskraft und einmal pro Runde aktivierbaren Fähigkeiten.
    Hero,
    /// Ausrüstung (Artefakt): bleibend, wird in der Hauptphase für Mana an eine Kreatur angelegt.
    Equipment,
    /// Artefakt: bleibend, nur generische Kosten; passiver Bonus oder Tapp-Fähigkeit.
    Artifact,
    /// Verzauberung: bleibend, globaler Effekt (z. B. alle eigenen Kreaturen +1/+1).
    Enchantment,
    /// Aura (Verzauberung): wird zwingend auf eine Kreatur gewirkt und hängt an ihr.
    /// Verlässt die Kreatur das Spielfeld, geht die Aura auf den Friedhof.
    Aura,
}

impl CardKind {
    pub fn title(self) -> &'static str {
        match self {
            CardKind::Creature => tr!("Kreatur", "Creature"),
            CardKind::Resource => tr!("Ressource", "Resource"),
            CardKind::Instant => tr!("Sofortzauber", "Instant"),
            CardKind::Sorcery => tr!("Hexerei", "Sorcery"),
            CardKind::Hero => tr!("Held", "Hero"),
            CardKind::Equipment => tr!("Ausrüstung", "Equipment"),
            CardKind::Artifact => tr!("Artefakt", "Artifact"),
            CardKind::Enchantment => tr!("Verzauberung", "Enchantment"),
            CardKind::Aura => tr!("Aura", "Aura"),
        }
    }

    /// Typzeile auf der Karte (mit Obertyp).
    pub fn type_line(self) -> &'static str {
        match self {
            CardKind::Equipment => tr!("Artefakt – Ausrüstung", "Artifact – Equipment"),
            CardKind::Aura => tr!("Verzauberung – Aura", "Enchantment – Aura"),
            other => other.title(),
        }
    }

    pub fn is_artifact(self) -> bool {
        matches!(self, CardKind::Artifact | CardKind::Equipment)
    }

    pub fn is_enchantment(self) -> bool {
        matches!(self, CardKind::Enchantment | CardKind::Aura)
    }

}

/// Effekt eines Zaubers oder einer Heldenfähigkeit. Die Regeln kommen von Forge, daher
/// vorerst nur `None` (kein eigener Effekttext).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SpellEffect {
    #[default]
    None,
}

impl SpellEffect {
    pub fn description(self) -> String {
        String::new()
    }
}

/// Welche bleibenden Karten eine Aura verzaubern darf ("Verzaubert eine …").
/// Nicht alle Varianten kommen in den Demo-Decks vor – sie stehen für eigene Karten bereit.
#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AuraTarget {
    Creature,
    Resource,
    Artifact,
    Enchantment,
    Hero,
    /// Jede bleibende Karte (Kreatur, Ressource, Artefakt, Ausrüstung, Verzauberung, Aura, Held).
    Permanent,
}

impl AuraTarget {

    pub fn description(self) -> &'static str {
        match self {
            AuraTarget::Creature => tr!("Verzaubert eine Kreatur.", "Enchant creature."),
            AuraTarget::Resource => tr!("Verzaubert eine Ressource.", "Enchant resource."),
            AuraTarget::Artifact => tr!("Verzaubert ein Artefakt.", "Enchant artifact."),
            AuraTarget::Enchantment => tr!("Verzaubert eine Verzauberung.", "Enchant enchantment."),
            AuraTarget::Hero => tr!("Verzaubert einen Helden.", "Enchant hero."),
            AuraTarget::Permanent => tr!("Verzaubert eine bleibende Karte.", "Enchant permanent."),
        }
    }
}

/// Wirkung einer Aura auf die verzauberte Karte.
#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AuraEffect {
    /// Kreatur erhält +a/+d (bzw. −).
    Stats(i32, i32),
    /// Ressource liefert beim Tappen n zusätzliches Mana ihrer Farbe.
    ExtraMana(u32),
    /// Karte wird getappt und enttappt nicht mehr.
    NoUntap,
    /// Held erhält beim Verzaubern n Willenskraft.
    Willpower(i32),
}

impl AuraEffect {
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AuraDef {
    pub target: AuraTarget,
    pub effect: AuraEffect,
}

impl AuraDef {
    pub fn description(&self) -> String {
        let fmt = |v: i32| if v >= 0 { format!("+{v}") } else { format!("−{}", -v) };
        let effect = match self.effect {
            AuraEffect::Stats(a, d) => trf!("Sie erhält {}/{}.", "It gets {}/{}.", fmt(a), fmt(d)),
            AuraEffect::ExtraMana(n) => trf!("Beim Tappen liefert sie {n} zusätzliches Mana.", "When tapped, it produces {n} additional mana."),
            AuraEffect::NoUntap => tr!("Sie wird getappt und enttappt nicht mehr.", "It becomes tapped and no longer untaps.").to_string(),
            AuraEffect::Willpower(n) => trf!("Er erhält {} Willenskraft.", "It gets {} willpower.", fmt(n)),
        };
        format!("{} {effect}", self.target.description())
    }

    /// Kurzwert für das Wertefeld (nur bei Stat-Auren).
    pub fn stats_text(&self) -> String {
        let fmt = |v: i32| if v >= 0 { format!("+{v}") } else { format!("−{}", -v) };
        match self.effect {
            AuraEffect::Stats(a, d) => format!("{}/{}", fmt(a), fmt(d)),
            _ => String::new(),
        }
    }
}

/// Aktivierbare Fähigkeit eines Artefakts: Tappen (+ ggf. Mana) -> Effekt.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct ActivatedAbility {
    pub cost: Cost,
    pub effect: SpellEffect,
}

impl ActivatedAbility {
    pub fn description(&self) -> String {
        let cost = if self.cost.total() > 0 { trf!("{}, Tappen", "{}, Tap", self.cost.to_text()) } else { tr!("Tappen", "Tap").into() };
        format!("{cost}: {}", self.effect.description())
    }
}

/// Aktivierbare Heldenfähigkeit: ändert die Willenskraft um `delta`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct HeroAbility {
    pub delta: i32,
    pub effect: SpellEffect,
}

impl HeroAbility {
    pub fn description(self) -> String {
        let sign = if self.delta >= 0 { "+" } else { "−" };
        format!("{sign}{}: {}", self.delta.abs(), self.effect.description())
    }
}

/// Werte einer Ausrüstung.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct EquipStats {
    pub attack: i32,
    pub defense: i32,
    /// Kosten zum Anlegen.
    pub cost: Cost,
}

impl EquipStats {
    pub fn description(&self) -> String {
        trf!(
            "Angelegte Kreatur erhält +{}/+{}. Anlegen: {}", "Equipped creature gets +{}/+{}. Equip: {}",
            self.attack,
            self.defense,
            self.cost.to_text()
        )
    }
}

#[derive(Clone)]
pub struct CardDef {
    pub name: String,
    pub kind: CardKind,
    pub cost: Cost,
    pub attack: i32,
    pub defense: i32,
    pub ability: CardAbility,
    /// Nur für Ressourcen: erzeugte Manafarbe.
    pub produces: ManaType,
    /// Sofortzauber / Hexerei.
    pub effect: SpellEffect,
    /// Held: Start-Willenskraft und Fähigkeiten.
    pub willpower: i32,
    pub hero_abilities: Vec<HeroAbility>,
    /// Ausrüstung.
    pub equip: Option<EquipStats>,
    /// Artefakt: Tapp-Fähigkeit.
    pub activated: Option<ActivatedAbility>,
    /// Aura: Zielklasse und Wirkung.
    pub aura: Option<AuraDef>,
}

impl CardDef {
    pub fn base(name: impl Into<String>, kind: CardKind, cost: Cost) -> Self {
        Self {
            name: name.into(),
            kind,
            cost,
            attack: 0,
            defense: 0,
            ability: CardAbility::None,
            produces: ManaType::Colorless,
            effect: SpellEffect::None,
            willpower: 0,
            hero_abilities: Vec::new(),
            equip: None,
            activated: None,
            aura: None,
        }
    }

    pub fn frame_color(&self) -> Color {
        if self.kind == CardKind::Resource {
            return self.produces.color();
        }
        let colors: Vec<ManaType> = ManaType::ALL
            .into_iter()
            .filter(|t| t.is_colored() && self.cost.specific.get(*t) > 0)
            .collect();
        match colors.as_slice() {
            [] => ManaType::Colorless.color(),
            [one] => one.color(),
            _ => Color::from_rgb(0.85, 0.7, 0.3),
        }
    }
}
