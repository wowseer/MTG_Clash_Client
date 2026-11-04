//! `Button3D`: anklickbarer Knopf in der 3D-Szene (Baustein „Bedienobjekt“ der 3D-Oberfläche).
//!
//! Aufbau: eine Fläche mit dem Knopf-Shader ([`BUTTON_SHADER`]: Nachbau der Blender-Materialien
//! aus `Assets/Buttons`, Feuer bzw. Wolken bewegen sich wie in Blender, das Leuchten pulsiert),
//! darüber die Beschriftung als `Label3D` und eine Kollisionsbox auf dem Physik-Layer
//! [`CONTROL_LAYER`]. Den Klick erkennt der
//! [`TableController`](super::table_controller::TableController) per Raycast und ruft
//! [`Button3D::activate`] auf.
//!
//! Verhalten (Zeiten nach `docs/claude/ANIMATION.md`): Erscheinen 0,20 s Cubic-Out (wächst aus der
//! Mitte), Hover weich in ca. 0,12 s (Knopf etwas größer, Leuchten heller), Klick drückt den
//! Knopf kurz ein, Verschwinden 0,15 s. Die Oberfläche läuft in einer 4-s-Schleife (Treiber der
//! Blender-Datei), jeder Knopf mit eigenem Zeitversatz.

use godot::classes::geometry_instance_3d::ShadowCastingSetting;
use godot::classes::label_3d::DrawFlags;
use godot::classes::{
    Area3D, BoxShape3D, CollisionShape3D, IArea3D, Label3D, MeshInstance3D, QuadMesh, Shader, ShaderMaterial,
};
use godot::prelude::*;

use super::button_shader::BUTTON_SHADER;
use super::layers::CONTROL_LAYER;

/// Farbstil eines Knopfes (Farben aus den Blender-Dateien): 0 = Orange (Feuer, „Button_Ornage“),
/// 1 = Blau (Wolken, „Button_Blau“).
#[derive(GodotConvert, Var, Export, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[godot(via = i64)]
pub enum ButtonStyle {
    #[default]
    Orange,
    Blue,
}

/// Shader-Farben eines Stils (linear, aus den Blender-Materialien).
struct Palette {
    ramp: [Color; 4],
    ramp_pos: [f32; 4],
    streak: Color,
    shade: Color,
    rim: Color,
    glow_inner: Color,
    glow_outer: Color,
}

fn palette(style: ButtonStyle) -> Palette {
    let c = Color::from_rgb;
    match style {
        ButtonStyle::Orange => Palette {
            ramp: [c(1.0, 0.402, 0.0802), c(0.956, 0.1559, 0.0168), c(0.8714, 0.0908, 0.008), c(0.956, 0.1714, 0.0262)],
            ramp_pos: [0.0, 0.18, 0.7, 1.0],
            streak: c(1.0, 0.4287, 0.1144),
            shade: c(0.6795, 0.0382, 0.0048),
            rim: c(1.0, 0.855, 0.4287),
            glow_inner: c(0.305, 0.013, 0.1878),
            glow_outer: c(0.8308, 0.0382, 0.0908),
        },
        ButtonStyle::Blue => Palette {
            ramp: [c(0.3763, 0.6105, 1.0), c(0.1714, 0.2623, 1.0), c(0.0802, 0.1144, 1.0), c(0.1559, 0.2051, 0.956)],
            ramp_pos: [0.0, 0.2, 0.7, 1.0],
            streak: c(0.6795, 0.7913, 1.0),
            shade: c(0.0612, 0.0802, 0.8714),
            rim: c(0.7529, 0.9131, 1.0),
            glow_inner: c(0.0612, 0.013, 0.5776),
            glow_outer: c(0.1559, 0.0612, 1.0),
        },
    }
}

/// Seitenverhältnis der gerenderten Knopfbilder (2144 × 600).
const IMAGE_ASPECT: f32 = 2144.0 / 600.0;
/// Anteil des Knopfes an der Bildfläche (Rest ist Leuchten): Breite, Höhe.
const FACE_FRACTION: Vector2 = Vector2::new(0.838, 0.634);
/// Erscheinen, Verschwinden, Hover-Glättung, Klick-Eindrücken (Sekunden).
const APPEAR_TIME: f32 = 0.20;
const DISMISS_TIME: f32 = 0.15;
const HOVER_SPEED: f32 = 25.0;
const PRESS_TIME: f32 = 0.12;
/// Abstand vor der Kamera für Knöpfe, die an einer Bildschirmstelle stehen (vor allen Karten).
const SCREEN_DEPTH: f32 = 1.2;

