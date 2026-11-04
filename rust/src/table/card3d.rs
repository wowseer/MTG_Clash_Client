//! Card3D: eine einzelne Karte auf dem 3D-Tisch.
//!
//! Aufbau (wird in `ready()` per Code erzeugt):
//! ```text
//! Card3D (Area3D, Kollisions-Layer CARD_LAYER)   <- wird per Tween bewegt
//! ├── CollisionShape3D (BoxShape3D)                <- Raycast-Ziel, bleibt ruhig
//! └── Visual (Node3D)                              <- Tilt / Hover-Zoom / Drag-Schwung
//!     ├── Body  (BoxMesh, dünner Kartenkörper)
//!     ├── Face  (PlaneMesh, +Y, Vorderseiten-Textur)
//!     ├── Back  (PlaneMesh, -Y, Rückseiten-Textur)
//!     ├── Marker (PlaneMesh unter der Karte, Lichtsaum: kann handeln / Angriff / Block / Auswahl)
//!     └── TapIcon (PlaneMesh auf der Vorderseite, graues Tapp-Symbol; getappt, aber nicht angegriffen)
//! ```
//! **Getappt:** Die Zone stellt die Karte leicht schräg (`zone::TAP_ANGLE_DEGREES`); sie wird
//! ausgegraut (Vorderseite und Beschriftung abgedunkelt) und zeigt das graue Tapp-Symbol.
//! **Hat angegriffen:** ebenfalls schräg und ausgegraut, aber **ohne** Tapp-Symbol.
//!
//! Die Karte liegt in ihrem lokalen Raum flach in der XZ-Ebene, Vorderseite zeigt
//! nach +Y, die Oberkante der Karte zeigt nach -Z.
//!
//! **Hover-Zoom:** Ist die Vorderseite sichtbar, schwebt nur das `Visual` zur
//! Kamera, dreht sich aufrecht zum Betrachter und wird so groß, dass sie
//! `zoom_screen_height` der Bildschirmhöhe einnimmt – immer vollständig im Bild.
//! Die Kollisionsform bleibt am Tisch, das Hovern bleibt dadurch stabil.

use godot::classes::base_material_3d::{ShadingMode, TextureFilter, TextureParam};
use godot::classes::geometry_instance_3d::ShadowCastingSetting;
use godot::classes::tween::{EaseType, TransitionType};
use godot::classes::{
    Area3D, BoxMesh, BoxShape3D, CollisionShape3D, IArea3D, MeshInstance3D, PlaneMesh,
    Shader, ShaderMaterial, StandardMaterial3D, Texture2D, Tween,
};
use godot::prelude::*;

use godot::classes::label_3d::DrawFlags;
use godot::classes::Label3D;
use godot::global::HorizontalAlignment;

use crate::game::card_text;
use crate::game::cards::{CardDef, CardKind};
use crate::game::mana::ManaType;
use super::placeholder;
use super::layers::CARD_LAYER;
use super::zone::Zone;

pub const CARD_WIDTH: f32 = 0.63;
pub const CARD_HEIGHT: f32 = 0.88;
pub const CARD_THICKNESS: f32 = 0.006;
/// Kachel-Form auf dem Spielfeld (wie im Video des Users): nur der obere Teil der Karte – Name
/// und Bild –, Stärke/Widerstand rechts unten. Anteil der Kartenhöhe und Höhe der Kachel.
pub const TILE_FRACTION: f32 = 0.6;
pub const TILE_HEIGHT: f32 = CARD_HEIGHT * TILE_FRACTION;

/// Farben des Kartenrahmens (Rahmen-Glow aus `Assets/Mana Glow Effekte`, linear wie in Blender):
/// **Blau** = Standard, die Karte ist spielbar bzw. wählbar; **Orange** = aktiviert (wird gezogen
/// oder gespielt, Fähigkeit aktiviert, als Angreifer/Blocker/Ziel gewählt, Land für Mana getappt).
pub const PLAYABLE_GLOW: Color = Color::from_rgb(0.02, 0.28, 1.0);
pub const ACTIVE_GLOW: Color = Color::from_rgb(1.0, 0.22, 0.0);
/// Helligkeit des Rahmens: spielbar (ruhig) bzw. aktiviert (kräftiger).
const READY_STRENGTH: f32 = 1.0;
/// Aufleuchten beim Aktivieren ([`Card3D::flash`]; Blender: Einschlag Frame 30 → 80):
/// Verstärkung über die Zeit (Sekunden, Faktor), danach orange bis zum Ende.
const FLASH_KEYS: [(f32, f32); 5] = [(0.0, 1.0), (0.07, 4.6), (0.27, 2.7), (0.67, 1.7), (1.6, 1.0)];
/// Angreifer: rückt so weit nach vorn (zum Gegner, lokal −Z).
const ADVANCE: f32 = 0.22;
/// Schlag im Kampf: Ausholen zum Ziel, Rückweg (Sekunden), Anheben in der Mitte.
pub const LUNGE_OUT: f32 = 0.22;
const LUNGE_BACK: f32 = 0.38;
const LUNGE_LIFT: f32 = 0.6;
/// Doppelseitige Karte umdrehen (ANIMATION.md): Dauer, Anheben in der Mitte (Cubic-In-Out).
const FLIP_DURATION: f32 = 0.35;
const FLIP_LIFT: f32 = 0.25;
const MARK_STRENGTH: f32 = 1.45;
/// Farbe, mit der getappte Karten (und Kreaturen, die angegriffen haben) ausgegraut werden.
const TAPPED_TINT: Color = Color::from_rgb(0.52, 0.52, 0.55);
/// Kantenlänge des Tapp-Symbols auf der Karte (Einheiten; Karte = 0,63 breit).
const TAP_ICON_SIZE: f32 = 0.34;
/// Wie weit die Aura über den Kartenrand hinausragt (Einheiten).
const GLOW_MARGIN: f32 = 0.16;

/// Größe der Angriffsspitze (Breite, Höhe) über der Oberkante der Karte.
const BANNER_SIZE: Vector2 = Vector2::new(0.36, 0.15);

/// Angriffsspitze: Fünfeck als Dach (unten breit, oben Spitze), rot mit hellem Rand und Schein.
const BANNER_SHADER: &str = r#"
shader_type spatial;
render_mode unshaded, blend_mix, cull_disabled, depth_draw_never, shadows_disabled;
float sd_roof(vec2 p) {
    // p: x −1…1, y 0 (unten) … 1 (Spitze)
    float side = abs(p.x) - 0.92;
    float roof = (p.y - 0.45) + abs(p.x) * 0.55 - 0.5;
    float bottom = -p.y + 0.02;
    return max(max(side, roof), bottom);
}
void fragment() {
    vec2 p = vec2(UV.x * 2.0 - 1.0, 1.0 - UV.y);
    float d = sd_roof(p);
    float fill = 1.0 - smoothstep(-0.01, 0.02, d);
    float rim = 1.0 - smoothstep(0.0, 0.06, abs(d + 0.05));
    float glow = exp(-max(d, 0.0) / 0.08) * 0.6;
    vec3 col = mix(vec3(0.75, 0.08, 0.04), vec3(1.0, 0.55, 0.3), rim * fill);
    ALBEDO = col;
    ALPHA = clamp(fill + glow * (1.0 - fill), 0.0, 1.0);
}
"#;

