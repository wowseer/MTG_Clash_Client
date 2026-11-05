//! `LoyaltyBar`: Leiste der Loyalitätsfähigkeiten eines Planeswalkers (CR 606), wie in Arena:
//! Klick auf den eigenen Planeswalker öffnet direkt über der Karte eine Spalte mit je einer
//! Zeile pro Fähigkeit – links das Kosten-Symbol („+1“ heller Schild mit Spitze nach oben, „−3“
//! dunkler Schild mit Spitze nach unten, „0“ grau), rechts der Text mit Manasymbolen. Gerade
//! nicht aktivierbare Fähigkeiten (zu wenig Loyalität, schon eine aktiviert, falscher Zeitpunkt,
//! kein Ziel) sind ausgegraut und nicht anklickbar.
//!
//! Die Leiste steht kurz vor der Kamera an der Bildschirmstelle der Karte, in fester Pixelgröße
//! (wie das Farbrad [`ManaWheel`](super::mana_wheel::ManaWheel)), vor allen Karten. Bedienung:
//! Der [`TableController`](super::table_controller::TableController) trifft die Kollisionsfläche
//! (Layer [`CONTROL_LAYER`]) per Raycast, meldet den Trefferpunkt ([`LoyaltyBar::update_pointer`])
//! – die Zeile darunter wird hervorgehoben – und löst beim Klick [`LoyaltyBar::activate`] aus:
//! Signal `picked(index)`.
//!
//! Zeiten nach `docs/claude/ANIMATION.md`: Aufgehen 0,20 s Cubic-Out (Zeilen gleiten von der Karte
//! nach oben, leicht versetzt), Schließen 0,15 s linear, Hover exponentiell (≈ 0,12 s).

use godot::classes::base_material_3d::{BillboardMode, Flags, ShadingMode, TextureFilter, Transparency};
use godot::classes::geometry_instance_3d::ShadowCastingSetting;
use godot::classes::sprite_base_3d::DrawFlags as SpriteFlags;
use godot::classes::{Area3D, BoxShape3D, CollisionShape3D, INode3D, MeshInstance3D, QuadMesh, Sprite3D, StandardMaterial3D};
use godot::prelude::*;

use super::card3d::Card3D;
use super::cost_symbols;
use super::layers::CONTROL_LAYER;

/// Abstand vor der Kamera (Welteinheiten).
const DISTANCE: f32 = 2.5;
/// Maße in Bildschirm-Pixeln bei 1080 px Bildhöhe (skaliert mit der Fensterhöhe).
const ROW_W: f32 = 520.0;
const ICON: f32 = 64.0;
const PAD: f32 = 10.0;
const GAP: f32 = 6.0;
const FONT: i32 = 21;
const LINE_H: f32 = 26.0;
const MIN_ROW_H: f32 = 70.0;
/// Abstand der Spalte über der Karte.
const CARD_GAP: f32 = 14.0;
/// Bezugshöhe der Pixelmaße.
const REF_HEIGHT: f32 = 1080.0;
const APPEAR_TIME: f32 = 0.2;
const CLOSE_TIME: f32 = 0.15;
const HOVER_SPEED: f32 = 18.0;
/// Versatz der Zeilen beim Aufgehen (Sekunden je Zeile).
const STAGGER: f32 = 0.035;

/// Eine Zeile der Leiste: Loyalitätskosten („+1“, „-3“, „0“; leer = keine Loyalitätsfähigkeit),
/// Text der Fähigkeit und ob sie gerade aktiviert werden kann.
#[derive(Clone, Debug)]
pub struct LoyaltyEntry {
    pub cost: String,
    pub text: String,
    pub enabled: bool,
}

struct Row {
    height: f32,
    /// Mitte der Zeile (y in Pixeln, von oben, relativ zur Spaltenoberkante).
    center: f32,
    root: Gd<Node3D>,
    rim: Gd<StandardMaterial3D>,
    panel: Gd<StandardMaterial3D>,
    sprites: Vec<Gd<Sprite3D>>,
    hover: f32,
}

/// Leiste der Loyalitätsfähigkeiten an einem Planeswalker (wie in Arena): je Fähigkeit eine
/// Zeile mit Kosten-Symbol und Text, nicht aktivierbare ausgegraut. Signal [signal picked] beim
/// Klick auf eine aktivierbare Zeile.
#[derive(GodotClass)]
#[class(base=Node3D)]
pub struct LoyaltyBar {
    entries: Vec<LoyaltyEntry>,
    follow: Option<Gd<Card3D>>,
    rows: Vec<Row>,
    total_h: f32,
    hovered: i32,
    t: f32,
    closing: bool,
    hit: Option<Gd<Area3D>>,
    content: Option<Gd<Node3D>>,
    base: Base<Node3D>,
}

#[godot_api]
impl INode3D for LoyaltyBar {
    fn init(base: Base<Node3D>) -> Self {
        Self {
            entries: Vec::new(),
            follow: None,
            rows: Vec::new(),
            total_h: 0.0,
            hovered: -1,
            t: 0.0,
            closing: false,
            hit: None,
            content: None,
            base,
        }
    }

