//! Spielprotokoll im HUD: die letzten Einträge des Protokolls der Engine (wer was gewirkt hat,
//! Schaden, Zonenwechsel, Kampf …), oben rechts, neuester Eintrag unten. Klickdurchlässig.
//!
//! Kein eigener Godot-Knoten: das [`TurnHud`](super::hud::TurnHud) baut die Liste und füttert sie
//! jedes Frame mit Zeilen vom [`ForgeTable`](crate::forge::table::ForgeTable).

use godot::classes::control::{GrowDirection, MouseFilter};
use godot::classes::text_server::AutowrapMode;
use godot::classes::{Control, PanelContainer, VBoxContainer};
use godot::prelude::*;

use super::hud::{label, panel, place};

const PARCHMENT: Color = Color::from_rgb(0.93, 0.87, 0.74);
const GOLD: Color = Color::from_rgb(0.95, 0.78, 0.4);
const WIDTH: f32 = 340.0;

/// Arten von Protokolleinträgen, die das HUD nicht zeigt (zu häufig, stehen schon im Zeitstrahl
/// bzw. Manapool).
pub const HIDDEN_KINDS: [&str; 2] = ["PHASE", "MANA"];

/// Entfernt Objekt-IDs in Klammern hinter Kartennamen („Forest (105)“ → „Forest“).
pub fn strip_ids(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find(" (") {
        let after = &rest[start + 2..];
        match after.find(')') {
            Some(end) if end > 0 && after[..end].chars().all(|c| c.is_ascii_digit()) => {
                out.push_str(&rest[..start]);
                rest = &after[end + 1..];
            }
            _ => {
                out.push_str(&rest[..start + 2]);
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Eine Zeile des Protokolls.
#[derive(Clone, PartialEq, Debug)]
pub struct HudLogLine {
    pub text: String,
    /// Rundenbeginn („Runde 3“): golden hervorgehoben.
    pub turn_start: bool,
}

/// Das Protokollfenster oben rechts.
pub struct GameLogList {
    root: Gd<PanelContainer>,
    rows: Gd<VBoxContainer>,
    shown: Vec<HudLogLine>,
}

impl GameLogList {
    pub fn new(parent: &mut Gd<Control>) -> Self {
        let mut root = panel();
        root.set_custom_minimum_size(Vector2::new(WIDTH, 0.0));
        root.set_modulate(Color::from_rgba(1.0, 1.0, 1.0, 0.85));
        place(&mut root, (1.0, 0.0), (-14.0, 10.0), GrowDirection::BEGIN, GrowDirection::END);
        let mut rows = VBoxContainer::new_alloc();
        rows.set_mouse_filter(MouseFilter::IGNORE);
        rows.add_theme_constant_override("separation", 2);
        root.add_child(&rows);
        root.set_visible(false);
        parent.add_child(&root);
        Self { root, rows, shown: Vec::new() }
    }

    /// Zeigt `lines` (ältester zuerst); baut nur bei Änderung neu.
    pub fn show(&mut self, lines: &[HudLogLine]) {
        if self.shown == lines {
            return;
        }
        self.shown = lines.to_vec();
        for mut child in self.rows.get_children().iter_shared() {
            child.queue_free();
        }
        for line in lines {
            let mut l = label(13, if line.turn_start { GOLD } else { PARCHMENT });
            l.set_autowrap_mode(AutowrapMode::WORD_SMART);
            l.set_custom_minimum_size(Vector2::new(WIDTH - 24.0, 0.0));
            l.set_text(&line.text);
            self.rows.add_child(&l);
        }
        self.root.set_visible(!lines.is_empty());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_object_ids() {
        assert_eq!(strip_ids("Forge-KI played Forest (105)"), "Forge-KI played Forest");
        assert_eq!(strip_ids("cast Naturalize targeting [Bonesplitter (39)]"), "cast Naturalize targeting [Bonesplitter]");
        assert_eq!(strip_ids("gets +2/+0 (until end of turn)"), "gets +2/+0 (until end of turn)");
    }
}