/// Bildschirmstelle eines angehefteten Knopfes: Mitte = `anchor` × Fenstergröße + `offset`
/// (Pixel), Breite der Bildfläche in Pixeln.
#[derive(Clone, Copy, Debug)]
struct ScreenPin {
    anchor: Vector2,
    offset: Vector2,
    width_px: f32,
}

/// Anklickbarer Knopf in der 3D-Szene: bewegte Oberfläche und Leuchten im Farbstil
/// [member style] (Nachbau der Blender-Materialien) mit Beschriftung ([member text]). Wird vom [TableController] per
/// Raycast angeklickt (Kollisions-Layer 3) und meldet das mit [signal pressed]. Richtet sich
/// auf Wunsch zur Kamera aus ([member face_camera]).
#[derive(GodotClass)]
#[class(base=Area3D)]
pub struct Button3D {
    /// Farbstil: 0 = Orange (Feuer), 1 = Blau (Wolken).
    #[export]
    style: ButtonStyle,
    /// Beschriftung auf dem Knopf.
    #[export]
    text: GString,
    /// Breite der Bildfläche in Welteinheiten (Höhe folgt aus dem Seitenverhältnis).
    #[export]
    width: f32,
    /// Knopf immer parallel zur Bildebene der Kamera ausrichten.
    #[export]
    face_camera: bool,
    /// Anklickbar? (Ausgegraut und ohne Klickwirkung, wenn nicht.)
    #[export]
    #[var(pub)]
    enabled: bool,

    material: Option<Gd<ShaderMaterial>>,
    label: Option<Gd<Label3D>>,
    visual: Option<Gd<Node3D>>,
    hovered: bool,
    /// 0 = nicht gehovert, 1 = gehovert (weich nachgeführt).
    hover_amount: f32,
    /// Zeit seit dem Erscheinen bzw. seit Beginn des Verschwindens.
    appear: f32,
    dismissing: Option<f32>,
    press: Option<f32>,
    screen_pin: Option<ScreenPin>,

    base: Base<Area3D>,
}

#[godot_api]
impl IArea3D for Button3D {
    fn init(base: Base<Area3D>) -> Self {
        Self {
            style: ButtonStyle::Orange,
            text: GString::new(),
            width: 2.4,
            face_camera: true,
            enabled: true,
            material: None,
            label: None,
            visual: None,
            hovered: false,
            hover_amount: 0.0,
            appear: 0.0,
            dismissing: None,
            press: None,
            screen_pin: None,
            base,
        }
    }

    fn ready(&mut self) {
        self.build();
    }