/// Rahmen-Glow (Nachbau von „Rahmen_Kern“ und „Rahmen_Schein“ der Mana-Glow-Assets): dünne,
/// fast weiße Kernlinie genau an der Kartenkante (Blender: Bevel 0,035 bei Karte 2,5 breit), darum
/// ein breiter Schein (Bevel 0,14) und ein weicher Halo als Ersatz für Blenders Bloom. Additiv,
/// unbeleuchtet. `strength` = Kern, `schein` = Schein; ruhiges Pulsieren wie das Idle der Datei.
const GLOW_SHADER: &str = r#"
shader_type spatial;
render_mode unshaded, blend_add, cull_disabled, depth_draw_never, shadows_disabled;
uniform vec3 glow_color = vec3(0.02, 0.28, 1.0);
uniform float strength = 1.0;
uniform float schein = 0.3;
uniform vec2 quad_size;
uniform vec2 card_half;

float sd_round_rect(vec2 p, vec2 b, float r) {
    vec2 q = abs(p) - b + vec2(r);
    return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r;
}

void fragment() {
    vec2 p = (UV - 0.5) * quad_size;
    float edge = sd_round_rect(p, card_half, 0.038);
    float core = 1.0 - smoothstep(0.004, 0.012, abs(edge - 0.004));
    float sch = exp(-pow(abs(edge - 0.013) / 0.035, 2.0));
    float halo = exp(-max(edge, 0.0) / 0.06) * 0.45;
    float pulse = 0.85 + 0.15 * sin(TIME * 5.0);
    float margin = quad_size.x * 0.5 - card_half.x;
    float fade = smoothstep(margin, margin * 0.55, max(edge, 0.0));
    vec3 col = mix(glow_color, vec3(1.0), core * 0.35);
    ALBEDO = col * (core * strength * 1.4 + (sch + halo) * schein * strength) * pulse * fade;
}
"#;

/// Eine Spielkarte auf dem 3D-Tisch: Mesh mit Vorder-/Rückseite und Beschriftung,
/// Hover-Neigung und Hover-Zoom (offene Karten werden groß und lesbar), Raycast-Ziel
/// (Kollisions-Layer 1), Gleit-Animationen per Tween sowie alle Spielwerte
/// (Name, Typzeile, Kosten, Regeltext, Werte aus den Kartendaten der Engine; Tapp-Zustand).
#[derive(GodotClass)]
#[class(base=Area3D)]
pub struct Card3D {
    /// Textur der Vorderseite.
    #[export]
    #[var(set = set_face_texture)]
    face_texture: Option<Gd<Texture2D>>,
    /// Textur der Rückseite.
    #[export]
    #[var(set = set_back_texture)]
    back_texture: Option<Gd<Texture2D>>,
    /// Maximale Neigung bei Mouseover (Grad).
    #[export]
    max_tilt_degrees: f32,
    /// Glättung von Tilt/Zoom (größer = schneller).
    #[export]
    tilt_speed: f32,
    /// Dauer der Gleit-Animation zu einem Slot (Sekunden).
    #[export]
    move_duration: f64,

    #[export_group(name = "Hover-Zoom")]
    /// Offene Karten beim Hovern selbst groß und lesbar vor die Kamera holen (Standard: aus –
    /// stattdessen zeigt der [CardInspector] eine Kopie mit Fähigkeiten und Buffs).
    #[export]
    zoom_enabled: bool,
    /// Anteil der Bildschirmhöhe, den die gezoomte Karte einnimmt.
    #[export(range = (0.1, 0.95))]
    zoom_screen_height: f32,
    /// Abstand der gezoomten Karte zur Kamera (liegt damit vor allem anderen).
    #[export]
    zoom_distance: f32,
    /// Rand zum Bildschirmrand in Pixeln.
    #[export]
    zoom_margin_px: f32,
    /// Anheben (entlang der Kartennormalen) für verdeckte Karten / ohne Zoom.
    #[export]
    hover_lift: f32,
    /// Skalierung für verdeckte Karten / ohne Zoom.
    #[export]
    hover_scale: f32,

    /// Kartenname (wie von der Engine).
    #[export_group(name = "Karte")]
    #[export]
    card_name: GString,
    /// Kartenart: 0 = Kreatur, 1 = Land, 2 = Spontanzauber, 3 = Hexerei, 4 = Planeswalker,
    /// 5 = Ausrüstung, 6 = Artefakt, 7 = Verzauberung, 8 = Aura, 9 = Schlacht.
    #[export]
    card_kind: CardKind,
    /// Kartendaten für das Bild (Typzeile, Regeltext, Werte), siehe [`CardDef`].
    def: CardDef,

    // Anzeige-Zustand
    tapped: bool,
    marker: CardMarker,
    /// Dezentes Leuchten in dieser Farbe: Karte kann gerade handeln (None = aus).
    ready_glow: Option<Color>,
    /// Aufleuchten seit (Sekunden; `None` = keins): Rahmen kurz hell und orange.
    flash_t: Option<f32>,
    /// Angreifer im Kampf: rückt vor und trägt die Angriffsspitze.
    advanced: bool,
    advance_amount: f32,
    attack_banner: Option<Gd<MeshInstance3D>>,
    /// Schlag zum Ziel (Welt), Zeit, Reichweite (Anteil des Wegs), Verzögerung.
    lunge: Option<(Vector3, f32, f32, f32)>,
    lunge_applied: Vector3,
    /// Kachel-Form gewünscht (Spielfeld) bzw. gerade gezeigt (beim Zoomen/Ziehen ganze Karte).
    tile: bool,
    tile_shown: bool,
    shape_refs: Option<ShapeRefs>,
    /// Immer unbeleuchtet in Originalfarben (Karten auf einer 3D-Bühne vor der Kamera, wo das
    /// Tischlicht nicht hinreicht – sonst wirken sie grau).
    bright: bool,
    /// Hat in diesem Zug angegriffen (ausgegraut, bis sie enttappt wird).
    attacked: bool,
    /// Laufendes Umdrehen: vergangene Zeit und die neue Seite (Kartendaten, Bild), die bei
    /// halber Drehung übernommen wird.
    flip: Option<(f32, Option<(CardDef, Gd<Texture2D>)>)>,
    /// Besitzer (0 = Spieler, 1 = Gegner): auf dessen Friedhof geht die Karte.
    #[var]
    pub(crate) owner_seat: i64,

    labels: Option<CardLabels>,
    /// Text der Plaketten (eine Zeile je Marke bzw. Kennzeichen).
    badges: String,
    marker_mesh: Option<Gd<MeshInstance3D>>,
    marker_material: Option<Gd<ShaderMaterial>>,
    tap_icon: Option<Gd<MeshInstance3D>>,
    visual: Option<Gd<Node3D>>,
    meshes: Vec<Gd<MeshInstance3D>>,
    face_material: Option<Gd<StandardMaterial3D>>,
    back_material: Option<Gd<StandardMaterial3D>>,
    hovered: bool,
    zoomed: bool,
    /// Mausposition relativ zur Karte, normalisiert auf -1..1 (x = links/rechts, y = oben/unten).
    hover_local: Vector2,
    dragging: bool,
    /// Neigung während des Ziehens (vom Controller aus der Bewegung berechnet), Radiant.
    drag_sway: Vector2,
    tween: Option<Gd<Tween>>,
    zone: Option<Gd<Zone>>,

