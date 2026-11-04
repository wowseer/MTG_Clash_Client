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

use crate::game::ability::CardAbility;
use crate::game::cards::{ActivatedAbility, AuraDef, CardDef, CardKind, EquipStats, HeroAbility, SpellEffect};
use crate::game::mana::{ManaCost, ManaType};
use super::placeholder;
use super::layers::CARD_LAYER;
use super::zone::Zone;

pub const CARD_WIDTH: f32 = 0.63;
pub const CARD_HEIGHT: f32 = 0.88;
pub const CARD_THICKNESS: f32 = 0.006;

/// Farben des Lichtsaums – dezent = „kann“, kräftig = Markierung:
/// Rot = kann angreifen / greift an, Blau = möglicher Blocker / blockt,
/// Gold = kann eine Fähigkeit aktivieren / ausgewählt (wählt ein Ziel).
pub const ATTACK_GLOW: Color = Color::from_rgb(1.0, 0.22, 0.15);
pub const BLOCK_GLOW: Color = Color::from_rgb(0.3, 0.65, 1.0);
pub const SELECT_GLOW: Color = Color::from_rgb(1.0, 0.85, 0.35);
/// Gold: Karte kann eine Fähigkeit aktivieren (Held, Artefakt mit Tapp-Fähigkeit, Ausrüstung).
pub const ABILITY_GLOW: Color = Color::from_rgb(1.0, 0.75, 0.28);
/// Helligkeit des Leuchtens „kann handeln“ und der Markierungen (Angreifer/Blocker/Auswahl);
/// höhere Stärke = heller und etwas breiterer Schein.
const READY_STRENGTH: f32 = 0.7;
const MARK_STRENGTH: f32 = 1.5;
/// Farbe, mit der getappte Karten (und Kreaturen, die angegriffen haben) ausgegraut werden.
const TAPPED_TINT: Color = Color::from_rgb(0.52, 0.52, 0.55);
/// Kantenlänge des Tapp-Symbols auf der Karte (Einheiten; Karte = 0,63 breit).
const TAP_ICON_SIZE: f32 = 0.34;
/// Wie weit die Aura über den Kartenrand hinausragt (Einheiten).
const GLOW_MARGIN: f32 = 0.16;

/// Aura-Shader: schmaler, heller Rand direkt an der Kartenkante mit weichem, kurzem Schein
/// nach außen und ruhigem „Atmen“ – wie ein Lichtsaum unter der Karte. Additiv, ohne Licht;
/// `strength` skaliert Helligkeit und Breite (dezent = Karte kann handeln, kräftig = Angreifer/Blocker).
const GLOW_SHADER: &str = r#"
shader_type spatial;
render_mode unshaded, blend_add, cull_disabled, depth_draw_never, shadows_disabled;
uniform vec4 glow_color : source_color = vec4(1.0, 0.5, 0.12, 1.0);
uniform float strength = 1.0;
uniform vec2 quad_size;
uniform vec2 card_half;

float sd_round_rect(vec2 p, vec2 b, float r) {
    vec2 q = abs(p) - b + vec2(r);
    return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r;
}

void fragment() {
    vec2 p = (UV - 0.5) * quad_size;
    float d = sd_round_rect(p, card_half, 0.03);
    float halo = exp(-max(d, 0.0) / (0.05 + 0.03 * strength)) * 0.6;
    float rim = smoothstep(0.022, 0.0, abs(d - 0.006));
    float breathe = 0.88 + 0.12 * sin(TIME * 2.2);
    // Vor dem Rand der Fläche weich auf 0 auslaufen (keine sichtbare Rechteckkante)
    float margin = quad_size.x * 0.5 - card_half.x;
    float a = (halo + rim * 0.75) * strength * breathe * smoothstep(-0.01, 0.0, d) * smoothstep(margin, margin * 0.45, d);
    ALBEDO = mix(glow_color.rgb, vec3(1.0), rim * 0.35);
    ALPHA = clamp(a, 0.0, 1.0);
}
"#;

