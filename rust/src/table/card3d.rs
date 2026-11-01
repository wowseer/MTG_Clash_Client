//! Card3D: eine einzelne Karte auf dem 3D-Tisch.
//!
//! Aufbau (wird in `ready()` per Code erzeugt):
//! ```text
//! Card3D (Area3D, Kollisions-Layer CARD_LAYER)   <- wird per Tween bewegt
//! ├── CollisionShape3D (BoxShape3D)                <- Raycast-Ziel, bleibt ruhig
//! └── Visual (Node3D)                              <- Tilt / Hover-Zoom / Drag-Schwung
//!     ├── Body  (BoxMesh, dünner Kartenkörper)
//!     ├── Face  (PlaneMesh, +Y, Vorderseiten-Textur)
//!     └── Back  (PlaneMesh, -Y, Rückseiten-Textur)
//! ```
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
    StandardMaterial3D, Texture2D, Tween,
};
use godot::prelude::*;

use godot::classes::label_3d::DrawFlags;
use godot::classes::Label3D;
use godot::global::HorizontalAlignment;

use crate::game::ability::CardAbility;
use crate::game::cards::{ActivatedAbility, AuraDef, CardDef, CardKind, EquipStats, HeroAbility, SpellEffect, StaticEffect};
use crate::game::mana::{Cost, ManaCost, ManaType};
use super::placeholder;
use super::layers::CARD_LAYER;
use super::zone::Zone;