    base: Base<Area3D>,
}

#[godot_api]
impl IArea3D for Card3D {
    fn init(base: Base<Area3D>) -> Self {
        Self {
            face_texture: None,
            back_texture: None,
            max_tilt_degrees: 10.0,
            tilt_speed: 16.0,
            move_duration: 0.35,
            zoom_enabled: false,
            zoom_screen_height: 0.55,
            zoom_distance: 2.0,
            zoom_margin_px: 12.0,
            hover_lift: 0.06,
            hover_scale: 1.08,
            card_name: "Karte".into(),
            card_kind: CardKind::Creature,
            def: CardDef::named("Karte"),
            tapped: false,
            marker: CardMarker::None,
            ready_glow: None,
            flash_t: None,
            advanced: false,
            advance_amount: 0.0,
            attack_banner: None,
            lunge: None,
            lunge_applied: Vector3::ZERO,
            tile: false,
            tile_shown: false,
            shape_refs: None,
            bright: false,
            attacked: false,
            flip: None,
            owner_seat: 0,
            labels: None,
            badges: String::new(),
            marker_mesh: None,
            marker_material: None,
            tap_icon: None,
            visual: None,
            meshes: Vec::new(),
            face_material: None,
            back_material: None,
            hovered: false,
            zoomed: false,
            hover_local: Vector2::ZERO,
            dragging: false,
            drag_sway: Vector2::ZERO,
            tween: None,
            zone: None,
            base,
        }
    }

    fn ready(&mut self) {
        self.build_nodes();
    }

    fn process(&mut self, delta: f64) {
        if self.visual.is_none() {
            return;
        }

        let max_tilt = self.max_tilt_degrees.to_radians();
        // Die Kante unter dem Mauszeiger wird "hineingedrückt":
        // +X-Rotation senkt die +Z-Kante, +Z-Rotation hebt die +X-Kante.
        let tilt = Basis::from_euler(
            EulerOrder::XYZ,
            Vector3::new(self.hover_local.y * max_tilt, 0.0, -self.hover_local.x * max_tilt),
        );

        let mut zoomed = false;
        let target = if self.dragging {
            let sway = Vector3::new(self.drag_sway.y, 0.0, self.drag_sway.x);
            Transform3D::new(Basis::from_euler(EulerOrder::XYZ, sway), Vector3::ZERO)
        } else if self.hovered {
            let card_xf = self.base().get_global_transform();
            match self.zoom_transform(&card_xf) {
                Some(mut global) => {
                    zoomed = true;
                    global.basis = global.basis * tilt;
                    card_xf.affine_inverse() * global
                }
                None => Transform3D::new(
                    tilt.scaled(Vector3::splat(self.hover_scale)),
                    Vector3::new(0.0, self.hover_lift, 0.0),
                ),
            }
        } else {
            Transform3D::IDENTITY
        };

        if zoomed != self.zoomed {
            self.zoomed = zoomed;
            self.apply_brightness();
            // Die schwebende Karte soll keinen riesigen Schatten auf den Tisch werfen.
            let setting = if zoomed { ShadowCastingSetting::OFF } else { ShadowCastingSetting::ON };
            for mesh in self.meshes.iter_mut() {
                mesh.set_cast_shadows_setting(setting);
            }
        }

        let dt = delta as f32;
        // Kachel auf dem Spielfeld, beim Zoomen und Ziehen die ganze Karte (lesbar)
        let want_tile = self.tile && !zoomed && !self.dragging;
        if want_tile != self.tile_shown {
            self.tile_shown = want_tile;
            self.apply_shape();
        }
        // Aufleuchten abklingen lassen
        if let Some(t) = self.flash_t.as_mut() {
            *t += dt;
            if *t > FLASH_KEYS[FLASH_KEYS.len() - 1].0 {
                self.flash_t = None;
            }
            self.apply_glow();
        }
        // Angreifer rückt vor (nicht beim Ziehen/Zoomen)
        let adv_goal = if self.advanced && !self.dragging && !zoomed { 1.0 } else { 0.0 };
        self.advance_amount += (adv_goal - self.advance_amount) * (1.0 - (-10.0 * dt).exp());
        let mut target = target;
        target.origin.z -= ADVANCE * self.advance_amount;
        let lunge = self.lunge_offset(dt);

        let w = 1.0 - (-self.tilt_speed * dt).exp();
        let flip = self.advance_flip(dt);
        let applied = self.lunge_applied;
        let visual = self.visual.as_mut().unwrap();
        let mut current = visual.get_transform();
        current.origin -= applied;
        let mut next = current.interpolate_with(&target, w);
        next.origin += lunge;
        match flip {
            // Umdrehen: Drehung um die Längsachse (lokal z) und Anheben direkt setzen
            Some(xf) => visual.set_transform(target * xf),
            None => visual.set_transform(next),
        }
        self.lunge_applied = lunge;
    }
}

#[godot_api]
impl Card3D {
    /// Setzt die Textur der Vorderseite.
    #[func]
    pub fn set_face_texture(&mut self, tex: Option<Gd<Texture2D>>) {
        self.face_texture = tex;
        if let Some(mat) = self.face_material.as_mut() {
            apply_texture(mat, self.face_texture.as_ref(), Color::from_rgb(0.9, 0.9, 0.9));
        }
        self.apply_tapped_look();
    }

    /// Setzt die Textur der Rückseite.
    #[func]
    pub fn set_back_texture(&mut self, tex: Option<Gd<Texture2D>>) {
        self.back_texture = tex;
        if let Some(mat) = self.back_material.as_mut() {
            apply_texture(mat, self.back_texture.as_ref(), Color::from_rgb(0.15, 0.2, 0.45));
        }
    }

    /// Aktiviert/deaktiviert den Hover-Zustand.
    #[func]
    pub fn set_hovered(&mut self, hovered: bool) {
        self.hovered = hovered;
        if !hovered {
            self.hover_local = Vector2::ZERO;
        }
    }

    /// Liegt der Mauszeiger gerade auf der Karte?
    #[func]
    pub fn is_hovered(&self) -> bool {
        self.hovered
    }

    /// Wird die Karte gerade vergrößert angezeigt?
    #[func]
    pub fn is_zoomed(&self) -> bool {
        self.zoomed
    }

    /// Die vier Ecken der Karte, wie sie gerade auf dem Bildschirm erscheint (inklusive
    /// Hover-Zoom, Neigung und Drag-Schwung), in Pixeln. Leer, wenn die Karte hinter der Kamera liegt.
    #[func]
    pub fn screen_quad(&self) -> PackedVector2Array {
        let (Some(visual), Some(cam)) = (self.visual.as_ref(), self.base().get_viewport().and_then(|v| v.get_camera_3d())) else {
            return PackedVector2Array::new();
        };
        let xf = visual.get_global_transform();
        let (w, h) = (CARD_WIDTH * 0.5, CARD_HEIGHT * 0.5);
        let mut quad = PackedVector2Array::new();
        for (x, z) in [(-w, -h), (w, -h), (w, h), (-w, h)] {
            let world = xf * Vector3::new(x, 0.0, z);
            if cam.is_position_behind(world) {
                return PackedVector2Array::new();
            }
            quad.push(cam.unproject_position(world));
        }
        quad
    }

