//! `LoyaltyView`: Planeswalker-Ansicht wie in Arena (CR 606). Klick auf den eigenen Planeswalker
//! zeigt ihn groß links im Bild (abgedunkelter Tisch dahinter); sein Textfeld ist in je ein
//! Feld pro Loyalitätsfähigkeit geteilt, das man **auf der Karte** anklickt. Jedes Feld trägt
//! links am Kartenrand das Kosten-Symbol wie die gedruckte Karte („+1“ heller Schild mit Spitze
//! nach oben, „−3“ dunkler Schild mit Spitze nach unten, „0“ grau) und daneben den Text mit
//! Manasymbolen. Gerade nicht aktivierbare Fähigkeiten (zu wenig Loyalität, schon eine
//! aktiviert, falscher Zeitpunkt, kein Ziel) sind ausgegraut. Klick neben die Karte schließt die
//! Ansicht (Signal `dismissed`).
//!
//! Die Ansicht steht kurz vor der Kamera in fester Pixelgröße, vor allen Karten. Bedienung: Der
//! [`TableController`](super::table_controller::TableController) trifft die Kollisionsfläche
//! (Layer [`CONTROL_LAYER`], ganzer Bildschirm) per Raycast, meldet den Trefferpunkt
//! ([`LoyaltyView::update_pointer`]) – das Feld darunter leuchtet golden – und löst beim Klick
//! [`LoyaltyView::activate`] aus: Signal `picked(index)` bzw. `dismissed`.
//!
//! Zeiten nach `docs/claude/ANIMATION.md`: Aufgehen 0,20 s Cubic-Out (Karte wächst von 90 % und
//! blendet ein), Schließen 0,15 s linear, Hover exponentiell (≈ 0,12 s).

use godot::classes::base_material_3d::{BillboardMode, Flags, ShadingMode, TextureFilter, Transparency};
use godot::classes::geometry_instance_3d::ShadowCastingSetting;
use godot::classes::sprite_base_3d::DrawFlags as SpriteFlags;
use godot::classes::{Area3D, BoxShape3D, CollisionShape3D, INode3D, MeshInstance3D, QuadMesh, Sprite3D, StandardMaterial3D, Texture2D};
use godot::prelude::*;

use super::card3d::Card3D;
use super::cost_symbols;
use super::layers::CONTROL_LAYER;
use super::placeholder;

/// Abstand vor der Kamera (Welteinheiten).
const DISTANCE: f32 = 2.5;
/// Maße in Bezugs-Pixeln bei 1080 px Bildhöhe (skaliert mit der Fensterhöhe).
const REF_HEIGHT: f32 = 1080.0;
const CARD_H: f32 = 700.0;
const CARD_W: f32 = CARD_H * 63.0 / 88.0;
/// Mitte der Karte auf dem Bildschirm (Anteile von Bildbreite/-höhe): links, neben den
/// Planeswalkern und frei von den Medaillons in der Mitte.
const CENTER_X: f32 = 0.29;
const CENTER_Y: f32 = 0.45;
/// Textfeld der Karte (Anteile von Kartenbreite/-höhe ab links oben), in das die Felder kommen.
const BOX_TOP: f32 = 0.56;
const BOX_BOTTOM: f32 = 0.935;
const BOX_LEFT: f32 = 0.04;
const BOX_RIGHT: f32 = 0.96;
/// Kosten-Symbol (Höhe), Abstand zwischen den Feldern, Schrift, Rand im Feld.
const ICON: f32 = 70.0;
const GAP: f32 = 4.0;
/// Größte und kleinste Schrift der Felder (passt sich der Textlänge an).
const FONT: i32 = 24;
const MIN_FONT: i32 = 12;
const PAD: f32 = 8.0;
const APPEAR_TIME: f32 = 0.2;
const CLOSE_TIME: f32 = 0.15;
const HOVER_SPEED: f32 = 18.0;

/// Eine Fähigkeit der Ansicht: Loyalitätskosten („+1“, „-3“, „0“; leer = keine
/// Loyalitätsfähigkeit), Text und ob sie gerade aktiviert werden kann.
#[derive(Clone, Debug)]
pub struct LoyaltyEntry {
    pub cost: String,
    pub text: String,
    pub enabled: bool,
}

struct Row {
    /// Mitte und Höhe des Feldes (Bezugs-Pixel, y nach unten, relativ zur Kartenmitte).
    center: f32,
    height: f32,
    rim: Gd<StandardMaterial3D>,
    panel: Gd<StandardMaterial3D>,
    sprites: Vec<Gd<Sprite3D>>,
    root: Gd<Node3D>,
    hover: f32,
}

