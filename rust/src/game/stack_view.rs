//! Stack-Liste im HUD (CR 405): alle Zauber und Fähigkeiten auf dem Stack mit Quelle, Art und
//! Forges Beschreibung, oberster Eintrag oben. Sitzt am linken Bildrand in der Mitte; der Rand
//! jedes Eintrags trägt die Farbe der Seite, die ihn kontrolliert. Leerer Stack = unsichtbar.
//!
//! Kein eigener Godot-Knoten: das [`TurnHud`](super::hud::TurnHud) baut die Liste und füttert sie
//! jedes Frame mit [`HudStackEntry`]s vom [`ForgeTable`](crate::forge::table::ForgeTable).

use godot::classes::control::{GrowDirection, MouseFilter};
use godot::classes::text_server::AutowrapMode;
use godot::classes::{Control, Label, PanelContainer, VBoxContainer};
use godot::prelude::*;

use super::hud::{label, panel, place, style};

const GOLD: Color = Color::from_rgb(0.95, 0.78, 0.4);
const PARCHMENT: Color = Color::from_rgb(0.93, 0.87, 0.74);
/// Seitenfarben wie Medaillons und Banner (Sitzplatz 0, 1).
const SEAT_ACCENTS: [Color; 2] = [Color::from_rgb(0.25, 0.55, 0.95), Color::from_rgb(0.9, 0.25, 0.22)];
const WIDTH: f32 = 300.0;

/// Ein Eintrag der Stack-Liste.
#[derive(Clone, PartialEq, Debug)]
pub struct HudStackEntry {
    /// „Quelle · Art“, z. B. „Shock · Zauber“.
    pub title: String,
    /// Forges Beschreibung des Eintrags (mit Zielen).
    pub text: String,
    /// Sitzplatz des Controllers (−1 = unbekannt).
    pub seat: i64,
}

/// Die Stack-Liste: Überschrift und je Eintrag ein Kästchen.
pub struct StackList {
    root: Gd<PanelContainer>,
    rows: Gd<VBoxContainer>,
    shown: Vec<HudStackEntry>,
}

impl StackList {
    pub fn new(parent: &mut Gd<Control>) -> Self {
        let mut root = panel();
        root.set_custom_minimum_size(Vector2::new(WIDTH, 0.0));
        place(&mut root, (0.0, 0.5), (14.0, 0.0), GrowDirection::END, GrowDirection::BOTH);
        let mut column = VBoxContainer::new_alloc();
        column.set_mouse_filter(MouseFilter::IGNORE);
        column.add_theme_constant_override("separation", 6);
        let mut title = label(17, GOLD);
        title.set_text(tr!("Stack", "Stack"));
        column.add_child(&title);
        let mut rows = VBoxContainer::new_alloc();
        rows.set_mouse_filter(MouseFilter::IGNORE);
        rows.add_theme_constant_override("separation", 5);
        column.add_child(&rows);
        root.add_child(&column);
        root.set_visible(false);
        parent.add_child(&root);
        Self { root, rows, shown: Vec::new() }
    }

    /// Zeigt `entries` (oberster zuerst); baut die Zeilen nur bei Änderung neu.
    pub fn show(&mut self, entries: &[HudStackEntry]) {
        if self.shown == entries {
            return;
        }
        self.shown = entries.to_vec();
        for mut child in self.rows.get_children().iter_shared() {
            child.queue_free();
        }
        for (i, e) in entries.iter().enumerate() {
            let accent = usize::try_from(e.seat).ok().and_then(|s| SEAT_ACCENTS.get(s)).copied().unwrap_or(PARCHMENT);
            let mut bg = style(Color::from_rgba(0.1, 0.11, 0.15, if i == 0 { 0.95 } else { 0.75 }), 6.0);
            bg.set_border_color(accent);
            bg.set_content_margin_all(7.0);
            let mut cell = PanelContainer::new_alloc();
            cell.add_theme_stylebox_override("panel", &bg);
            cell.set_mouse_filter(MouseFilter::IGNORE);
            let mut col = VBoxContainer::new_alloc();
            col.set_mouse_filter(MouseFilter::IGNORE);
            let mut title = label(15, if i == 0 { GOLD } else { PARCHMENT });
            title.set_text(&e.title);
            col.add_child(&title);
            if !e.text.is_empty() {
                col.add_child(&text_label(&e.text));
            }
            cell.add_child(&col);
            self.rows.add_child(&cell);
        }
        self.root.set_visible(!entries.is_empty());
    }
}

fn text_label(text: &str) -> Gd<Label> {
    let mut l = label(13, PARCHMENT);
    l.set_autowrap_mode(AutowrapMode::WORD_SMART);
    l.set_custom_minimum_size(Vector2::new(WIDTH - 40.0, 0.0));
    l.set_text(text);
    l
}