    /// Übergibt den Raycast-Trefferpunkt (Weltkoordinaten) für den Tilt-Effekt.
    #[func]
    pub fn update_hover_point(&mut self, world_point: Vector3) {
        let local = self.base().get_global_transform().affine_inverse() * world_point;
        self.hover_local = Vector2::new(
            (local.x / (CARD_WIDTH * 0.5)).clamp(-1.0, 1.0),
            (local.z / (CARD_HEIGHT * 0.5)).clamp(-1.0, 1.0),
        );
    }

    /// Markiert die Karte als gezogen (Hover aus, laufende Animation stoppt). Setzt der [TableController].
    #[func]
    pub fn set_dragging(&mut self, dragging: bool) {
        self.dragging = dragging;
        self.apply_glow();
        if dragging {
            self.hovered = false;
            self.stop_motion();
        } else {
            self.drag_sway = Vector2::ZERO;
        }
    }

    /// Wird die Karte gerade gezogen?
    #[func]
    pub fn is_dragging(&self) -> bool {
        self.dragging
    }

    /// Neigung während des Ziehens (x = Rollen um Z, y = Nicken um X), Radiant.
    #[func]
    pub fn set_drag_sway(&mut self, sway: Vector2) {
        self.drag_sway = sway;
    }

    /// Lässt die Karte per Tween sanft zu `target` (globale Transform) gleiten.
    #[func]
    pub fn move_to(&mut self, target: Transform3D, delay: f64) {
        self.stop_motion();
        let duration = self.move_duration;
        let this = self.to_gd();
        let mut tween = self.base_mut().create_tween();
        tween.set_trans(TransitionType::CUBIC);
        tween.set_ease(EaseType::OUT);
        if delay > 0.0 {
            tween.tween_interval(delay);
        }
        tween.tween_property(&this, "global_transform", &target.to_variant(), duration);
        self.tween = Some(tween);
    }

    /// Setzt die Karte sofort ohne Animation.
    #[func]
    pub fn snap_to(&mut self, target: Transform3D) {
        self.stop_motion();
        self.base_mut().set_global_transform(target);
    }

    /// Bricht eine laufende Gleit-Animation ab.
    #[func]
    pub fn stop_motion(&mut self) {
        if let Some(mut tween) = self.tween.take() {
            if tween.is_valid() {
                tween.kill();
            }
        }
    }

    /// Dezenter goldener Rand „diese Karte kann eine Fähigkeit aktivieren“ an/aus (für Skripte;
    /// Rot/Blau setzt das Spiel selbst). Eine Markierung hat Vorrang.
    #[func]
    pub fn set_ready_glow(&mut self, on: bool) {
        self.set_ready_color(on.then_some(PLAYABLE_GLOW));
    }

    /// Rahmen leuchtet kurz hell und orange auf (etwas wurde aktiviert: gespielt, Fähigkeit,
    /// Mana-Einschlag, Angriff …), danach zurück zum normalen Leuchten.
    #[func]
    pub fn flash(&mut self) {
        self.flash_t = Some(0.0);
        self.apply_glow();
    }

    /// Angreifer im Kampf: Karte rückt etwas zum Gegner vor und trägt die rote Angriffsspitze
    /// an der Oberkante (`false` = zurück an ihren Platz).
    #[func]
    pub fn set_attacking(&mut self, attacking: bool) {
        self.advanced = attacking;
        if let Some(b) = self.attack_banner.as_mut() {
            b.set_visible(attacking);
        }
    }

    /// Kachel-Form (Spielfeld): nur Name und Bild, Stärke/Widerstand unten rechts; beim Zoomen
    /// und Ziehen wird die ganze Karte gezeigt. Setzt die Zone beim Anordnen.
    #[func]
    pub fn set_tile(&mut self, tile: bool) {
        if self.tile != tile {
            self.tile = tile;
            self.tile_shown = tile && !self.zoomed && !self.dragging;
            self.apply_shape();
        }
    }

    /// Immer hell in Originalfarben zeigen (Karten auf einer 3D-Bühne vor der Kamera).
    #[func]
    pub fn set_bright(&mut self, bright: bool) {
        self.bright = bright;
        self.apply_brightness();
    }

    /// Wird die Karte als Kachel gezeigt?
    #[func]
    pub fn is_tile(&self) -> bool {
        self.tile
    }

    /// Ist die Karte als Angreifer markiert (vorgerückt, mit Angriffsspitze)?
    #[func]
    pub fn is_attacking(&self) -> bool {
        self.advanced
    }

    /// Schlag im Kampf: die Karte holt zum Weltpunkt `target` aus (Anteil `reach` des Wegs,
    /// angehoben), trifft nach [`LUNGE_OUT`] Sekunden und kehrt zurück; Start nach `delay`.
    #[func]
    pub fn lunge(&mut self, target: Vector3, reach: f32, delay: f32) {
        self.lunge = Some((target, 0.0, reach, delay));
    }

    /// Aura um die Karte setzen: 0 = keine, 1 = Angreifer (rot), 2 = Blocker (blau),
    /// 3 = ausgewählt/wählt ein Ziel (gold).
    #[func]
    pub fn set_marker_kind(&mut self, kind: i64) {
        let marker = match kind {
            1 => CardMarker::Attacking,
            2 => CardMarker::Blocking,
            3 => CardMarker::Selected,
            _ => CardMarker::None,
        };
        self.set_marker(marker);
    }

    /// Setzt nur den Tapp-Zustand der Karte (schräg, ausgegraut, Tapp-Symbol), ohne Spielregeln
    /// (kein Mana, kein Neu-Anordnen) – für Skripte und Tests.
    #[func]
    pub fn set_tapped_state(&mut self, tapped: bool) {
        self.set_tapped(tapped);
    }

    /// Ist die Karte getappt (leicht schräg, ausgegraut, Tapp-Symbol)?
    #[func]
    pub fn is_tapped(&self) -> bool {
        self.tapped
    }

    /// Die Zone, in der die Karte aktuell liegt (None während des Ziehens).
    #[func]
    pub fn get_zone(&self) -> Option<Gd<Zone>> {
        self.zone.clone()
    }
}

impl Card3D {
    // ------------------------------------------------------------ Spielwerte

    /// Übernimmt die Kartendaten (Name, Typ, Text, Werte) und beschriftet die Vorderseite neu.
    pub fn apply_def(&mut self, def: &CardDef) {
        self.card_name = def.name.as_str().into();
        self.card_kind = def.kind;
        self.def = def.clone();
        self.refresh_labels();
    }

    /// Name der Karte (wie auf dem Kartenbild, ohne Übersetzung).
    pub fn card_name(&self) -> GString {
        self.card_name.clone()
    }

    pub fn kind(&self) -> CardKind {
        self.card_kind
    }

    /// Dreht die Karte animiert auf eine andere Seite um (doppelseitige Karten, CR 711): bei
    /// halber Drehung werden `def` und `face` übernommen. Ohne Szene (noch nicht bereit) sofort.
    pub fn flip_to(&mut self, def: &CardDef, face: Gd<Texture2D>) {
        if self.visual.is_none() {
            self.set_face_texture(Some(face));
            self.apply_def(def);
            return;
        }
        // Läuft schon ein Umdrehen, wird nur die Zielseite ersetzt
        let elapsed = self.flip.as_ref().map_or(0.0, |f| f.0);
        self.flip = Some((elapsed, Some((def.clone(), face))));
    }

