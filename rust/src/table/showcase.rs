//! `Showcase`: Bühne vor der Kamera für Karten, über die der Spieler entscheidet (Baustein der
//! 3D-Oberfläche; zuerst für den Mulligan).
//!
//! Aufbau im Koordinatensystem der Kamera (x = rechts, y = oben, z = zur Kamera), in festem
//! Abstand vor ihr:
//! - **Abdunkelung**: dunkle, halbtransparente Fläche hinter allem (blendet den Tisch ab),
//! - **Kartenfächer**: eine echte [`Zone`] (Art Hand, flach und breit), in die Karten von ihrem
//!   Platz auf dem Tisch hineingleiten und aus der sie wieder zurückgehen,
//! - **Titel / Untertitel / Hinweis**: `Label3D`,
//! - **Knöpfe**: [`Button3D`] in einer Reihe unter dem Fächer,
//! - **Optionen** ([`Showcase::show_options`]): eigene Karten (z. B. Kopien je Spielweise einer
//!   Karte) mit Beschriftung darüber, die die Bühne beim Schließen freigibt.
//!
//! Zeiten nach `docs/claude/ANIMATION.md`: Öffnen 0,20 s (Abdunkeln 0,15 s), Schließen 0,15 s.

use godot::classes::base_material_3d::{ShadingMode, Transparency};
use godot::classes::geometry_instance_3d::ShadowCastingSetting;
use godot::classes::label_3d::DrawFlags;
use godot::classes::{INode3D, Label3D, MeshInstance3D, QuadMesh, StandardMaterial3D};
use godot::prelude::*;

use super::button3d::Button3D;
use super::card3d::{Card3D, CARD_HEIGHT};
use super::zone::{Zone, ZoneKind};

/// Abstand der Bühne vor der Kamera (Welteinheiten).
const DISTANCE: f32 = 4.5;
/// Deckkraft der Abdunkelung und ihre Ein-/Ausblendzeit.
const DIM_ALPHA: f32 = 0.62;
const DIM_TIME: f32 = 0.15;
/// Kartenfächer: Mitte (lokal), Radius und Abstand der Karten (sehr flacher Bogen).
const FAN_CENTER: Vector3 = Vector3::new(0.0, 0.18, 0.0);
const FAN_RADIUS: f32 = 10.0;
const FAN_STEP: f32 = 0.58;
/// Knopfreihe: Höhe (lokal), Breite je Knopf und Abstand der Knopfmitten.
const BUTTON_ROW_Y: f32 = -0.72;
const BUTTON_WIDTH: f32 = 0.95;
const BUTTON_GAP: f32 = 0.92;
/// Abstand der Options-Beschriftung über der Kartenoberkante.
const CAPTION_GAP: f32 = 0.07;

/// Bühne vor der Kamera: Abdunkelung, Kartenfächer ([Zone]), Titel und [Button3D]-Knöpfe.
/// [method open] stellt sie vor die aktuelle Kamera, [method show_cards] holt Karten herein,
/// [method close] gibt sie an ihre Zonen zurück und blendet aus.
#[derive(GodotClass)]
#[class(base=Node3D)]
pub struct Showcase {
    zone: Option<Gd<Zone>>,
    dimmer: Option<Gd<StandardMaterial3D>>,
    title: Option<Gd<Label3D>>,
    subtitle: Option<Gd<Label3D>>,
    note: Option<Gd<Label3D>>,
    buttons: Vec<Gd<Button3D>>,
    /// Karten und die Zone, aus der sie kamen (für die Rückgabe).
    borrowed: Vec<(Gd<Card3D>, Gd<Zone>)>,
    /// Eigene Karten der Bühne (z. B. Kopien als Optionen); werden beim Schließen freigegeben.
    owned: Vec<Gd<Card3D>>,
    /// Zeit seit dem Öffnen bzw. Beginn des Schließens.
    t: f32,
    closing: bool,
    base: Base<Node3D>,
}

#[godot_api]
impl INode3D for Showcase {
    fn init(base: Base<Node3D>) -> Self {
        Self {
            zone: None,
            dimmer: None,
            title: None,
            subtitle: None,
            note: None,
            buttons: Vec::new(),
            borrowed: Vec::new(),
            owned: Vec::new(),
            t: 0.0,
            closing: false,
            base,
        }
    }

    fn ready(&mut self) {
        self.build();
    }

