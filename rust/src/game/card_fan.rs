//! Kartenfächer: Overlay in der Bildmitte, das Karten groß zeigt und auswählen bzw. anordnen
//! lässt – auch Karten, die nicht offen auf dem Tisch liegen (Bibliothek durchsuchen, Dig,
//! Mulligan, Hellsicht).
//!
//! Zwei Modi ([`FanMode`]):
//! - **Wählen** (`ChooseCards`, `MulliganBottom`): Klick auf eine Karte wählt sie (goldener
//!   Rahmen) bzw. wählt sie ab; „Fertig (n/…)“ ist nur bei gültiger Anzahl aktiv.
//! - **Anordnen** (`Scry`, Surveil): zwei Reihen „Oben auf die Bibliothek“ und „Unter die
//!   Bibliothek“ bzw. „In den Friedhof“; Klick legt eine Karte ans Ende der anderen Reihe. Die
//!   Reihenfolge in einer Reihe ist die Reihenfolge von oben nach unten.
//!
//! Der Hintergrund dunkelt den Tisch ab und fängt Mausklicks ab. Karten-Knöpfe rufen
//! `on_fan_card(index)`, „Fertig“ `on_fan_done()` und „Abbrechen“ `on_fan_cancel()` am
//! [`TurnHud`](super::hud::TurnHud) auf.

use godot::classes::control::{LayoutPreset, MouseFilter, SizeFlags};
use godot::classes::text_server::AutowrapMode;
use godot::classes::{
    Button, CenterContainer, ColorRect, Control, GridContainer, HBoxContainer, PanelContainer, ScrollContainer,
    VBoxContainer,
};
use godot::prelude::*;

use super::card_preview;
use crate::rules::CardId;
use crate::table::card3d::Card3D;

/// Art des Kartenfächers.
#[derive(Clone, Debug, PartialEq)]
pub enum FanMode {
    /// `min`–`max` Karten wählen.
    Pick { min: usize, max: usize },
    /// Karten auf zwei Reihen verteilen (oben / unten bzw. Friedhof).
    Arrange { graveyard: bool },
}

/// Inhalt des Kartenfächers (vom [`ForgeTable`](crate::forge::table::ForgeTable) geliefert).
#[derive(Clone, Debug)]
pub struct HudFan {
    /// Kennung der Anfrage: der Fächer wird nur bei neuer Kennung neu aufgebaut.
    pub key: i64,
    pub title: String,
    pub detail: String,
    pub mode: FanMode,
    /// Karten (ID und Kartenbild-Quelle) in Anzeige-Reihenfolge.
    pub cards: Vec<(CardId, Gd<Card3D>)>,
    pub cancelable: bool,
}

/// Höhe einer Karte im Fächer (Pixel): wenige Karten groß, viele kleiner.
const BIG: f32 = 230.0;
const SMALL: f32 = 160.0;
/// Ab so vielen Karten werden sie kleiner und in mehreren Zeilen gezeigt.
const MANY: usize = 7;
/// Höchstens so viele Karten pro Zeile.
const COLUMNS: usize = 8;
const GOLD: Color = Color::from_rgb(1.0, 0.82, 0.35);
const DIM_BORDER: Color = Color::from_rgb(0.35, 0.33, 0.3);
const PARCHMENT: Color = Color::from_rgb(0.93, 0.87, 0.74);

/// Das Overlay. Gehört dem [`TurnHud`](super::hud::TurnHud).
pub struct CardFan {
    root: Gd<Control>,
    body: Gd<VBoxContainer>,
    current: Option<HudFan>,
    /// Wählen: gewählte Karten (Indizes in Klick-Reihenfolge).
    selected: Vec<usize>,
    /// Anordnen: Reihenfolge oben bzw. unten (Indizes).
    top: Vec<usize>,
    bottom: Vec<usize>,
    hud: Option<Gd<Object>>,
}