/// Eine Spielkarte auf dem 3D-Tisch: Mesh mit Vorder-/Rückseite und Beschriftung,
/// Hover-Neigung und Hover-Zoom (offene Karten werden groß und lesbar), Raycast-Ziel
/// (Kollisions-Layer 1), Gleit-Animationen per Tween sowie alle Spielwerte
/// (Typ, Kosten, Angriff/Verteidigung, Fähigkeiten, Tapp-Zustand).
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
    /// Offene Karten beim Hovern groß und lesbar anzeigen.
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

    /// Kartenname.
    #[export_group(name = "Spielwerte")]
    #[export]
    card_name: GString,
    /// Kartentyp: 0 = Kreatur, 1 = Ressource, 2 = Sofortzauber, 3 = Hexerei, 4 = Held, 5 = Ausrüstung, 6 = Artefakt, 7 = Verzauberung, 8 = Aura.
    #[export]
    card_kind: CardKind,
    /// Manakosten (leer = kostenlos).
    #[export]
    mana_cost: Option<Gd<ManaCost>>,
    /// Nur Ressourcen: Farbe des Manas, das Tappen erzeugt.
    #[export]
    produces: ManaType,
    /// Grundangriff (ohne Boni durch Ausrüstung, Auren, Verzauberungen).
    #[export]
    attack: i32,
    /// Grundverteidigung (ohne Boni und ohne erlittenen Schaden).
    #[export]
    defense: i32,
    /// Fähigkeit einer Kreatur: 0 = keine, 1 = Heiler, 2 = Feuer, 3 = Raufbold.
    #[export]
    ability: CardAbility,

    // Laufzeit-Zustand (Regeln)
    tapped: bool,
    damage: i32,
    attack_bonus: i32,
    marker: CardMarker,
    /// Dezentes Leuchten in dieser Farbe: Karte kann gerade handeln (None = aus).
    ready_glow: Option<Color>,
    /// Hat in diesem Zug angegriffen (ausgegraut, bis sie enttappt wird).
    attacked: bool,

    /// Sofortzauber / Hexerei: Effekt beim Auflösen.
    pub(crate) effect: SpellEffect,
    /// Held: Fähigkeiten und aktuelle Willenskraft.
    pub(crate) hero_abilities: Vec<HeroAbility>,
    pub(crate) willpower: i32,
    /// Ausrüstung: Werte und Kreatur, an der sie angelegt ist.
    pub(crate) equip: Option<EquipStats>,
    pub(crate) equipped_to: Option<Gd<Card3D>>,
    /// Kreatur: Summe der Boni aller angelegten Ausrüstungen und Auren.
    equip_attack: i32,
    equip_defense: i32,
    /// Kreatur: Summe der globalen Boni (Artefakte/Verzauberungen), laufend neu berechnet.
    global_attack: i32,
    global_defense: i32,
    /// Artefakt: Tapp-Fähigkeit.
    pub(crate) activated: Option<ActivatedAbility>,
    /// Aura: Zielklasse/Wirkung und verzauberte Karte.
    pub(crate) aura: Option<AuraDef>,
    pub(crate) enchanted: Option<Gd<Card3D>>,
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
            zoom_enabled: true,
            zoom_screen_height: 0.55,
            zoom_distance: 2.0,
            zoom_margin_px: 12.0,
            hover_lift: 0.06,
            hover_scale: 1.08,
            card_name: "Karte".into(),
            card_kind: CardKind::Creature,
            mana_cost: None,
            produces: ManaType::Colorless,
            attack: 1,
            defense: 1,
            ability: CardAbility::None,
            tapped: false,
            damage: 0,
            attack_bonus: 0,
            marker: CardMarker::None,
            ready_glow: None,
            attacked: false,
            effect: SpellEffect::None,
            hero_abilities: Vec::new(),
            willpower: 0,
            equip: None,
            equipped_to: None,
            equip_attack: 0,
            equip_defense: 0,
            global_attack: 0,
            global_defense: 0,
            activated: None,
            aura: None,
            enchanted: None,
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
            // Die schwebende Karte soll keinen riesigen Schatten auf den Tisch werfen.
            let setting = if zoomed { ShadowCastingSetting::OFF } else { ShadowCastingSetting::ON };
            for mesh in self.meshes.iter_mut() {
                mesh.set_cast_shadows_setting(setting);
            }
            // Vor der Kamera liegt die Karte außerhalb der Tischbeleuchtung:
            // gezoomt unbeleuchtet in Originalfarben anzeigen, damit sie lesbar ist.
            let shading = if zoomed { ShadingMode::UNSHADED } else { ShadingMode::PER_PIXEL };
            if let Some(mat) = self.face_material.as_mut() {
                mat.set_shading_mode(shading);
            }
            if let Some(labels) = self.labels.as_mut() {
                for l in labels.all() {
                    l.set_draw_flag(DrawFlags::SHADED, !zoomed);
                }
            }
        }

        let w = 1.0 - (-self.tilt_speed * delta as f32).exp();
        let visual = self.visual.as_mut().unwrap();
        let next = visual.get_transform().interpolate_with(&target, w);
        visual.set_transform(next);
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

    /// Angriff inklusive temporärer Boni.
    #[func]
    pub fn get_current_attack(&self) -> i32 {
        self.attack + self.attack_bonus + self.equip_attack + self.global_attack
    }

    /// Verteidigung abzüglich erlittenen Schadens in dieser Runde.
    #[func]
    pub fn get_remaining_defense(&self) -> i32 {
        self.defense + self.equip_defense + self.global_defense - self.damage
    }

    /// Dezenter goldener Rand „diese Karte kann eine Fähigkeit aktivieren“ an/aus (für Skripte;
    /// Rot/Blau setzt das Spiel selbst). Eine Markierung hat Vorrang.
    #[func]
    pub fn set_ready_glow(&mut self, on: bool) {
        self.set_ready_color(on.then_some(ABILITY_GLOW));
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

    /// Artefakt (inkl. Ausrüstung)?
    #[func]
    pub fn is_artifact(&self) -> bool {
        self.card_kind.is_artifact()
    }

    /// Verzauberung (inkl. Aura)?
    #[func]
    pub fn is_enchantment(&self) -> bool {
        self.card_kind.is_enchantment()
    }

    /// Kreatur, die diese Aura verzaubert (null = keine).
    #[func]
    pub fn get_enchanted(&self) -> Option<Gd<Card3D>> {
        self.enchanted.clone()
    }

    /// Aktuelle Willenskraft (nur Helden).
    #[func]
    pub fn get_willpower(&self) -> i32 {
        self.willpower
    }

    /// Kreatur, an der diese Ausrüstung angelegt ist (null = frei).
    #[func]
    pub fn get_equipped_to(&self) -> Option<Gd<Card3D>> {
        self.equipped_to.clone()
    }

    /// Die Zone, in der die Karte aktuell liegt (None während des Ziehens).
    #[func]
    pub fn get_zone(&self) -> Option<Gd<Zone>> {
        self.zone.clone()
    }
}