    fn process(&mut self, delta: f64) {
        self.t += delta as f32;
        let k = (self.t / DIM_TIME).min(1.0);
        let alpha = if self.closing { DIM_ALPHA * (1.0 - k) } else { DIM_ALPHA * k };
        if let Some(m) = self.dimmer.as_mut() {
            m.set_albedo(Color::from_rgba(0.02, 0.02, 0.04, alpha));
        }
        let text_alpha = if self.closing { 1.0 - k } else { k };
        for l in [&mut self.title, &mut self.subtitle, &mut self.note].into_iter().flatten() {
            let mut c = l.get_modulate();
            c.a = text_alpha;
            l.set_modulate(c);
        }
        if self.closing && k >= 1.0 {
            self.base_mut().queue_free();
        }
    }
}

#[godot_api]
impl Showcase {
    /// Stellt die Bühne vor die aktuelle Kamera (Abstand fest, parallel zur Bildebene).
    #[func]
    pub fn open(&mut self) {
        let Some(cam) = self.base().get_viewport().and_then(|v| v.get_camera_3d()) else { return };
        let xf = cam.get_global_transform();
        let forward = -xf.basis.col_c();
        let basis = xf.basis.orthonormalized();
        self.base_mut().set_global_transform(Transform3D::new(basis, xf.origin + forward * DISTANCE));
        self.t = 0.0;
        self.closing = false;
    }

    /// Titel, Untertitel und Hinweis setzen (leer = ausblenden).
    #[func]
    pub fn set_texts(&mut self, title: GString, subtitle: GString, note: GString) {
        for (label, text) in [(&mut self.title, title), (&mut self.subtitle, subtitle), (&mut self.note, note)] {
            if let Some(l) = label.as_mut() {
                l.set_text(&text);
                l.set_visible(!text.is_empty());
            }
        }
    }

    /// Karten in den Fächer holen (sie gleiten von ihrem Platz herein); beim Schließen gehen sie
    /// in ihre bisherige Zone zurück.
    pub fn show_cards(&mut self, cards: &[Gd<Card3D>]) {
        let Some(mut zone) = self.zone.clone() else { return };
        for card in cards {
            let Some(mut from) = card.bind().get_zone() else { continue };
            if from == zone {
                continue;
            }
            from.bind_mut().remove_card(card.clone());
            self.borrowed.push((card.clone(), from));
            zone.bind_mut().add_card(card.clone(), -1, 0.0);
        }
    }

    /// Eigene Karten als Optionen in den Fächer legen (z. B. Kopien einer Karte je Spielweise),
    /// je mit Beschriftung darüber (leer = keine). Die Bühne gibt sie beim Schließen frei.
    pub fn show_options(&mut self, options: Vec<(Gd<Card3D>, String)>) {
        let Some(mut zone) = self.zone.clone() else { return };
        let start = zone.get_global_transform();
        for (mut card, caption) in options {
            // in den Baum (unter die Bühne), aus der Fächermitte heraus
            self.base_mut().add_child(&card);
            card.set_global_transform(start);
            zone.bind_mut().add_card(card.clone(), -1, 0.0);
            if !caption.is_empty() {
                let mut l = Label3D::new_alloc();
                l.set_text(&caption);
                l.set_font_size(64);
                l.set_pixel_size(0.0016);
                l.set_outline_size(12);
                l.set_modulate(Color::from_rgb(0.93, 0.95, 1.0));
                l.set_outline_modulate(Color::from_rgba(0.02, 0.03, 0.08, 0.9));
                l.set_draw_flag(DrawFlags::SHADED, false);
                l.set_draw_flag(DrawFlags::DOUBLE_SIDED, false);
                // flach auf der Kartenebene, über der Oberkante (Karte: Vorderseite +Y, oben −Z)
                l.set_transform(Transform3D::new(
                    Basis::from_axis_angle(Vector3::RIGHT, -std::f32::consts::FRAC_PI_2),
                    Vector3::new(0.0, 0.01, -CARD_HEIGHT * 0.5 - CAPTION_GAP),
                ));
                card.add_child(&l);
            }
            self.owned.push(card);
        }
    }

    /// Knopf in die Knopfreihe unter dem Fächer setzen (Reihenfolge = links nach rechts).
    pub fn add_button(&mut self, button: Gd<Button3D>) {
        self.base_mut().add_child(&button);
        self.buttons.push(button.clone());
        let n = self.buttons.len() as f32;
        for (i, b) in self.buttons.iter_mut().enumerate() {
            let x = (i as f32 - (n - 1.0) * 0.5) * BUTTON_GAP;
            b.set_position(Vector3::new(x, BUTTON_ROW_Y, 0.05));
        }
    }