    fn process(&mut self, delta: f64) {
        let dt = delta as f32;
        self.appear += dt;
        if let Some(pin) = self.screen_pin {
            self.follow_screen(pin);
        } else if self.face_camera {
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
        // Leuchten beim Hover heller; ausgegraut, wenn nicht anklickbar (Puls macht der Shader)
        let boost = 1.0 + 0.45 * self.hover_amount;
        let disabled = if self.enabled { 0.0 } else { 1.0 };
        if let Some(m) = self.material.as_mut() {
            m.set_shader_parameter("glow_boost", &boost.to_variant());
            m.set_shader_parameter("disabled", &disabled.to_variant());
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

    /// Knopf an eine Bildschirmstelle heften (wie ein HUD-Element, aber in der 3D-Szene): Mitte
    /// = `anchor` × Fenstergröße + `offset` (Pixel, z. B. `anchor` (1, 1) = unten rechts),
    /// `width_px` = Breite der Bildfläche in Pixeln. Er steht dann jedes Frame kurz vor der
    /// Kamera, parallel zur Bildebene, vor allen Karten.
    #[func]
    pub fn pin_to_screen(&mut self, anchor: Vector2, offset: Vector2, width_px: f32) {
        self.screen_pin = Some(ScreenPin { anchor, offset, width_px });
    }

    /// Breite der Bildfläche in Pixeln, die zu einer Knopf-Fläche (ohne Leuchten) von `face_px`
    /// Pixeln gehört.
    pub fn image_width_for_face(face_px: f32) -> f32 {
        face_px / FACE_FRACTION.x
    }

    /// Höhe der Knopf-Fläche (ohne Leuchten) in Pixeln bei `face_px` Pixeln Breite.
    pub fn face_height_for_face(face_px: f32) -> f32 {
        face_px / FACE_FRACTION.x / IMAGE_ASPECT * FACE_FRACTION.y
    }

    /// Beschriftung ändern (ohne Wirkung, wenn sie gleich bleibt).
    #[func]
    pub fn set_label_text(&mut self, text: GString) {
        if self.text == text && self.label.is_some() {
            return;
        }
        self.text = text.clone();
        if let Some(l) = self.label.as_mut() {
            l.set_text(&text);
        }
        self.fit_label();
    }
}

impl Button3D {
    /// Neuer Knopf im Farbstil `style` mit Beschriftung (noch nicht in der Szene).
    pub fn create(style: ButtonStyle, text: &str, width: f32) -> Gd<Button3D> {
        let mut b = Button3D::new_alloc();
        {
            let mut m = b.bind_mut();
            m.style = style;
            m.text = text.into();
            m.width = width;
        }
        b
    }

    /// Angeheftet: vor die Kamera an die Bildschirmstelle setzen, Größe in Pixeln halten.
    fn follow_screen(&mut self, pin: ScreenPin) {
        let Some(viewport) = self.base().get_viewport() else { return };
        let Some(cam) = viewport.get_camera_3d() else { return };
        let size = viewport.get_visible_rect().size;
        let point = pin.anchor * size + pin.offset;
        let origin = cam.project_position(point, SCREEN_DEPTH);
        // Welteinheiten je Pixel in dieser Tiefe (senkrechter Blickwinkel)
        let per_px = 2.0 * SCREEN_DEPTH * (cam.get_fov().to_radians() * 0.5).tan() / size.y.max(1.0);
        let scale = pin.width_px * per_px / self.width.max(0.001);
        let basis = cam.get_global_transform().basis.orthonormalized().scaled(Vector3::splat(scale));
        self.base_mut().set_global_transform(Transform3D::new(basis, origin));
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
        let (surface, material) = surface(self.style, Vector2::new(self.width, height));
        visual.add_child(&surface);

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
        self.material = Some(material);
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

/// Fläche mit dem Knopf-Shader im Farbstil `style` (Vorderseite +Z). Jeder Knopf bekommt einen
/// zufälligen Zeitversatz, damit nebeneinanderliegende Knöpfe nicht im Gleichtakt laufen.
fn surface(style: ButtonStyle, size: Vector2) -> (Gd<MeshInstance3D>, Gd<ShaderMaterial>) {
    let mut shader = Shader::new_gd();
    shader.set_code(BUTTON_SHADER);
    let mut mat = ShaderMaterial::new_gd();
    mat.set_shader(&shader);
    mat.set_render_priority(1);
    let p = palette(style);
    let v4 = |c: Color| Vector4::new(c.r, c.g, c.b, 1.0);
    for (i, name) in ["ramp0", "ramp1", "ramp2", "ramp3"].iter().enumerate() {
        mat.set_shader_parameter(*name, &v4(p.ramp[i]).to_variant());
    }
    let rp = p.ramp_pos;
    mat.set_shader_parameter("ramp_pos", &Vector4::new(rp[0], rp[1], rp[2], rp[3]).to_variant());
    for (name, c) in [
        ("streak_color", p.streak),
        ("shade_color", p.shade),
        ("rim_color", p.rim),
        ("glow_inner", p.glow_inner),
        ("glow_outer", p.glow_outer),
    ] {
        mat.set_shader_parameter(name, &v4(c).to_variant());
    }
    let offset = godot::global::randf_range(0.0, 4.0) as f32;
    mat.set_shader_parameter("time_offset", &offset.to_variant());
    let mut mesh = QuadMesh::new_gd();
    mesh.set_size(size);
    mesh.set_material(&mat);
    let mut node = MeshInstance3D::new_alloc();
    node.set_name("Surface");
    node.set_mesh(&mesh);
    node.set_cast_shadows_setting(ShadowCastingSetting::OFF);
    (node, mat)
}