/// Planeswalker-Ansicht wie in Arena: die Karte groß, ihre Loyalitätsfähigkeiten als Felder auf
/// der Karte (nicht aktivierbare ausgegraut). Signale [signal picked] (Feld angeklickt) und
/// [signal dismissed] (Klick neben die Karte).
#[derive(GodotClass)]
#[class(base=Node3D)]
pub struct LoyaltyView {
    entries: Vec<LoyaltyEntry>,
    follow: Option<Gd<Card3D>>,
    face: Option<Gd<Texture2D>>,
    rows: Vec<Row>,
    hovered: i32,
    on_card: bool,
    t: f32,
    closing: bool,
    hit: Option<Gd<Area3D>>,
    content: Option<Gd<Node3D>>,
    card: Option<Gd<Sprite3D>>,
    backdrop: Option<Gd<StandardMaterial3D>>,
    base: Base<Node3D>,
}

#[godot_api]
impl INode3D for LoyaltyView {
    fn init(base: Base<Node3D>) -> Self {
        Self {
            entries: Vec::new(),
            follow: None,
            face: None,
            rows: Vec::new(),
            hovered: -1,
            on_card: false,
            t: 0.0,
            closing: false,
            hit: None,
            content: None,
            card: None,
            backdrop: None,
            base,
        }
    }

    fn ready(&mut self) {
        self.build();
    }

    fn process(&mut self, delta: f64) {
        let dt = delta as f32;
        self.t += dt;
        let (k, alpha) = if self.closing {
            let k = (self.t / CLOSE_TIME).min(1.0);
            if k >= 1.0 {
                self.base_mut().queue_free();
                return;
            }
            (1.0, 1.0 - k)
        } else {
            let k = (self.t / APPEAR_TIME).min(1.0);
            let e = 1.0 - (1.0 - k).powi(3);
            (e, e)
        };
        if let Some(c) = self.content.as_mut() {
            c.set_scale(Vector3::splat(0.9 + 0.1 * k));
        }
        if let Some(c) = self.card.as_mut() {
            c.set_modulate(Color::from_rgba(1.0, 1.0, 1.0, alpha));
        }
        if let Some(b) = self.backdrop.as_mut() {
            b.set_albedo(Color::from_rgba(0.0, 0.0, 0.0, 0.55 * alpha));
        }
        let hovered = self.hovered;
        for (i, row) in self.rows.iter_mut().enumerate() {
            let enabled = self.entries.get(i).is_some_and(|e| e.enabled);
            let target = if i as i32 == hovered && enabled { 1.0 } else { 0.0 };
            row.hover += (target - row.hover) * (1.0 - (-HOVER_SPEED * dt).exp());
            row.root.set_scale(Vector3::splat(1.0 + 0.03 * row.hover));
            row.root.set_position(Vector3::new(0.0, -row.center, 0.02 + 0.01 * row.hover));
            let rim = Color::from_rgb(0.25, 0.22, 0.18).lerp(Color::from_rgb(1.0, 0.8, 0.3), f64::from(row.hover));
            row.rim.set_albedo(Color::from_rgba(rim.r, rim.g, rim.b, alpha));
            let bg = if enabled { Color::from_rgb(0.95, 0.93, 0.88) } else { Color::from_rgb(0.42, 0.42, 0.43) };
            let bg = bg.lerp(Color::from_rgb(1.0, 0.96, 0.82), f64::from(row.hover));
            row.panel.set_albedo(Color::from_rgba(bg.r, bg.g, bg.b, alpha));
            let dim = if enabled { 1.0 } else { 0.5 };
            for s in row.sprites.iter_mut() {
                s.set_modulate(Color::from_rgba(dim, dim, dim, alpha));
            }
        }
        self.place();
    }
}

#[godot_api]
impl LoyaltyView {
    /// Ein aktivierbares Feld wurde angeklickt: `index` = Nummer der Fähigkeit (Reihenfolge wie übergeben).
    #[signal]
    fn picked(index: i64);

    /// Klick neben die Karte: die Ansicht soll schließen.
    #[signal]
    fn dismissed();

    /// Trefferpunkt des Mausstrahls (Welt): das Feld darunter wird hervorgehoben; merkt sich, ob
    /// die Maus auf der Karte liegt.
    #[func]
    pub fn update_pointer(&mut self, world: Vector3) {
        let Some(content) = self.content.as_ref() else { return };
        let local = content.get_global_transform().affine_inverse() * world;
        let (x, y) = (local.x, -local.y);
        self.on_card = x.abs() <= CARD_W * 0.5 && y.abs() <= CARD_H * 0.5;
        let in_box = x.abs() <= CARD_W * 0.5;
        self.hovered = if self.closing || !in_box {
            -1
        } else {
            self.rows
                .iter()
                .position(|r| (y - r.center).abs() <= r.height * 0.5 + GAP * 0.5)
                .map_or(-1, |i| i as i32)
        };
    }

