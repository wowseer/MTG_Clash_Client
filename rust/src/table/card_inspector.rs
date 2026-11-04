//! `CardInspector`: Kartenansicht beim Hovern (wie `Fähigkeiten.png` des Users).
//!
//! Statt dass die Karte selbst zur Bildmitte fliegt, zeigt der Inspektor eine **Kopie** der
//! Karte groß, hell und lesbar an der Bildschirmseite gegenüber der Maus, daneben eine dunkle
//! Tafel mit
//! - den **Fähigkeiten** der Karte (Schlüsselwörter mit Name und Erklärung, z. B. „Flying – A
//!   creature with flying can't be blocked …“, von der Engine), und
//! - den **Buffs** (Marken, veränderte Stärke/Widerstand, Schaden, angelegte Karten …).
//!
//! Der Inspektor steht in festem Abstand vor der Kamera (wie die angehefteten 3D-Knöpfe) und folgt
//! ihr jedes Frame. Er ist nicht anklickbar (die Kopie liegt auf keinem Kollisions-Layer).

use godot::classes::base_material_3d::{ShadingMode, Transparency};
use godot::classes::geometry_instance_3d::ShadowCastingSetting;
use godot::classes::label_3d::DrawFlags;
use godot::classes::text_server::AutowrapMode;
use godot::classes::{INode3D, Label3D, MeshInstance3D, QuadMesh, StandardMaterial3D};
use godot::global::HorizontalAlignment;
use godot::prelude::*;

use super::card3d::{Card3D, CARD_HEIGHT, CARD_WIDTH};
use super::placeholder;
use crate::game::cards::CardDef;

/// Abstand vor der Kamera und Größe der Kartenkopie (Höhe in Welteinheiten).
const DEPTH: f32 = 3.0;
const CARD_SHOWN_HEIGHT: f32 = 1.0;
/// Rand zum Bildschirmrand, Abstand Karte–Tafel, Breite der Tafel.
const EDGE: f32 = 0.08;
const GAP: f32 = 0.04;
const PANEL_WIDTH: f32 = 0.82;
/// Schrift: Pixelgröße der Label3D, Größe für Titel und Text, Zeilenhöhen (Welteinheiten).
const PIXEL: f32 = 0.0009;
const TITLE_SIZE: i32 = 34;
const TEXT_SIZE: i32 = 26;
const PAD: f32 = 0.04;

/// Ein Eintrag der Tafel: Überschrift (z. B. „Flying“) und Erklärung (darf leer sein).
#[derive(Clone, Debug, Default)]
pub struct InspectorEntry {
    pub title: String,
    pub text: String,
}

/// Kartenansicht beim Hovern: große, helle Kopie der Karte am Bildschirmrand und daneben eine
/// Tafel mit Fähigkeiten und Buffs. [method hide] blendet sie aus.
#[derive(GodotClass)]
#[class(base=Node3D)]
pub struct CardInspector {
    /// Seite: true = Karte am rechten Bildschirmrand (Tafel links davon), false = links.
    right_side: bool,
    content: Option<Gd<Node3D>>,
    base: Base<Node3D>,
}

#[godot_api]
impl INode3D for CardInspector {
    fn init(base: Base<Node3D>) -> Self {
        Self { right_side: false, content: None, base }
    }

    fn process(&mut self, _delta: f64) {
        if self.content.is_none() {
            return;
        }
        let Some(viewport) = self.base().get_viewport() else { return };
        let Some(cam) = viewport.get_camera_3d() else { return };
        let xf = cam.get_global_transform();
        let basis = xf.basis.orthonormalized();
        let forward = -basis.col_c();
        self.base_mut().set_global_transform(Transform3D::new(basis, xf.origin + forward * DEPTH));
        // Inhalt an den Bildschirmrand: halbe sichtbare Breite in dieser Tiefe
        let size = viewport.get_visible_rect().size;
        let half_h = DEPTH * (cam.get_fov().to_radians() * 0.5).tan();
        let half_w = half_h * size.x / size.y.max(1.0);
        let card_w = CARD_WIDTH * CARD_SHOWN_HEIGHT / CARD_HEIGHT;
        let x = if self.right_side { half_w - EDGE - card_w * 0.5 } else { -half_w + EDGE + card_w * 0.5 };
        if let Some(c) = self.content.as_mut() {
            c.set_position(Vector3::new(x, 0.0, 0.0));
        }
    }
}

#[godot_api]
impl CardInspector {
    /// Ansicht ausblenden.
    #[func]
    pub fn hide_card(&mut self) {
        if let Some(mut c) = self.content.take() {
            c.queue_free();
        }
    }

    /// Wird gerade eine Karte gezeigt?
    #[func]
    pub fn is_showing(&self) -> bool {
        self.content.is_some()
    }
}

