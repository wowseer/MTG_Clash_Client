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
        Decision::NameCard { options, .. } => {
            (tr!("Kartennamen nennen", "Name a card").into(), ChoiceMode::Single, plain(options.clone()))
        }
        Decision::ChooseType { options, .. } => {
            (tr!("Typ wählen", "Choose a type").into(), ChoiceMode::Single, plain(options.clone()))
        }
        Decision::Confirm { prompt, .. } => (prompt.clone(), ChoiceMode::Confirm, yes_no()),
        Decision::PayUnlessCost { cost, .. } => (
            trf!("Zahlen, um den Effekt zu verhindern: {cost}?", "Pay {cost} to prevent the effect?"),
            ChoiceMode::Confirm,
            plain(vec![tr!("Zahlen", "Pay").into(), tr!("Nicht zahlen", "Don't pay").into()]),
        ),
        Decision::PayLifeForPhyrexian { symbol, .. } => (
            trf!("Phyrexianisches Mana ({symbol}): mit 2 Leben bezahlen?", "Phyrexian mana ({symbol}): pay 2 life?"),
            ChoiceMode::Confirm,
            plain(vec![tr!("2 Leben zahlen", "Pay 2 life").into(), tr!("Mit Mana zahlen", "Pay with mana").into()]),
        ),
        Decision::OptionalTrigger { description, .. } => (
            trf!("Ausgelöste Fähigkeit nutzen?\n{description}", "Use the triggered ability?\n{description}"),
            ChoiceMode::Confirm,
            yes_no(),
        ),
        Decision::ChooseOption { prompt, options, .. } => (prompt.clone(), ChoiceMode::Single, ext_options(options)),
        Decision::ChooseOptions { prompt, options, min, max, .. } => {
            (prompt.clone(), single_or_multi(*min, *max), ext_options(options))
        }
        // Verteilen: Knöpfe an den Zielen auf dem Tisch (ForgeTable::show_divide), kein Fenster
        Decision::AssignCombatDamage { .. } | Decision::DivideAmount { .. } => return None,
        Decision::OrderTriggers { descriptions, .. } => (
            tr!("Reihenfolge wählen: in der Reihenfolge anklicken (erste zuerst)", "Choose the order: click in order (first goes first)").into(),
            ChoiceMode::Multi { min: descriptions.len(), max: descriptions.len() },
            plain(descriptions.clone()),
        ),
        Decision::ChooseNumber { prompt, min, max, .. } => {
            (prompt.clone(), ChoiceMode::Number { min: *min as i64, max: *max as i64 }, vec![])
        }
        // Auf dem Tisch beantwortet bzw. (noch) nicht von der Java-Engine gestellt
        _ => return None,
    };
    // Der Fragetext der Engine hat Vorrang; bei OptionalTrigger steht der Text schon im Titel
    let title = match (&req.decision, req.prompt.is_empty()) {
        (Decision::OptionalTrigger { .. }, _) | (_, true) => default_title,
        _ => req.prompt.clone(),
    };
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
