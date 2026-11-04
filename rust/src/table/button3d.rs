//! `Button3D`: anklickbarer Knopf in der 3D-Szene (Baustein „Bedienobjekt“ der 3D-Oberfläche).
//!
//! Aufbau: zwei Flächen aus gerenderten Bildern (Knopf vorne, Leuchten dahinter, beide mit
//! Transparenz und unbeleuchtet, damit sie wie in Blender aussehen), darüber die Beschriftung als
//! `Label3D` und eine Kollisionsbox auf dem Physik-Layer [`CONTROL_LAYER`]. Den Klick erkennt der
//! [`TableController`](super::table_controller::TableController) per Raycast und ruft
//! [`Button3D::activate`] auf.
//!
//! Verhalten (Zeiten nach `docs/claude/ANIMATION.md`): Erscheinen 0,20 s Cubic-Out (wächst aus der
//! Mitte), Hover weich in ca. 0,12 s (Knopf etwas größer, Leuchten heller), Leuchten pulsiert
//! leicht, Klick drückt den Knopf kurz ein, Verschwinden 0,15 s.

use std::f32::consts::TAU;

use godot::classes::base_material_3d::{ShadingMode, TextureFilter, TextureParam, Transparency};
use godot::classes::geometry_instance_3d::ShadowCastingSetting;
use godot::classes::label_3d::DrawFlags;
use godot::classes::{
    Area3D, BoxShape3D, CollisionShape3D, IArea3D, Label3D, MeshInstance3D, QuadMesh, StandardMaterial3D, Texture2D,
};
use godot::prelude::*;

use super::layers::CONTROL_LAYER;

/// Seitenverhältnis der gerenderten Knopfbilder (2144 × 600).
const IMAGE_ASPECT: f32 = 2144.0 / 600.0;
/// Anteil des Knopfes an der Bildfläche (Rest ist Leuchten): Breite, Höhe.
const FACE_FRACTION: Vector2 = Vector2::new(0.838, 0.634);
/// Erscheinen, Verschwinden, Hover-Glättung, Klick-Eindrücken (Sekunden).
const APPEAR_TIME: f32 = 0.20;
const DISMISS_TIME: f32 = 0.15;
const HOVER_SPEED: f32 = 25.0;
const PRESS_TIME: f32 = 0.12;

/// Anklickbarer Knopf in der 3D-Szene: Knopfbild ([member face_texture]) mit Leuchten dahinter
/// ([member glow_texture]) und Beschriftung ([member text]). Wird vom [TableController] per
/// Raycast angeklickt (Kollisions-Layer 3) und meldet das mit [signal pressed]. Richtet sich
/// auf Wunsch zur Kamera aus ([member face_camera]).
#[derive(GodotClass)]
#[class(base=Area3D)]
pub struct Button3D {
    /// Bild des Knopfes (transparenter Hintergrund).
    #[export]
    face_texture: Option<Gd<Texture2D>>,
    /// Bild des Leuchtens hinter dem Knopf (gleicher Bildausschnitt wie der Knopf).
    #[export]
    glow_texture: Option<Gd<Texture2D>>,
    /// Beschriftung auf dem Knopf.
    #[export]
    text: GString,
    /// Breite der Bildfläche in Welteinheiten (Höhe folgt aus dem Seitenverhältnis).
    #[export]
    width: f32,
    /// Knopf immer parallel zur Bildebene der Kamera ausrichten.
    #[export]
    face_camera: bool,
    /// Anklickbar? (Ausgegraut, wenn nicht.)
    #[export]
    enabled: bool,

    face: Option<Gd<MeshInstance3D>>,
    glow: Option<Gd<MeshInstance3D>>,
    glow_material: Option<Gd<StandardMaterial3D>>,
    label: Option<Gd<Label3D>>,
    visual: Option<Gd<Node3D>>,
    hovered: bool,
    /// 0 = nicht gehovert, 1 = gehovert (weich nachgeführt).
    hover_amount: f32,
    time: f32,
    /// Zeit seit dem Erscheinen bzw. seit Beginn des Verschwindens.
    appear: f32,
    dismissing: Option<f32>,
    press: Option<f32>,

    base: Base<Area3D>,
}

