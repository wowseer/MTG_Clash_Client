//! Zone: ein Bereich auf dem Tisch, der Karten hält und in Slots anordnet.
//!
//! - `Hand`: Fächer/Bogen
//! - `Pile`: Stapel (Deck, Ablage)
//! - `Battlefield`: Raster
//!
//! Die Zone ist eine Area3D auf `ZONE_LAYER`; der Controller findet Drop-Ziele per
//! Raycast. Slots werden im lokalen Raum der Zone berechnet und mit der globalen
//! Transform der Zone verrechnet – eine geneigte Hand-Zone ergibt so automatisch
//! einen zur Kamera gedrehten Fächer.

use godot::classes::base_material_3d::{ShadingMode, Transparency};
use godot::classes::{
    Area3D, BoxShape3D, CollisionShape3D, IArea3D, MeshInstance3D, PlaneMesh, StandardMaterial3D,
};
use godot::prelude::*;

use std::collections::HashMap;

use crate::game::mana::ManaType;

use super::card3d::{CARD_HEIGHT, CARD_THICKNESS, CARD_WIDTH, Card3D};
use super::layers::ZONE_LAYER;

pub const ZONE_GROUP: &str = "card_zones";

#[derive(GodotConvert, Var, Export, Clone, Copy, PartialEq, Eq, Debug)]
#[godot(via = i64)]
pub enum ZoneKind {
    Hand,
    Pile,
    Battlefield,
    /// Ressourcen-Reihe: eine Spalte je Manafarbe, Karten darin aufgefächert.
    Resources,
    /// Stack: Zauber auf dem Stack, nach rechts aufgefächert (oberster rechts oben).
    Stack,
}

#[derive(GodotClass)]
#[class(base=Area3D)]
pub struct Zone {
    #[export]
    kind: ZoneKind,
    /// Ausdehnung der Drop-Fläche (x = Breite, y = Höhe der Kollisionsbox, z = Tiefe).
    #[export]
    size: Vector3,
    /// Dürfen Karten hier abgelegt werden?
    #[export]
    accepts_drops: bool,
    /// Dürfen Karten aus dieser Zone gegriffen werden?
    #[export]
    cards_draggable: bool,
    /// Klick auf eine Karte zieht die oberste Karte auf die Hand (Deck).
    #[export]
    click_to_draw: bool,
    /// Karten offen (true) oder verdeckt (false) anzeigen.
    #[export]
    face_up: bool,
    /// Zonenfläche sichtbar markieren und beim Drüberziehen hervorheben.
    #[export]
    show_area: bool,

    #[export_group(name = "Hand")]
    #[export]
    hand_radius: f32,
    #[export]
    hand_card_angle_degrees: f32,
    #[export]
    hand_max_spread_degrees: f32,

    #[export_group(name = "Pile")]
    #[export]
    pile_spacing: f32,

    #[export_group(name = "Battlefield")]
    #[export]
    grid_columns: i32,
    #[export]
    grid_spacing: Vector2,
    /// Karten bleiben dort liegen, wo sie abgelegt werden (statt Raster).
    #[export]
    free_placement: bool,

    cards: Vec<Gd<Card3D>>,
    /// Freie Positionen (lokal x/z) je Karte.
    free_positions: HashMap<InstanceId, Vector2>,
    /// Angelegte Karten (Ausrüstung) -> Trägerkarte. Sie folgen dem Träger beim Layout.
    attachments: HashMap<InstanceId, Gd<Card3D>>,
    highlight_material: Option<Gd<StandardMaterial3D>>,
    highlighted: bool,

    base: Base<Area3D>,
}

#[godot_api]
impl IArea3D for Zone {
    fn init(base: Base<Area3D>) -> Self {
        Self {
            kind: ZoneKind::Battlefield,
            size: Vector3::new(3.0, 0.2, 2.0),
            accepts_drops: true,
            cards_draggable: true,
            click_to_draw: false,
            face_up: true,
            hand_radius: 5.0,
            hand_card_angle_degrees: 6.0,
            hand_max_spread_degrees: 40.0,
            pile_spacing: 0.004,
            grid_columns: 6,
            grid_spacing: Vector2::new(0.78, 1.02),
            free_placement: false,
            show_area: false,
            cards: Vec::new(),
            free_positions: HashMap::new(),
            attachments: HashMap::new(),
            highlight_material: None,
            highlighted: false,
            base,
        }
    }

