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

    /// Bleibt die Karte nach dem Ausspielen auf dem Tisch?
    pub fn is_permanent(self) -> bool {
        !matches!(self, CardKind::Instant | CardKind::Sorcery)
    }
}

/// Effekt eines Zaubers oder einer Heldenfähigkeit (ohne Zielauswahl).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SpellEffect {
    #[default]
    None,
    /// Der Spieler erhält n Leben.
    Heal(i32),
    /// Der Spieler zieht n Karten.
    Draw(i32),
    /// n Schaden am Gegner.
    DamageOpponent(i32),
    /// n Schaden an der gegnerischen Kreatur mit dem höchsten Angriff.
    DamageStrongestCreature(i32),
    /// Neutralisiert den obersten gegnerischen Zauber auf dem Stack (Counterspell).
    Counter,
}

impl SpellEffect {
    pub fn description(self) -> String {
        match self {
            SpellEffect::None => String::new(),
            SpellEffect::Heal(n) => trf!("Du erhältst {n} Leben.", "You gain {n} life."),
            SpellEffect::Draw(1) => tr!("Ziehe 1 Karte.", "Draw 1 card.").into(),
            SpellEffect::Draw(n) => trf!("Ziehe {n} Karten.", "Draw {n} cards."),
            SpellEffect::DamageOpponent(n) => trf!("{n} Schaden am Gegner.", "{n} damage to the opponent."),
            SpellEffect::DamageStrongestCreature(n) => {
                trf!("{n} Schaden an der stärksten gegnerischen Kreatur.", "{n} damage to the strongest enemy creature.")
            }
            SpellEffect::Counter => tr!("Neutralisiere den obersten gegnerischen Zauber auf dem Stack.", "Counter the topmost enemy spell on the stack.").into(),
        }
    }
}

/// Dauerhafter (statischer) Effekt eines Artefakts oder einer Verzauberung.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StaticEffect {
    /// Kreaturen des Besitzers erhalten +a/+d.
    OwnCreatures(i32, i32),
    /// Kreaturen des Gegners erhalten a/d (meist negativ).
    OpponentCreatures(i32, i32),
}

