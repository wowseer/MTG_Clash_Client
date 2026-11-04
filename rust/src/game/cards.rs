//! Kartendaten für Kartenbild und Anordnung, gelesen aus den Kartendaten der Engine (`state`).
//! Keine Regeln: Werte, Text und Typ kommen fertig von der Engine; hier wird nur bestimmt, wie
//! die Karte aussieht (Rahmenfarbe, Typsymbol, Wertefeld) und wo sie liegt.

use godot::prelude::*;

use super::card_text;
use super::mana::{Cost, ManaType};

/// Hauptkategorie einer Karte (aus der Typzeile, CR 205.2). Bestimmt Typsymbol, Wertefeld und
/// Platz auf dem Tisch (Länder in der Länder-Reihe, Kreaturen in der Mitte, Planeswalker und
/// Schlachten links, alles andere rechts).
#[derive(GodotConvert, Var, Export, Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
#[godot(via = i64)]
pub enum CardKind {
    #[default]
    Creature,
    Land,
    Instant,
    Sorcery,
    Planeswalker,
    /// Artefakt – Ausrüstung.
    Equipment,
    Artifact,
    Enchantment,
    /// Verzauberung – Aura.
    Aura,
    /// Schlacht (CR 310).
    Battle,
}

impl CardKind {
    /// Kategorie aus einer englischen Typzeile (z. B. „Artifact Creature — Golem“). Reihenfolge
    /// nach Vorrang: Land vor Kreatur (Artefaktländer), Kreatur vor Artefakt/Verzauberung.
    pub fn from_type_line(ty: &str) -> CardKind {
        let has = |w: &str| ty.split(|c: char| !c.is_alphanumeric()).any(|t| t == w);
        if has("Land") {
            CardKind::Land
        } else if has("Creature") {
            CardKind::Creature
        } else if has("Planeswalker") {
            CardKind::Planeswalker
        } else if has("Battle") {
            CardKind::Battle
        } else if has("Instant") {
            CardKind::Instant
        } else if has("Sorcery") {
            CardKind::Sorcery
        } else if has("Aura") {
            CardKind::Aura
        } else if has("Equipment") {
            CardKind::Equipment
        } else if has("Enchantment") {
            CardKind::Enchantment
        } else {
            CardKind::Artifact
        }
    }
}

/// Alles, was das Kartenbild braucht.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct CardDef {
    pub name: String,
    /// Typzeile wie von der Engine (Gedankenstrich als „—“).
    pub type_line: String,
    pub kind: CardKind,
    /// Manakosten für die Kostensymbole.
    pub cost: Cost,
    /// Farben der Karte (CR 105), leer = farblos.
    pub colors: Vec<ManaType>,
    /// Regeltext (Symbole wie `{T}` noch nicht ersetzt).
    pub text: String,
    /// Aktuelle und aufgedruckte Stärke/Widerstand (nur Kreaturen).
    pub stats: Option<(i32, i32)>,
    pub base_stats: Option<(i32, i32)>,
    /// Loyalität (Planeswalker) bzw. Verteidigung (Schlachten).
    pub loyalty: Option<i32>,
    pub defense: Option<i32>,
    /// Erlittener Schaden in diesem Zug.
    pub damage: i32,
    /// Verdeckt (Morph, Manifest …, CR 708): Bild mit Rückseitenmuster, Werte der verdeckten Seite.
    pub face_down: bool,
    /// Angezeigte Seite doppelseitiger Karten (0 = Vorderseite, 1 = Rückseite / umgedreht).
    pub face: u8,
    /// Nur Länder: Manafarbe für Rahmen und Spalte der Länder-Reihe (aus dem Grundlandtyp bzw.
    /// der einzigen Farbe; sonst farblos).
    pub produces: ManaType,
}

impl CardDef {
    /// Karte ohne Werte (z. B. verdeckt: Name „?“).
    pub fn named(name: impl Into<String>) -> Self {
        Self { name: name.into(), ..Self::default() }
    }

