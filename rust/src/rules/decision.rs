//! Entscheidungsmodell von NeoForge (`forge-engine/src/decision.rs`), 1:1 nachgebildet.
//!
//! Die Regel-Engine hält an und liefert eine [`Decision`]; die UI antwortet mit einer [`Choice`].
//! Variantennamen und Felder entsprechen NeoForge, damit die UI später ohne Umbau direkt mit
//! NeoForge arbeiten kann. Varianten mit dem Vermerk **Erweiterung** gibt es in NeoForge (noch)
//! nicht; sie decken Fragen der Java-Engine ab und sind in `docs/Forge-Schnittstelle.md`
//! aufgelistet, damit NeoForge sie nachziehen kann.
//!
//! JSON-Form (Protokoll v3): jede Variante als Objekt mit `"type": "<Variantenname>"` und den
//! Feldern in `snake_case`; IDs als Zahlen, [`Target`] als `{"card": id}` / `{"player": id}`,
//! [`Color`] als `"W" "U" "B" "R" "G"`.

use godot::prelude::*;

/// Karte (heute: Forge-Karten-ID).
pub type CardId = u32;
/// Spieler (Sitzplatz-Index = Zugreihenfolge).
pub type PlayerId = u8;

/// Ziel einer Fähigkeit (CR 115).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Target {
    Card(CardId),
    Player(PlayerId),
}

/// Farbe (WUBRG).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Color {
    White,
    Blue,
    Black,
    Red,
    Green,
}

/// Aktion mit Priorität (CR 117.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    Pass,
    PlayLand(CardId),
    Cast(CardId),
    /// Andere Seite wirken (Adventure, modale doppelseitige Karte).
    CastAlt(CardId, usize),
    /// Andere Seite als Land spielen.
    PlayLandAlt(CardId, usize),
    /// Aktivierte Fähigkeit: Karte + Fähigkeitsindex.
    Activate(CardId, usize),
    /// Zauber mit Alternativkosten wirken.
    CastWithAltCost(CardId, usize),
    CastFaceDown(CardId),
    TurnFaceUp(CardId),
}

/// Option beim Bezahlen von Manakosten.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PayAction {
    ActivateMana(CardId, usize),
    AutoPay,
    Cancel,
}

/// Grund einer Kartenauswahl ([`Decision::ChooseCards`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardChoiceReason {
    DiscardToHandSize,
    SacrificeCost,
    DiscardCost,
    LegendRule,
    Dig,
    Search,
    Discard,
    Sacrifice,
    Transform,
    ExileCost,
    TapCost,
    ReturnCost,
    Choose,
    Convoke,
    Improvise,
    Delve,
    Play,
}

/// Eine Option einer Erweiterungs-Auswahl: Text, optional Farbe (für farbige Knöpfe; `None` bei
/// „farblos“ oder Nicht-Farben).
#[derive(Debug, Clone, PartialEq)]
pub struct OptionText {
    pub text: String,
    pub color: Option<Color>,
    /// Steht die Option für eine Farbe/Farblos? (`"C"` = farblos)
    pub colorless: bool,
}