    /// Breite eines Knopfes in dieser Bühne (Welteinheiten).
    #[func]
    pub fn button_width() -> f32 {
        BUTTON_WIDTH
    }

    /// Liegt `card` gerade in der Bühne?
    #[func]
    pub fn holds(&self, card: Gd<Card3D>) -> bool {
        // gelöschte Karten nie vergleichen (der Vergleich bräuchte ihre Instanz-ID)
        card.is_instance_valid() && self.borrowed.iter().any(|(c, _)| c.is_instance_valid() && *c == card)
    }

    /// Karten zurückgeben, Knöpfe ausblenden, abdunkeln aufheben und danach freigeben.
    #[func]
    pub fn close(&mut self) {
        if self.closing {
            return;
        }
        if let Some(mut zone) = self.zone.clone() {
            for (card, mut to) in self.borrowed.drain(..) {
                if card.is_instance_valid() && to.is_instance_valid() && card.bind().get_zone().as_ref() == Some(&zone) {
                    zone.bind_mut().remove_card(card.clone());
                    to.bind_mut().add_card(card, -1, 0.0);
                }
            }
        }
        for mut b in self.buttons.drain(..) {
            if b.is_instance_valid() {
                b.bind_mut().dismiss();
            }
        }
        for card in self.owned.drain(..) {
            if card.is_instance_valid() {
                let mut card = card;
                card.queue_free();
            }
        }
        self.closing = true;
        self.t = 0.0;
    }
}

impl Showcase {
    fn build(&mut self) {
        // Abdunkelung hinter allem
        let mut mat = StandardMaterial3D::new_gd();
        mat.set_shading_mode(ShadingMode::UNSHADED);
        mat.set_transparency(Transparency::ALPHA);
        mat.set_albedo(Color::from_rgba(0.02, 0.02, 0.04, 0.0));
        mat.set_render_priority(-1);
        let mut quad = QuadMesh::new_gd();
        quad.set_size(Vector2::new(12.0, 7.0));
        quad.set_material(&mat);
        let mut dim = MeshInstance3D::new_alloc();
        dim.set_name("Dimmer");
        dim.set_mesh(&quad);
        dim.set_cast_shadows_setting(ShadowCastingSetting::OFF);
        dim.set_position(Vector3::new(0.0, 0.0, -0.6));
        self.base_mut().add_child(&dim);
        self.dimmer = Some(mat);

        // Kartenfächer: Zone (Hand) – Karten liegen in ihrer Ebene, Vorderseite zur Kamera
        let mut zone = Zone::new_alloc();
        zone.set_name("Fan");
        for (prop, value) in [
            ("kind", ZoneKind::Hand.to_variant()),
            ("size", Vector3::new(4.4, 0.1, 1.2).to_variant()),
            ("accepts_drops", false.to_variant()),
            ("cards_draggable", false.to_variant()),
            ("face_up", true.to_variant()),
            ("hand_radius", FAN_RADIUS.to_variant()),
            ("hand_card_angle_degrees", (FAN_STEP / FAN_RADIUS).to_degrees().to_variant()),
            ("hand_max_spread_degrees", 40.0f32.to_variant()),
        ] {
            zone.set(prop, &value);
        }
        // Zone-Ebene: lokal y → zur Kamera, z → nach unten
        zone.set_transform(Transform3D::new(
            Basis::from_axis_angle(Vector3::RIGHT, std::f32::consts::FRAC_PI_2),
            FAN_CENTER,
        ));
        self.base_mut().add_child(&zone);
        self.zone = Some(zone);

        self.title = Some(self.label(0.92, 96, 0.0019, Color::from_rgb(0.98, 0.86, 0.55)));
        self.subtitle = Some(self.label(0.76, 64, 0.0013, Color::from_rgb(0.9, 0.86, 0.78)));
        self.note = Some(self.label(-0.93, 56, 0.0012, Color::from_rgb(0.9, 0.86, 0.78)));
    }

    fn label(&mut self, y: f32, font: i32, pixel: f32, color: Color) -> Gd<Label3D> {
        let mut l = Label3D::new_alloc();
        l.set_font_size(font);
        l.set_pixel_size(pixel);
        l.set_outline_size(font / 6);
        l.set_modulate(Color::from_rgba(color.r, color.g, color.b, 0.0));
        l.set_outline_modulate(Color::from_rgba(0.03, 0.02, 0.02, 0.85));
        l.set_draw_flag(DrawFlags::SHADED, false);
        l.set_position(Vector3::new(0.0, y, 0.05));
        l.set_visible(false);
        self.base_mut().add_child(&l);
        l
    }
}