#[godot_api]
impl IArea3D for Button3D {
    fn init(base: Base<Area3D>) -> Self {
        Self {
            face_texture: None,
            glow_texture: None,
            text: GString::new(),
            width: 2.4,
            face_camera: true,
            enabled: true,
            face: None,
            glow: None,
            glow_material: None,
            label: None,
            visual: None,
            hovered: false,
            hover_amount: 0.0,
            time: 0.0,
            appear: 0.0,
            dismissing: None,
            press: None,
            base,
        }
    }

    fn ready(&mut self) {
        self.build();
    }

    fn process(&mut self, delta: f64) {
        let dt = delta as f32;
        self.time += dt;
        self.appear += dt;
        if self.face_camera {
            if let Some(cam) = self.base().get_viewport().and_then(|v| v.get_camera_3d()) {
                let basis = cam.get_global_transform().basis;
                let mut xf = self.base().get_global_transform();
                xf.basis = basis.orthonormalized().scaled(xf.basis.get_scale());
                self.base_mut().set_global_transform(xf);
            }
        }
        let target = if self.hovered && self.enabled { 1.0 } else { 0.0 };
        self.hover_amount += (target - self.hover_amount) * (1.0 - (-HOVER_SPEED * dt).exp());

        // Größe: Erscheinen (Cubic-Out), Hover, Klick, Verschwinden
        let p = (self.appear / APPEAR_TIME).min(1.0);
        let mut scale = 1.0 - (1.0 - p).powi(3);
        scale *= 1.0 + 0.05 * self.hover_amount;
        if let Some(t) = self.press.as_mut() {
            *t += dt;
            let k = (*t / PRESS_TIME).min(1.0);
            scale *= 1.0 - 0.07 * (k * std::f32::consts::PI).sin();
            if k >= 1.0 {
                self.press = None;
            }
        }
        if let Some(t) = self.dismissing.as_mut() {
            *t += dt;
            let k = (*t / DISMISS_TIME).min(1.0);
            scale *= 1.0 - k;
            if k >= 1.0 {
                self.base_mut().queue_free();
                return;
            }
        }
        if let Some(v) = self.visual.as_mut() {
            v.set_scale(Vector3::splat(scale.max(0.001)));
        }
        // Leuchten: pulsiert leicht, beim Hover heller und größer
        let pulse = 0.5 + 0.5 * (self.time * TAU * 0.6).sin();
        let alpha = if self.enabled { 0.55 + 0.15 * pulse + 0.30 * self.hover_amount } else { 0.15 };
        if let Some(m) = self.glow_material.as_mut() {
            m.set_albedo(Color::from_rgba(1.0, 1.0, 1.0, alpha));
        }
        if let Some(g) = self.glow.as_mut() {
            g.set_scale(Vector3::splat(1.0 + 0.06 * self.hover_amount + 0.02 * pulse));
        }
    }
}

#[godot_api]
impl Button3D {
    /// Der Knopf wurde angeklickt.
    #[signal]
    fn pressed();

    /// Maus über dem Knopf (vom [TableController] gesetzt): Knopf wächst, Leuchten wird heller.
    #[func]
    pub fn set_hovered(&mut self, hovered: bool) {
        self.hovered = hovered;
    }

    /// Ist die Maus über dem Knopf?
    #[func]
    pub fn is_hovered(&self) -> bool {
        self.hovered
    }

    /// Klick auslösen (vom [TableController] bzw. von Skripten): kurzes Eindrücken und
    /// [signal pressed]. Ohne Wirkung, wenn der Knopf nicht [member enabled] ist oder gerade
    /// verschwindet.
    #[func]
    pub fn activate(&mut self) {
        if !self.enabled || self.dismissing.is_some() {
            return;
        }
        self.press = Some(0.0);
        self.signals().pressed().emit();
    }

    /// Knopf ausblenden (0,15 s) und danach freigeben; nicht mehr anklickbar.
    #[func]
    pub fn dismiss(&mut self) {
        if self.dismissing.is_none() {
            self.dismissing = Some(0.0);
            self.base_mut().set_collision_layer(0);
        }
    }

    /// Beschriftung ändern.
    #[func]
    pub fn set_label_text(&mut self, text: GString) {
        self.text = text.clone();
        if let Some(l) = self.label.as_mut() {
            l.set_text(&text);
        }
        self.fit_label();
    }
}