/// Was die Engine vom Spieler wissen will.
#[derive(Debug, Clone, PartialEq)]
pub enum Decision {
    /// Modi eines modalen Zaubers (Index, Text).
    ChooseModes { player: PlayerId, modes: Vec<(usize, String)>, min: u32, max: u32, can_repeat: bool },
    /// London-Mulligan: behalten oder neu ziehen.
    Mulligan { player: PlayerId, mulligans_taken: u32 },
    /// Nach dem Behalten `count` Karten unter die Bibliothek legen.
    MulliganBottom { player: PlayerId, hand: Vec<CardId>, count: u32 },
    Priority { player: PlayerId, actions: Vec<Action> },
    ChooseTargets {
        player: PlayerId,
        source: CardId,
        description: String,
        candidates: Vec<Target>,
        min: u32,
        max: u32,
    },
    ChooseX { player: PlayerId, max: u32 },
    PayMana { player: PlayerId, remaining: String, options: Vec<PayAction> },
    PayLifeForPhyrexian { player: PlayerId, symbol: String },
    ChooseColor { player: PlayerId, options: Vec<Color> },
    /// Kartennamen nennen; Antwort `Choice::Number(index)`.
    NameCard { player: PlayerId, options: Vec<String> },
    /// Typ wählen; Antwort `Choice::Number(index)`.
    ChooseType { player: PlayerId, options: Vec<String> },
    DeclareAttackers { player: PlayerId, candidates: Vec<CardId>, defenders: Vec<Target> },
    DeclareBlockers { player: PlayerId, candidates: Vec<CardId>, attackers: Vec<CardId> },
    AssignCombatDamage {
        player: PlayerId,
        attacker: CardId,
        amount: u32,
        blockers: Vec<CardId>,
        lethal: Vec<u32>,
        trample_to: Option<Target>,
    },
    ChooseCards { player: PlayerId, reason: CardChoiceReason, options: Vec<CardId>, min: u32, max: u32 },
    OrderTriggers { player: PlayerId, descriptions: Vec<String> },
    Scry { player: PlayerId, cards: Vec<CardId>, graveyard: bool },
    PayUnlessCost { player: PlayerId, cost: String },
    OptionalTrigger { player: PlayerId, description: String },
    Confirm { player: PlayerId, prompt: String },
    /// **Erweiterung:** eine Option aus einer Liste (Startspieler, Abstimmung, Marken-Typ,
    /// Ersatzeffekt, Ja/Nein mit eigenen Beschriftungen …); Antwort `Choice::Number(index)`.
    ChooseOption { player: PlayerId, prompt: String, options: Vec<OptionText> },
    /// **Erweiterung:** `min`–`max` Optionen (optionale Kosten, mehrere Farben, Würfel …);
    /// Antwort `Choice::Modes(indizes)`.
    ChooseOptions { player: PlayerId, prompt: String, options: Vec<OptionText>, min: u32, max: u32 },
    /// **Erweiterung:** Zahl zwischen `min` und `max`; Antwort `Choice::Number(n)`.
    ChooseNumber { player: PlayerId, prompt: String, min: u32, max: u32 },
}

/// Antwort des Spielers.
#[derive(Debug, Clone, PartialEq)]
#[expect(dead_code, reason = "vollständiges NeoForge-Modell; Pay, Attackers, Blocks … folgen mit den nächsten Bedienmustern")]
pub enum Choice {
    Keep,
    Mulligan,
    Action(Action),
    Targets(Vec<Target>),
    Number(u32),
    Pay(PayAction),
    Yes,
    No,
    Color(Color),
    Attackers(Vec<(CardId, Target)>),
    Blocks(Vec<(CardId, CardId)>),
    Damage(Vec<u32>),
    Cards(Vec<CardId>),
    Order(Vec<usize>),
    Modes(Vec<usize>),
    Arrange { top: Vec<CardId>, bottom: Vec<CardId> },
    /// **Erweiterung:** Auswahl abbrechen (z. B. Ziel- oder X-Wahl beim Wirken; das Wirken wird
    /// zurückgenommen). In NeoForge entspricht das bei Zahlungen `PayAction::Cancel`.
    Cancel,
}

impl Decision {
    /// Spieler, der entscheiden muss.
    #[expect(dead_code, reason = "für Mehrspieler-Ansichten (mehrere Sitzplätze) vorgesehen")]
    pub fn player(&self) -> PlayerId {
        use Decision::*;
        match self {
            ChooseModes { player, .. }
            | Mulligan { player, .. }
            | MulliganBottom { player, .. }
            | Priority { player, .. }
            | ChooseTargets { player, .. }
            | ChooseX { player, .. }
            | PayMana { player, .. }
            | PayLifeForPhyrexian { player, .. }
            | ChooseColor { player, .. }
            | NameCard { player, .. }
            | ChooseType { player, .. }
            | DeclareAttackers { player, .. }
            | DeclareBlockers { player, .. }
            | AssignCombatDamage { player, .. }
            | ChooseCards { player, .. }
            | OrderTriggers { player, .. }
            | Scry { player, .. }
            | PayUnlessCost { player, .. }
            | OptionalTrigger { player, .. }
            | Confirm { player, .. }
            | ChooseOption { player, .. }
            | ChooseOptions { player, .. }
            | ChooseNumber { player, .. } => *player,
        }
    }
}

// ====================================================================== JSON (Protokoll v3)

impl Color {
    pub fn letter(self) -> &'static str {
        match self {
            Color::White => "W",
            Color::Blue => "U",
            Color::Black => "B",
            Color::Red => "R",
            Color::Green => "G",
        }
    }

    pub fn from_letter(s: &str) -> Option<Color> {
        Some(match s {
            "W" => Color::White,
            "U" => Color::Blue,
            "B" => Color::Black,
            "R" => Color::Red,
            "G" => Color::Green,
            _ => return None,
        })
    }
}

