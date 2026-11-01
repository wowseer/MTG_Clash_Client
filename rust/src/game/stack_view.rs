//! StackView: zeigt den Stack groß in der oberen Bildschirmhälfte.
//!
//! - Sichtbar, solange etwas auf dem Stack liegt – also bei jedem Ausspielen einer
//!   Karte (außer Ressourcen, die nicht über den Stack gehen) und in jeder
//!   Prioritätsrunde.
//! - Jedes Stack-Element ist eine große Karte. Antwortet ein Spieler, wird die neue
//!   Karte versetzt **darüber** gelegt; die darunterliegenden bleiben am Rand sichtbar.
//!   Das oberste Element (wird als Nächstes aufgelöst) hat einen goldenen Rahmen.
//! - Ein Band zeigt, wer das Element kontrolliert (Spieler/Gegner) und ob es ein
//!   Zauber oder eine Fähigkeit ist.
//! - Fängt keine Mausklicks ab.
//!
//! Wird vom [`TurnHud`](super::hud::TurnHud) jedes Frame aktualisiert; die Karten
//! werden nur neu aufgebaut, wenn sich der Stack oder die Sprache ändert.

use godot::classes::control::MouseFilter;
use godot::classes::texture_rect::{ExpandMode, StretchMode};
use godot::classes::tween::{EaseType, TransitionType};
use godot::classes::{Control, Label, PanelContainer, StyleBoxFlat, TextureRect};
use godot::global::{HorizontalAlignment, VerticalAlignment};
use godot::prelude::*;

use super::stack::StackItem;
use crate::i18n::Lang;
use crate::table::card3d::Card3D;
use crate::table::placeholder;

/// Höhe einer Stack-Karte in Pixeln (Breite folgt aus dem Kartenformat 256 × 358).
const CARD_H: f32 = 290.0;
/// Versatz jeder weiteren (darübergelegten) Karte.
const STEP: Vector2 = Vector2::new(72.0, 12.0);
const TOP_MARGIN: f32 = 12.0;

pub struct StackView {
    root: Gd<Control>,
    shown: Vec<InstanceId>,
    lang: Option<Lang>,
}

impl StackView {
    /// Erzeugt die Anzeige als Kind von `parent` (Vollbild, klickdurchlässig).
    pub fn new(parent: &mut Gd<Control>) -> Self {
        let mut root = Control::new_alloc();
        root.set_name("StackView");
        root.set_anchors_preset(godot::classes::control::LayoutPreset::FULL_RECT);
        root.set_mouse_filter(MouseFilter::IGNORE);
        parent.add_child(&root);
        Self { root, shown: Vec::new(), lang: None }
    }

    /// Mit den aktuellen Stack-Elementen (unterstes zuerst) abgleichen.
    pub fn update(&mut self, items: &[Gd<StackItem>]) {
        let ids: Vec<InstanceId> = items.iter().map(|i| i.instance_id()).collect();
        let lang = Some(crate::i18n::lang());
        if ids == self.shown && lang == self.lang {
            return;
        }
        let new_top = ids.last().copied().filter(|id| !self.shown.contains(id));
        for child in self.root.get_children().iter_shared() {
            child.free();
        }
        self.shown = ids;
        self.lang = lang;
        if items.is_empty() {
            return;
        }

        let screen = self.root.get_viewport().map_or(Vector2::new(1152.0, 648.0), |v| v.get_visible_rect().size);
        let scale = CARD_H / 358.0;
        let card_size = Vector2::new(256.0, 358.0) * scale;
        // Bei vielen Elementen den Versatz stauchen, damit alles in die Bildbreite passt
        let n = items.len();
        let max_width = screen.x * 0.8;
        let step_x = if n > 1 { STEP.x.min((max_width - card_size.x) / (n - 1) as f32).max(18.0) } else { 0.0 };
        let group_w = card_size.x + step_x * (n - 1) as f32;
        let start = Vector2::new((screen.x - group_w) * 0.5, TOP_MARGIN);

        for (i, item) in items.iter().enumerate() {
            let top = i + 1 == n;
            let pos = start + Vector2::new(step_x * i as f32, STEP.y * i as f32);
            let mut card = build_card(item, scale, top);
            card.set_position(pos);
            self.root.add_child(&card);
            if top && new_top == Some(item.instance_id()) {
                // Neue Karte gleitet von oben ein
                card.set_position(pos - Vector2::new(0.0, 40.0));
                card.set_modulate(Color::from_rgba(1.0, 1.0, 1.0, 0.0));
                let mut tween = card.create_tween();
                tween.set_parallel();
                tween.set_trans(TransitionType::CUBIC);
                tween.set_ease(EaseType::OUT);
                tween.tween_property(&card, "position", &pos.to_variant(), 0.25);
                tween.tween_property(&card, "modulate", &Color::WHITE.to_variant(), 0.2);
            }
        }
    }
}