    fn ready(&mut self) {
        self.base_mut().add_to_group(ZONE_GROUP);
        self.base_mut().set_collision_layer(ZONE_LAYER);
        self.base_mut().set_collision_mask(0);
        self.base_mut().set_monitoring(false);
        self.base_mut().set_monitorable(false);

        let size = self.size;
        let mut shape = BoxShape3D::new_gd();
        shape.set_size(size);
        let mut collision = CollisionShape3D::new_alloc();
        collision.set_shape(&shape);
        self.base_mut().add_child(&collision);

        if !self.show_area {
            return;
        }
        // Dezente Markierung der Zone + Highlight beim Drüberziehen.
        let mut mat = StandardMaterial3D::new_gd();
        mat.set_shading_mode(ShadingMode::UNSHADED);
        mat.set_transparency(Transparency::ALPHA);
        mat.set_albedo(self.idle_color());
        let mut plane = PlaneMesh::new_gd();
        plane.set_size(Vector2::new(size.x, size.z));
        plane.set_material(&mat);
        let mut mesh = MeshInstance3D::new_alloc();
        mesh.set_name("Highlight");
        mesh.set_mesh(&plane);
        mesh.set_position(Vector3::new(0.0, 0.004, 0.0));
        mesh.set_cast_shadows_setting(godot::classes::geometry_instance_3d::ShadowCastingSetting::OFF);
        self.base_mut().add_child(&mesh);
        self.highlight_material = Some(mat);
    }
}

#[godot_api]
impl Zone {
    /// Fügt eine Karte ein (index < 0 = ans Ende) und ordnet alle Karten neu an.
    /// `delay` verzögert nur die Animation der neuen Karte (z. B. zum Austeilen).
    #[func]
    pub fn add_card(&mut self, mut card: Gd<Card3D>, index: i64, delay: f64) {
        self.cards.retain(|c| c != &card);
        let idx = if index < 0 {
            self.cards.len()
        } else {
            (index as usize).min(self.cards.len())
        };
        self.cards.insert(idx, card.clone());
        card.bind_mut().set_zone(Some(self.to_gd()));
        self.layout_with_delay(Some((&card, delay)));
    }

    /// Legt eine Karte frei am Weltpunkt ab (nur mit `free_placement`,
    /// sonst wie `add_card` ans Ende). Die Position wird in der Zone begrenzt.
    #[func]
    pub fn add_card_at(&mut self, card: Gd<Card3D>, world_point: Vector3, delay: f64) {
        if self.free_placement {
            let local = self.base().get_global_transform().affine_inverse() * world_point;
            let hx = (self.size.x - CARD_WIDTH) * 0.5;
            let hz = (self.size.z - CARD_HEIGHT) * 0.5;
            let pos = Vector2::new(local.x.clamp(-hx, hx.max(-hx)), local.z.clamp(-hz, hz.max(-hz)));
            self.free_positions.insert(card.instance_id(), pos);
        }
        self.add_card(card, -1, delay);
    }

    #[func]
    pub fn is_free_placement(&self) -> bool {
        self.free_placement
    }

    /// Entfernt eine Karte; gibt ihren bisherigen Index zurück (-1 falls nicht enthalten).
    #[func]
    pub fn remove_card(&mut self, mut card: Gd<Card3D>) -> i64 {
        let Some(idx) = self.cards.iter().position(|c| c == &card) else {
            return -1;
        };
        self.cards.remove(idx);
        card.bind_mut().set_zone(None);
        // Angelegt gewesen? Verbindung lösen. Träger entfernt? Seine Ausrüstung bleibt
        // an seinem alten Platz liegen.
        let id = card.instance_id();
        self.attachments.remove(&id);
        let orphans: Vec<InstanceId> = self.attachments.iter().filter(|(_, h)| **h == card).map(|(e, _)| *e).collect();
        let host_pos = self.free_positions.get(&id).copied();
        for (k, e) in orphans.into_iter().enumerate() {
            self.attachments.remove(&e);
            if let Some(p) = host_pos {
                self.free_positions.insert(e, p + Vector2::new(0.25 * k as f32, 0.0));
            }
        }
        self.layout();
        idx as i64
    }