impl CardChoiceReason {
    fn from_name(s: &str) -> CardChoiceReason {
        use CardChoiceReason::*;
        match s {
            "DiscardToHandSize" => DiscardToHandSize,
            "SacrificeCost" => SacrificeCost,
            "DiscardCost" => DiscardCost,
            "LegendRule" => LegendRule,
            "Dig" => Dig,
            "Search" => Search,
            "Discard" => Discard,
            "Sacrifice" => Sacrifice,
            "Transform" => Transform,
            "ExileCost" => ExileCost,
            "TapCost" => TapCost,
            "ReturnCost" => ReturnCost,
            "Convoke" => Convoke,
            "Improvise" => Improvise,
            "Delve" => Delve,
            "Play" => Play,
            _ => Choose,
        }
    }
}

/// Lesehilfe für ein JSON-Objekt (Godot-Dictionary).
struct Obj<'a>(&'a VarDictionary);

impl Obj<'_> {
    fn get(&self, k: &str) -> Option<Variant> {
        self.0.get(k).filter(|v| !v.is_nil())
    }
    fn str(&self, k: &str) -> String {
        self.get(k).map(|v| v.to_string()).unwrap_or_default()
    }
    fn int(&self, k: &str) -> i64 {
        self.get(k).and_then(|v| v.try_to::<i64>().ok()).unwrap_or(0)
    }
    fn u32(&self, k: &str) -> u32 {
        self.int(k).clamp(0, u32::MAX as i64) as u32
    }
    fn bool(&self, k: &str) -> bool {
        self.get(k).is_some_and(|v| v.booleanize())
    }
    fn arr(&self, k: &str) -> Vec<Variant> {
        self.get(k).and_then(|v| v.try_to::<VarArray>().ok()).map(|a| a.iter_shared().collect()).unwrap_or_default()
    }
    fn ids(&self, k: &str) -> Vec<CardId> {
        self.arr(k).iter().filter_map(|v| v.try_to::<i64>().ok()).map(|i| i.max(0) as CardId).collect()
    }
    fn strings(&self, k: &str) -> Vec<String> {
        self.arr(k).iter().map(|v| v.to_string()).collect()
    }
}

fn dict(v: &Variant) -> Option<VarDictionary> {
    v.try_to::<VarDictionary>().ok()
}

impl Target {
    pub fn from_var(v: &Variant) -> Option<Target> {
        let d = dict(v)?;
        let o = Obj(&d);
        if o.get("card").is_some() {
            Some(Target::Card(o.int("card").max(0) as CardId))
        } else if o.get("player").is_some() {
            Some(Target::Player(o.int("player").max(0) as PlayerId))
        } else {
            None
        }
    }

    pub fn to_var(self) -> Variant {
        let mut d = VarDictionary::new();
        match self {
            Target::Card(c) => d.set("card", c as i64),
            Target::Player(p) => d.set("player", p as i64),
        }
        d.to_variant()
    }
}

impl Action {
    pub fn from_var(v: &Variant) -> Option<Action> {
        let d = dict(v)?;
        let o = Obj(&d);
        let card = o.u32("card");
        let index = o.int("index").max(0) as usize;
        Some(match o.str("type").as_str() {
            "Pass" => Action::Pass,
            "PlayLand" => Action::PlayLand(card),
            "Cast" => Action::Cast(card),
            "CastAlt" => Action::CastAlt(card, index),
            "PlayLandAlt" => Action::PlayLandAlt(card, index),
            "Activate" => Action::Activate(card, index),
            "CastWithAltCost" => Action::CastWithAltCost(card, index),
            "CastFaceDown" => Action::CastFaceDown(card),
            "TurnFaceUp" => Action::TurnFaceUp(card),
            _ => return None,
        })
    }

    pub fn to_var(self) -> Variant {
        let mut d = VarDictionary::new();
        let (name, card, index): (&str, Option<CardId>, Option<usize>) = match self {
            Action::Pass => ("Pass", None, None),
            Action::PlayLand(c) => ("PlayLand", Some(c), None),
            Action::Cast(c) => ("Cast", Some(c), None),
            Action::CastAlt(c, i) => ("CastAlt", Some(c), Some(i)),
            Action::PlayLandAlt(c, i) => ("PlayLandAlt", Some(c), Some(i)),
            Action::Activate(c, i) => ("Activate", Some(c), Some(i)),
            Action::CastWithAltCost(c, i) => ("CastWithAltCost", Some(c), Some(i)),
            Action::CastFaceDown(c) => ("CastFaceDown", Some(c), None),
            Action::TurnFaceUp(c) => ("TurnFaceUp", Some(c), None),
        };
        d.set("type", name);
        if let Some(c) = card {
            d.set("card", c as i64);
        }
        if let Some(i) = index {
            d.set("index", i as i64);
        }
        d.to_variant()
    }