/// Eine große Karte für ein Stack-Element: Kartenbild mit Beschriftung, Rahmen
/// (gold = oberstes Element) und Band „Spieler/Gegner · Zauber/Fähigkeit“.
fn build_card(item: &Gd<StackItem>, scale: f32, top: bool) -> Gd<PanelContainer> {
    let (source, controller, payload, description) = {
        let i = item.bind();
        (i.source.clone(), i.controller, i.payload.clone(), i.description.to_string())
    };
    let is_ability = payload.try_to::<i64>().is_ok() || payload.try_to::<GString>().is_ok_and(|p| p == "ability");

    let mut frame = PanelContainer::new_alloc();
    frame.set_mouse_filter(MouseFilter::IGNORE);
    let border = if top { Color::from_rgb(1.0, 0.82, 0.35) } else { Color::from_rgba(0.5, 0.45, 0.4, 0.9) };
    frame.add_theme_stylebox_override("panel", &frame_style(border, if top { 4 } else { 2 }));

    let size = Vector2::new(256.0, 358.0) * scale;
    let mut card = Control::new_alloc();
    card.set_custom_minimum_size(size);
    card.set_mouse_filter(MouseFilter::IGNORE);
    frame.add_child(&card);

    let Ok(source) = source.try_to::<Gd<Card3D>>() else {
        return frame;
    };
    let (texture, texts) = {
        let c = source.bind();
        (c.face_tex(), c.face_texts())
    };
    let mut tex = TextureRect::new_alloc();
    tex.set_expand_mode(ExpandMode::IGNORE_SIZE);
    tex.set_stretch_mode(StretchMode::SCALE);
    tex.set_size(size);
    tex.set_mouse_filter(MouseFilter::IGNORE);
    if let Some(t) = texture {
        tex.set_texture(&t);
    }
    card.add_child(&tex);

    let light = Color::from_rgb(0.97, 0.93, 0.85);
    let dark = Color::from_rgb(0.18, 0.12, 0.08);
    let ty = placeholder::title_center_y();
    let mut label = |text: &str, center: Vector2, box_size: Vector2, font: i32, color: Color, wrap: bool| {
        let mut l = Label::new_alloc();
        l.set_text(text);
        l.set_horizontal_alignment(HorizontalAlignment::CENTER);
        l.set_vertical_alignment(VerticalAlignment::CENTER);
        l.add_theme_font_size_override("font_size", (font as f32 * scale).round() as i32);
        l.add_theme_color_override("font_color", color);
        l.set_mouse_filter(MouseFilter::IGNORE);
        if wrap {
            l.set_autowrap_mode(godot::classes::text_server::AutowrapMode::WORD_SMART);
        }
        l.set_size(box_size * scale);
        l.set_position((center - box_size * 0.5) * scale);
        card.add_child(&l);
        l
    };
    let name_w = (texts.name_x - 26.0) * 2.0;
    label(&texts.name, Vector2::new(texts.name_x, ty), Vector2::new(name_w.max(60.0), 28.0), 17, light, false);
    if !texts.cost.is_empty() {
        label(&texts.cost, Vector2::new(texts.cost_x, ty), Vector2::new(24.0, 24.0), 15, dark, false);
    }
    label(&texts.kind, Vector2::new(128.0, 219.0), Vector2::new(216.0, 22.0), 14, light, false);
    // Bei Fähigkeiten den konkreten Effekt zeigen (Beschreibung des Stack-Elements)
    let body = if is_ability {
        description.split_once(": ").map_or(description.clone(), |(_, e)| e.to_string())
    } else {
        texts.body.clone()
    };
    label(&body, Vector2::new(128.0, 282.0), Vector2::new(206.0, 88.0), 14, dark, true);
    let mut stats = label(&texts.stats, Vector2::new(206.0, 334.0), Vector2::new(60.0, 28.0), 18, Color::WHITE, false);
    stats.add_theme_color_override("font_color", texts.stats_color);

    // Band: wer und was
    let who = if controller == 0 { tr!("Spieler", "Player") } else { tr!("Gegner", "Opponent") };
    let what = if is_ability { tr!("Fähigkeit", "Ability") } else { tr!("Zauber", "Spell") };
    let mut ribbon = PanelContainer::new_alloc();
    ribbon.set_mouse_filter(MouseFilter::IGNORE);
    let ribbon_color = if controller == 0 { Color::from_rgba(0.12, 0.45, 0.2, 0.95) } else { Color::from_rgba(0.55, 0.1, 0.1, 0.95) };
    ribbon.add_theme_stylebox_override("panel", &frame_style(ribbon_color, 0));
    let mut ribbon_text = Label::new_alloc();
    ribbon_text.set_text(&format!("{who} · {what}"));
    ribbon_text.add_theme_font_size_override("font_size", (15.0 * scale).round() as i32);
    ribbon_text.add_theme_color_override("font_color", Color::WHITE);
    ribbon_text.set_mouse_filter(MouseFilter::IGNORE);
    ribbon.add_child(&ribbon_text);
    ribbon.set_position(Vector2::new(24.0, 66.0) * scale);
    card.add_child(&ribbon);

    frame
}

fn frame_style(border: Color, width: i32) -> Gd<StyleBoxFlat> {
    let mut s = StyleBoxFlat::new_gd();
    s.set_bg_color(if width == 0 { border } else { Color::from_rgba(0.05, 0.03, 0.02, 0.9) });
    s.set_border_color(border);
    s.set_border_width_all(width);
    s.set_corner_radius_all(6);
    s.set_content_margin_all(if width == 0 { 6.0 } else { width as f32 });
    s.set_shadow_color(Color::from_rgba(0.0, 0.0, 0.0, 0.6));
    s.set_shadow_size(if width == 0 { 0 } else { 10 });
    s
}
