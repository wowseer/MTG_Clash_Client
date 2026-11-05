//! Dungeons wie in Arena (CR 309, Venture CR 701.49):
//!
//! - **Dungeon-Karte** ([`dungeon_texture`]): selbst gezeichnet aus den Engine-Daten – dunkle
//!   Steinkarte mit dem Namen oben, darunter die Räume als Pergament-Felder in Ebenen (Eingang
//!   oben, letzter Raum unten), Wege als Linien; der aktuelle Raum golden umrandet mit einer
//!   Spielfigur. Sie ist die Vorderseite des Dungeons in der Kommandozone (über
//!   `placeholder::card_face`).
//! - **[`DungeonView`]**: Ansicht vor der Kamera (Tisch abgedunkelt) für drei Fälle:
//!   *Dungeon wählen* (alle Dungeons groß nebeneinander, Klick wählt), *Raum wählen* (der Dungeon
//!   groß, die erreichbaren Räume leuchten blau, Klick auf einen Raum wählt) und *ansehen* (Klick
//!   auf den Dungeon in der Kommandozone; Klick schließt).
//!
//! Bedienung wie bei [`LoyaltyView`](super::loyalty_view::LoyaltyView): Der
//! [`TableController`](super::table_controller::TableController) trifft die bildschirmfüllende
//! Kollisionsfläche (Layer [`CONTROL_LAYER`]), meldet den Trefferpunkt
//! ([`DungeonView::update_pointer`]) und löst beim Klick [`DungeonView::activate`] aus.
//! Zeiten nach `docs/claude/ANIMATION.md`: Aufgehen 0,25 s Cubic-Out, Schließen 0,15 s,
//! wählbare Räume pulsieren (1,2 s), Hover exponentiell.

use std::cell::RefCell;
use std::collections::HashMap;

use godot::classes::base_material_3d::{BillboardMode, Flags, ShadingMode, TextureFilter, Transparency};
use godot::classes::geometry_instance_3d::ShadowCastingSetting;
use godot::classes::label_3d::DrawFlags as LabelFlags;
use godot::classes::sprite_base_3d::DrawFlags as SpriteFlags;
use godot::classes::sub_viewport::UpdateMode;
use godot::classes::control::MouseFilter;
use godot::classes::{
    Area3D, BoxShape3D, CanvasItem, CollisionShape3D, Control, Engine, IControl, INode3D, Label3D, MeshInstance3D, QuadMesh,
    SceneTree, Sprite3D, StandardMaterial3D, SubViewport, Texture2D, ThemeDb,
};
use godot::global::HorizontalAlignment;
use godot::prelude::*;

use super::layers::CONTROL_LAYER;
use crate::game::dungeon::{Dungeon, TITLE_BAND};

/// Größe der gezeichneten Karte (Pixel, Seitenverhältnis einer Magic-Karte).
const TEX_W: i32 = 640;
const TEX_H: i32 = 896;

// ------------------------------------------------------------------ Dungeon-Karte (Textur)

/// Zeichenfläche einer Dungeon-Karte (intern: liegt in einer unsichtbaren `SubViewport`).
#[derive(GodotClass)]
#[class(base=Control, init)]
pub struct DungeonCanvas {
    dungeon: Dungeon,
    base: Base<Control>,
}

#[godot_api]
impl IControl for DungeonCanvas {
    fn draw(&mut self) {
        let d = self.dungeon.clone();
        let mut b = self.base_mut();
        draw_dungeon(&mut b, &d);
    }
}

const STONE: Color = Color::from_rgb(0.13, 0.12, 0.14);
const STONE_LIGHT: Color = Color::from_rgb(0.2, 0.18, 0.2);
const GOLD: Color = Color::from_rgb(0.86, 0.68, 0.32);
const PARCHMENT: Color = Color::from_rgb(0.88, 0.84, 0.74);
const INK: Color = Color::from_rgb(0.12, 0.1, 0.08);
const PATH: Color = Color::from_rgb(0.62, 0.52, 0.36);