impl CardFan {
    /// Legt das (unsichtbare) Overlay als Vollbild-Kind von `parent` an.
    pub fn new(parent: &mut Gd<Control>) -> Self {
        let mut root = Control::new_alloc();
        root.set_name("CardFan");
        root.set_anchors_preset(LayoutPreset::FULL_RECT);
        root.set_mouse_filter(MouseFilter::STOP);
        root.set_visible(false);
        // Abgedunkelter Tisch dahinter
        let mut dim = ColorRect::new_alloc();
        dim.set_color(Color::from_rgba(0.02, 0.02, 0.04, 0.72));
        dim.set_anchors_preset(LayoutPreset::FULL_RECT);
        dim.set_mouse_filter(MouseFilter::IGNORE);
        root.add_child(&dim);
        let mut center = CenterContainer::new_alloc();
        center.set_anchors_preset(LayoutPreset::FULL_RECT);
        center.set_mouse_filter(MouseFilter::IGNORE);
        let mut panel = super::hud::panel();
        let mut body = VBoxContainer::new_alloc();
        body.add_theme_constant_override("separation", 10);
        panel.add_child(&body);
        center.add_child(&panel);
        root.add_child(&center);
        parent.add_child(&root);
        Self { root, body, current: None, selected: Vec::new(), top: Vec::new(), bottom: Vec::new(), hud: None }
    }

    /// Zeigt `fan` (baut nur bei neuer Anfrage neu auf) bzw. blendet das Overlay aus.
    pub fn show(&mut self, fan: Option<&HudFan>, hud: &Gd<Object>) {
        let Some(fan) = fan else {
            if self.current.take().is_some() {
                self.clear();
                self.root.set_visible(false);
            }
            return;
        };
        if self.current.as_ref().is_some_and(|c| c.key == fan.key && c.mode == fan.mode) {
            return;
        }
        self.current = Some(fan.clone());
        self.hud = Some(hud.clone());
        self.selected.clear();
        self.top = (0..fan.cards.len()).collect();
        self.bottom.clear();
        self.rebuild();
        self.root.set_visible(true);
    }

    /// Klick auf Karte `index`: wählen/abwählen bzw. in die andere Reihe legen.
    pub fn on_card(&mut self, index: usize) {
        let Some(fan) = self.current.as_ref() else { return };
        if index >= fan.cards.len() {
            return;
        }
        match fan.mode {
            FanMode::Pick { max, .. } => {
                if let Some(pos) = self.selected.iter().position(|i| *i == index) {
                    self.selected.remove(pos);
                } else if self.selected.len() < max {
                    self.selected.push(index);
                } else if max == 1 {
                    self.selected = vec![index];
                }
            }
            FanMode::Arrange { .. } => {
                if let Some(pos) = self.top.iter().position(|i| *i == index) {
                    self.top.remove(pos);
                    self.bottom.push(index);
                } else if let Some(pos) = self.bottom.iter().position(|i| *i == index) {
                    self.bottom.remove(pos);
                    self.top.push(index);
                }
            }
        }
        self.rebuild();
    }

    /// „Fertig“: (gewählte bzw. obere Karten, untere Karten) als IDs; `None`, wenn die Auswahl
    /// noch nicht gültig ist.
    pub fn done(&self) -> Option<(Vec<CardId>, Vec<CardId>)> {
        let fan = self.current.as_ref()?;
        let ids = |list: &[usize]| list.iter().map(|i| fan.cards[*i].0).collect::<Vec<_>>();
        match fan.mode {
            FanMode::Pick { min, max } => {
                let n = self.selected.len();
                (n >= min && n <= max).then(|| (ids(&self.selected), Vec::new()))
            }
            FanMode::Arrange { .. } => Some((ids(&self.top), ids(&self.bottom))),
        }
    }

    fn clear(&mut self) {
        for child in self.body.get_children().iter_shared() {
            // queue_free: der Aufruf kann aus dem pressed-Signal eines dieser Knöpfe kommen
            child.clone().queue_free();
            self.body.remove_child(&child);
        }
    }