    /// Schreitet das Umdrehen fort; liefert die Zusatz-Transformation des Kartenbilds (None =
    /// kein Umdrehen). Erste Hälfte: 0° → 90°; dann Seitenwechsel; zweite Hälfte −90° → 0°.
    fn advance_flip(&mut self, delta: f32) -> Option<Transform3D> {
        let (t, _) = self.flip.as_mut()?;
        *t += delta;
        let p = (*t / FLIP_DURATION).min(1.0);
        // Cubic-In-Out
        let e = if p < 0.5 { 4.0 * p * p * p } else { 1.0 - (-2.0 * p + 2.0).powi(3) * 0.5 };
        if e >= 0.5 {
            if let Some((def, face)) = self.flip.as_mut().and_then(|f| f.1.take()) {
                self.set_face_texture(Some(face));
                self.apply_def(&def);
            }
        }
        if p >= 1.0 {
            self.flip = None;
            return Some(Transform3D::IDENTITY);
        }
        let angle = if e < 0.5 { std::f32::consts::PI * e } else { std::f32::consts::PI * (e - 1.0) };
        let lift = (std::f32::consts::PI * p).sin() * FLIP_LIFT;
        Some(Transform3D::new(Basis::from_axis_angle(Vector3::BACK, angle), Vector3::new(0.0, lift, 0.0)))
    }

    /// Aktuelle Kartendaten des Bildes.
    pub fn def(&self) -> &CardDef {
        &self.def
    }

    /// Nur Länder: Manafarbe (Spalte in der Länder-Reihe).
    pub fn produced_mana(&self) -> ManaType {
        self.def.produces
    }

    /// Tapp-Zustand setzen; die Drehung übernimmt die Zone beim nächsten `layout()`.
    /// Tappt bzw. enttappt die Karte: getappt = leicht schräg (durch die Zone), ausgegraut und mit
    /// Tapp-Symbol. Enttappen beendet auch den Zustand „hat angegriffen“.
    pub fn set_tapped(&mut self, tapped: bool) {
        self.tapped = tapped;
        if !tapped {
            self.attacked = false;
        }
        self.apply_tapped_look();
    }

    /// Dezentes Leuchten „kann handeln“ in `color` (Rot = kann angreifen, Blau = kann blocken,
    /// Gold = kann eine Fähigkeit aktivieren); None = aus. Eine Markierung hat Vorrang.
    pub fn set_ready_color(&mut self, color: Option<Color>) {
        if self.ready_glow != color {
            self.ready_glow = color;
            self.apply_glow();
        }
    }

    /// Ausgrauen (getappt oder hat angegriffen) und Tapp-Symbol (nur getappt ohne Angriff).
    fn apply_tapped_look(&mut self) {
        let tint = if self.tapped || self.attacked { TAPPED_TINT } else { Color::WHITE };
        if let Some(mat) = self.face_material.as_mut() {
            if self.face_texture.is_some() {
                mat.set_albedo(tint);
            }
        }
        if let Some(labels) = self.labels.as_mut() {
            let colors = labels.colors;
            for (l, c) in labels.all().into_iter().zip(colors) {
                l.set_modulate(c * tint);
            }
        }
        if let Some(icon) = self.tap_icon.as_mut() {
            icon.set_visible(self.tapped && !self.attacked);
        }
    }

    pub fn set_marker(&mut self, marker: CardMarker) {
        self.marker = marker;
        self.apply_glow();
    }

    /// Unbeleuchtet in Originalfarben, wenn gezoomt oder [method set_bright]: vor der Kamera liegt
    /// die Karte außerhalb der Tischbeleuchtung und wäre sonst dunkel bzw. grau.
    fn apply_brightness(&mut self) {
        let bright = self.zoomed || self.bright;
        let shading = if bright { ShadingMode::UNSHADED } else { ShadingMode::PER_PIXEL };
        if let Some(mat) = self.face_material.as_mut() {
            mat.set_shading_mode(shading);
        }
        if let Some(labels) = self.labels.as_mut() {
            for l in labels.all() {
                l.set_draw_flag(DrawFlags::SHADED, !bright);
            }
        }
    }

    /// Sichtbare Länge der Karte (ganz bzw. Kachel).
    fn length(&self) -> f32 {
        if self.tile_shown { TILE_HEIGHT } else { CARD_HEIGHT }
    }

    /// Lokale z-Lage zu einer Pixelzeile der Kartentextur (358 hoch), passend zur Form.
    fn px_z(&self, y: f32) -> f32 {
        if self.tile_shown { y / 358.0 * CARD_HEIGHT - TILE_HEIGHT * 0.5 } else { (y / 358.0 - 0.5) * CARD_HEIGHT }
    }

    /// Meshes, Textur-Zuschnitt, Kollision, Rahmen-Glow und Beschriftung an die Form anpassen.
    fn apply_shape(&mut self) {
        let len = self.length();
        let frac = len / CARD_HEIGHT;
        if let Some(r) = self.shape_refs.as_mut() {
            r.body.set_size(Vector3::new(CARD_WIDTH, CARD_THICKNESS, len));
            r.face.set_size(Vector2::new(CARD_WIDTH, len));
            r.back.set_size(Vector2::new(CARD_WIDTH, len));
            r.shape.set_size(Vector3::new(CARD_WIDTH, CARD_THICKNESS * 4.0, len));
            let quad = Vector2::new(CARD_WIDTH, len) + Vector2::splat(2.0 * GLOW_MARGIN);
            r.marker.set_size(quad);
        }
        for m in [self.face_material.as_mut(), self.back_material.as_mut()].into_iter().flatten() {
            m.set_uv1_scale(Vector3::new(1.0, frac, 1.0));
        }
        if let Some(m) = self.marker_material.as_mut() {
            let quad = Vector2::new(CARD_WIDTH, len) + Vector2::splat(2.0 * GLOW_MARGIN);
            m.set_shader_parameter("quad_size", &quad.to_variant());
            m.set_shader_parameter("card_half", &(Vector2::new(CARD_WIDTH, len) * 0.5).to_variant());
        }
        if let Some(b) = self.attack_banner.as_mut() {
            b.set_position(Vector3::new(0.0, CARD_THICKNESS, -len * 0.5 - BANNER_SIZE.y * 0.5 + 0.02));
        }
        let icon_z = if self.tile_shown { 0.0 } else { -CARD_HEIGHT * 0.12 };
        if let Some(i) = self.tap_icon.as_mut() {
            i.set_position(Vector3::new(0.0, CARD_THICKNESS * 0.5 + 0.003, icon_z));
        }
        self.refresh_labels();
    }

    /// Versatz des Schlags im Kampf (lokal) für dieses Frame.
    fn lunge_offset(&mut self, dt: f32) -> Vector3 {
        let Some((target, t, reach, delay)) = self.lunge.as_mut() else { return Vector3::ZERO };
        if *delay > 0.0 {
            *delay -= dt;
            return Vector3::ZERO;
        }
        *t += dt;
        let k = if *t < LUNGE_OUT {
            let u = *t / LUNGE_OUT;
            u * u // Ausholen beschleunigt
        } else if *t < LUNGE_OUT + LUNGE_BACK {
            let u = (*t - LUNGE_OUT) / LUNGE_BACK;
            1.0 - (1.0 - (1.0 - u).powi(3)) // zurück, weich auslaufend
        } else {
            self.lunge = None;
            return Vector3::ZERO;
        };
        let (target, reach) = (*target, *reach);
        let xf = self.base().get_global_transform();
        let local = xf.affine_inverse() * target;
        let mut off = local * (reach * k);
        off.y += LUNGE_LIFT * (std::f32::consts::PI * k).sin() / xf.basis.col_a().length().max(0.01);
        off
    }