pub const CARD_WIDTH: f32 = 0.63;
pub const CARD_HEIGHT: f32 = 0.88;
pub const CARD_THICKNESS: f32 = 0.006;

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

    /// Sofortzauber / Hexerei: Effekt beim Auflösen.
    pub(crate) effect: SpellEffect,
    /// Held: Fähigkeiten, aktuelle Willenskraft, diese Runde schon aktiviert?
    pub(crate) hero_abilities: Vec<HeroAbility>,
    pub(crate) willpower: i32,
    pub(crate) hero_used: bool,
    /// Ausrüstung: Werte und Kreatur, an der sie angelegt ist.
    pub(crate) equip: Option<EquipStats>,
    pub(crate) equipped_to: Option<Gd<Card3D>>,
    /// Kreatur: Summe der Boni aller angelegten Ausrüstungen und Auren.
    equip_attack: i32,
    equip_defense: i32,
    /// Kreatur: Summe der globalen Boni (Artefakte/Verzauberungen), laufend neu berechnet.
    global_attack: i32,
    global_defense: i32,
    /// Artefakt / Verzauberung: dauerhafter Effekt.
    pub(crate) static_effect: Option<StaticEffect>,
    /// Artefakt: Tapp-Fähigkeit.
    pub(crate) activated: Option<ActivatedAbility>,
    /// Aura: Zielklasse/Wirkung und verzauberte Karte.
    pub(crate) aura: Option<AuraDef>,
    pub(crate) enchanted: Option<Gd<Card3D>>,
    /// Besitzer (0 = Spieler, 1 = Gegner): auf dessen Friedhof geht die Karte.
    #[var]
    pub(crate) owner_seat: i64,

    labels: Option<CardLabels>,
    marker_mesh: Option<Gd<MeshInstance3D>>,
    marker_material: Option<Gd<StandardMaterial3D>>,
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
            effect: SpellEffect::None,
            hero_abilities: Vec::new(),
            willpower: 0,
            hero_used: false,
            equip: None,
            equipped_to: None,
            equip_attack: 0,
            equip_defense: 0,
            global_attack: 0,
            global_defense: 0,
            static_effect: None,
            activated: None,
            aura: None,
            enchanted: None,
            owner_seat: 0,
            labels: None,
            marker_mesh: None,
            marker_material: None,
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

    /// Ist die Karte getappt (um 90° gedreht)?
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
        self.static_effect = def.static_effect;
        self.activated = def.activated;
        self.aura = def.aura;
        self.refresh_labels();
    }

    pub fn kind(&self) -> CardKind {
        self.card_kind
    }

    pub fn is_creature(&self) -> bool {
        self.card_kind == CardKind::Creature
    }

    /// Ausrüstungsbonus auf diese Kreatur anwenden (negativ = entfernen).
    pub fn add_equip_bonus(&mut self, attack: i32, defense: i32) {
        self.equip_attack += attack;
        self.equip_defense += defense;
        self.refresh_labels();
    }

    /// Bleibende Boni/Verbindungen zurücksetzen (Karte verlässt das Spielfeld).
    pub fn reset_on_leave(&mut self) {
        self.equip_attack = 0;
        self.equip_defense = 0;
        self.global_attack = 0;
        self.global_defense = 0;
        self.equipped_to = None;
        self.enchanted = None;
        self.hero_used = false;
        self.refresh_labels();
    }

    /// Globale Boni setzen (nur bei Änderung neu beschriften).
    pub fn set_global_bonus(&mut self, attack: i32, defense: i32) {
        if (self.global_attack, self.global_defense) != (attack, defense) {
            self.global_attack = attack;
            self.global_defense = defense;
            self.refresh_labels();
        }
    }

    /// Willenskraft eines Helden ändern; gibt den neuen Wert zurück.
    pub fn change_willpower(&mut self, delta: i32) -> i32 {
        self.willpower += delta;
        self.refresh_labels();
        self.willpower
    }

    /// Kartenname als Rust-String (für Protokolltexte).
    pub fn display_name(&self) -> String {
        self.card_name.to_string()
    }

    /// Manakosten als Rust-Wert (ohne Resource = kostenlos).
    pub fn cost(&self) -> Cost {
        self.mana_cost.as_ref().map(|c| c.bind().cost()).unwrap_or_default()
    }

    pub fn is_resource(&self) -> bool {
        self.card_kind == CardKind::Resource
    }

    pub fn produced_mana(&self) -> ManaType {
        self.produces
    }

    pub fn ability_kind(&self) -> CardAbility {
        self.ability
    }

    /// Tapp-Zustand setzen; die Drehung übernimmt die Zone beim nächsten `layout()`.
    pub fn set_tapped(&mut self, tapped: bool) {
        self.tapped = tapped;
    }

    pub fn add_damage(&mut self, amount: i32) {
        self.damage += amount;
        self.refresh_labels();
    }

    pub fn add_attack_bonus(&mut self, amount: i32) {
        self.attack_bonus += amount;
        self.refresh_labels();
    }

    /// Schaden und Markierungen der Runde entfernen (End-Phase).
    pub fn clear_turn_effects(&mut self) {
        self.damage = 0;
        self.set_marker(CardMarker::None);
        self.refresh_labels();
    }

    pub fn set_marker(&mut self, marker: CardMarker) {
        self.marker = marker;
        let color = match marker {
            CardMarker::None => None,
            CardMarker::Attacking => Some(Color::from_rgb(1.0, 0.25, 0.15)),
            CardMarker::Blocking => Some(Color::from_rgb(0.3, 0.6, 1.0)),
            CardMarker::Selected => Some(Color::from_rgb(1.0, 0.85, 0.3)),
        };
        if let (Some(mesh), Some(mat)) = (self.marker_mesh.as_mut(), self.marker_material.as_mut()) {
            mesh.set_visible(color.is_some());
            if let Some(c) = color {
                mat.set_albedo(c);
                mat.set_emission(c);
            }
        }
    }

    pub(crate) fn refresh_labels(&mut self) {
        let atk = self.get_current_attack();
        let def = self.get_remaining_defense();
        let bonus = self.attack_bonus != 0
            || self.equip_attack != 0
            || self.equip_defense != 0
            || self.global_attack != 0
            || self.global_defense != 0;
        let damaged = self.damage > 0;
        let (kind_text, body, stats) = match self.card_kind {
            CardKind::Creature => (self.ability.title().to_string(), self.ability.description().to_string(), format!("{atk}/{def}")),
            CardKind::Instant | CardKind::Sorcery => (self.card_kind.title().to_string(), self.effect.description(), String::new()),
            CardKind::Hero => (
                "Held".to_string(),
                self.hero_abilities.iter().map(|a| a.description()).collect::<Vec<_>>().join("\n"),
                format!("WK {}", self.willpower),
            ),
            CardKind::Equipment => {
                let e = self.equip.unwrap_or_default();
                (self.card_kind.type_line().to_string(), e.description(), format!("+{}/+{}", e.attack, e.defense))
            }
            CardKind::Artifact | CardKind::Enchantment => {
                let mut parts: Vec<String> = Vec::new();
                if let Some(e) = self.static_effect {
                    parts.push(e.description());
                }
                if let Some(a) = self.activated {
                    parts.push(a.description());
                }
                (self.card_kind.type_line().to_string(), parts.join("\n"), String::new())
            }
            CardKind::Aura => match self.aura {
                Some(a) => (self.card_kind.type_line().to_string(), a.description(), a.stats_text()),
                None => (self.card_kind.type_line().to_string(), String::new(), String::new()),
            },
            CardKind::Resource => (String::new(), String::new(), String::new()),
        };
        let Some(l) = self.labels.as_mut() else {
            return;
        };
        let px = |x: f32, y: f32| {
            Vector3::new((x / 256.0 - 0.5) * CARD_WIDTH, CARD_THICKNESS * 0.5 + 0.0015, (y / 358.0 - 0.5) * CARD_HEIGHT)
        };
        let ty = placeholder::title_center_y();
        l.name.set_text(&self.card_name);
        if self.card_kind == CardKind::Resource {
            let color = self.produces.name();
            l.cost.set_text("");
            l.name.set_position(px(128.0, ty));
            l.kind.set_text(&format!("Ressource – {color}"));
            l.text.set_text(&format!("Tappen: +1 {color}-Mana."));
            l.stats.set_text("");
        } else {
            // Generische Kosten als Zahl im grauen Kostensymbol (ganz links)
            let cost = self.mana_cost.as_ref().map(|c| c.bind().cost()).unwrap_or_default();
            let pips = cost.specific.total() as usize + usize::from(cost.generic > 0);
            let centers = placeholder::pip_centers(pips);
            if cost.generic > 0 {
                l.cost.set_text(&cost.generic.to_string());
                l.cost.set_position(px(centers[0], ty));
            } else {
                l.cost.set_text("");
            }
            // Name mittig im freien Bereich links der Kostensymbole
            let left_pip = centers.first().copied().unwrap_or(256.0 - 20.0);
            l.name.set_position(px((26.0 + left_pip - 16.0) * 0.5, ty));
            l.kind.set_text(&kind_text);
            l.text.set_text(&body);
            l.stats.set_text(&stats);
        }
        let stat_color = if damaged {
            Color::from_rgb(1.0, 0.45, 0.4)
        } else if bonus {
            Color::from_rgb(0.6, 1.0, 0.5)
        } else {
            Color::WHITE
        };
        l.stats.set_modulate(stat_color);
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
        };
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
        // Kampfmarkierung: leuchtender Rand unter der Karte (sonst unsichtbar)
        let mut marker_mat = StandardMaterial3D::new_gd();
        marker_mat.set_shading_mode(ShadingMode::UNSHADED);
        marker_mat.set_feature(godot::classes::base_material_3d::Feature::EMISSION, true);
        marker_mat.set_emission_energy_multiplier(2.0);
        let mut marker_mesh = PlaneMesh::new_gd();
        marker_mesh.set_size(Vector2::new(CARD_WIDTH + 0.09, CARD_HEIGHT + 0.09));
        marker_mesh.set_material(&marker_mat);
        let mut marker = MeshInstance3D::new_alloc();
        marker.set_name("Marker");
        marker.set_mesh(&marker_mesh);
        marker.set_cast_shadows_setting(ShadowCastingSetting::OFF);
        marker.set_visible(false);
        visual.add_child(&marker);
        self.marker_mesh = Some(marker);
        self.marker_material = Some(marker_mat);

        self.build_labels(&mut visual);
        self.meshes = vec![body, face, back];
        self.face_material = Some(face_mat);
        self.back_material = Some(back_mat);
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
}

impl CardLabels {
    fn all(&mut self) -> [&mut Gd<Label3D>; 5] {
        [&mut self.name, &mut self.cost, &mut self.kind, &mut self.text, &mut self.stats]
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