fn draw_dungeon(b: &mut CanvasItem, d: &Dungeon) {
    let (w, h) = (TEX_W as f32, TEX_H as f32);
    // Steinkarte mit goldenem Rand
    b.draw_rect(Rect2::new(Vector2::ZERO, Vector2::new(w, h)), Color::from_rgb(0.05, 0.05, 0.06));
    b.draw_rect(Rect2::new(Vector2::splat(8.0), Vector2::new(w - 16.0, h - 16.0)), GOLD);
    b.draw_rect(Rect2::new(Vector2::splat(14.0), Vector2::new(w - 28.0, h - 28.0)), STONE);
    // leichte Steinfugen
    for k in 1..9 {
        let y = 14.0 + k as f32 * (h - 28.0) / 9.0;
        b.draw_line_ex(Vector2::new(14.0, y), Vector2::new(w - 14.0, y), STONE_LIGHT).width(2.0).done();
    }
    let Some(font) = ThemeDb::singleton().get_fallback_font() else { return };
    // Titel
    let title_h = h * TITLE_BAND;
    b.draw_rect(Rect2::new(Vector2::new(14.0, 14.0), Vector2::new(w - 28.0, title_h - 18.0)), Color::from_rgb(0.08, 0.07, 0.08));
    let fs = fit_line(&d.name, w - 60.0, 40, 22);
    b.draw_string_ex(&font, Vector2::new(0.0, 14.0 + (title_h - 18.0) * 0.5 + fs as f32 * 0.36), &d.name)
        .alignment(HorizontalAlignment::CENTER)
        .width(w)
        .font_size(fs)
        .modulate(GOLD)
        .done();
    let rects: Vec<Rect2> = d
        .layout()
        .iter()
        .map(|r| Rect2::new(Vector2::new(r[0] * w, r[1] * h), Vector2::new(r[2] * w, r[3] * h)))
        .collect();
    // Wege: unten Mitte → oben Mitte des Folgeraums
    for (i, room) in d.rooms.iter().enumerate() {
        for next in &room.next {
            if let Some(j) = d.index_of(next) {
                let a = rects[i].position + Vector2::new(rects[i].size.x * 0.5, rects[i].size.y);
                let z = rects[j].position + Vector2::new(rects[j].size.x * 0.5, 0.0);
                b.draw_line_ex(a, z, Color::from_rgb(0.05, 0.04, 0.04)).width(9.0).antialiased(true).done();
                b.draw_line_ex(a, z, PATH).width(5.0).antialiased(true).done();
            }
        }
    }
    // Räume
    for (i, room) in d.rooms.iter().enumerate() {
        let r = rects[i];
        let current = room.name == d.current;
        let rim = if current { GOLD } else { Color::from_rgb(0.32, 0.26, 0.18) };
        let grow = if current { 6.0 } else { 3.0 };
        b.draw_rect(Rect2::new(r.position - Vector2::splat(grow), r.size + Vector2::splat(2.0 * grow)), rim);
        b.draw_rect(r, if current { Color::from_rgb(0.98, 0.9, 0.66) } else { PARCHMENT });
        // Name und Text, Schrift passend zur Feldgröße
        let pad = 8.0;
        let inner_w = r.size.x - 2.0 * pad;
        let (name_fs, text_fs) = fit_room(&room.name, &room.text, inner_w, r.size.y - 2.0 * pad);
        let mut y = r.position.y + pad + name_fs as f32 * 0.95;
        b.draw_string_ex(&font, Vector2::new(r.position.x + pad, y), &room.name)
            .alignment(HorizontalAlignment::CENTER)
            .width(inner_w)
            .font_size(name_fs)
            .modulate(Color::from_rgb(0.32, 0.12, 0.06))
            .done();
        y += text_fs as f32 * 1.25;
        b.draw_multiline_string_ex(&font, Vector2::new(r.position.x + pad, y), &room.text)
            .alignment(HorizontalAlignment::CENTER)
            .width(inner_w)
            .font_size(text_fs)
            .modulate(INK)
            .done();
        if current {
            // Spielfigur auf dem aktuellen Raum (rechts oben am Feld)
            let c = r.position + Vector2::new(r.size.x - 4.0, 4.0);
            b.draw_circle(c + Vector2::new(0.0, 3.0), 19.0, Color::from_rgba(0.0, 0.0, 0.0, 0.5));
            b.draw_circle(c, 18.0, Color::from_rgb(0.85, 0.28, 0.12));
            b.draw_circle(c - Vector2::splat(5.0), 7.0, Color::from_rgb(1.0, 0.6, 0.4));
            b.draw_arc_ex(c, 18.0, 0.0, std::f32::consts::TAU, 32, GOLD).width(3.0).antialiased(true).done();
        }
    }
}