    /// Karte, die die Aktion betrifft (`None` bei `Pass`).
    pub fn card(self) -> Option<CardId> {
        match self {
            Action::Pass => None,
            Action::PlayLand(c)
            | Action::Cast(c)
            | Action::CastAlt(c, _)
            | Action::PlayLandAlt(c, _)
            | Action::Activate(c, _)
            | Action::CastWithAltCost(c, _)
            | Action::CastFaceDown(c)
            | Action::TurnFaceUp(c) => Some(c),
        }
    }
}

impl PayAction {
    pub fn from_var(v: &Variant) -> Option<PayAction> {
        let d = dict(v)?;
        let o = Obj(&d);
        Some(match o.str("type").as_str() {
            "ActivateMana" => PayAction::ActivateMana(o.u32("card"), o.int("index").max(0) as usize),
            "AutoPay" => PayAction::AutoPay,
            "Cancel" => PayAction::Cancel,
            _ => return None,
        })
    }

    pub fn to_var(self) -> Variant {
        let mut d = VarDictionary::new();
        match self {
            PayAction::ActivateMana(c, i) => {
                d.set("type", "ActivateMana");
                d.set("card", c as i64);
                d.set("index", i as i64);
            }
            PayAction::AutoPay => d.set("type", "AutoPay"),
            PayAction::Cancel => d.set("type", "Cancel"),
        }
        d.to_variant()
    }
}

fn options_from(o: &Obj) -> Vec<OptionText> {
    o.arr("options")
        .iter()
        .map(|v| match dict(v) {
            Some(d) => {
                let oo = Obj(&d);
                let c = oo.str("color");
                OptionText { text: oo.str("text"), color: Color::from_letter(&c), colorless: c == "C" }
            }
            None => OptionText { text: v.to_string(), color: None, colorless: false },
        })
        .collect()
}

impl Decision {
    /// Liest eine Entscheidung aus dem `decision`-Objekt einer Anfrage. `None` bei unbekanntem Typ.
    pub fn from_var(v: &Variant) -> Option<Decision> {
        let d = dict(v)?;
        let o = Obj(&d);
        let player = o.int("player").clamp(0, 255) as PlayerId;
        let targets = |k: &str| o.arr(k).iter().filter_map(Target::from_var).collect::<Vec<_>>();
        Some(match o.str("type").as_str() {
            "ChooseModes" => Decision::ChooseModes {
                player,
                modes: o
                    .arr("modes")
                    .iter()
                    .filter_map(|m| m.try_to::<VarArray>().ok())
                    .map(|m| {
                        let i = m.get(0).and_then(|v| v.try_to::<i64>().ok()).unwrap_or(0).max(0) as usize;
                        (i, m.get(1).map(|v| v.to_string()).unwrap_or_default())
                    })
                    .collect(),
                min: o.u32("min"),
                max: o.u32("max"),
                can_repeat: o.bool("can_repeat"),
            },
            "Mulligan" => Decision::Mulligan { player, mulligans_taken: o.u32("mulligans_taken") },
            "MulliganBottom" => Decision::MulliganBottom { player, hand: o.ids("hand"), count: o.u32("count") },
            "Priority" => Decision::Priority {
                player,
                actions: o.arr("actions").iter().filter_map(Action::from_var).collect(),
            },
            "ChooseTargets" => Decision::ChooseTargets {
                player,
                source: o.u32("source"),
                description: o.str("description"),
                candidates: targets("candidates"),
                min: o.u32("min"),
                max: o.u32("max"),
            },
            "ChooseX" => Decision::ChooseX { player, max: o.u32("max") },
            "PayMana" => Decision::PayMana {
                player,
                remaining: o.str("remaining"),
                options: o.arr("options").iter().filter_map(PayAction::from_var).collect(),
            },
            "PayLifeForPhyrexian" => Decision::PayLifeForPhyrexian { player, symbol: o.str("symbol") },
            "ChooseColor" => Decision::ChooseColor {
                player,
                options: o.strings("options").iter().filter_map(|c| Color::from_letter(c)).collect(),
            },
            "NameCard" => Decision::NameCard { player, options: o.strings("options") },
            "ChooseType" => Decision::ChooseType { player, options: o.strings("options") },
            "DeclareAttackers" => {
                Decision::DeclareAttackers { player, candidates: o.ids("candidates"), defenders: targets("defenders") }
            }
            "DeclareBlockers" => {
                Decision::DeclareBlockers { player, candidates: o.ids("candidates"), attackers: o.ids("attackers") }
            }
            "AssignCombatDamage" => Decision::AssignCombatDamage {
                player,
                attacker: o.u32("attacker"),
                amount: o.u32("amount"),
                blockers: o.ids("blockers"),
                lethal: o.ids("lethal"),
                trample_to: o.get("trample_to").as_ref().and_then(Target::from_var),
            },
            "ChooseCards" => Decision::ChooseCards {
                player,
                reason: CardChoiceReason::from_name(&o.str("reason")),
                options: o.ids("options"),
                min: o.u32("min"),
                max: o.u32("max"),
            },
            "OrderTriggers" => Decision::OrderTriggers { player, descriptions: o.strings("descriptions") },
            "Scry" => Decision::Scry { player, cards: o.ids("cards"), graveyard: o.bool("graveyard") },
            "PayUnlessCost" => Decision::PayUnlessCost { player, cost: o.str("cost") },
            "OptionalTrigger" => Decision::OptionalTrigger { player, description: o.str("description") },
            "Confirm" => Decision::Confirm { player, prompt: o.str("prompt") },
            "ChooseOption" => Decision::ChooseOption { player, prompt: o.str("prompt"), options: options_from(&o) },
            "ChooseOptions" => Decision::ChooseOptions {
                player,
                prompt: o.str("prompt"),
                options: options_from(&o),
                min: o.u32("min"),
                max: o.u32("max"),
            },
            "ChooseNumber" => {
                Decision::ChooseNumber { player, prompt: o.str("prompt"), min: o.u32("min"), max: o.u32("max") }
            }
            _ => return None,
        })
    }
}

