//! Darstellung von Engine-Entscheidungen im HUD: [`Decision`] → Auswahlfenster
//! ([`HudChoice`]) und die Eingabe im Fenster → [`Choice`].
//!
//! Engine-neutral: kennt nur das Entscheidungsmodell (`crate::rules`), keine Java-/Forge-Details.
//! Entscheidungen, die auf dem Tisch beantwortet werden (`Priority`: Karte ziehen, Button
//! „Passen“), erzeugen kein Fenster.

use godot::prelude::*;

use super::choice_panel::{ChoiceMode, ChoiceOption, HudChoice};
use crate::game::mana::ManaType;
use crate::rules::{Choice, Color, Decision, OptionText, Request};

/// Überschrift einer Auswahl nach ihrem Zweck (`purpose`, Erweiterung des Protokolls): nur die
/// Mechanik, nie Kartennamen oder -texte. `None` = Zweck ohne eigene Überschrift.
pub fn purpose_title(purpose: &str) -> Option<String> {
    let t = match purpose {
        "Vote" => tr!("Abstimmen", "Vote"),
        "Keyword" => tr!("Schlüsselwort wählen", "Choose a keyword"),
        "CounterType" => tr!("Markenart wählen", "Choose a counter type"),
        "Protection" => tr!("Schutz wählen", "Choose protection"),
        "NumberList" => tr!("Zahl wählen", "Choose a number"),
        "ReplacementEffect" => tr!("Ersatzeffekt wählen", "Choose a replacement effect"),
        "StaticAbility" => tr!("Statische Fähigkeit wählen", "Choose a static ability"),
        "CardFace" => tr!("Seite wählen", "Choose a face"),
        "Dungeon" => tr!("Wähle einen Dungeon", "Choose a dungeon"),
        "DungeonRoom" => tr!("Wähle den nächsten Raum", "Choose the next room"),
        "Piles" => tr!("Stapel wählen", "Choose a pile"),
        "Dice" => tr!("Würfel", "Dice"),
        "Retarget" => tr!("Neues Ziel wählen", "Choose a new target"),
        "ManaPool" => tr!("Mana aus dem Pool", "Mana from your pool"),
        "OpeningHand" => tr!("Aus der Starthand", "From your opening hand"),
        _ => return None,
    };
    Some(t.to_string())
}