    /// Fügt eine Karte hinzu; bei freier Platzierung ohne bekannte Position wird
    /// automatisch ein freier Platz gesucht (z. B. für Karten, die vom Stack kommen).
    pub fn add_card_free(&mut self, card: Gd<Card3D>, delay: f64) {
        let id = card.instance_id();
        if self.free_placement && !self.free_positions.contains_key(&id) {
            let spot = self.find_free_spot();
            self.free_positions.insert(id, spot);
        }
        self.add_card(card, -1, delay);
    }

    /// Erster Rasterplatz (von der Tischmitte aus), an dem noch keine Karte liegt.
    fn find_free_spot(&self) -> Vector2 {
        let step = Vector2::new(CARD_WIDTH + 0.2, CARD_HEIGHT + 0.15);
        let half = Vector2::new((self.size.x - CARD_WIDTH) * 0.5, (self.size.z - CARD_HEIGHT) * 0.5);
        let cols = ((half.x * 2.0) / step.x).floor() as i32 + 1;
        let rows = ((half.y * 2.0) / step.y).floor() as i32 + 1;
        let taken: Vec<Vector2> = self
            .cards
            .iter()
            .filter(|c| !self.attachments.contains_key(&c.instance_id()))
            .filter_map(|c| self.free_positions.get(&c.instance_id()).copied())
            .collect();
        for r in 0..rows {
            // Spalten von der Mitte nach außen: 0, +1, -1, +2, -2 …
            for k in 0..cols {
                let offset = if k % 2 == 0 { k / 2 } else { -(k + 1) / 2 };
                let p = Vector2::new(offset as f32 * step.x, -half.y + r as f32 * step.y);
                if p.x.abs() > half.x {
                    continue;
                }
                if taken.iter().all(|t| t.distance_to(p) > 0.6) {
                    return p;
                }
            }
        }
        Vector2::ZERO
    }

    /// Legt `card` (Ausrüstung) an `host` an: Sie liegt danach versetzt unter dem
    /// Träger und folgt ihm. Beide müssen in dieser Zone liegen.
    pub fn attach(&mut self, card: Gd<Card3D>, host: Gd<Card3D>) {
        if self.cards.contains(&card) && self.cards.contains(&host) && card != host {
            self.attachments.insert(card.instance_id(), host);
            self.layout();
        }
    }

    /// Ordnet alle Karten neu an (Tween zu ihren Slots).
    #[func]
    pub fn layout(&mut self) {
        self.layout_with_delay(None);
    }

    #[func]
    pub fn get_card_count(&self) -> i64 {
        self.cards.len() as i64
    }

    /// Oberste/letzte Karte der Zone.
    #[func]
    pub fn get_top_card(&self) -> Option<Gd<Card3D>> {
        self.cards.last().cloned()
    }

    #[func]
    pub fn get_cards(&self) -> Array<Gd<Card3D>> {
        self.cards.iter().cloned().collect()
    }

    #[func]
    pub fn accepts_drops(&self) -> bool {
        self.accepts_drops
    }

    #[func]
    pub fn cards_draggable(&self) -> bool {
        self.cards_draggable
    }

    #[func]
    pub fn click_to_draw(&self) -> bool {
        self.click_to_draw
    }

    #[func]
    pub fn set_highlighted(&mut self, on: bool) {
        if self.highlighted == on {
            return;
        }
        self.highlighted = on;
        let color = if on { Self::active_color() } else { self.idle_color() };
        if let Some(mat) = self.highlight_material.as_mut() {
            mat.set_albedo(color);
        }
    }