    fn ready(&mut self) {
        self.build();
    }

    fn process(&mut self, delta: f64) {
        let dt = delta as f32;
        self.t += dt;
        let alpha = if self.closing {
            let k = (self.t / CLOSE_TIME).min(1.0);
            if k >= 1.0 {
                self.base_mut().queue_free();
                return;
            }
            1.0 - k
        } else {
            1.0
        };
        let (t, closing, hovered) = (self.t, self.closing, self.hovered);
        let total = self.total_h;
        let n = self.rows_len();
        for (i, row) in self.rows.iter_mut().enumerate() {
            let enabled = self.entries.get(i).is_some_and(|e| e.enabled);
            let target = if i as i32 == hovered && enabled { 1.0 } else { 0.0 };
            row.hover += (target - row.hover) * (1.0 - (-HOVER_SPEED * dt).exp());
            // Aufgehen: von der Karte (Unterkante der Spalte) nach oben gleiten, unterste Zeile zuerst
            let order = (n - 1 - i) as f32;
            let k = if closing { 1.0 } else { ((t - order * STAGGER) / APPEAR_TIME).clamp(0.0, 1.0) };
            let ease = 1.0 - (1.0 - k).powi(3);
            let from = total - row.height * 0.5;
            let y = from + (row.center - from) * ease;
            let scale = 1.0 + 0.04 * row.hover;
            row.root.set_position(Vector3::new(0.0, -y, 0.01 * row.hover));
            row.root.set_scale(Vector3::splat(scale * (0.85 + 0.15 * ease)));
            let a = alpha * ease;
            let dim = if enabled { 1.0 } else { 0.45 };
            let rim = Color::from_rgb(0.3, 0.3, 0.33).lerp(Color::from_rgb(1.0, 0.82, 0.4), f64::from(row.hover));
            row.rim.set_albedo(Color::from_rgba(rim.r, rim.g, rim.b, 0.95 * a));
            let bg = Color::from_rgb(0.06, 0.06, 0.08).lerp(Color::from_rgb(0.14, 0.12, 0.08), f64::from(row.hover));
            row.panel.set_albedo(Color::from_rgba(bg.r, bg.g, bg.b, 0.92 * a));
            for s in row.sprites.iter_mut() {
                s.set_modulate(Color::from_rgba(dim, dim, dim, a * if enabled { 1.0 } else { 0.7 }));
            }
        }
        self.place();
    }
}

#[godot_api]
impl LoyaltyBar {
    /// Eine aktivierbare Zeile wurde angeklickt: `index` = Nummer der Fähigkeit (Reihenfolge wie übergeben).
    #[signal]
    fn picked(index: i64);

    /// Trefferpunkt des Mausstrahls (Welt) auf der Leiste: die Zeile darunter wird hervorgehoben.
    #[func]
    pub fn update_pointer(&mut self, world: Vector3) {
        let local = self.base().get_global_transform().affine_inverse() * world;
        let y = -local.y;
        self.hovered = if self.closing {
            -1
        } else {
            self.rows
                .iter()
                .position(|r| (y - r.center).abs() <= r.height * 0.5 + GAP * 0.5)
                .map_or(-1, |i| i as i32)
        };
    }

    /// Maus verlässt die Leiste bzw. betritt sie (ohne Trefferpunkt).
    #[func]
    pub fn set_hovered(&mut self, hovered: bool) {
        if !hovered {
            self.hovered = -1;
        }
    }

    /// Klick: liegt die Maus auf einer aktivierbaren Zeile, wird [signal picked] gesendet.
    #[func]
    pub fn activate(&mut self) {
        let i = self.hovered;
        if i >= 0 && !self.closing && self.entries.get(i as usize).is_some_and(|e| e.enabled) {
            self.signals().picked().emit(i64::from(i));
        }
    }

    /// Leiste schließen (0,15 s ausblenden), danach freigeben; nicht mehr bedienbar.
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

    /// Gehört die Leiste zur Karte `card`?
    #[func]
    pub fn belongs_to(&self, card: Gd<Card3D>) -> bool {
        self.follow.as_ref() == Some(&card)
    }
}

impl LoyaltyBar {
    /// Neue Leiste (noch nicht in der Szene) für die Karte `follow` mit diesen Zeilen (von oben nach unten).
    pub fn create(entries: Vec<LoyaltyEntry>, follow: Gd<Card3D>) -> Gd<LoyaltyBar> {
        let mut b = LoyaltyBar::new_alloc();
        {
            let mut m = b.bind_mut();
            m.entries = entries;
            m.follow = Some(follow);
        }
        b
    }