impl Choice {
    /// JSON-Form der Antwort (Feld `choice` im Befehl `answer`).
    pub fn to_var(&self) -> Variant {
        let mut d = VarDictionary::new();
        let ids = |v: &[CardId]| v.iter().map(|c| (*c as i64).to_variant()).collect::<VarArray>();
        let nums = |v: &[usize]| v.iter().map(|c| (*c as i64).to_variant()).collect::<VarArray>();
        let name = match self {
            Choice::Keep => "Keep",
            Choice::Mulligan => "Mulligan",
            Choice::Action(a) => {
                d.set("action", &a.to_var());
                "Action"
            }
            Choice::Targets(t) => {
                d.set("targets", &t.iter().map(|t| t.to_var()).collect::<VarArray>());
                "Targets"
            }
            Choice::Number(n) => {
                d.set("value", *n as i64);
                "Number"
            }
            Choice::Pay(p) => {
                d.set("pay", &p.to_var());
                "Pay"
            }
            Choice::Yes => "Yes",
            Choice::No => "No",
            Choice::Color(c) => {
                d.set("color", c.letter());
                "Color"
            }
            Choice::Attackers(list) => {
                let arr: VarArray = list
                    .iter()
                    .map(|(c, t)| {
                        let mut pair = VarArray::new();
                        pair.push(&(*c as i64).to_variant());
                        pair.push(&t.to_var());
                        pair.to_variant()
                    })
                    .collect();
                d.set("attackers", &arr);
                "Attackers"
            }
            Choice::Blocks(list) => {
                let arr: VarArray = list
                    .iter()
                    .map(|(b, a)| {
                        let mut pair = VarArray::new();
                        pair.push(&(*b as i64).to_variant());
                        pair.push(&(*a as i64).to_variant());
                        pair.to_variant()
                    })
                    .collect();
                d.set("blocks", &arr);
                "Blocks"
            }
            Choice::Damage(v) => {
                d.set("amounts", &v.iter().map(|n| (*n as i64).to_variant()).collect::<VarArray>());
                "Damage"
            }
            Choice::Cards(v) => {
                d.set("cards", &ids(v));
                "Cards"
            }
            Choice::Order(v) => {
                d.set("order", &nums(v));
                "Order"
            }
            Choice::Modes(v) => {
                d.set("modes", &nums(v));
                "Modes"
            }
            Choice::Arrange { top, bottom } => {
                d.set("top", &ids(top));
                d.set("bottom", &ids(bottom));
                "Arrange"
            }
            Choice::Cancel => "Cancel",
        };
        d.set("type", name);
        d.to_variant()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_letters_roundtrip() {
        for c in [Color::White, Color::Blue, Color::Black, Color::Red, Color::Green] {
            assert_eq!(Color::from_letter(c.letter()), Some(c));
        }
        assert_eq!(Color::from_letter("C"), None);
    }

    #[test]
    fn action_card() {
        assert_eq!(Action::Pass.card(), None);
        assert_eq!(Action::Activate(7, 1).card(), Some(7));
    }
}