/// Schriftgröße, bei der `text` in `width` passt (Schätzung: Zeichenbreite ≈ 0,52 × Größe).
fn fit_line(text: &str, width: f32, max: i32, min: i32) -> i32 {
    let n = text.chars().count().max(1) as f32;
    ((width / (n * 0.52)) as i32).clamp(min, max)
}

/// Schriftgrößen (Name, Text) eines Raums, damit beides in das Feld passt.
fn fit_room(name: &str, text: &str, width: f32, height: f32) -> (i32, i32) {
    let name_fs = fit_line(name, width, 22, 11);
    let mut fs = 18;
    while fs > 9 {
        let per_line = (width / (fs as f32 * 0.5)).max(1.0);
        let lines = (text.chars().count() as f32 / per_line).ceil().max(1.0);
        if name_fs as f32 * 1.3 + lines * fs as f32 * 1.25 <= height {
            break;
        }
        fs -= 1;
    }
    (name_fs, fs)
}

thread_local! {
    static CACHE: RefCell<HashMap<String, Gd<SubViewport>>> = RefCell::new(HashMap::new());
}

/// Textur der Dungeon-Karte (einmal je Dungeon und aktuellem Raum gezeichnet, danach aus dem
/// Zwischenspeicher). None, wenn kein Szenenbaum da ist.
pub fn dungeon_texture(d: &Dungeon) -> Option<Gd<Texture2D>> {
    let key = format!("{}|{}|{}", d.name, d.current, d.rooms.len());
    if let Some(vp) = CACHE.with(|c| c.borrow().get(&key).cloned()).filter(|v| v.is_instance_valid()) {
        return vp.get_texture().map(|t| t.upcast::<Texture2D>());
    }
    let tree = Engine::singleton().get_main_loop()?.try_cast::<SceneTree>().ok()?;
    let mut root = tree.get_root();
    let mut vp = SubViewport::new_alloc();
    vp.set_size(Vector2i::new(TEX_W, TEX_H));
    vp.set_transparent_background(true);
    vp.set_disable_3d(true);
    vp.set_update_mode(UpdateMode::ONCE);
    let mut canvas = DungeonCanvas::new_alloc();
    canvas.bind_mut().dungeon = d.clone();
    canvas.set_size(Vector2::new(TEX_W as f32, TEX_H as f32));
    canvas.set_mouse_filter(MouseFilter::IGNORE);
    vp.add_child(&canvas);
    root.call_deferred("add_child", &[vp.to_variant()]);
    let tex = vp.get_texture()?.upcast::<Texture2D>();
    CACHE.with(|c| c.borrow_mut().insert(key, vp));
    Some(tex)
}

// ------------------------------------------------------------------ Ansicht

/// Was die Ansicht erlaubt.
#[derive(Clone, Debug, PartialEq)]
pub enum DungeonMode {
    /// Nur ansehen; Klick schließt (Signal `dismissed`).
    View,
    /// Einen der Dungeons wählen (Signal `picked(index des Dungeons)`).
    PickDungeon,
    /// Einen Raum wählen: je wählbarem Raum (Name) die Nummer der Option (Signal `picked(nummer)`).
    PickRoom(Vec<(String, i64)>),
}

const DISTANCE: f32 = 2.4;
const REF_HEIGHT: f32 = 1080.0;
const MAX_PANEL_H: f32 = 820.0;
const ROW_W: f32 = 1760.0;
const PANEL_GAP: f32 = 40.0;
const PANEL_Y: f32 = 590.0;
const APPEAR_TIME: f32 = 0.25;
const CLOSE_TIME: f32 = 0.15;
const HOVER_SPEED: f32 = 18.0;
const PULSE_TIME: f32 = 1.2;

struct Panel {
    /// Mitte und Größe (Bezugs-Pixel, y nach unten, Ursprung Bildmitte oben = (0, 0)).
    center: Vector2,
    size: Vector2,
    rim: Gd<StandardMaterial3D>,
    sprite: Gd<Sprite3D>,
    hover: f32,
}

struct RoomMark {
    rect: Rect2,
    option: i64,
    rim: Gd<StandardMaterial3D>,
    hover: f32,
}