    /// Maus verlässt die Ansicht bzw. betritt sie (ohne Trefferpunkt).
    #[func]
    pub fn set_hovered(&mut self, hovered: bool) {
        if !hovered {
            self.hovered = -1;
            self.on_card = false;
        }
    }

    /// Klick: auf ein aktivierbares Feld → [signal picked]; neben die Karte → [signal dismissed].
    #[func]
    pub fn activate(&mut self) {
        if self.closing {
            return;
        }
        let i = self.hovered;
        if i >= 0 && self.entries.get(i as usize).is_some_and(|e| e.enabled) {
            self.signals().picked().emit(i64::from(i));
        } else if !self.on_card {
            self.signals().dismissed().emit();
        }
    }

    /// Ansicht schließen (0,15 s ausblenden), danach freigeben; nicht mehr bedienbar.
    #[func]
    pub fn close(&mut self) {
        if self.closing {
            return;
        }
        self.closing = true;
        self.t = 0.0;
        if let Some(mut hit) = self.hit.clone() {
            hit.set_collision_layer(0);
        }
    }

    /// Gehört die Ansicht zur Karte `card`?
    #[func]
    pub fn belongs_to(&self, card: Gd<Card3D>) -> bool {
        self.follow.as_ref() == Some(&card)
    }
}

impl LoyaltyView {
    /// Neue Ansicht (noch nicht in der Szene) für die Karte `card` mit diesen Fähigkeiten (von oben nach unten).
    pub fn create(entries: Vec<LoyaltyEntry>, card: Gd<Card3D>) -> Gd<LoyaltyView> {
        let face = placeholder::card_face(card.bind().def());
        let mut v = LoyaltyView::new_alloc();
        {
            let mut m = v.bind_mut();
            m.entries = entries;
            m.follow = Some(card);
            m.face = Some(face);
        }
        v
    }

    fn material(color: Color, priority: i32) -> Gd<StandardMaterial3D> {
        let mut m = StandardMaterial3D::new_gd();
        m.set_shading_mode(ShadingMode::UNSHADED);
        m.set_transparency(Transparency::ALPHA);
        m.set_flag(Flags::DISABLE_DEPTH_TEST, true);
        m.set_albedo(color);
        m.set_render_priority(priority);
        m
    }

    fn quad(size: Vector2, mat: &Gd<StandardMaterial3D>, z: f32) -> Gd<MeshInstance3D> {
        let mut q = QuadMesh::new_gd();
        q.set_size(size);
        q.set_material(mat);
        let mut mesh = MeshInstance3D::new_alloc();
        mesh.set_mesh(&q);
        mesh.set_cast_shadows_setting(ShadowCastingSetting::OFF);
        mesh.set_position(Vector3::new(0.0, 0.0, z));
        mesh
    }

    fn sprite(tex: &Gd<Texture2D>, pixel: f32, priority: i32) -> Gd<Sprite3D> {
        let mut s = Sprite3D::new_alloc();
        s.set_texture(tex);
        s.set_pixel_size(pixel);
        s.set_billboard_mode(BillboardMode::DISABLED);
        s.set_draw_flag(SpriteFlags::SHADED, false);
        s.set_draw_flag(SpriteFlags::DOUBLE_SIDED, false);
        s.set_draw_flag(SpriteFlags::DISABLE_DEPTH_TEST, true);
        s.set_texture_filter(TextureFilter::LINEAR_WITH_MIPMAPS);
        s.set_render_priority(priority);
        s
    }