impl StaticEffect {
    pub fn description(self) -> String {
        let fmt = |v: i32| if v >= 0 { format!("+{v}") } else { format!("−{}", -v) };
        match self {
            StaticEffect::OwnCreatures(a, d) => trf!("Deine Kreaturen erhalten {}/{}.", "Your creatures get {}/{}.", fmt(a), fmt(d)),
            StaticEffect::OpponentCreatures(a, d) => {
                trf!("Gegnerische Kreaturen erhalten {}/{}.", "Enemy creatures get {}/{}.", fmt(a), fmt(d))
            }
        }
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
    pub fn allows(self, kind: CardKind) -> bool {
        match self {
            AuraTarget::Creature => kind == CardKind::Creature,
            AuraTarget::Resource => kind == CardKind::Resource,
            AuraTarget::Artifact => kind.is_artifact(),
            AuraTarget::Enchantment => kind.is_enchantment(),
            AuraTarget::Hero => kind == CardKind::Hero,
            AuraTarget::Permanent => kind.is_permanent(),
        }
    }

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
    /// Ist die Wirkung für den Verzauberer gut (eigene Karte) oder schlecht (gegnerische)?
    pub fn is_beneficial(self) -> bool {
        match self {
            AuraEffect::Stats(a, d) => a + d >= 0,
            AuraEffect::NoUntap => false,
            AuraEffect::ExtraMana(_) | AuraEffect::Willpower(_) => true,
        }
    }
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
    /// Artefakt / Verzauberung: dauerhafter Effekt.
    pub static_effect: Option<StaticEffect>,
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
            static_effect: None,
            activated: None,
            aura: None,
        }
    }

    /// Rahmenfarbe: Farbe der Ressource bzw. der farbigen Kosten (mehrfarbig = gold).
    /// Texte für die Kartenvorderseite: (Typzeile, Kartentext, Wertefeld).
    #[cfg(debug_assertions)]
    pub fn face_texts(&self) -> (String, String, String) {
        match self.kind {
            CardKind::Creature => (
                self.ability.title().to_string(),
                self.ability.description().to_string(),
                format!("{}/{}", self.attack, self.defense),
            ),
            CardKind::Resource => (
                trf!("Ressource – {}", "Resource – {}", self.produces.name()),
                trf!("Tappen: +1 {}-Mana.", "Tap: +1 {} mana.", self.produces.name()),
                String::new(),
            ),
            CardKind::Instant | CardKind::Sorcery => {
                (self.kind.title().to_string(), self.effect.description(), String::new())
            }
            CardKind::Hero => (
                tr!("Held", "Hero").to_string(),
                self.hero_abilities.iter().map(|a| a.description()).collect::<Vec<_>>().join("\n"),
                format!("WK {}", self.willpower),
            ),
            CardKind::Equipment => {
                let e = self.equip.unwrap_or_default();
                (self.kind.type_line().to_string(), e.description(), format!("+{}/+{}", e.attack, e.defense))
            }
            CardKind::Artifact | CardKind::Enchantment => {
                let mut parts: Vec<String> = Vec::new();
                if let Some(e) = self.static_effect {
                    parts.push(e.description());
                }
                if let Some(a) = self.activated {
                    parts.push(a.description());
                }
                (self.kind.type_line().to_string(), parts.join("\n"), String::new())
            }
            CardKind::Aura => match self.aura {
                Some(a) => (self.kind.type_line().to_string(), a.description(), a.stats_text()),
                None => (self.kind.type_line().to_string(), String::new(), String::new()),
            },
        }
    }

    /// Ausführliche Regel-Erklärung: wann spielbar und was die Karte bewirkt.
    #[cfg(debug_assertions)]
    pub fn rules_text(&self) -> String {
        let timing = match self.kind {
            CardKind::Creature => tr!("Eigene Hauptphase bei leerem Stack. Bleibt auf dem Spielfeld, kann angreifen und blocken.", "Your own main phase with an empty stack. Stays on the battlefield and can attack and block."),
            CardKind::Resource => tr!("Eigene Hauptphase, höchstens 1 pro Runde, kostet nichts. Liegt in der Ressourcen-Reihe; Klick = tappen für Mana.", "Your own main phase, at most 1 per turn, free. Lies in the resource row; click = tap for mana."),
            CardKind::Instant => tr!("Jederzeit spielbar – auch im gegnerischen Zug und als Antwort auf dem Stack.", "Playable at any time – also during the opponent's turn and in response on the stack."),
            CardKind::Sorcery => tr!("Nur in der eigenen Hauptphase bei leerem Stack. Geht danach auf den Friedhof.", "Only during your own main phase with an empty stack. Goes to the graveyard afterwards."),
            CardKind::Hero => tr!("Eigene Hauptphase bei leerem Stack. Klick = Fähigkeit wählen (1× pro Runde); sie ändert die Willenskraft. Bei 0 Willenskraft wird der Held zerstört.", "Your own main phase with an empty stack. Click = choose an ability (once per turn); it changes willpower. At 0 willpower the hero is destroyed."),
            CardKind::Equipment => tr!("Eigene Hauptphase. Liegt danach auf dem Spielfeld; Klick auf die Ausrüstung und dann auf eine eigene Kreatur legt sie für die Anlegekosten an.", "Your own main phase. Then lies on the battlefield; click the equipment and then one of your creatures to equip it for its equip cost."),
            CardKind::Artifact => tr!("Eigene Hauptphase. Passive Effekte wirken dauerhaft; eine Tapp-Fähigkeit kann jederzeit per Klick genutzt werden.", "Your own main phase. Passive effects are permanent; a tap ability can be used at any time by clicking."),
            CardKind::Enchantment => tr!("Eigene Hauptphase. Wirkt dauerhaft auf das ganze Spiel.", "Your own main phase. Affects the whole game permanently."),
            CardKind::Aura => tr!("Eigene Hauptphase; muss auf ein gültiges Ziel gezogen werden. Verlässt das Ziel das Spiel, geht die Aura auf den Friedhof.", "Your own main phase; must be dragged onto a valid target. If the target leaves play, the aura goes to the graveyard."),
        };
        let cost = if self.kind == CardKind::Resource { tr!("keine", "none").to_string() } else { self.cost.to_text() };
        let (type_line, body, stats) = self.face_texts();
        let name = crate::i18n::name(&self.name);
        let mut text = trf!("{name}\n{type_line}  ·  Kosten: {cost}", "{name}\n{type_line}  ·  Cost: {cost}");
        if !stats.is_empty() {
            text.push_str(&trf!("  ·  Werte: {stats}", "  ·  Stats: {stats}"));
        }
        if !body.is_empty() {
            text.push_str(&trf!("\n\nWirkung: {body}", "\n\nEffect: {body}"));
        }
        if self.effect == SpellEffect::Counter {
            text.push_str(tr!("\nNur spielbar, wenn ein gegnerischer Zauber auf dem Stack liegt.", "\nOnly playable while an enemy spell is on the stack."));
        }
        text.push_str(&trf!("\n\nWann: {timing}", "\n\nWhen: {timing}"));
        text
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

/// Eigene Namen für die Ressourcen-Karten je Farbe.
pub fn resource_name(t: ManaType) -> &'static str {
    match t {
        ManaType::White => "Kapellenglocke",
        ManaType::Blue => "Brunnenquelle",
        ManaType::Black => "Kellergewölbe",
        ManaType::Red => "Glutherd",
        ManaType::Green => "Kräutergarten",
        ManaType::Colorless => "Alter Steinkreis",
    }
}