/// Dungeon-Ansicht wie in Arena: Dungeon(s) groß vor dem abgedunkelten Tisch; je nach Modus
/// Dungeon oder Raum per Klick wählen (Signal [signal picked]) bzw. ansehen (Klick → [signal dismissed]).
#[derive(GodotClass)]
#[class(base=Node3D)]
pub struct DungeonView {
    dungeons: Vec<Dungeon>,
    mode: DungeonMode,
    title: String,
    panels: Vec<Panel>,
    marks: Vec<RoomMark>,
    /// Maus über Dungeon `0` bzw. Raum-Markierung `1` (−1 = keiner).
    hovered: (i32, i32),
    t: f32,
    closing: bool,
    content: Option<Gd<Node3D>>,
    hit: Option<Gd<Area3D>>,
    backdrop: Option<Gd<StandardMaterial3D>>,
    label: Option<Gd<Label3D>>,
    base: Base<Node3D>,
}

#[godot_api]
impl INode3D for DungeonView {
    fn init(base: Base<Node3D>) -> Self {
        Self {
            dungeons: Vec::new(),
            mode: DungeonMode::View,
            title: String::new(),
            panels: Vec::new(),
            marks: Vec::new(),
            hovered: (-1, -1),
            t: 0.0,
            closing: false,
            content: None,
            hit: None,
            backdrop: None,
            label: None,
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
            c.set_scale(Vector3::splat(0.92 + 0.08 * k));
        }
        if let Some(b) = self.backdrop.as_mut() {
            b.set_albedo(Color::from_rgba(0.0, 0.0, 0.0, 0.62 * alpha));
        }
        if let Some(l) = self.label.as_mut() {
            l.set_modulate(Color::from_rgba(0.98, 0.9, 0.7, alpha));
            l.set_outline_modulate(Color::from_rgba(0.0, 0.0, 0.0, 0.8 * alpha));
        }
        let pick_dungeon = self.mode == DungeonMode::PickDungeon;
        let (hp, hm) = self.hovered;
        for (i, p) in self.panels.iter_mut().enumerate() {
            let target = if pick_dungeon && i as i32 == hp { 1.0 } else { 0.0 };
            p.hover += (target - p.hover) * (1.0 - (-HOVER_SPEED * dt).exp());
            p.sprite.set_modulate(Color::from_rgba(1.0, 1.0, 1.0, alpha));
            let base = if pick_dungeon { Color::from_rgb(0.35, 0.65, 1.0) } else { Color::from_rgb(0.3, 0.25, 0.18) };
            let c = base.lerp(Color::from_rgb(1.0, 0.82, 0.35), f64::from(p.hover));
            p.rim.set_albedo(Color::from_rgba(c.r, c.g, c.b, alpha * if pick_dungeon { 0.9 } else { 0.6 }));
            p.sprite.set_scale(Vector3::splat(1.0 + 0.03 * p.hover));
        }
        let pulse = 0.5 + 0.5 * (self.t * std::f32::consts::TAU / PULSE_TIME).sin();
        for (i, m) in self.marks.iter_mut().enumerate() {
            let target = if i as i32 == hm { 1.0 } else { 0.0 };
            m.hover += (target - m.hover) * (1.0 - (-HOVER_SPEED * dt).exp());
            let c = Color::from_rgb(0.3, 0.75, 1.0).lerp(Color::from_rgb(1.0, 0.82, 0.35), f64::from(m.hover));
            m.rim.set_albedo(Color::from_rgba(c.r, c.g, c.b, alpha * (0.45 + 0.4 * pulse * (1.0 - m.hover) + 0.55 * m.hover)));
        }
        self.place();
    }
}

#[godot_api]
impl DungeonView {
    /// Gewählt: Dungeon-Index (Dungeon wählen) bzw. Nummer der Option des Raums (Raum wählen).
    #[signal]
    fn picked(index: i64);

    /// Klick in der reinen Ansicht: schließen.
    #[signal]
    fn dismissed();