    fn apply_glow(&mut self) {
        let flash = self.flash_t.map(|t| flash_gain(t));
        let active = self.marker != CardMarker::None || self.dragging || flash.is_some();
        let glow = if active {
            Some((ACTIVE_GLOW, MARK_STRENGTH * flash.unwrap_or(1.0)))
        } else {
            self.ready_glow.map(|c| (c, READY_STRENGTH))
        };
        if let (Some(mesh), Some(mat)) = (self.marker_mesh.as_mut(), self.marker_material.as_mut()) {
            mesh.set_visible(glow.is_some());
            if let Some((c, strength)) = glow {
                mat.set_shader_parameter("glow_color", &Vector3::new(c.r, c.g, c.b).to_variant());
                mat.set_shader_parameter("strength", &strength.to_variant());
                mat.set_shader_parameter("schein", &(if active { 0.45f32 } else { 0.32 }).to_variant());
            }
        }
    }

    /// Alle Texte der Kartenvorderseite in der aktuellen Sprache, mit Positionen in
    /// Pixeln der Kartentextur (256 × 358). Gemeinsame Quelle für die 3D-Beschriftung
    /// und 2D-Ansichten (z. B. die Stack-Anzeige im HUD).
    pub fn face_texts(&self) -> FaceTexts {
        let (stats, stats_color) = self.def.stats_text();
        let name = self.card_name.to_string();
        let (cost, cost_x, name_x, name_width) = if self.card_kind == CardKind::Land {
            (String::new(), 0.0, 128.0, 216.0)
        } else {
            // Generische Kosten als Zahl im grauen Kostensymbol (ganz links),
            // Name mittig im freien Bereich links der Kostensymbole
            let c = self.def.cost;
            let pips = c.specific.total() as usize + usize::from(c.generic > 0);
            let centers = placeholder::pip_centers(pips);
            let left_pip = centers.first().copied().unwrap_or(256.0 - 20.0);
            let cost = if c.generic > 0 { c.generic.to_string() } else { String::new() };
            (cost, centers.first().copied().unwrap_or(0.0), (26.0 + left_pip - 16.0) * 0.5, left_pip - 16.0 - 26.0)
        };
        FaceTexts {
            name,
            name_x,
            name_width,
            cost,
            cost_x,
            kind: self.def.type_line.clone(),
            body: self.def.display_text(),
            stats,
            stats_color,
        }
    }

    /// Plaketten über dem Bildfeld der Vorderseite (eine Zeile je Marke bzw. Kennzeichen,
    /// z. B. „+1/+1 ×2“, „Spielstein“); leer = keine.
    pub fn set_badges(&mut self, text: &str) {
        if self.badges == text {
            return;
        }
        self.badges = text.to_string();
        if let Some(l) = self.labels.as_mut() {
            l.badges.set_text(text);
        }
    }

    /// Vorderseiten-Textur (für 2D-Ansichten).
    pub fn face_tex(&self) -> Option<Gd<Texture2D>> {
        self.face_texture.clone()
    }

    pub(crate) fn refresh_labels(&mut self) {
        let t = self.face_texts();
        let tile = self.tile_shown;
        let zs = [219.0f32, TEXT_BOX_CENTER, 130.0, placeholder::title_center_y()].map(|y| self.px_z(y));
        let len = self.length();
        let Some(l) = self.labels.as_mut() else {
            return;
        };
        let px = |x: f32, z: f32| Vector3::new((x / 256.0 - 0.5) * CARD_WIDTH, CARD_THICKNESS * 0.5 + 0.0015, z);
        // Kachel: nur Name, Plaketten und Stärke/Widerstand (unten rechts); keine Kosten, Typzeile, Text
        l.kind.set_visible(!tile);
        l.text.set_visible(!tile);
        l.cost.set_visible(!tile && !t.cost.is_empty());
        l.kind.set_position(px(128.0, zs[0]));
        l.text.set_position(px(128.0, zs[1]));
        l.badges.set_position(px(128.0, zs[2]));
        l.stats.set_position(if tile { px(212.0, len * 0.5 - 0.045) } else { px(206.0, (333.0 / 358.0 - 0.5) * CARD_HEIGHT) });
        let ty = zs[3];
        // Schriftgrößen an die Textlänge anpassen. Umrechnung: 1 Schriftpunkt = PIXEL m,
        // 1 Texturpixel = CARD_WIDTH / 256 m.
        let per_px = (CARD_WIDTH / 256.0) / LABEL_PIXEL;
        l.name.set_text(&t.name);
        l.name.set_font_size(card_text::fit_line(&t.name, t.name_width * per_px, 16, 36));
        l.name.set_position(px(t.name_x, ty));
        l.kind.set_font_size(card_text::fit_line(&t.kind, 216.0 * per_px, 14, 34));
        // Textfeld: 0,52 m breit (Umbruch), ca. 90 Texturpixel hoch
        let area = (0.52 / LABEL_PIXEL) * (TEXT_BOX_HEIGHT * per_px);
        l.text.set_font_size(card_text::fit_font(&t.body, area, 10, 30));
        l.cost.set_text(&t.cost);
        if !t.cost.is_empty() {
            l.cost.set_position(px(t.cost_x, ty));
        }
        l.kind.set_text(&t.kind);
        l.text.set_text(&t.body);
        l.stats.set_text(&t.stats);
        l.colors[4] = t.stats_color;
        let tint = if self.tapped || self.attacked { TAPPED_TINT } else { Color::WHITE };
        l.stats.set_modulate(t.stats_color * tint);
    }

    fn build_labels(&mut self, visual: &mut Gd<Node3D>) {
        // Positionen passend zum Layout der Platzhalter-Textur (256 × 358 px).
        let px = |x: f32, y: f32| {
            Vector3::new((x / 256.0 - 0.5) * CARD_WIDTH, CARD_THICKNESS * 0.5 + 0.0015, (y / 358.0 - 0.5) * CARD_HEIGHT)
        };
        let light = Color::from_rgb(0.97, 0.93, 0.85);
        let dark = Color::from_rgb(0.18, 0.12, 0.08);
        let labels = CardLabels {
            name: card_label(visual, px(110.0, 36.0), 36, light, 0.0),
            cost: card_label(visual, px(218.0, 36.0), 34, dark, 0.0),
            kind: card_label(visual, px(128.0, 219.0), 34, light, 0.0),
            text: card_label(visual, px(128.0, TEXT_BOX_CENTER), 30, dark, 0.52),
            stats: card_label(visual, px(206.0, 333.0), 44, Color::WHITE, 0.0),
            badges: card_label(visual, px(128.0, 130.0), 34, Color::WHITE, 0.58),
            colors: [light, dark, light, dark, Color::WHITE, Color::WHITE],
        };
        let mut badges = labels.badges.clone();
        badges.set_outline_size(14);
        badges.set_outline_modulate(Color::from_rgba(0.05, 0.04, 0.03, 0.92));
        badges.set_text(&self.badges);
        self.labels = Some(labels);
        self.refresh_labels();
    }