    /// Hintergrund, Karte und Felder bauen (Bezugs-Pixel, Ursprung = Kartenmitte, y oben).
    fn build(&mut self) {
        let mut content = Node3D::new_alloc();
        self.base_mut().add_child(&content);
        // abgedunkelter Tisch
        let backdrop = Self::material(Color::from_rgba(0.0, 0.0, 0.0, 0.0), 38);
        content.add_child(&Self::quad(Vector2::new(6000.0, 4000.0), &backdrop, -0.05));
        // die Karte
        if let Some(face) = self.face.clone() {
            let w = face.get_width().max(1) as f32;
            let mut card = Self::sprite(&face, CARD_W / w, 39);
            card.set_modulate(Color::from_rgba(1.0, 1.0, 1.0, 0.0));
            content.add_child(&card);
            self.card = Some(card);
        }
        // Felder im Textfeld: Höhe nach Textlänge
        let (top, bottom) = (CARD_H * (BOX_TOP - 0.5), CARD_H * (BOX_BOTTOM - 0.5));
        let (left, right) = (CARD_W * (BOX_LEFT - 0.5), CARD_W * (BOX_RIGHT - 0.5));
        let width = right - left;
        let n = self.entries.len().max(1);
        let free = (bottom - top) - GAP * (n as f32 - 1.0);
        let text_left = left + ICON * 0.55 + PAD;
        let text_w = right - PAD - text_left;
        // Höhe je Feld nach Textmenge; dann die größte gemeinsame Schrift, bei der jeder Text passt
        let weights: Vec<f32> = self.entries.iter().map(|e| (e.text.chars().count() as f32).max(40.0)).collect();
        let sum: f32 = weights.iter().sum::<f32>().max(1.0);
        let heights: Vec<f32> = weights.iter().map(|w| free * w / sum).collect();
        let fits = |font: i32| {
            self.entries.iter().zip(&heights).all(|(e, h)| {
                let per_line = (text_w / (font as f32 * 0.52)).max(1.0);
                let lines = (e.text.chars().count() as f32 / per_line).ceil().max(1.0);
                lines * font as f32 * 1.3 <= h - 6.0
            })
        };
        let mut font = FONT;
        while font > MIN_FONT && !fits(font) {
            font -= 1;
        }
        let mut y = top;
        let entries = self.entries.clone();
        for (e, h) in entries.iter().zip(heights.clone()) {
            let mut root = Node3D::new_alloc();
            let rim = Self::material(Color::from_rgba(0.25, 0.22, 0.18, 0.0), 40);
            let panel = Self::material(Color::from_rgba(0.95, 0.93, 0.88, 0.0), 41);
            let cx = (left + right) * 0.5;
            let mut r = Self::quad(Vector2::new(width + 6.0, h + GAP + 2.0), &rim, 0.0);
            r.set_position(Vector3::new(cx, 0.0, 0.0));
            root.add_child(&r);
            let mut p = Self::quad(Vector2::new(width, h), &panel, 0.001);
            p.set_position(Vector3::new(cx, 0.0, 0.001));
            root.add_child(&p);
            let mut sprites = Vec::new();
            // Text (dunkel auf hell, wie das gedruckte Textfeld)
            if let Some(tex) = cost_symbols::rich_text_texture(&e.text, font, Vector2i::new(text_w as i32, (h - 4.0) as i32), Color::from_rgb(0.08, 0.07, 0.06)) {
                let mut s = Self::sprite(&tex, 1.0, 42);
                s.set_position(Vector3::new(text_left + text_w * 0.5, 0.0, 0.002));
                root.add_child(&s);
                sprites.push(s);
            }
            // Kosten-Symbol am linken Kartenrand (halb über den Rand wie auf der Karte)
            if let Some(tex) = cost_symbols::loyalty_cost_token(&e.cost).and_then(|t| cost_symbols::symbol_texture(&t)) {
                let mut s = Self::sprite(&tex, ICON / 128.0, 43);
                s.set_position(Vector3::new(left + ICON * 0.1, 0.0, 0.003));
                root.add_child(&s);
                sprites.push(s);
            }
            content.add_child(&root);
            self.rows.push(Row { center: y + h * 0.5, height: h, rim, panel, sprites, root, hover: 0.0 });
            y += h + GAP;
        }
        // Trefferfläche: ganzer Bildschirm (Klick neben die Karte schließt)
        let mut hit = Area3D::new_alloc();
        hit.set_collision_layer(CONTROL_LAYER);
        hit.set_collision_mask(0);
        hit.set_monitoring(false);
        hit.set_monitorable(false);
        let mut shape = BoxShape3D::new_gd();
        shape.set_size(Vector3::new(6000.0, 4000.0, 2.0));
        let mut col = CollisionShape3D::new_alloc();
        col.set_shape(&shape);
        hit.add_child(&col);
        content.add_child(&hit);
        self.hit = Some(hit);
        self.backdrop = Some(backdrop);
        self.content = Some(content);
        self.place();
    }

    /// Vor die Kamera setzen: Kartenmitte bei [`CENTER_X`]/[`CENTER_Y`], Größe in Pixeln fest.
    fn place(&mut self) {
        let Some(viewport) = self.base().get_viewport() else { return };
        let Some(cam) = viewport.get_camera_3d() else { return };
        let screen = viewport.get_visible_rect().size;
        let view_h = 2.0 * DISTANCE * (cam.get_fov().to_radians() * 0.5).tan();
        let world_per_ref = view_h / REF_HEIGHT;
        let basis = cam.get_global_transform().basis.orthonormalized();
        let origin = cam.project_position(Vector2::new(screen.x * CENTER_X, screen.y * CENTER_Y), DISTANCE);
        self.base_mut().set_global_transform(Transform3D::new(basis.scaled(Vector3::splat(world_per_ref)), origin));
    }
}