const NAMES: [&str; 15] = [
    "Schankmaid",
    "Taverndieb",
    "Söldnerin",
    "Wanderbarde",
    "Trunkenbold",
    "Hofhund",
    "Feuerschlucker",
    "Kräuterhexe",
    "Zwergenschmied",
    "Würfelspieler",
    "Rausschmeißer",
    "Rattenfänger",
    "Pilgerin",
    "Kesselflicker",
    "Nachtwächter",
];

/// Sofortzauber je Hauptfarbe: (Name, Effekt).
fn instant_for(t: ManaType) -> (&'static str, SpellEffect) {
    match t {
        ManaType::Red => ("Kneipenschlägerei", SpellEffect::DamageStrongestCreature(2)),
        ManaType::Green => ("Kräutertrank", SpellEffect::Heal(3)),
        ManaType::Blue => ("Klarer Kopf", SpellEffect::Draw(1)),
        ManaType::Black => ("Giftiger Schnaps", SpellEffect::DamageStrongestCreature(2)),
        _ => ("Stoßgebet", SpellEffect::Heal(2)),
    }
}

/// Hexerei je Hauptfarbe: (Name, Effekt).
fn sorcery_for(t: ManaType) -> (&'static str, SpellEffect) {
    match t {
        ManaType::Red => ("Brandfass", SpellEffect::DamageOpponent(3)),
        ManaType::Green => ("Erntedank", SpellEffect::Draw(2)),
        ManaType::Blue => ("Ausschank", SpellEffect::Draw(2)),
        ManaType::Black => ("Zechprellerei", SpellEffect::DamageOpponent(3)),
        _ => ("Sammelbüchse", SpellEffect::Heal(4)),
    }
}

fn colored(main: ManaType, generic: u32) -> Cost {
    let mut c = Cost::default();
    c.specific.add(main, 1);
    c.generic = generic;
    c
}

/// Blauer Counterspell: Sofortzauber für BB.
pub fn counterspell() -> CardDef {
    let mut cost = Cost::default();
    cost.specific.add(ManaType::Blue, 2);
    let mut def = CardDef::base("Hausverbot", CardKind::Instant, cost);
    def.effect = SpellEffect::Counter;
    def
}

/// Alle Karten, die es im Spiel gibt: Ressourcen in allen Farben plus jede
/// unterschiedliche Karte aus beiden Demo-Decks (für das Debug-Tool).
#[cfg(debug_assertions)]
pub fn catalog() -> Vec<CardDef> {
    let mut list: Vec<CardDef> = ManaType::ALL
        .into_iter()
        .map(|t| {
            let mut d = CardDef::base(resource_name(t), CardKind::Resource, Cost::default());
            d.produces = t;
            d
        })
        .collect();
    let decks = [(0usize, [ManaType::Red, ManaType::Green]), (1000, [ManaType::Blue, ManaType::Black])];
    list.push(counterspell());
    for (offset, colors) in decks {
        for id in offset..offset + 30 {
            let def = demo_card(id, colors);
            if !list.iter().any(|d| d.name == def.name) {
                list.push(def);
            }
        }
    }
    list
}