    pub(crate) fn set_zone(&mut self, zone: Option<Gd<Zone>>) {
        self.zone = zone;
    }

    /// Globale Ziel-Transform des `Visual` für den Hover-Zoom,
    /// oder None, wenn nicht gezoomt werden soll (verdeckt, schon groß genug, …).
    fn zoom_transform(&self, card_xf: &Transform3D) -> Option<Transform3D> {
        if !self.zoom_enabled {
            return None;
        }
        let viewport = self.base().get_viewport()?;
        let cam = viewport.get_camera_3d()?;
        let cam_xf = cam.get_global_transform();
        if cam.is_position_behind(card_xf.origin) {
            return None;
        }
        // Nur offene Karten zoomen: Vorderseite (+Y) muss zur Kamera zeigen.
        if card_xf.basis.col_b().dot(cam_xf.origin - card_xf.origin) <= 0.0 {
            return None;
        }

        let screen = viewport.get_visible_rect().size;
        let target_px = screen.y * self.zoom_screen_height;

        // Ist die Karte schon so groß im Bild (z. B. Kamera-Nahaufnahme)? Dann nicht zoomen.
        let top = cam.unproject_position(*card_xf * Vector3::new(0.0, 0.0, -CARD_HEIGHT * 0.5));
        let bottom = cam.unproject_position(*card_xf * Vector3::new(0.0, 0.0, CARD_HEIGHT * 0.5));
        if top.distance_to(bottom) >= target_px * 0.9 {
            return None;
        }

        // Bildschirmposition so begrenzen, dass die große Karte vollständig sichtbar ist.
        let half = Vector2::new(target_px * CARD_WIDTH / CARD_HEIGHT, target_px) * 0.5
            + Vector2::splat(self.zoom_margin_px);
        let p = cam.unproject_position(card_xf.origin);
        let p = Vector2::new(
            p.x.clamp(half.x, (screen.x - half.x).max(half.x)),
            p.y.clamp(half.y, (screen.y - half.y).max(half.y)),
        );

        let d = self.zoom_distance;
        let pos = cam.project_position(p, d);
        let view_h = 2.0 * d * (cam.get_fov().to_radians() * 0.5).tan();
        let s = view_h * self.zoom_screen_height / CARD_HEIGHT;

        // Karte aufrecht zur Kamera: Kartennormale (+Y) -> zur Kamera (+Z der Kamera),
        // Kartenoberkante (-Z) -> Bildschirm oben (+Y der Kamera).
        let cb = cam_xf.basis.orthonormalized();
        let basis = Basis::from_cols(cb.col_a() * s, cb.col_c() * s, -cb.col_b() * s);
        Some(Transform3D::new(basis, pos))
    }

    fn build_nodes(&mut self) {
        // Kollision: nur für Raycasts, die Karte selbst überwacht nichts.
        self.base_mut().set_collision_layer(CARD_LAYER);
        self.base_mut().set_collision_mask(0);
        self.base_mut().set_monitoring(false);
        self.base_mut().set_monitorable(false);

        let mut shape = BoxShape3D::new_gd();
        shape.set_size(Vector3::new(CARD_WIDTH, CARD_THICKNESS * 4.0, CARD_HEIGHT));
        let shape_ref = shape.clone();
        let mut collision = CollisionShape3D::new_alloc();
        collision.set_shape(&shape);
        self.base_mut().add_child(&collision);

        let mut visual = Node3D::new_alloc();
        visual.set_name("Visual");

        // Kartenkörper
        let mut body_mesh = BoxMesh::new_gd();
        body_mesh.set_size(Vector3::new(CARD_WIDTH, CARD_THICKNESS, CARD_HEIGHT));
        let mut body_mat = StandardMaterial3D::new_gd();
        body_mat.set_albedo(Color::from_rgb(0.92, 0.9, 0.85));
        body_mat.set_roughness(0.8);
        body_mesh.set_material(&body_mat);
        let mut body = MeshInstance3D::new_alloc();
        body.set_name("Body");
        body.set_mesh(&body_mesh);
        visual.add_child(&body);

        // Vorderseite (+Y)
        let mut face_mat = StandardMaterial3D::new_gd();
        face_mat.set_roughness(0.55);
        face_mat.set_texture_filter(TextureFilter::LINEAR_WITH_MIPMAPS_ANISOTROPIC);
        apply_texture(&mut face_mat, self.face_texture.as_ref(), Color::from_rgb(0.9, 0.9, 0.9));
        let mut face_mesh = PlaneMesh::new_gd();
        face_mesh.set_size(Vector2::new(CARD_WIDTH, CARD_HEIGHT));
        face_mesh.set_material(&face_mat);
        let mut face = MeshInstance3D::new_alloc();
        face.set_name("Face");
        face.set_mesh(&face_mesh);
        face.set_position(Vector3::new(0.0, CARD_THICKNESS * 0.5 + 0.0005, 0.0));
        visual.add_child(&face);

        // Rückseite (-Y)
        let mut back_mat = StandardMaterial3D::new_gd();
        back_mat.set_roughness(0.55);
        back_mat.set_texture_filter(TextureFilter::LINEAR_WITH_MIPMAPS_ANISOTROPIC);
        apply_texture(&mut back_mat, self.back_texture.as_ref(), Color::from_rgb(0.15, 0.2, 0.45));
        let mut back_mesh = PlaneMesh::new_gd();
        back_mesh.set_size(Vector2::new(CARD_WIDTH, CARD_HEIGHT));
        back_mesh.set_flip_faces(true);
        back_mesh.set_material(&back_mat);
        let mut back = MeshInstance3D::new_alloc();
        back.set_name("Back");
        back.set_mesh(&back_mesh);
        back.set_position(Vector3::new(0.0, -(CARD_THICKNESS * 0.5 + 0.0005), 0.0));
        visual.add_child(&back);

        self.base_mut().add_child(&visual);
        self.visual = Some(visual.clone());
        // Kampfmarkierung: flackernde Aura rund um die Karte (sonst unsichtbar)
        let mut glow_shader = Shader::new_gd();
        glow_shader.set_code(GLOW_SHADER);
        let mut marker_mat = ShaderMaterial::new_gd();
        marker_mat.set_shader(&glow_shader);
        let quad = Vector2::new(CARD_WIDTH, CARD_HEIGHT) + Vector2::splat(2.0 * GLOW_MARGIN);
        marker_mat.set_shader_parameter("quad_size", &quad.to_variant());
        marker_mat.set_shader_parameter("card_half", &(Vector2::new(CARD_WIDTH, CARD_HEIGHT) * 0.5).to_variant());
        let mut marker_mesh = PlaneMesh::new_gd();
        marker_mesh.set_size(quad);
        marker_mesh.set_material(&marker_mat);
        let mut marker = MeshInstance3D::new_alloc();
        marker.set_name("Marker");
        marker.set_mesh(&marker_mesh);
        marker.set_cast_shadows_setting(ShadowCastingSetting::OFF);
        marker.set_visible(false);
        // Knapp unter der Karte: die Karte verdeckt die Mitte, nur der Schein ragt heraus
        marker.set_position(Vector3::new(0.0, -CARD_THICKNESS, 0.0));
        visual.add_child(&marker);
        self.marker_mesh = Some(marker);
        self.marker_material = Some(marker_mat);

        // Angriffsspitze: rote Dachform über der Oberkante (zeigt zum Ziel), nur bei Angreifern
        let mut banner_shader = Shader::new_gd();
        banner_shader.set_code(BANNER_SHADER);
        let mut banner_mat = ShaderMaterial::new_gd();
        banner_mat.set_shader(&banner_shader);
        let mut banner_mesh = PlaneMesh::new_gd();
        banner_mesh.set_size(BANNER_SIZE);
        banner_mesh.set_material(&banner_mat);
        let mut banner = MeshInstance3D::new_alloc();
        banner.set_name("AttackBanner");
        banner.set_mesh(&banner_mesh);
        banner.set_cast_shadows_setting(ShadowCastingSetting::OFF);
        banner.set_position(Vector3::new(0.0, CARD_THICKNESS, -CARD_HEIGHT * 0.5 - BANNER_SIZE.y * 0.5 + 0.02));
        banner.set_visible(self.advanced);
        visual.add_child(&banner);
        self.attack_banner = Some(banner);

        // Tapp-Symbol: knapp über der Vorderseite, mittig im Bildfeld, nur wenn getappt sichtbar
        let mut icon_mat = StandardMaterial3D::new_gd();
        icon_mat.set_shading_mode(ShadingMode::UNSHADED);
        icon_mat.set_transparency(godot::classes::base_material_3d::Transparency::ALPHA);
        icon_mat.set_texture(TextureParam::ALBEDO, &super::textures::tap_icon());
        icon_mat.set_texture_filter(TextureFilter::LINEAR_WITH_MIPMAPS);
        let mut icon_mesh = PlaneMesh::new_gd();
        icon_mesh.set_size(Vector2::splat(TAP_ICON_SIZE));
        icon_mesh.set_material(&icon_mat);
        let mut icon = MeshInstance3D::new_alloc();
        icon.set_name("TapIcon");
        icon.set_mesh(&icon_mesh);
        icon.set_cast_shadows_setting(ShadowCastingSetting::OFF);
        icon.set_position(Vector3::new(0.0, CARD_THICKNESS * 0.5 + 0.003, -CARD_HEIGHT * 0.12));
        icon.set_visible(false);
        visual.add_child(&icon);
        self.tap_icon = Some(icon);

        self.build_labels(&mut visual);
        self.shape_refs = Some(ShapeRefs { body: body_mesh, face: face_mesh, back: back_mesh, shape: shape_ref, marker: marker_mesh });
        self.meshes = vec![body, face, back];
        self.face_material = Some(face_mat);
        self.back_material = Some(back_mat);
        self.apply_tapped_look();
        if self.tile_shown {
            self.apply_shape();
        }
        self.apply_brightness();
    }
}