    /// Liest die Kartendaten des Protokolls: `name`, `type`, `cost`, `colors`, `text`, `power`,
    /// `toughness`, `base_power`, `base_toughness`, `loyalty`, `defense`, `damage`, `face_down`,
    /// `face` (fehlende Felder = leer).
    pub fn from_state(card: &VarDictionary) -> Self {
        let s = |k: &str| card.get(k).filter(|v| !v.is_nil()).map(|v| v.to_string()).unwrap_or_default();
        let i = |k: &str| card.get(k).and_then(|v| v.try_to::<i64>().ok()).map(|v| v as i32);
        let pair = |a: &str, b: &str| i(a).zip(i(b));
        let type_line = s("type").replace(" - ", " — ");
        let kind = CardKind::from_type_line(&type_line);
        let colors: Vec<ManaType> = s("colors").chars().filter_map(ManaType::from_wubrg).filter(|t| t.is_colored()).collect();
        let produces = if kind == CardKind::Land {
            [
                ("Plains", ManaType::White),
                ("Island", ManaType::Blue),
                ("Swamp", ManaType::Black),
                ("Mountain", ManaType::Red),
                ("Forest", ManaType::Green),
            ]
            .into_iter()
            .find(|(n, _)| type_line.contains(n))
            .map(|(_, t)| t)
            .or_else(|| (colors.len() == 1).then(|| colors[0]))
            .unwrap_or(ManaType::Colorless)
        } else {
            ManaType::Colorless
        };
        let stats = pair("power", "toughness");
        let face_down = card.get("face_down").and_then(|v| v.try_to::<bool>().ok()).unwrap_or(false);
        let name = match s("name") {
            n if n.is_empty() && face_down => tr!("Verdeckt", "Face-down").to_string(),
            n => n,
        };
        Self {
            name,
            face_down,
            face: i("face").unwrap_or(0).clamp(0, 1) as u8,
            kind,
            cost: parse_cost(&s("cost")),
            colors,
            text: s("text"),
            base_stats: pair("base_power", "base_toughness").or(stats),
            stats,
            loyalty: i("loyalty"),
            defense: i("defense"),
            damage: i("damage").unwrap_or(0),
            produces,
            type_line,
        }
    }

    /// Rahmenfarbe: Länder nach ihrer Manafarbe, sonst die Kartenfarbe (mehrfarbig = Gold,
    /// farblos = Grau).
    pub fn frame_color(&self) -> Color {
        if self.kind == CardKind::Land {
            return self.produces.color();
        }
        match self.colors.as_slice() {
            [] => ManaType::Colorless.color(),
            [one] => one.color(),
            _ => Color::from_rgb(0.85, 0.7, 0.3),
        }
    }

    /// Text und Farbe des Wertefelds: Kreatur „S/W“ (grün = über, rot = unter dem Aufdruck oder
    /// beschädigt), Planeswalker Loyalität, Schlacht Verteidigung; sonst leer.
    pub fn stats_text(&self) -> (String, Color) {
        const NORMAL: Color = Color::WHITE;
        const UP: Color = Color::from_rgb(0.6, 1.0, 0.5);
        const DOWN: Color = Color::from_rgb(1.0, 0.45, 0.4);
        if let Some((p, t)) = self.stats {
            let (bp, bt) = self.base_stats.unwrap_or((p, t));
            let color = if self.damage > 0 || p < bp || t < bt {
                DOWN
            } else if p > bp || t > bt {
                UP
            } else {
                NORMAL
            };
            return (format!("{p}/{t}"), color);
        }
        match (self.loyalty, self.defense) {
            (Some(l), _) => (l.to_string(), NORMAL),
            (None, Some(d)) => (d.to_string(), NORMAL),
            _ => (String::new(), NORMAL),
        }
    }

    /// Regeltext zum Anzeigen (Symbole ersetzt).
    pub fn display_text(&self) -> String {
        card_text::with_symbols(&self.text)
    }
}

/// Manakosten aus der Schreibweise der Engine (z. B. „2 R R“, „{1}{G}“, „X U“) → [`Cost`].
/// Symbole ohne festen Wert (X, hybrid, phyrexianisch …) werden ignoriert.
pub fn parse_cost(text: &str) -> Cost {
    let mut cost = Cost::default();
    for token in text.split(|c: char| c.is_whitespace() || c == '{' || c == '}').filter(|t| !t.is_empty()) {
        if let Ok(n) = token.parse::<u32>() {
            cost.generic += n;
            continue;
        }
        if token.len() != 1 {
            continue; // hybrid / phyrexianisch
        }
        if let Some(t) = token.chars().next().and_then(ManaType::from_wubrg) {
            cost.specific.add(t, 1);
        }
    }
    cost
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_costs() {
        let c = parse_cost("2 R R");
        assert_eq!(c.generic, 2);
        assert_eq!(c.specific.get(ManaType::Red), 2);
        let c = parse_cost("{1}{G}");
        assert_eq!(c.generic, 1);
        assert_eq!(c.specific.get(ManaType::Green), 1);
        assert_eq!(parse_cost("X U").specific.get(ManaType::Blue), 1);
        assert_eq!(parse_cost("{R/P}").total(), 0);
    }

    #[test]
    fn kinds_from_type_lines() {
        assert_eq!(CardKind::from_type_line("Artifact Creature — Golem"), CardKind::Creature);
        assert_eq!(CardKind::from_type_line("Artifact Land"), CardKind::Land);
        assert_eq!(CardKind::from_type_line("Legendary Planeswalker — Chandra"), CardKind::Planeswalker);
        assert_eq!(CardKind::from_type_line("Enchantment — Aura"), CardKind::Aura);
        assert_eq!(CardKind::from_type_line("Artifact — Equipment"), CardKind::Equipment);
        assert_eq!(CardKind::from_type_line("Kindred Instant — Goblin"), CardKind::Instant);
    }
}