    /// Trefferpunkt des Mausstrahls (Welt): Dungeon bzw. wählbarer Raum darunter wird hervorgehoben.
    #[func]
    pub fn update_pointer(&mut self, world: Vector3) {
        let Some(content) = self.content.as_ref() else { return };
        let local = content.get_global_transform().affine_inverse() * world;
        let p = Vector2::new(local.x, -local.y);
        let panel = self.panels.iter().position(|pn| Rect2::new(pn.center - pn.size * 0.5, pn.size).contains_point(p));
        let mark = self.marks.iter().position(|m| m.rect.grow(6.0).contains_point(p));
        self.hovered = (panel.map_or(-1, |i| i as i32), mark.map_or(-1, |i| i as i32));
    }

    /// Maus verlässt die Ansicht.
    #[func]
    pub fn set_hovered(&mut self, hovered: bool) {
        if !hovered {
            self.hovered = (-1, -1);
        }
    }

    /// Klick: wählt je nach Modus; in der reinen Ansicht schließt jeder Klick.
    #[func]
    pub fn activate(&mut self) {
        if self.closing {
            return;
        }
        match &self.mode {
            DungeonMode::View => {
                self.signals().dismissed().emit();
            }
            DungeonMode::PickDungeon => {
                if self.hovered.0 >= 0 {
                    let i = i64::from(self.hovered.0);
                    self.signals().picked().emit(i);
                }
            }
            DungeonMode::PickRoom(_) => {
                if let Some(m) = usize::try_from(self.hovered.1).ok().and_then(|i| self.marks.get(i)) {
                    let o = m.option;
                    self.signals().picked().emit(o);
                }
            }
        }
    }

    /// Schließen (0,15 s ausblenden), danach freigeben.
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
}