impl Card3D {
    // ------------------------------------------------------------ Spielwerte

    /// Übernimmt alle Spielwerte aus einer Kartendefinition.
    pub fn apply_def(&mut self, def: &CardDef) {
        self.card_name = def.name.as_str().into();
        self.card_kind = def.kind;
        self.mana_cost = Some(ManaCost::from_cost(def.cost));
        self.produces = def.produces;
        self.attack = def.attack;
        self.defense = def.defense;
        self.ability = def.ability;
        self.effect = def.effect;
        self.hero_abilities = def.hero_abilities.clone();
        self.willpower = def.willpower;
        self.equip = def.equip;
        self.activated = def.activated;
        self.aura = def.aura;
        self.refresh_labels();
    }

    /// Name der Karte (wie auf dem Kartenbild, ohne Übersetzung).
    pub fn card_name(&self) -> GString {
        self.card_name.clone()
    }

    pub fn kind(&self) -> CardKind {
        self.card_kind
    }

    pub fn produced_mana(&self) -> ManaType {
        self.produces
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
            for l in labels.all() {
                l.set_modulate(tint);
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

    fn apply_glow(&mut self) {
        let glow = match self.marker {
            CardMarker::Attacking => Some((ATTACK_GLOW, MARK_STRENGTH)),
            CardMarker::Blocking => Some((BLOCK_GLOW, MARK_STRENGTH)),
            CardMarker::Selected => Some((SELECT_GLOW, MARK_STRENGTH)),
            CardMarker::None => self.ready_glow.map(|c| (c, READY_STRENGTH)),
        };
        if let (Some(mesh), Some(mat)) = (self.marker_mesh.as_mut(), self.marker_material.as_mut()) {
            mesh.set_visible(glow.is_some());
            if let Some((c, strength)) = glow {
                mat.set_shader_parameter("glow_color", &c.to_variant());
                mat.set_shader_parameter("strength", &strength.to_variant());
            }
        }
    }

    /// Alle Texte der Kartenvorderseite in der aktuellen Sprache, mit Positionen in
    /// Pixeln der Kartentextur (256 × 358). Gemeinsame Quelle für die 3D-Beschriftung
    /// und 2D-Ansichten (z. B. die Stack-Anzeige im HUD).
    pub fn face_texts(&self) -> FaceTexts {
        let atk = self.get_current_attack();
        let def = self.get_remaining_defense();
        let bonus = self.attack_bonus != 0
            || self.equip_attack != 0
            || self.equip_defense != 0
            || self.global_attack != 0
            || self.global_defense != 0;
        let damaged = self.damage > 0;
        let (kind, body, stats) = match self.card_kind {
            CardKind::Creature => (self.ability.title().to_string(), self.ability.description().to_string(), format!("{atk}/{def}")),
            CardKind::Instant | CardKind::Sorcery => (self.card_kind.title().to_string(), self.effect.description(), String::new()),
            CardKind::Hero => (
                tr!("Held", "Hero").to_string(),
                self.hero_abilities.iter().map(|a| a.description()).collect::<Vec<_>>().join("\n"),
                format!("WK {}", self.willpower),
            ),
            CardKind::Equipment => {
                let e = self.equip.unwrap_or_default();
                (self.card_kind.type_line().to_string(), e.description(), format!("+{}/+{}", e.attack, e.defense))
            }
            CardKind::Artifact | CardKind::Enchantment => {
                let mut parts: Vec<String> = Vec::new();
                if let Some(a) = self.activated {
                    parts.push(a.description());
                }
                (self.card_kind.type_line().to_string(), parts.join("\n"), String::new())
            }
            CardKind::Aura => match self.aura {
                Some(a) => (self.card_kind.type_line().to_string(), a.description(), a.stats_text()),
                None => (self.card_kind.type_line().to_string(), String::new(), String::new()),
            },
            CardKind::Resource => {
                let color = self.produces.name();
                (
                    trf!("Ressource – {color}", "Resource – {color}"),
                    trf!("Tappen: +1 {color}-Mana.", "Tap: +1 {color} mana."),
                    String::new(),
                )
            }
        };
        let name = crate::i18n::name(&self.card_name.to_string());
        let (cost, cost_x, name_x) = if self.card_kind == CardKind::Resource {
            (String::new(), 0.0, 128.0)
        } else {
            // Generische Kosten als Zahl im grauen Kostensymbol (ganz links),
            // Name mittig im freien Bereich links der Kostensymbole
            let c = self.mana_cost.as_ref().map(|c| c.bind().cost()).unwrap_or_default();
            let pips = c.specific.total() as usize + usize::from(c.generic > 0);
            let centers = placeholder::pip_centers(pips);
            let left_pip = centers.first().copied().unwrap_or(256.0 - 20.0);
            let cost = if c.generic > 0 { c.generic.to_string() } else { String::new() };
            (cost, centers.first().copied().unwrap_or(0.0), (26.0 + left_pip - 16.0) * 0.5)
        };
        let stats_color = if damaged {
            Color::from_rgb(1.0, 0.45, 0.4)
        } else if bonus {
            Color::from_rgb(0.6, 1.0, 0.5)
        } else {
            Color::WHITE
        };
        FaceTexts { name, name_x, cost, cost_x, kind, body, stats, stats_color }
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
        let Some(l) = self.labels.as_mut() else {
            return;
        };
        let px = |x: f32, y: f32| {
            Vector3::new((x / 256.0 - 0.5) * CARD_WIDTH, CARD_THICKNESS * 0.5 + 0.0015, (y / 358.0 - 0.5) * CARD_HEIGHT)
        };
        let ty = placeholder::title_center_y();
        l.name.set_text(&t.name);
        l.name.set_position(px(t.name_x, ty));
        l.cost.set_text(&t.cost);
        if !t.cost.is_empty() {
            l.cost.set_position(px(t.cost_x, ty));
        }
        l.kind.set_text(&t.kind);
        l.text.set_text(&t.body);
        l.stats.set_text(&t.stats);
        l.stats.set_modulate(t.stats_color);
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
            text: card_label(visual, px(128.0, 274.0), 30, dark, 0.52),
            stats: card_label(visual, px(206.0, 333.0), 44, Color::WHITE, 0.0),
            badges: card_label(visual, px(128.0, 130.0), 34, Color::WHITE, 0.58),
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
        self.meshes = vec![body, face, back];
        self.face_material = Some(face_mat);
        self.back_material = Some(back_mat);
        self.apply_tapped_look();
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
struct CardLabels {
    name: Gd<Label3D>,
    cost: Gd<Label3D>,
    kind: Gd<Label3D>,
    text: Gd<Label3D>,
    stats: Gd<Label3D>,
    /// Plaketten über dem Bildfeld: Marken, „Spielstein“ (siehe [`Card3D::set_badges`]).
    badges: Gd<Label3D>,
}

impl CardLabels {
    fn all(&mut self) -> [&mut Gd<Label3D>; 6] {
        [&mut self.name, &mut self.cost, &mut self.kind, &mut self.text, &mut self.stats, &mut self.badges]
    }
}

fn card_label(parent: &mut Gd<Node3D>, pos: Vector3, font_size: i32, color: Color, wrap_width: f32) -> Gd<Label3D> {
    const PIXEL: f32 = 0.0008;
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
    /// Generische Kosten als Zahl ("" = keine), Mitte des grauen Kostensymbols bei `cost_x`.
    pub cost: String,
    pub cost_x: f32,
    pub kind: String,
    pub body: String,
    pub stats: String,
    pub stats_color: Color,
}