/// Auswahlfenster für die Anfrage, oder `None`, wenn sie nicht im Fenster beantwortet wird.
pub fn hud_choice(req: &Request) -> Option<HudChoice> {
    let plain = |texts: Vec<String>| -> Vec<ChoiceOption> {
        texts.into_iter().map(|text| ChoiceOption { text, color: None }).collect()
    };
    let (default_title, mode, options): (String, ChoiceMode, Vec<ChoiceOption>) = match &req.decision {
        // Mulligan: Bühne mit 3D-Knöpfen auf dem Tisch (ForgeTable::show_mulligan)
        Decision::Mulligan { .. } => return None,
        Decision::ChooseModes { modes, min, max, .. } => (
            tr!("Modus wählen", "Choose a mode").into(),
            single_or_multi(*min, *max),
            plain(modes.iter().map(|(_, t)| t.clone()).collect()),
        ),
        // Zielwahl: Pfeil auf dem Tisch zeichnen (ForgeTable), kein Fenster
        Decision::ChooseTargets { .. } => return None,
        // X wählen: 3D-Knöpfe am Stack (ForgeTable::show_choose_x)
        Decision::ChooseX { .. } => return None,
        Decision::ChooseColor { options, .. } => (
            tr!("Farbe wählen", "Choose a color").into(),
            ChoiceMode::Single,
            options.iter().map(|c| ChoiceOption { text: color_name(*c).into(), color: Some(mana(*c).color()) }).collect(),
        ),
        // Kartennamen / Typ: Bühne mit Suchfeld (ForgeTable::show_search), kein Fenster
        Decision::NameCard { .. } | Decision::ChooseType { .. } => return None,
        Decision::Confirm { prompt, .. } => (prompt.clone(), ChoiceMode::Confirm, yes_no()),
        // kurzer Titel (nur die Mechanik), die Kosten stehen auf dem Knopf („Zahlen {2}“)
        Decision::PayUnlessCost { cost, .. } => {
            let cost = cost.trim().trim_start_matches("Pay ").trim();
            let pay = if cost.is_empty() { tr!("Zahlen", "Pay").to_string() } else { trf!("Zahlen {cost}", "Pay {cost}") };
            (
                tr!("Effekt verhindern?", "Prevent the effect?").into(),
                ChoiceMode::Confirm,
                plain(vec![pay, tr!("Nicht zahlen", "Don't pay").into()]),
            )
        }
        Decision::PayLifeForPhyrexian { symbol, .. } => (
            trf!("{symbol} mit 2 Leben bezahlen?", "Pay {symbol} with 2 life?"),
            ChoiceMode::Confirm,
            plain(vec![tr!("2 Leben zahlen", "Pay 2 life").into(), tr!("Mit Mana zahlen", "Pay with mana").into()]),
        ),
        // nur die Mechanik; der Text der Fähigkeit steht auf der Karte
        Decision::OptionalTrigger { .. } => (
            tr!("Ausgelöste Fähigkeit nutzen?", "Use the triggered ability?").into(),
            ChoiceMode::Confirm,
            yes_no(),
        ),
        Decision::ChooseOption { purpose, options, .. } => (
            purpose_title(purpose).unwrap_or_else(|| tr!("Wähle eins", "Choose one").into()),
            ChoiceMode::Single,
            ext_options(options),
        ),
        Decision::ChooseOptions { purpose, options, min, max, .. } => (
            purpose_title(purpose).unwrap_or_else(|| if min == max { trf!("Wähle {max}", "Choose {max}") } else { trf!("Wähle {min}–{max}", "Choose {min}–{max}") }),
            single_or_multi(*min, *max),
            ext_options(options),
        ),
        // Verteilen: Knöpfe an den Zielen auf dem Tisch (ForgeTable::show_divide), kein Fenster
        Decision::AssignCombatDamage { .. } | Decision::DivideAmount { .. } => return None,
        Decision::OrderTriggers { descriptions, .. } => (
            tr!("Reihenfolge wählen: in der Reihenfolge anklicken (erste zuerst)", "Choose the order: click in order (first goes first)").into(),
            ChoiceMode::Multi { min: descriptions.len(), max: descriptions.len() },
            plain(descriptions.clone()),
        ),
        Decision::ChooseNumber { min, max, .. } => {
            (tr!("Zahl wählen", "Choose a number").into(), ChoiceMode::Number { min: *min as i64, max: *max as i64 }, vec![])
        }
        // Auf dem Tisch beantwortet bzw. (noch) nicht von der Java-Engine gestellt
        _ => return None,
    };
    // Überschrift = nur die Mechanik (eigener Text der UI); Fragetexte der Engine enthalten oft
    // Kartennamen und -texte. Nur Ja/Nein-Fragen (`Confirm`) behalten die Frage der Engine.
    let title = default_title;
    let detail = match (req.source_name.is_empty(), req.ability.is_empty() || req.ability == req.source_name) {
        (true, true) => String::new(),
        (false, true) => req.source_name.clone(),
        (true, false) => req.ability.clone(),
        (false, false) => format!("{}: {}", req.source_name, req.ability),
    };
    let numbered = matches!(req.decision, Decision::OrderTriggers { .. });
    // „You may choose the same mode more than once“ (z. B. Confluence-Zauber)
    let repeat = matches!(req.decision, Decision::ChooseModes { can_repeat: true, .. });
    Some(HudChoice { key: req.id, title, detail, mode, options, cancelable: req.cancelable, side: false, numbered, repeat })
}