    /// Einfüge-Index für eine Karte, die am Weltpunkt `world_point` abgelegt wird.
    #[func]
    pub fn insertion_index(&self, world_point: Vector3) -> i64 {
        let n = self.cards.len();
        let local = self.base().get_global_transform().affine_inverse() * world_point;
        let idx = match self.kind {
            ZoneKind::Pile | ZoneKind::Resources | ZoneKind::Stack => n,
            ZoneKind::Hand => {
                // Zwischen welche Slots (nach x sortiert) fällt der Punkt?
                (0..n)
                    .filter(|&i| self.slot_transform(i, n + 1).origin.x < local.x)
                    .count()
            }
            ZoneKind::Battlefield => {
                // Index des nächstgelegenen Slots in einem Raster mit n+1 Karten.
                (0..=n)
                    .min_by(|&a, &b| {
                        let da = self.slot_transform(a, n + 1).origin.distance_squared_to(local);
                        let db = self.slot_transform(b, n + 1).origin.distance_squared_to(local);
                        da.total_cmp(&db)
                    })
                    .unwrap_or(n)
            }
        };
        idx as i64
    }
}

impl Zone {
    /// Alle Karten (unterste/erste zuerst).
    pub fn cards(&self) -> Vec<Gd<Card3D>> {
        self.cards.clone()
    }

    /// Ruhezustand: dezente Markierung, die Hand bleibt unsichtbar.
    fn idle_color(&self) -> Color {
        // Leicht abgedunkelte, "eingelassene" Fläche auf dem Filz.
        let alpha = if self.kind == ZoneKind::Hand { 0.0 } else { 0.16 };
        Color::from_rgba(0.0, 0.0, 0.0, alpha)
    }

    fn active_color() -> Color {
        Color::from_rgba(0.4, 1.0, 0.6, 0.22)
    }

    fn layout_with_delay(&mut self, delayed: Option<(&Gd<Card3D>, f64)>) {
        let zone_xf = self.base().get_global_transform();
        let n = self.cards.len();

        let tap = |local: &mut Transform3D, card: &Gd<Card3D>| {
            if card.bind().is_tapped() {
                // Getappt: um 90° im Uhrzeigersinn gedreht
                local.basis = local.basis * Basis::from_axis_angle(Vector3::UP, -std::f32::consts::FRAC_PI_2);
            }
        };

        // 1) Freie (nicht angelegte) Karten bekommen eigene Slots.
        let base: Vec<usize> = (0..n)
            .filter(|&i| !self.attachments.contains_key(&self.cards[i].instance_id()))
            .collect();
        let nb = base.len();
        // Ressourcen-Spalten nur aus freien Karten (Auren auf Ressourcen hängen darunter)
        let resource_slots = if self.kind == ZoneKind::Resources { self.resource_slots(&base) } else { Vec::new() };
        let mut targets: HashMap<InstanceId, Transform3D> = HashMap::new();
        for (bi, &i) in base.iter().enumerate() {
            let card = &self.cards[i];
            let mut local = match self.free_positions.get(&card.instance_id()) {
                _ if self.kind == ZoneKind::Resources => resource_slots[bi],
                Some(p) if self.free_placement => {
                    // Später abgelegte Karten liegen minimal höher (kein Z-Fighting bei Überlappung).
                    Transform3D::new(Basis::IDENTITY, Vector3::new(p.x, CARD_THICKNESS * 0.5 + i as f32 * 0.002, p.y))
                }
                _ => self.slot_transform(bi, nb),
            };
            if self.attachments.values().any(|h| h == card) {
                local.origin.y += 0.01; // Träger liegt über seiner Ausrüstung
            }
            tap(&mut local, card);
            targets.insert(card.instance_id(), local);
        }

        // 2) Angelegte Karten (Ausrüstung, Auren): versetzt unter ihrem Träger, im
        //    Koordinatensystem des Trägers. Mehrere Durchläufe für verschachtelte
        //    Anhänge (z. B. Aura auf einer Ausrüstung, die an einer Kreatur hängt).
        let mut per_host: HashMap<InstanceId, usize> = HashMap::new();
        for _pass in 0..4 {
            for i in 0..n {
                let card = &self.cards[i];
                let id = card.instance_id();
                if targets.contains_key(&id) {
                    continue;
                }
                let Some(host) = self.attachments.get(&id) else {
                    continue;
                };
                let Some(host_xf) = targets.get(&host.instance_id()).copied() else {
                    continue; // Träger noch nicht platziert -> nächster Durchlauf
                };
                let k = per_host.entry(host.instance_id()).or_insert(0);
                let offset = Vector3::new(0.12 + *k as f32 * 0.09, -0.004, -0.16 - *k as f32 * 0.09);
                *k += 1;
                targets.insert(id, host_xf * Transform3D::new(Basis::IDENTITY, offset));
            }
        }

        for i in 0..n {
            let mut card = self.cards[i].clone();
            let local = targets.get(&card.instance_id()).copied().unwrap_or(Transform3D::IDENTITY);
            let delay = match delayed {
                Some((c, d)) if c == &card => d,
                _ => 0.0,
            };
            card.bind_mut().move_to(zone_xf * local, delay);
        }
    }