impl Button3D {
    /// Neuer Knopf mit Bildern und Beschriftung (noch nicht in der Szene).
    pub fn create(face: Option<Gd<Texture2D>>, glow: Option<Gd<Texture2D>>, text: &str, width: f32) -> Gd<Button3D> {
        let mut b = Button3D::new_alloc();
        {
            let mut m = b.bind_mut();
            m.face_texture = face;
            m.glow_texture = glow;
            m.text = text.into();
            m.width = width;
        }
        b
    }

    fn build(&mut self) {
        let height = self.width / IMAGE_ASPECT;
        self.base_mut().set_collision_layer(CONTROL_LAYER);
        self.base_mut().set_collision_mask(0);
        self.base_mut().set_monitoring(false);
        self.base_mut().set_monitorable(false);

        // Trefferfläche: nur der Knopf, nicht das Leuchten
        let mut shape = BoxShape3D::new_gd();
        shape.set_size(Vector3::new(self.width * FACE_FRACTION.x, height * FACE_FRACTION.y, 0.05));
        let mut collision = CollisionShape3D::new_alloc();
        collision.set_shape(&shape);
        self.base_mut().add_child(&collision);

        let mut visual = Node3D::new_alloc();
        visual.set_name("Visual");
        let size = Vector2::new(self.width, height);
        let (glow, glow_mat) = quad("Glow", self.glow_texture.as_ref(), size, -0.01, 0);
        let (face, _) = quad("Face", self.face_texture.as_ref(), size, 0.0, 1);
        visual.add_child(&glow);
        visual.add_child(&face);

        let mut label = Label3D::new_alloc();
        label.set_name("Label");
        label.set_text(&self.text);
        label.set_font_size(96);
        label.set_outline_size(18);
        label.set_modulate(Color::from_rgb(1.0, 0.98, 0.94));
        label.set_outline_modulate(Color::from_rgba(0.12, 0.05, 0.02, 0.85));
        label.set_draw_flag(DrawFlags::SHADED, false);
        label.set_draw_flag(DrawFlags::DOUBLE_SIDED, false);
        label.set_render_priority(2);
        label.set_outline_render_priority(1);
        label.set_cast_shadows_setting(ShadowCastingSetting::OFF);
        label.set_position(Vector3::new(0.0, 0.0, 0.01));
        visual.add_child(&label);

        self.base_mut().add_child(&visual);
        self.face = Some(face);
        self.glow = Some(glow);
        self.glow_material = Some(glow_mat);
        self.label = Some(label);
        self.visual = Some(visual);
        self.fit_label();
    }

    /// Schriftgröße so wählen, dass der Text auf den Knopf passt (Höhe ca. 30 % des Knopfes).
    fn fit_label(&mut self) {
        let height = self.width / IMAGE_ASPECT;
        let text_height = height * FACE_FRACTION.y * 0.42;
        let max_width = self.width * FACE_FRACTION.x * 0.86;
        let chars = self.text.to_string().chars().count().max(1) as f32;
        // Breite eines Zeichens ca. 0,55 Schriftgrößen
        let size = text_height.min(max_width / (chars * 0.55));
        if let Some(l) = self.label.as_mut() {
            l.set_pixel_size(size / 96.0);
        }
    }
}

/// Unbeleuchtete, transparente Bildfläche (Vorderseite +Z).
fn quad(
    name: &str,
    texture: Option<&Gd<Texture2D>>,
    size: Vector2,
    z: f32,
    priority: i32,
) -> (Gd<MeshInstance3D>, Gd<StandardMaterial3D>) {
    let mut mat = StandardMaterial3D::new_gd();
    mat.set_shading_mode(ShadingMode::UNSHADED);
    mat.set_transparency(Transparency::ALPHA);
    mat.set_texture_filter(TextureFilter::LINEAR_WITH_MIPMAPS);
    mat.set_render_priority(priority);
    if let Some(t) = texture {
        mat.set_texture(TextureParam::ALBEDO, t);
    }
    let mut mesh = QuadMesh::new_gd();
    mesh.set_size(size);
    mesh.set_material(&mat);
    let mut node = MeshInstance3D::new_alloc();
    node.set_name(name);
    node.set_mesh(&mesh);
    node.set_cast_shadows_setting(ShadowCastingSetting::OFF);
    node.set_position(Vector3::new(0.0, 0.0, z));
    (node, mat)
}