    fn rebuild(&mut self) {
        self.clear();
        let (Some(fan), Some(hud)) = (self.current.clone(), self.hud.clone()) else { return };
        let method = |name: &str| Callable::from_object_method(&hud, name);

        let mut title = super::hud::label(19, GOLD);
        title.set_text(&fan.title);
        title.set_autowrap_mode(AutowrapMode::WORD_SMART);
        title.set_custom_minimum_size(Vector2::new(560.0, 0.0));
        self.body.add_child(&title);
        if !fan.detail.is_empty() {
            let mut detail = super::hud::label(14, Color::from_rgba(PARCHMENT.r, PARCHMENT.g, PARCHMENT.b, 0.8));
            detail.set_text(&fan.detail);
            detail.set_autowrap_mode(AutowrapMode::WORD_SMART);
            detail.set_custom_minimum_size(Vector2::new(560.0, 0.0));
            self.body.add_child(&detail);
        }

        let height = if fan.cards.len() > MANY { SMALL } else { BIG };
        match fan.mode {
            FanMode::Pick { .. } => {
                let all: Vec<usize> = (0..fan.cards.len()).collect();
                let selected = self.selected.clone();
                let grid = self.card_grid(&fan, &all, height, &|i| selected.contains(&i), &method);
                self.body.add_child(&grid);
            }
            FanMode::Arrange { graveyard } => {
                let rows: [(String, Vec<usize>); 2] = [
                    (tr!("Oben auf die Bibliothek (von oben nach unten)", "On top of the library (top first)").into(), self.top.clone()),
                    (
                        if graveyard {
                            tr!("In den Friedhof", "Into the graveyard").into()
                        } else {
                            tr!("Unter die Bibliothek", "On the bottom of the library").into()
                        },
                        self.bottom.clone(),
                    ),
                ];
                for (name, list) in rows {
                    let mut l = super::hud::label(15, PARCHMENT);
                    l.set_text(&format!("{name} ({})", list.len()));
                    self.body.add_child(&l);
                    let grid = self.card_grid(&fan, &list, height, &|_| true, &method);
                    self.body.add_child(&grid);
                }
            }
        }

        // Knöpfe
        let mut row = HBoxContainer::new_alloc();
        row.add_theme_constant_override("separation", 8);
        row.set_alignment(godot::classes::box_container::AlignmentMode::END);
        let mut done = super::hud::styled_button(16);
        done.set_custom_minimum_size(Vector2::new(180.0, 40.0));
        match fan.mode {
            FanMode::Pick { min, max } => {
                let n = self.selected.len();
                let range = if min == max { format!("{max}") } else { format!("{min}–{max}") };
                done.set_text(&trf!("Fertig ({n}/{range})", "Done ({n}/{range})"));
                done.set_disabled(n < min || n > max);
            }
            FanMode::Arrange { .. } => done.set_text(tr!("Fertig", "Done")),
        }
        done.connect("pressed", &method("on_fan_done"));
        row.add_child(&done);
        if fan.cancelable {
            let mut cancel = super::hud::styled_button(15);
            cancel.set_text(tr!("Abbrechen", "Cancel"));
            cancel.connect("pressed", &method("on_fan_cancel"));
            row.add_child(&cancel);
        }
        self.body.add_child(&row);
    }

    /// Raster aus Karten-Knöpfen für die Indizes `list`; `lit(i)` = goldener Rahmen.
    fn card_grid(
        &self,
        fan: &HudFan,
        list: &[usize],
        height: f32,
        lit: &dyn Fn(usize) -> bool,
        method: &dyn Fn(&str) -> Callable,
    ) -> Gd<Control> {
        let mut grid = GridContainer::new_alloc();
        grid.set_columns(list.len().clamp(1, COLUMNS) as i32);
        grid.add_theme_constant_override("h_separation", 8);
        grid.add_theme_constant_override("v_separation", 8);
        for &i in list {
            let (_, source) = &fan.cards[i];
            if !source.is_instance_valid() {
                continue;
            }
            let on = lit(i);
            let mut preview = card_preview::build_preview_sized(source, height, if on { GOLD } else { DIM_BORDER });
            preview.set_mouse_filter(MouseFilter::IGNORE);
            let mut button = Button::new_alloc();
            button.set_flat(true);
            button.set_focus_mode(godot::classes::control::FocusMode::NONE);
            button.set_custom_minimum_size(preview.get_combined_minimum_size());
            button.set_modulate(if on || matches!(fan.mode, FanMode::Arrange { .. }) {
                Color::WHITE
            } else {
                Color::from_rgba(1.0, 1.0, 1.0, 0.82)
            });
            button.add_child(&preview);
            button.connect("pressed", &method("on_fan_card").bind(&[(i as i64).to_variant()]));
            grid.add_child(&button);
        }
        // Viele Karten: scrollbar
        let rows = list.len().div_ceil(COLUMNS);
        if rows > 2 {
            let mut scroll = ScrollContainer::new_alloc();
            scroll.set_custom_minimum_size(Vector2::new(0.0, (height + 16.0) * 2.4));
            scroll.set_h_size_flags(SizeFlags::EXPAND_FILL);
            scroll.add_child(&grid);
            scroll.upcast()
        } else {
            let mut wrap = PanelContainer::new_alloc();
            wrap.add_theme_stylebox_override("panel", &godot::classes::StyleBoxEmpty::new_gd());
            wrap.add_child(&grid);
            wrap.upcast()
        }
    }
}