/// Deterministische Demo-Karte. `colors` sind die beiden Farben des Decks.
///
/// Verteilung je 30 Karten: 12 Ressourcen, 6 Kreaturen, 1 Held, 2 Sofortzauber,
/// 2 Hexereien, 2 Ausrüstungen, 2 Artefakte, 1 Verzauberung, 2 Auren.
pub fn demo_card(id: usize, colors: [ManaType; 2]) -> CardDef {
    let main = colors[id % 2];
    let local = id % 1000;
    let player = id < 1000;

    if id % 5 == 0 || id % 5 == 2 {
        let t = colors[(id / 5 + id % 5 / 2) % 2];
        let mut def = CardDef::base(resource_name(t), CardKind::Resource, Cost::default());
        def.produces = t;
        return def;
    }
    match local {
        1 => {
            let mut cost = colored(main, 2);
            cost.specific.add(colors[(id + 1) % 2], 1);
            let name = if player { "Wirtin Brunhild" } else { "Schmuggler Veit" };
            let mut def = CardDef::base(name, CardKind::Hero, cost);
            def.willpower = 3;
            def.hero_abilities = vec![
                HeroAbility { delta: 1, effect: if player { SpellEffect::Heal(2) } else { SpellEffect::Draw(1) } },
                HeroAbility { delta: -3, effect: SpellEffect::DamageStrongestCreature(4) },
            ];
            return def;
        }
        // Gegner (Blau/Schwarz): ein Sofortzauber ist der blaue Counterspell
        23 if !player => return counterspell(),
        3 | 23 => {
            let (name, effect) = instant_for(main);
            let mut def = CardDef::base(name, CardKind::Instant, colored(main, 1));
            def.effect = effect;
            return def;
        }
        8 | 28 => {
            let (name, effect) = sorcery_for(main);
            let mut def = CardDef::base(name, CardKind::Sorcery, colored(main, 2));
            def.effect = effect;
            return def;
        }
        9 | 24 => {
            let (name, attack, defense) = if local == 9 { ("Schankmesser", 2, 0) } else { ("Eisenkrughelm", 0, 2) };
            let mut def = CardDef::base(name, CardKind::Equipment, Cost { generic: 2, ..Cost::default() });
            def.equip = Some(EquipStats { attack, defense, cost: Cost { generic: 1, ..Cost::default() } });
            return def;
        }
        13 => {
            // Artefakt mit passivem Bonus (nur generische Kosten)
            let mut def = CardDef::base("Kriegstrommel", CardKind::Artifact, Cost { generic: 3, ..Cost::default() });
            def.static_effect = Some(StaticEffect::OwnCreatures(1, 0));
            return def;
        }
        18 => {
            // Artefakt mit Tapp-Fähigkeit
            let mut def = CardDef::base("Bierfass", CardKind::Artifact, Cost { generic: 2, ..Cost::default() });
            def.activated = Some(ActivatedAbility { cost: Cost::default(), effect: SpellEffect::Heal(1) });
            return def;
        }
        19 => {
            let (name, effect) = if player {
                ("Gemeinsames Gelage", StaticEffect::OwnCreatures(1, 1))
            } else {
                ("Dichter Tabakrauch", StaticEffect::OpponentCreatures(-1, 0))
            };
            let mut def = CardDef::base(name, CardKind::Enchantment, colored(main, 2));
            def.static_effect = Some(effect);
            return def;
        }
        6 => {
            let mut def = CardDef::base("Wirtshaussegen", CardKind::Aura, colored(main, 1));
            def.aura = Some(AuraDef { target: AuraTarget::Creature, effect: AuraEffect::Stats(2, 2) });
            return def;
        }
        21 => {
            let mut def = if player {
                let mut d = CardDef::base("Goldader", CardKind::Aura, colored(main, 1));
                d.aura = Some(AuraDef { target: AuraTarget::Resource, effect: AuraEffect::ExtraMana(1) });
                d
            } else {
                let mut d = CardDef::base("Rostfluch", CardKind::Aura, colored(main, 2));
                d.aura = Some(AuraDef { target: AuraTarget::Permanent, effect: AuraEffect::NoUntap });
                d
            };
            def.produces = ManaType::Colorless;
            return def;
        }
        _ => {}
    }

    // Kreatur
    let total = 1 + ((id * 7) % 5) as u32;
    let mut cost = Cost::default();
    if total >= 4 {
        let second = if id % 4 == 3 { colors[(id + 1) % 2] } else { main };
        cost.specific.add(main, 1);
        cost.specific.add(second, 1);
        cost.generic = total - 2;
    } else {
        cost = colored(main, total - 1);
    }
    let t = total as i32;
    let mut def = CardDef::base(NAMES[id % NAMES.len()], CardKind::Creature, cost);
    def.attack = (t + (id % 3) as i32 - 1).max(1);
    def.defense = (t + ((id / 3) % 3) as i32 - 1).max(1);
    def.ability = match id % 7 {
        1 => CardAbility::Regeneration,
        3 => CardAbility::Firebreath,
        4 => CardAbility::Rage,
        _ => CardAbility::None,
    };
    def
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_deck_distribution() {
        for (colors, offset) in [([ManaType::Red, ManaType::Green], 0), ([ManaType::Blue, ManaType::Black], 1000)] {
            let mut counts = std::collections::HashMap::new();
            for id in offset..offset + 30 {
                *counts.entry(demo_card(id, colors).kind).or_insert(0) += 1;
            }
            assert_eq!(counts[&CardKind::Resource], 12);
            assert_eq!(counts[&CardKind::Creature], 6);
            assert_eq!(counts[&CardKind::Hero], 1);
            assert_eq!(counts[&CardKind::Instant], 2);
            assert_eq!(counts[&CardKind::Sorcery], 2);
            assert_eq!(counts[&CardKind::Equipment], 2);
            assert_eq!(counts[&CardKind::Artifact], 2);
            assert_eq!(counts[&CardKind::Enchantment], 1);
            assert_eq!(counts[&CardKind::Aura], 2);
        }
    }

    #[test]
    fn aura_target_classes() {
        use CardKind::*;
        let all = [Creature, Resource, Instant, Sorcery, Hero, Equipment, Artifact, Enchantment, Aura];
        let allowed = |t: AuraTarget| all.iter().filter(|k| t.allows(**k)).copied().collect::<Vec<_>>();
        assert_eq!(allowed(AuraTarget::Creature), vec![Creature]);
        assert_eq!(allowed(AuraTarget::Resource), vec![Resource]);
        assert_eq!(allowed(AuraTarget::Hero), vec![Hero]);
        assert_eq!(allowed(AuraTarget::Artifact), vec![Equipment, Artifact]);
        assert_eq!(allowed(AuraTarget::Enchantment), vec![Enchantment, Aura]);
        // Jede bleibende Karte – aber nie Zauber
        assert_eq!(
            allowed(AuraTarget::Permanent),
            vec![Creature, Resource, Hero, Equipment, Artifact, Enchantment, Aura]
        );
    }

    #[test]
    fn aura_effect_direction() {
        assert!(AuraEffect::Stats(2, 2).is_beneficial());
        assert!(!AuraEffect::Stats(-2, -1).is_beneficial());
        assert!(!AuraEffect::NoUntap.is_beneficial());
        assert!(AuraEffect::ExtraMana(1).is_beneficial());
        assert!(AuraEffect::Willpower(2).is_beneficial());
    }

    #[test]
    fn artifacts_need_no_colored_mana() {
        for id in 0..30 {
            let def = demo_card(id, [ManaType::Red, ManaType::Green]);
            if def.kind.is_artifact() {
                assert_eq!(def.cost.specific.total(), 0, "{} darf keine Farbe kosten", def.name);
            }
        }
    }
}