impl CardInspector {
    /// Karte `def` zeigen; `right_side` = am rechten Bildschirmrand (sonst links); Tafel mit
    /// `abilities` (Fähigkeiten) und `buffs` (veränderte Werte) – leere Listen = keine Tafel.
    pub fn show(&mut self, def: &CardDef, abilities: &[InspectorEntry], buffs: &[InspectorEntry], right_side: bool) {
        self.hide_card();
        self.right_side = right_side;
        let mut content = Node3D::new_alloc();
        content.set_name("Content");

        // Kartenkopie: hell, stehend zur Kamera (Vorderseite der Karte +Y → zur Kamera +Z)
        let mut copy = Card3D::new_alloc();
        {
            let mut c = copy.bind_mut();
            c.set_face_texture(Some(placeholder::card_face(def)));
            c.apply_def(def);
        }
        let scale = CARD_SHOWN_HEIGHT / CARD_HEIGHT;
        copy.set_transform(Transform3D::new(
            Basis::from_axis_angle(Vector3::RIGHT, std::f32::consts::FRAC_PI_2).scaled(Vector3::splat(scale)),
            Vector3::ZERO,
        ));
        copy.bind_mut().set_bright(true);
        content.add_child(&copy);

        // Tafel neben der Karte (zur Bildmitte hin)
        if !abilities.is_empty() || !buffs.is_empty() {
            let card_w = CARD_WIDTH * scale;
            let dir = if right_side { -1.0 } else { 1.0 };
            let panel_x = dir * (card_w * 0.5 + GAP + PANEL_WIDTH * 0.5);
            let mut lines: Vec<(String, bool)> = Vec::new();
            for a in abilities {
                lines.push((a.title.clone(), true));
                if !a.text.is_empty() {
                    lines.push((a.text.clone(), false));
                }
            }
            if !buffs.is_empty() {
                lines.push((crate::tr!("Veränderungen", "Changes").to_string(), true));
                for b in buffs {
                    lines.push((if b.text.is_empty() { b.title.clone() } else { format!("{}: {}", b.title, b.text) }, false));
                }
            }
            let mut panel = Node3D::new_alloc();
            panel.set_position(Vector3::new(panel_x, 0.0, 0.0));
            let mut y = CARD_SHOWN_HEIGHT * 0.5 - PAD;
            let inner = PANEL_WIDTH - 2.0 * PAD;
            for (text, title) in lines {
                let mut l = Label3D::new_alloc();
                l.set_text(&text);
                l.set_font_size(if title { TITLE_SIZE } else { TEXT_SIZE });
                l.set_pixel_size(PIXEL);
                l.set_outline_size(0);
                l.set_modulate(if title { Color::from_rgb(1.0, 0.92, 0.7) } else { Color::from_rgb(0.86, 0.87, 0.9) });
                l.set_draw_flag(DrawFlags::SHADED, false);
                l.set_draw_flag(DrawFlags::DOUBLE_SIDED, false);
                l.set_horizontal_alignment(HorizontalAlignment::LEFT);
                l.set_vertical_alignment(godot::global::VerticalAlignment::TOP);
                l.set_autowrap_mode(AutowrapMode::WORD_SMART);
                l.set_width(inner / PIXEL);
                l.set_render_priority(3);
                // Höhe grob aus der Textlänge (Zeichen je Zeile ≈ Breite / 0,55 Schriftgrößen)
                let size = if title { TITLE_SIZE } else { TEXT_SIZE } as f32;
                let per_line = ((inner / PIXEL) / (size * 0.52)).max(8.0);
                let rows = (text.chars().count() as f32 / per_line).ceil().max(1.0);
                let height = rows * size * 1.2 * PIXEL;
                l.set_position(Vector3::new(-PANEL_WIDTH * 0.5 + PAD + inner * 0.5, y - height * 0.5, 0.002));
                panel.add_child(&l);
                y -= height + if title { 0.005 } else { 0.03 };
            }
            // dunkler Hintergrund passend zur Textmenge
            let used = (CARD_SHOWN_HEIGHT * 0.5 - y + PAD).max(0.2);
            let mut mat = StandardMaterial3D::new_gd();
            mat.set_shading_mode(ShadingMode::UNSHADED);
            mat.set_transparency(Transparency::ALPHA);
            mat.set_albedo(Color::from_rgba(0.05, 0.05, 0.07, 0.86));
            mat.set_render_priority(2);
            let mut quad = QuadMesh::new_gd();
            quad.set_size(Vector2::new(PANEL_WIDTH, used));
            quad.set_material(&mat);
            let mut bg = MeshInstance3D::new_alloc();
            bg.set_mesh(&quad);
            bg.set_cast_shadows_setting(ShadowCastingSetting::OFF);
            bg.set_position(Vector3::new(0.0, CARD_SHOWN_HEIGHT * 0.5 - used * 0.5, -0.002));
            panel.add_child(&bg);
            content.add_child(&panel);
        }
        self.base_mut().add_child(&content);
        // erst jetzt im Baum (Kollision wird beim Aufbau gesetzt): nicht anklickbar machen
        copy.set_collision_layer(0);
        self.content = Some(content);
    }
}