    /// Slots der Ressourcen-Reihe: Spalten nach Manafarbe (in fester Farbreihenfolge),
    /// innerhalb einer Spalte nach vorn (+z) aufgefächert.
    fn resource_slots(&self, base: &[usize]) -> Vec<Transform3D> {
        let types: Vec<ManaType> = base.iter().map(|&i| self.cards[i].bind().produced_mana()).collect();
        let columns: Vec<ManaType> = ManaType::ALL.into_iter().filter(|t| types.contains(t)).collect();
        let step_x = CARD_WIDTH + 0.12;
        let cascade = 0.2;
        let width = (columns.len().max(1) - 1) as f32 * step_x;
        let mut in_column = [0usize; 6];
        types
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let col = columns.iter().position(|c| c == t).unwrap_or(0);
                let row = in_column[t.index()];
                in_column[t.index()] += 1;
                let x = -width * 0.5 + col as f32 * step_x;
                let z = -self.size.z * 0.5 + CARD_HEIGHT * 0.5 + row as f32 * cascade;
                let y = CARD_THICKNESS * 0.5 + row as f32 * 0.004 + i as f32 * 0.0005;
                Transform3D::new(Basis::IDENTITY, Vector3::new(x, y, z))
            })
            .collect()
    }

    /// Lokale Slot-Transform für Karte `i` von `n`.
    fn slot_transform(&self, i: usize, n: usize) -> Transform3D {
        let flip = if self.face_up {
            Basis::IDENTITY
        } else {
            Basis::from_axis_angle(Vector3::BACK, std::f32::consts::PI)
        };
        let half_t = CARD_THICKNESS * 0.5;

        match self.kind {
            ZoneKind::Hand => {
                let step = if n > 1 {
                    self.hand_card_angle_degrees
                        .min(self.hand_max_spread_degrees / (n - 1) as f32)
                        .to_radians()
                } else {
                    0.0
                };
                let a = (i as f32 - (n as f32 - 1.0) * 0.5) * step;
                let r = self.hand_radius;
                // Mittlere Karte oben (-Z), äußere Karten sinken ab und drehen nach außen.
                let pos = Vector3::new(r * a.sin(), half_t + i as f32 * 0.004, r * (1.0 - a.cos()));
                let basis = Basis::from_axis_angle(Vector3::UP, -a) * flip;
                Transform3D::new(basis, pos)
            }
            ZoneKind::Pile => {
                // Deterministische leichte Unordnung im Stapel.
                let jitter = (((i as u32).wrapping_mul(2654435761) >> 16) & 0xff) as f32 / 255.0 - 0.5;
                let basis = Basis::from_axis_angle(Vector3::UP, jitter * 0.08) * flip;
                let pos = Vector3::new(0.0, half_t + i as f32 * self.pile_spacing, 0.0);
                Transform3D::new(basis, pos)
            }
            ZoneKind::Stack => {
                let x = (i as f32 - (n as f32 - 1.0) * 0.5) * 0.32;
                Transform3D::new(flip, Vector3::new(x, half_t + 0.05 + i as f32 * 0.012, -(i as f32) * 0.06))
            }
            ZoneKind::Battlefield | ZoneKind::Resources => {
                let cols = (self.grid_columns.max(1) as usize).min(n.max(1));
                let rows = n.div_ceil(cols).max(1);
                let row = i / cols;
                let col = i % cols;
                let in_row = cols.min(n - row * cols);
                let x = (col as f32 - (in_row as f32 - 1.0) * 0.5) * self.grid_spacing.x;
                let z = (row as f32 - (rows as f32 - 1.0) * 0.5) * self.grid_spacing.y;
                Transform3D::new(flip, Vector3::new(x, half_t, z))
            }
        }
    }
}