/// Eingabe aus dem Auswahlfenster (Index, Indizes, bool, Zahl; nil = Abbrechen) → [`Choice`]
/// für `decision`. `None` bei unpassender Eingabe.
pub fn to_choice(decision: &Decision, raw: &Variant) -> Option<Choice> {
    if raw.is_nil() {
        return Some(Choice::Cancel);
    }
    let index = || raw.try_to::<i64>().ok().filter(|i| *i >= 0).map(|i| i as usize);
    let indices = || -> Option<Vec<usize>> {
        match raw.try_to::<VarArray>() {
            Ok(arr) => arr.iter_shared().map(|v| v.try_to::<i64>().ok().filter(|i| *i >= 0).map(|i| i as usize)).collect(),
            Err(_) => index().map(|i| vec![i]),
        }
    };
    let yes = || raw.try_to::<bool>().ok();
    Some(match decision {
        Decision::Mulligan { .. } => {
            if yes()? {
                Choice::Keep
            } else {
                Choice::Mulligan
            }
        }
        Decision::ChooseModes { modes, .. } => {
            Choice::Modes(indices()?.into_iter().map(|i| modes.get(i).map(|m| m.0)).collect::<Option<_>>()?)
        }
        Decision::ChooseTargets { candidates, .. } => {
            Choice::Targets(indices()?.into_iter().map(|i| candidates.get(i).copied()).collect::<Option<_>>()?)
        }
        Decision::ChooseX { .. } | Decision::ChooseNumber { .. } => Choice::Number(raw.try_to::<i64>().ok()?.max(0) as u32),
        Decision::ChooseStartingPlayer { players, .. } => {
            let i = index()?;
            (i < players.len()).then_some(Choice::Number(i as u32))?
        }
        Decision::ChooseColor { options, .. } => Choice::Color(*options.get(index()?)?),
        Decision::NameCard { .. } | Decision::ChooseType { .. } | Decision::ChooseOption { .. } => {
            Choice::Number(index()? as u32)
        }
        Decision::ChooseOptions { .. } => Choice::Modes(indices()?),
        Decision::OrderTriggers { .. } => Choice::Order(indices()?),
        Decision::AssignCombatDamage { .. } | Decision::DivideAmount { .. } => {
            let arr = raw.try_to::<VarArray>().ok()?;
            Choice::Damage(arr.iter_shared().map(|v| v.try_to::<i64>().ok().map(|n| n.max(0) as u32)).collect::<Option<_>>()?)
        }
        Decision::Confirm { .. }
        | Decision::OptionalTrigger { .. }
        | Decision::PayUnlessCost { .. }
        | Decision::PayLifeForPhyrexian { .. } => {
            if yes()? {
                Choice::Yes
            } else {
                Choice::No
            }
        }
        _ => return None,
    })
}

fn single_or_multi(min: u32, max: u32) -> ChoiceMode {
    if min == 1 && max == 1 {
        ChoiceMode::Single
    } else {
        ChoiceMode::Multi { min: min as usize, max: max as usize }
    }
}

fn yes_no() -> Vec<ChoiceOption> {
    vec![
        ChoiceOption { text: tr!("Ja", "Yes").into(), color: None },
        ChoiceOption { text: tr!("Nein", "No").into(), color: None },
    ]
}

fn ext_options(options: &[OptionText]) -> Vec<ChoiceOption> {
    options
        .iter()
        .map(|o| ChoiceOption {
            text: o.text.clone(),
            color: o.color.map(|c| mana(c).color()).or(o.colorless.then(|| ManaType::Colorless.color())),
        })
        .collect()
}

fn mana(c: Color) -> ManaType {
    match c {
        Color::White => ManaType::White,
        Color::Blue => ManaType::Blue,
        Color::Black => ManaType::Black,
        Color::Red => ManaType::Red,
        Color::Green => ManaType::Green,
    }
}

fn color_name(c: Color) -> &'static str {
    match c {
        Color::White => tr!("Weiß", "White"),
        Color::Blue => tr!("Blau", "Blue"),
        Color::Black => tr!("Schwarz", "Black"),
        Color::Red => tr!("Rot", "Red"),
        Color::Green => tr!("Grün", "Green"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_when_exactly_one() {
        assert_eq!(single_or_multi(1, 1), ChoiceMode::Single);
        assert_eq!(single_or_multi(0, 2), ChoiceMode::Multi { min: 0, max: 2 });
    }
}