fn apply_texture(mat: &mut Gd<StandardMaterial3D>, tex: Option<&Gd<Texture2D>>, fallback: Color) {
    match tex {
        Some(tex) => {
            mat.set_texture(TextureParam::ALBEDO, tex);
            mat.set_albedo(Color::WHITE);
        }
        None => {
            mat.set_texture(TextureParam::ALBEDO, Gd::<Texture2D>::null_arg());
            mat.set_albedo(fallback);
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum CardMarker {
    #[default]
    None,
    Attacking,
    Blocking,
    Selected,
}

/// Beschriftung auf der Vorderseite (Label3D, liegen flach auf der Karte).
/// Formteile, die sich mit der Kachel-Form ändern.
struct ShapeRefs {
    body: Gd<BoxMesh>,
    face: Gd<PlaneMesh>,
    back: Gd<PlaneMesh>,
    shape: Gd<BoxShape3D>,
    marker: Gd<PlaneMesh>,
}

struct CardLabels {
    name: Gd<Label3D>,
    cost: Gd<Label3D>,
    kind: Gd<Label3D>,
    text: Gd<Label3D>,
    stats: Gd<Label3D>,
    /// Plaketten über dem Bildfeld: Marken, „Spielstein“ (siehe [`Card3D::set_badges`]).
    badges: Gd<Label3D>,
    /// Grundfarbe je Beschriftung (Reihenfolge wie [`CardLabels::all`]); das Ausgrauen
    /// getappter Karten multipliziert sie.
    colors: [Color; 6],
}

impl CardLabels {
    fn all(&mut self) -> [&mut Gd<Label3D>; 6] {
        [&mut self.name, &mut self.cost, &mut self.kind, &mut self.text, &mut self.stats, &mut self.badges]
    }
}

/// Textfeld für den Regeltext: Mitte und Höhe in Pixeln der Kartentextur (unten bleibt das
/// Wertefeld frei).
const TEXT_BOX_CENTER: f32 = 276.0;
const TEXT_BOX_HEIGHT: f32 = 72.0;

/// Größe eines Schriftpunkts der Kartenbeschriftung (m).
const LABEL_PIXEL: f32 = 0.0008;

fn card_label(parent: &mut Gd<Node3D>, pos: Vector3, font_size: i32, color: Color, wrap_width: f32) -> Gd<Label3D> {
    const PIXEL: f32 = LABEL_PIXEL;
    let mut l = Label3D::new_alloc();
    l.set_pixel_size(PIXEL);
    l.set_font_size(font_size);
    l.set_modulate(color);
    l.set_outline_size(0);
    l.set_horizontal_alignment(HorizontalAlignment::CENTER);
    l.set_draw_flag(DrawFlags::DOUBLE_SIDED, false);
    l.set_draw_flag(DrawFlags::SHADED, true);
    if wrap_width > 0.0 {
        l.set_autowrap_mode(godot::classes::text_server::AutowrapMode::WORD_SMART);
        l.set_width(wrap_width / PIXEL);
    }
    // Flach auf die Vorderseite legen: Label schaut nach +Y, Textoberkante nach -Z.
    l.set_rotation(Vector3::new(-std::f32::consts::FRAC_PI_2, 0.0, 0.0));
    l.set_position(pos);
    l.set_cast_shadows_setting(ShadowCastingSetting::OFF);
    parent.add_child(&l);
    l
}

/// Texte der Kartenvorderseite (siehe [`Card3D::face_texts`]). Positionen `*_x` in
/// Pixeln der Kartentextur (256 breit); die y-Positionen sind für alle Karten gleich.
pub struct FaceTexts {
    pub name: String,
    pub name_x: f32,
    /// Freie Breite für den Namen (Pixel der Kartentextur).
    pub name_width: f32,
    /// Generische Kosten als Zahl ("" = keine), Mitte des grauen Kostensymbols bei `cost_x`.
    pub cost: String,
    pub cost_x: f32,
    pub kind: String,
    pub body: String,
    pub stats: String,
    pub stats_color: Color,
}

/// Verstärkung des Aufleuchtens `t` Sekunden nach [`Card3D::flash`].
fn flash_gain(t: f32) -> f32 {
    for w in FLASH_KEYS.windows(2) {
        let ((t0, v0), (t1, v1)) = (w[0], w[1]);
        if t <= t1 {
            let u = ((t - t0) / (t1 - t0)).clamp(0.0, 1.0);
            return v0 + (v1 - v0) * u * u * (3.0 - 2.0 * u);
        }
    }
    1.0
}
