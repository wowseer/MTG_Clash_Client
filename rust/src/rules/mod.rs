//! Engine-Grenze der UI: Protokoll-Modell ([`decision`]) und die Schnittstelle
//! [`RulesEngine`], über die die UI Antworten an die Regel-Engine schickt.
//!
//! Die UI (`ForgeTable`, HUD) kennt nur diese Typen, keine Details der Java-Engine. Heute setzt
//! [`ForgeBridge`](crate::forge::bridge::ForgeBridge) die Schnittstelle um (Java-Prozess,
//! Protokoll v3) – eigenständig per Java-Prozess oder eingebettet in NeoForge (lokaler Socket).

pub mod decision;

use godot::prelude::*;

pub use decision::{Action, CardId, Choice, Color, Decision, OptionText, PlayerId, Target};

/// Eine offene Entscheidung mit allem, was die UI zur Anzeige braucht.
///
/// `decision` ist die eigentliche Entscheidung. Die übrigen Felder sind Anzeigehilfen
/// (Erweiterung der Hülle, nicht Teil von `Decision`): Fragetext, Quellkarte, ob
/// abgebrochen werden darf, und `labels` = Anzeigetext je Eintrag der Hauptliste der
/// Entscheidung (Priority: `actions`, ChooseTargets: `candidates`, ChooseCards: `options`, …).
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
    /// Liest eine Anfrage (`{"type":"request","id":…,"decision":{…},…}`). `None` bei
    /// unbekannter Entscheidung.
    pub fn from_var(msg: &VarDictionary) -> Option<Request> {
        let get = |k: &str| msg.get(k).filter(|v| !v.is_nil());
        let decision = Decision::from_var(&get("decision")?)?;
        let source = get("source").and_then(|v| v.try_to::<VarDictionary>().ok());
        Some(Request {
            id: get("id").and_then(|v| v.try_to::<i64>().ok()).unwrap_or(0),
            decision,
            prompt: get("prompt").map(|v| v.to_string()).unwrap_or_default(),
            source: source
                .as_ref()
                .and_then(|s| s.get("id"))
                .and_then(|v| v.try_to::<i64>().ok())
                .map(|i| i.max(0) as CardId),
            source_name: source.as_ref().and_then(|s| s.get("name")).map(|v| v.to_string()).unwrap_or_default(),
            ability: get("ability").map(|v| v.to_string()).unwrap_or_default(),
            cancelable: get("cancelable").is_some_and(|v| v.booleanize()),
            labels: get("labels")
                .and_then(|v| v.try_to::<VarArray>().ok())
                .map(|a| a.iter_shared().map(|v| v.to_string()).collect())
                .unwrap_or_default(),
            cards: get("cards")
                .and_then(|v| v.try_to::<VarArray>().ok())
                .map(|a| a.iter_shared().filter_map(|v| v.try_to::<VarDictionary>().ok()).collect())
                .unwrap_or_default(),
        })
    }

    /// Anzeigetext für Eintrag `i` der Hauptliste (leer, wenn keiner mitgeschickt wurde).
    pub fn label(&self, i: usize) -> String {
        self.labels.get(i).cloned().unwrap_or_default()
    }
}

/// Was die UI von einer Regel-Engine braucht. Antworten gehen immer auf eine offene
/// [`Request`]; der Spielzustand kommt über [`RulesEngine::request_state`] (asynchron).
pub trait RulesEngine {
    /// Antwortet auf die Anfrage `request_id`. false = Engine nicht erreichbar.
    fn submit(&mut self, request_id: i64, choice: &Choice) -> bool;
    /// Fordert den aktuellen Spielzustand an (aus Sicht von `viewer`).
    fn request_state(&mut self, viewer: PlayerId) -> bool;
}
