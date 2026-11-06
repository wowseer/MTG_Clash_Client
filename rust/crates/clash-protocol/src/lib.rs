//! # clash-protocol
//!
//! Protokoll-Modell und JSON-Form (Protokoll v3) der Forge-Brücke von Neo Forge 3D – ohne Godot,
//! ohne Java, nur `serde_json`. Damit kann jede Rust-Oberfläche (Godot, Bevy, Terminal, Tests)
//! Forge bedienen.
//!
//! - [`Decision`]: was die Engine einen Spieler fragt; [`Choice`]: seine Antwort
//!   ([`decision`], nach dem Vorbild von forge-rules-rs).
//! - [`Request`]: eine offene Entscheidung samt Anzeigehilfen (Nachricht `request`).
//! - [`command`]: die Befehle an die Brücke als JSON (`start_match`, `answer`, `state` …).
//! - [`RulesEngine`]: die Schnittstelle, über die eine Oberfläche Antworten schickt.
//!
//! Transport (Java-Prozess bzw. Socket zum Gastgeber) liefert das Crate `clash-forge-client`.
//! Das vollständige Protokoll beschreibt `docs/Forge-Protocol.md`.
//!
//! ```
//! use clash_protocol::{Choice, Action, Request, command};
//! let line = r#"{"type":"request","id":7,"decision":{"type":"Priority","player":0,"actions":[{"type":"Pass"}]}}"#;
//! let msg: serde_json::Value = serde_json::from_str(line).unwrap();
//! let req = Request::from_json(&msg).unwrap();
//! let answer = command::answer(req.id, &Choice::Action(Action::Pass));
//! assert_eq!(answer["cmd"], "answer");
//! ```

pub mod command;
pub mod decision;

pub use decision::{
    Action, CardChoiceReason, CardId, Choice, Color, Decision, ElsewhereCard, OptionText, PayAction, PlayerId, Target,
};
pub use serde_json::Value;

use decision::{int, text, Obj};

/// Version des Protokolls, die diese Typen sprechen (Nachricht `ready`, Feld `protocol`).
pub const PROTOCOL_VERSION: i64 = 3;

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
    pub cards: Vec<Value>,
}

impl Request {
    /// Liest eine Anfrage (`{"type":"request","id":…,"decision":{…},…}`). `None` bei
    /// unbekannter Entscheidung.
    pub fn from_json(msg: &Value) -> Option<Request> {
        let o = Obj(msg.as_object()?);
        let decision = Decision::from_json(o.get("decision")?)?;
        let source = o.get("source").and_then(Value::as_object).map(Obj);
        Some(Request {
            id: o.int("id"),
            decision,
            prompt: o.str("prompt"),
            source: source.as_ref().and_then(|s| s.get("id")).and_then(int).map(|i| i.max(0) as CardId),
            source_name: source.as_ref().and_then(|s| s.get("name")).map(text).unwrap_or_default(),
            ability: o.str("ability"),
            cancelable: o.bool("cancelable"),
            labels: o.arr("labels").iter().map(text).collect(),
            cards: o.arr("cards").iter().filter(|c| c.is_object()).cloned().collect(),
        })
    }

    /// Anzeigetext für Eintrag `i` der Hauptliste (leer, wenn keiner mitgeschickt wurde).
    pub fn label(&self, i: usize) -> String {
        self.labels.get(i).cloned().unwrap_or_default()
    }
}

/// Art einer Nachricht der Brücke (Feld `type`: `ready`, `request`, `state`, `event`, `log`,
/// `game_over`, `error` …), leer wenn keine.
pub fn message_type(msg: &Value) -> &str {
    msg.get("type").and_then(Value::as_str).unwrap_or("")
}

/// Was eine Oberfläche von einer Regel-Engine braucht. Antworten gehen immer auf eine offene
/// [`Request`]; der Spielzustand kommt über [`RulesEngine::request_state`] (asynchron).
pub trait RulesEngine {
    /// Antwortet auf die Anfrage `request_id`. false = Engine nicht erreichbar.
    fn submit(&mut self, request_id: i64, choice: &Choice) -> bool;
    /// Fordert den aktuellen Spielzustand an (aus Sicht von `viewer`).
    fn request_state(&mut self, viewer: PlayerId) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_request() {
        let msg = json!({"type": "request", "id": 4, "prompt": "Wähle", "cancelable": true,
            "source": {"id": 9, "name": "Shock"}, "labels": ["a", "b"],
            "decision": {"type": "ChooseX", "player": 0, "max": 3}, "cards": [{"id": 9}]});
        let r = Request::from_json(&msg).unwrap();
        assert_eq!(r.id, 4);
        assert_eq!(r.source, Some(9));
        assert_eq!(r.source_name, "Shock");
        assert!(r.cancelable);
        assert_eq!(r.label(1), "b");
        assert_eq!(r.decision, Decision::ChooseX { player: 0, max: 3 });
        assert_eq!(r.cards.len(), 1);
        assert_eq!(message_type(&msg), "request");
    }
}