impl DungeonView {
    /// Neue Ansicht (noch nicht in der Szene).
    pub fn create(dungeons: Vec<Dungeon>, mode: DungeonMode, title: &str) -> Gd<DungeonView> {
        let mut v = DungeonView::new_alloc();
        {
            let mut m = v.bind_mut();
            m.dungeons = dungeons;
            m.mode = mode;
            m.title = title.to_string();
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

    fn quad(size: Vector2, mat: &Gd<StandardMaterial3D>, at: Vector3) -> Gd<MeshInstance3D> {
        let mut q = QuadMesh::new_gd();
        q.set_size(size);
        q.set_material(mat);
        let mut mesh = MeshInstance3D::new_alloc();
        mesh.set_mesh(&q);
        mesh.set_cast_shadows_setting(ShadowCastingSetting::OFF);
        mesh.set_position(at);
        mesh
    }

    /// Bauen: Maße in Bezugs-Pixeln, Ursprung = Bildmitte oben, y nach unten (im Raum: −y).
    fn build(&mut self) {
        let mut content = Node3D::new_alloc();
        self.base_mut().add_child(&content);
        let backdrop = Self::material(Color::from_rgba(0.0, 0.0, 0.0, 0.0), 37);
        content.add_child(&Self::quad(Vector2::new(6000.0, 4000.0), &backdrop, Vector3::new(0.0, -540.0, -0.05)));

        let n = self.dungeons.len().max(1) as f32;
        let aspect = TEX_W as f32 / TEX_H as f32;
        let panel_h = MAX_PANEL_H.min((ROW_W - PANEL_GAP * (n - 1.0)) / n / aspect);
        let panel_w = panel_h * aspect;
        let total_w = panel_w * n + PANEL_GAP * (n - 1.0);
        let dungeons = self.dungeons.clone();
        for (i, d) in dungeons.iter().enumerate() {
            let center = Vector2::new(-total_w * 0.5 + panel_w * 0.5 + i as f32 * (panel_w + PANEL_GAP), PANEL_Y);
            let size = Vector2::new(panel_w, panel_h);
            let rim = Self::material(Color::from_rgba(0.0, 0.0, 0.0, 0.0), 38);
            content.add_child(&Self::quad(size + Vector2::splat(14.0), &rim, Vector3::new(center.x, -center.y, 0.0)));
            let mut sprite = Sprite3D::new_alloc();
            if let Some(tex) = dungeon_texture(d) {
                sprite.set_texture(&tex);
            }
            sprite.set_pixel_size(panel_w / TEX_W as f32);
            sprite.set_billboard_mode(BillboardMode::DISABLED);
            sprite.set_draw_flag(SpriteFlags::SHADED, false);
            sprite.set_draw_flag(SpriteFlags::DISABLE_DEPTH_TEST, true);
            sprite.set_texture_filter(TextureFilter::LINEAR_WITH_MIPMAPS);
            sprite.set_render_priority(39);
            sprite.set_position(Vector3::new(center.x, -center.y, 0.01));
            sprite.set_modulate(Color::from_rgba(1.0, 1.0, 1.0, 0.0));
            content.add_child(&sprite);
            let top_left = center - size * 0.5;
            let rooms: Vec<Rect2> = d
                .layout()
                .iter()
                .map(|r| Rect2::new(top_left + Vector2::new(r[0] * panel_w, r[1] * panel_h), Vector2::new(r[2] * panel_w, r[3] * panel_h)))
                .collect();
            // wählbare Räume: leuchtender Rahmen, darunter die Steinkarte
            if let DungeonMode::PickRoom(choices) = &self.mode {
                for (name, option) in choices {
                    let Some(j) = d.index_of(name) else { continue };
                    let r = rooms[j];
                    // Leuchtrahmen: vier Leisten vor der Karte um den Raum
                    let mat = Self::material(Color::from_rgba(0.3, 0.75, 1.0, 0.0), 40);
                    let t = 9.0;
                    let (x0, y0, x1, y1) = (r.position.x - t, r.position.y - t, r.end().x + t, r.end().y + t);
                    for (cx, cy, w, h) in [
                        ((x0 + x1) * 0.5, y0 + t * 0.5, x1 - x0, t),
                        ((x0 + x1) * 0.5, y1 - t * 0.5, x1 - x0, t),
                        (x0 + t * 0.5, (y0 + y1) * 0.5, t, y1 - y0),
                        (x1 - t * 0.5, (y0 + y1) * 0.5, t, y1 - y0),
                    ] {
                        content.add_child(&Self::quad(Vector2::new(w, h), &mat, Vector3::new(cx, -cy, 0.02)));
                    }
                    self.marks.push(RoomMark { rect: r, option: *option, rim: mat, hover: 0.0 });
                }
            }
            self.panels.push(Panel { center, size, rim, sprite, hover: 0.0 });
        }
        // Titel über den Dungeons
        let mut label = Label3D::new_alloc();
        label.set_text(&self.title);
        label.set_font_size(54);
        label.set_outline_size(14);
        label.set_pixel_size(1.0);
        label.set_draw_flag(LabelFlags::SHADED, false);
        label.set_draw_flag(LabelFlags::DISABLE_DEPTH_TEST, true);
        label.set_render_priority(41);
        label.set_position(Vector3::new(0.0, -(PANEL_Y - panel_h * 0.5 - 52.0), 0.02));
        label.set_visible(!self.title.is_empty());
        content.add_child(&label);
        self.label = Some(label);

        let mut hit = Area3D::new_alloc();
        hit.set_collision_layer(CONTROL_LAYER);
        hit.set_collision_mask(0);
        hit.set_monitoring(false);
        hit.set_monitorable(false);
        let mut shape = BoxShape3D::new_gd();
        shape.set_size(Vector3::new(6000.0, 4000.0, 2.0));
        let mut col = CollisionShape3D::new_alloc();
        col.set_shape(&shape);
        col.set_position(Vector3::new(0.0, -540.0, 0.0));
        hit.add_child(&col);
        content.add_child(&hit);
        self.hit = Some(hit);
        self.backdrop = Some(backdrop);
        self.content = Some(content);
        self.place();
    }

    /// Vor die Kamera: Ursprung (Bildmitte oben) an den oberen Bildrand, Größe in Pixeln fest.
    fn place(&mut self) {
        let Some(viewport) = self.base().get_viewport() else { return };
        let Some(cam) = viewport.get_camera_3d() else { return };
        let screen = viewport.get_visible_rect().size;
        let view_h = 2.0 * DISTANCE * (cam.get_fov().to_radians() * 0.5).tan();
        let world_per_ref = view_h / REF_HEIGHT;
        let basis = cam.get_global_transform().basis.orthonormalized();
        let origin = cam.project_position(Vector2::new(screen.x * 0.5, 0.0), DISTANCE);
        self.base_mut().set_global_transform(Transform3D::new(basis.scaled(Vector3::splat(world_per_ref)), origin));
    }
}

/// Zwischenspeicher leeren (beim Abbau des Tisches).
pub fn clear() {
    CACHE.with(|c| c.borrow_mut().clear());
}