    fn rows_len(&self) -> usize {
        self.rows.len().max(1)
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

    fn sprite(tex: Gd<godot::classes::Texture2D>, pixel: f32, priority: i32) -> Gd<Sprite3D> {
        let mut s = Sprite3D::new_alloc();
        s.set_texture(&tex);
        s.set_pixel_size(pixel);
        s.set_billboard_mode(BillboardMode::DISABLED);
        s.set_draw_flag(SpriteFlags::SHADED, false);
        s.set_draw_flag(SpriteFlags::DOUBLE_SIDED, false);
        s.set_draw_flag(SpriteFlags::DISABLE_DEPTH_TEST, true);
        s.set_texture_filter(TextureFilter::LINEAR_WITH_MIPMAPS);
        s.set_render_priority(priority);
        s
    }

    /// Zeilen bauen (Maße in Bezugs-Pixeln, die Spalte beginnt oben bei y = 0).
    fn build(&mut self) {
        let mut content = Node3D::new_alloc();
        self.base_mut().add_child(&content);
        let text_w = ROW_W - ICON - 3.0 * PAD;
        let chars_per_line = (text_w / (FONT as f32 * 0.5)).max(10.0);
        let mut y = 0.0;
        let entries = self.entries.clone();
        for e in &entries {
            let lines = ((e.text.chars().count() as f32 / chars_per_line).ceil()).max(1.0);
            let h = (lines * LINE_H + 2.0 * PAD).max(MIN_ROW_H);
            let mut root = Node3D::new_alloc();
            let rim = Self::material(Color::from_rgba(0.3, 0.3, 0.33, 0.0), 40);
            let panel = Self::material(Color::from_rgba(0.06, 0.06, 0.08, 0.0), 41);
            root.add_child(&Self::quad(Vector2::new(ROW_W + 4.0, h + 4.0), &rim, 0.0));
            root.add_child(&Self::quad(Vector2::new(ROW_W, h), &panel, 0.001));
            let mut sprites = Vec::new();
            if let Some(token) = cost_symbols::loyalty_cost_token(&e.cost) {
                if let Some(tex) = cost_symbols::symbol_texture(&token) {
                    let mut s = Self::sprite(tex, ICON / 128.0, 42);
                    s.set_position(Vector3::new(-ROW_W * 0.5 + PAD + ICON * 0.5, 0.0, 0.002));
                    root.add_child(&s);
                    sprites.push(s);
                }
            }
            let size = Vector2i::new(text_w as i32, (h - PAD) as i32);
            if let Some(tex) = cost_symbols::rich_text_texture(&e.text, FONT, size, Color::from_rgb(0.95, 0.93, 0.88)) {
                let mut s = Self::sprite(tex, 1.0, 42);
                s.set_position(Vector3::new(-ROW_W * 0.5 + 2.0 * PAD + ICON + text_w * 0.5, 0.0, 0.002));
                root.add_child(&s);
                sprites.push(s);
            }
            content.add_child(&root);
            self.rows.push(Row { height: h, center: y + h * 0.5, root, rim, panel, sprites, hover: 0.0 });
            y += h + GAP;
        }
        self.total_h = (y - GAP).max(0.0);

        let mut hit = Area3D::new_alloc();
        hit.set_collision_layer(CONTROL_LAYER);
        hit.set_collision_mask(0);
        hit.set_monitoring(false);
        hit.set_monitorable(false);
        let mut shape = BoxShape3D::new_gd();
        shape.set_size(Vector3::new(ROW_W, self.total_h, 2.0));
        let mut col = CollisionShape3D::new_alloc();
        col.set_shape(&shape);
        col.set_position(Vector3::new(0.0, -self.total_h * 0.5, 0.0));
        hit.add_child(&col);
        content.add_child(&hit);
        self.hit = Some(hit);
        self.content = Some(content);
        self.place();
    }

    /// Spalte über die Karte (Bildschirmstelle) setzen, ganz im Bild; passt sie oben nicht hin,
    /// steht sie unter der Karte.
    fn place(&mut self) {
        let Some(card) = self.follow.clone().filter(|c| c.is_instance_valid()) else { return };
        let quad = card.bind().screen_quad();
        if quad.len() != 4 {
            return;
        }
        let pts = quad.as_slice();
        let cx = pts.iter().map(|p| p.x).sum::<f32>() * 0.25;
        let top = pts.iter().map(|p| p.y).fold(f32::MAX, f32::min);
        let bottom = pts.iter().map(|p| p.y).fold(f32::MIN, f32::max);
        let Some(viewport) = self.base().get_viewport() else { return };
        let Some(cam) = viewport.get_camera_3d() else { return };
        let screen = viewport.get_visible_rect().size;
        let px = screen.y / REF_HEIGHT; // Bildschirm-Pixel je Bezugs-Pixel
        let (w, h) = (ROW_W * px, self.total_h * px);
        let margin = 12.0 * px;
        let mut y0 = top - CARD_GAP * px - h;
        if y0 < margin {
            y0 = (bottom + CARD_GAP * px).min(screen.y - margin - h).max(margin);
        }
        let x = cx.clamp(margin + w * 0.5, (screen.x - margin - w * 0.5).max(margin + w * 0.5));
        let view_h = 2.0 * DISTANCE * (cam.get_fov().to_radians() * 0.5).tan();
        let world_per_ref = view_h / screen.y * px;
        let basis = cam.get_global_transform().basis.orthonormalized();
        let origin = cam.project_position(Vector2::new(x, y0), DISTANCE);
        self.base_mut().set_global_transform(Transform3D::new(basis.scaled(Vector3::splat(world_per_ref)), origin));
    }
}
