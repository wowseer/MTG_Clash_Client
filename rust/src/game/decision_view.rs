//! Darstellung von Engine-Entscheidungen im HUD: [`Decision`] → Auswahlfenster
//! ([`HudChoice`]) und die Eingabe im Fenster → [`Choice`].
//!
//! Engine-neutral: kennt nur das Entscheidungsmodell (`crate::rules`), keine Java-/Forge-Details.
//! Entscheidungen, die auf dem Tisch beantwortet werden (`Priority`: Karte ziehen, Button
//! „Passen“), erzeugen kein Fenster.

use godot::prelude::*;

use super::choice_panel::{ChoiceMode, ChoiceOption, HudChoice};
use crate::game::mana::ManaType;
use crate::rules::{Choice, Color, Decision, OptionText, Request, Target};

/// Auswahlfenster für die Anfrage, oder `None`, wenn sie nicht im Fenster beantwortet wird.
/// `name_of(t)` liefert den Anzeigenamen eines Ziels bzw. einer Karte, falls die
/// Anfrage keine `labels` mitbringt.
pub fn hud_choice(req: &Request, name_of: &dyn Fn(Target) -> String) -> Option<HudChoice> {
    let plain = |texts: Vec<String>| -> Vec<ChoiceOption> {
        texts.into_iter().map(|text| ChoiceOption { text, color: None }).collect()
    };
    let (default_title, mode, options): (String, ChoiceMode, Vec<ChoiceOption>) = match &req.decision {
        Decision::Mulligan { .. } => (
            tr!("Starthand behalten?", "Keep your opening hand?").into(),
            ChoiceMode::Confirm,
            plain(vec![tr!("Behalten", "Keep").into(), tr!("Mulligan", "Mulligan").into()]),
        ),
        Decision::ChooseModes { modes, min, max, .. } => (
            tr!("Modus wählen", "Choose a mode").into(),
            single_or_multi(*min, *max),
            plain(modes.iter().map(|(_, t)| t.clone()).collect()),
        ),
        Decision::ChooseTargets { candidates, min, max, description, .. } => (
            description.clone(),
            single_or_multi(*min, *max),
            plain(
                (0..candidates.len())
                    .map(|i| Some(req.label(i)).filter(|l| !l.is_empty()).unwrap_or_else(|| name_of(candidates[i])))
                    .collect(),
            ),
        ),
        Decision::ChooseX { max, .. } => {
            (tr!("Wert für X wählen", "Choose a value for X").into(), ChoiceMode::Number { min: 0, max: *max as i64 }, vec![])
        }
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
        Decision::AssignCombatDamage { attacker, amount, blockers, lethal, trample_to, .. } => {
            let mut options: Vec<String> = blockers
                .iter()
                .zip(lethal.iter().chain(std::iter::repeat(&0)))
                .map(|(b, l)| trf!("{} (tödlich: {})", "{} (lethal: {})", name_of(Target::Card(*b)), l))
                .collect();
            // Vorbelegung: tödlicher Schaden der Reihe nach, Rest aufs Trampel-Ziel bzw. den letzten Blocker
            let mut left = *amount as i64;
            let mut initial: Vec<i64> = lethal
                .iter()
                .take(blockers.len())
                .map(|l| {
                    let v = (*l as i64).min(left);
                    left -= v;
                    v
                })
                .collect();
            initial.resize(blockers.len(), 0);
            if let Some(t) = trample_to {
                options.push(trf!("{} (Trampeln)", "{} (trample)", name_of(*t)));
                initial.push(left);
            } else if let Some(last) = initial.last_mut() {
                *last += left;
            }
            (
                trf!("Kampfschaden von {} verteilen", "Assign combat damage of {}", name_of(Target::Card(*attacker))),
                ChoiceMode::Distribute { total: *amount as i64, mins: vec![0; options.len()], initial },
                plain(options),
            )
        }
        Decision::DivideAmount { amount, targets, min_each, .. } => {
            let options: Vec<String> = (0..targets.len())
                .map(|i| Some(req.label(i)).filter(|l| !l.is_empty()).unwrap_or_else(|| name_of(targets[i])))
                .collect();
            let min = *min_each as i64;
            let mut initial = vec![min; targets.len()];
            if let Some(first) = initial.first_mut() {
                *first += *amount as i64 - min * targets.len() as i64;
            }
            (
                trf!("{amount} auf die Ziele verteilen", "Divide {amount} among the targets"),
                ChoiceMode::Distribute { total: *amount as i64, mins: vec![min; targets.len()], initial },
                plain(options),
            )
        }
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
    // Bei der Zielwahl an den Rand: Ziele werden auch direkt auf dem Tisch angeklickt
    let side = matches!(req.decision, Decision::ChooseTargets { .. });
    let numbered = matches!(req.decision, Decision::OrderTriggers { .. });
    // „You may choose the same mode more than once“ (z. B. Confluence-Zauber)
    let repeat = matches!(req.decision, Decision::ChooseModes { can_repeat: true, .. });
    Some(HudChoice { key: req.id, title, detail, mode, options, cancelable: req.cancelable, side, numbered, repeat })
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
