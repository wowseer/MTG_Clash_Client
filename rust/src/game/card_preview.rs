//! Große Kartenvorschau für das HUD (z. B. oben in der Bildmitte, während eine Karte aus der
//! Hand gezogen wird).

use godot::classes::control::MouseFilter;
use godot::classes::texture_rect::{ExpandMode, StretchMode};
use godot::classes::{Control, Label, PanelContainer, StyleBoxFlat, TextureRect};
use godot::global::{HorizontalAlignment, VerticalAlignment};
use godot::prelude::*;

use crate::game::card_text;
use crate::table::card3d::Card3D;
use crate::table::placeholder;

/// Höhe der Vorschaukarte in Pixeln (Breite folgt aus dem Kartenformat 256 × 358).
pub const CARD_H: f32 = 290.0;

/// Füllt `card` (Control in Kartengröße 256 × 358 · `scale`) mit Kartenbild und Beschriftung von
/// `source`.
fn fill_face(card: &mut Gd<Control>, source: &Gd<Card3D>, scale: f32) {
    let size = Vector2::new(256.0, 358.0) * scale;
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
    // Schriftgrößen an die Textlänge anpassen (Maße in Pixeln der Kartentextur)
    let name_w = texts.name_width.max(60.0);
    let name_font = card_text::fit_line(&texts.name, name_w, 9, 17);
    label(&texts.name, Vector2::new(texts.name_x, ty), Vector2::new(name_w, 28.0), name_font, light, false);
    if !texts.cost.is_empty() {
        label(&texts.cost, Vector2::new(texts.cost_x, ty), Vector2::new(24.0, 24.0), 15, dark, false);
    }
    label(&texts.kind, Vector2::new(128.0, 219.0), Vector2::new(216.0, 22.0), card_text::fit_line(&texts.kind, 216.0, 8, 14), light, false);
    let body_font = card_text::fit_font(&texts.body, 206.0 * 72.0, 7, 14);
    label(&texts.body, Vector2::new(128.0, 276.0), Vector2::new(206.0, 72.0), body_font, dark, true);
    let mut stats = label(&texts.stats, Vector2::new(206.0, 334.0), Vector2::new(60.0, 28.0), 18, Color::WHITE, false);
    stats.add_theme_color_override("font_color", texts.stats_color);
}

/// Große Vorschau einer Karte (goldener Rahmen, klickdurchlässig), z. B. im oberen Bildbereich,
/// während die Karte aus der Hand gezogen wird. Höhe der Karte = [`CARD_H`] Pixel.
pub fn build_preview(source: &Gd<Card3D>) -> Gd<PanelContainer> {
    build_preview_sized(source, CARD_H, Color::from_rgb(1.0, 0.82, 0.35))
}

/// Wie [`build_preview`], aber mit Kartenhöhe `height` (Pixel) und Rahmenfarbe `border`
/// (z. B. für den Kartenfächer).
pub fn build_preview_sized(source: &Gd<Card3D>, height: f32, border: Color) -> Gd<PanelContainer> {
    let scale = height / 358.0;
    let mut frame = PanelContainer::new_alloc();
    frame.set_mouse_filter(MouseFilter::IGNORE);
    frame.add_theme_stylebox_override("panel", &frame_style(border, 3));
    let mut card = Control::new_alloc();
    card.set_custom_minimum_size(Vector2::new(256.0, 358.0) * scale);
    card.set_mouse_filter(MouseFilter::IGNORE);
    frame.add_child(&card);
    fill_face(&mut card, source, scale);
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
