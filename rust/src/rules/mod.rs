//! Engine-Grenze der UI: das Protokoll-Modell kommt aus dem godot-freien Crate
//! `clash-protocol` (Decision, Choice, Action, …, [`RulesEngine`]); hier liegt nur die
//! Godot-Hülle – [`Request`] mit den Kartendaten als `Dictionary` und die Umwandlung
//! Godot ↔ JSON ([`json`]).
//!
//! Die UI (`ForgeTable`, HUD) kennt nur diese Typen, keine Details der Java-Engine. Heute setzt
//! [`ForgeBridge`](crate::forge::bridge::ForgeBridge) die Schnittstelle um (über
//! `clash-forge-client`: Java-Prozess oder eingebettet in NeoForge per lokalem Socket).

pub mod json;

use godot::prelude::*;

pub use clash_protocol::{Action, CardId, Choice, Color, Decision, ElsewhereCard, OptionText, PlayerId, RulesEngine, Target};

/// Die decision-Typen unter ihrem gewohnten Pfad (`crate::rules::decision::…`).
pub mod decision {
    pub use clash_protocol::decision::*;
}

/// Eine offene Entscheidung (siehe [`clash_protocol::Request`]) mit den Kartendaten als Godot-
/// `Dictionary` für die Anzeige.
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    /// Kennung der Anfrage (für die Antwort).
    pub id: i64,
    pub decision: Decision,
    pub prompt: String,
    pub source: Option<CardId>,
    pub source_name: String,
    /// Text der Fähigkeit, um die es geht (leer = keiner).
    pub ability: String,
    pub cancelable: bool,
    pub labels: Vec<String>,
    /// Kartendaten (`{id, name, type, cost, power, toughness, …}`) der Karten, um die es geht –
    /// auch aus verdeckten Zonen (Bibliothek), damit die UI sie darstellen kann.
    pub cards: Vec<VarDictionary>,
}

impl Request {
    /// Liest eine Anfrage aus der Nachricht `request` (wie sie das Signal
    /// `decision_requested` liefert). `None` bei unbekannter Entscheidung.
    pub fn from_var(msg: &VarDictionary) -> Option<Request> {
        let r = clash_protocol::Request::from_json(&json::from_variant(&msg.to_variant()))?;
        Some(Request {
            id: r.id,
            decision: r.decision,
            prompt: r.prompt,
            source: r.source,
            source_name: r.source_name,
            ability: r.ability,
            cancelable: r.cancelable,
            labels: r.labels,
            cards: r.cards.iter().filter_map(|c| c.as_object()).map(json::to_dictionary).collect(),
        })
    }

    /// Anzeigetext für Eintrag `i` der Hauptliste (leer, wenn keiner mitgeschickt wurde).
    pub fn label(&self, i: usize) -> String {
        self.labels.get(i).cloned().unwrap_or_default()
    }
}

/// Ziel (`{"card": id}` / `{"player": id}`) aus einem Godot-Wert, z. B. aus dem Spielzustand.
pub fn target_from_var(v: &Variant) -> Option<Target> {
    Target::from_json(&json::from_variant(v))
}
