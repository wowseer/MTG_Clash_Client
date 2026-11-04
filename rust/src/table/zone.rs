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

use crate::game::cards::CardKind;

use super::card3d::{CARD_HEIGHT, CARD_THICKNESS, CARD_WIDTH, TILE_HEIGHT, Card3D};
use super::layers::ZONE_LAYER;

pub const ZONE_GROUP: &str = "card_zones";
/// Stack-Stapel: Versatz je älterem Eintrag (nach links, nach oben; Zonen-Einheiten), Drehung
/// je Eintrag und wie viele ältere Einträge noch weiter versetzt werden (danach liegen sie gleich).
const STACK_OFFSET: Vector2 = Vector2::new(0.2, 0.11);
const STACK_TURN_DEGREES: f32 = 3.5;
const STACK_MAX_SHOWN: f32 = 6.0;
/// Drehung getappter Karten im Uhrzeigersinn (Grad) – nur leicht schräg statt quer.
pub const TAP_ANGLE_DEGREES: f32 = 15.0;
/// Automatische Anordnung: Abstand zwischen Karten (Anteil der Kartenbreite), Beginn der hinteren
/// Reihe rechts neben dem Medaillon (lokal x) und kleinste Verkleinerung (Anteil von
/// [member card_scale]), bevor Karten sich überlappen.
const AUTO_GAP: f32 = 0.12;
const BACK_ROW_START: f32 = 1.3;
const MIN_SHRINK: f32 = 0.45;
/// Ressourcen-Reihe: gleiche Länder überlappen um diesen Anteil der Kartenbreite versetzt;
/// Abstand zwischen den Gruppen.
const LAND_STACK_STEP: f32 = 0.3;
const LAND_GROUP_GAP: f32 = 0.12;
/// Cover-Flow ([member Zone.coverflow]): ab diesem seitlichen Abstand (Zonen-Einheiten) von der
/// Mitte sind Karten ganz zur Seite gedreht ([`COVERFLOW_YAW`] Grad) und um die Hälfte von
/// [`COVERFLOW_ROLL`] im Bogen geneigt; je Einheit Abstand liegen sie [`COVERFLOW_DEPTH`]
/// weiter hinten (größer als sin(Drehung), damit sich Nachbarn nicht durchdringen), höchstens
/// [`COVERFLOW_MAX_DEPTH`]. Jenseits von [`COVERFLOW_EDGE`] stapeln sie sich dicht am Rand.
const COVERFLOW_REACH: f32 = 0.9;
const COVERFLOW_YAW: f32 = 28.0;
const COVERFLOW_ROLL: f32 = 7.0;
const COVERFLOW_DEPTH: f32 = 0.55;
const COVERFLOW_MAX_DEPTH: f32 = 1.1;
const COVERFLOW_EDGE: f32 = 1.75;
const COVERFLOW_EDGE_STEP: f32 = 0.035;
const COVERFLOW_DROP: f32 = 0.06;

/// Art der Zone: 0 = Hand (Fächer), 1 = Stapel, 2 = Spielfeld, 3 = Ressourcen-Reihe, 4 = Stack.
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

/// Ein Bereich auf dem Tisch, der Karten hält und anordnet: Hand, Deck, Friedhof,
/// Spielfeld, Ressourcen-Reihe oder Stack. Karten gleiten per Tween an ihre Plätze;
/// angelegte Karten (Ausrüstung, Auren) liegen versetzt unter ihrem Träger.
/// Die Zone ist eine Area3D auf Kollisions-Layer 2 (Drop-Ziel für Raycasts).
#[derive(GodotClass)]
#[class(base=Area3D)]
pub struct Zone {
    /// Art der Anordnung: 0 = Hand, 1 = Stapel, 2 = Spielfeld, 3 = Ressourcen-Reihe, 4 = Stack.
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

    /// Hand: Radius des Fächerbogens.
    #[export_group(name = "Hand")]
    #[export]
    hand_radius: f32,
    /// Hand: Winkel zwischen zwei Karten (Grad).
    #[export]
    hand_card_angle_degrees: f32,
    /// Hand: maximaler Gesamtwinkel des Fächers (Grad).
    #[export]
    hand_max_spread_degrees: f32,
    /// Hand als Cover-Flow (Kartenwahl-Bühne): Karten im Abstand [member coverflow_step]
    /// nebeneinander, die mittlere vorn und gerade, die übrigen zur Seite gedreht, nach hinten
    /// versetzt und überlappend; am Rand dicht gestapelt. Ersetzt den Fächerbogen.
    #[export]
    coverflow: bool,
    /// Cover-Flow: Abstand der Karten (Zonen-Einheiten, kleiner als die Kartenbreite = überlappend).
    #[export]
    coverflow_step: f32,
    /// Cover-Flow: Verschiebung aller Karten nach rechts (Zonen-Einheiten); was dadurch in die
    /// Mitte rückt, dreht sich nach vorn (Schieberegler der Bühne).
    #[export]
    coverflow_scroll: f32,

    /// Stapel: Höhe je Karte.
    #[export_group(name = "Pile")]
    #[export]
    pile_spacing: f32,

    /// Spielfeld im Rastermodus: Anzahl Spalten.
    #[export_group(name = "Battlefield")]
    #[export]
    grid_columns: i32,
    /// Spielfeld im Rastermodus: Abstand der Karten (x, z).
    #[export]
    grid_spacing: Vector2,
    /// Karten bleiben dort liegen, wo sie abgelegt werden (statt Raster).
    #[export]
    free_placement: bool,
    /// Automatische Anordnung nach Kartenart (hat Vorrang vor Raster und freier Ablage),
    /// aus Sicht der Seite (lokal −z = zur Tischmitte = „oben“): Kreaturen vorn mittig an der
    /// Mittellinie, alle anderen bleibenden Karten hinten rechts neben dem Medaillon.
    /// Länder liegen in einer eigenen Ressourcen-Zone (hinten links).
    #[export]
    auto_arrange: bool,
    /// Größe der Karten in dieser Zone (1 = normal). Spielfeld und Ressourcen verkleinern sie
    /// automatisch, wenn sonst nicht alle in eine Reihe passen.
    #[export]
    card_scale: f32,
    /// Karten als Kachel zeigen (nur Name und Bild, Stärke/Widerstand unten rechts; siehe
    /// [method Card3D.set_tile]) – Form der Karten auf dem Spielfeld.
    #[export]
    tile_cards: bool,
    /// Karten dieser Zone beim Hovern selbst groß vor die Kamera holen (eigene Hand); sonst zeigt
    /// die Kartenansicht eine Kopie.
    #[export]
    zoom_cards: bool,

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
            card_scale: 1.0,
            tile_cards: false,
            zoom_cards: false,
            size: Vector3::new(3.0, 0.2, 2.0),
            accepts_drops: true,
            cards_draggable: true,
            click_to_draw: false,
            face_up: true,
            hand_radius: 5.0,
            hand_card_angle_degrees: 6.0,
            coverflow: false,
            coverflow_step: 0.3,
            coverflow_scroll: 0.0,
            hand_max_spread_degrees: 40.0,
            pile_spacing: 0.004,
            grid_columns: 6,
            grid_spacing: Vector2::new(0.78, 1.02),
            free_placement: false,
            auto_arrange: false,
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
    /// Legt `card` an `host` an (Aura, Ausrüstung, Befestigung; CR 301.5, 303.4): Sie liegt
    /// dann versetzt unter dem Träger und folgt ihm beim Anordnen. `host` = null löst die
    /// Verbindung. Beide Karten müssen in dieser Zone liegen; ordnet nur bei Änderung neu an.
    #[func]
    pub fn set_attached(&mut self, card: Gd<Card3D>, host: Option<Gd<Card3D>>) {
        let id = card.instance_id();
        let host = host.filter(|h| h != &card && self.cards.contains(h));
        if self.attachments.get(&id) == host.as_ref() {
            return;
        }
        match host {
            Some(h) => self.attachments.insert(id, h),
            None => self.attachments.remove(&id),
        };
        self.layout();
    }

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

    /// Bleiben Karten dort liegen, wo sie abgelegt werden (statt Raster)?
    #[func]
    pub fn is_free_placement(&self) -> bool {
        self.free_placement
    }

    /// Ist das ein Handfächer (eigene Hand, Gegnerhand, Fächer einer 3D-Bühne)?
    #[func]
    pub fn is_hand(&self) -> bool {
        self.kind == ZoneKind::Hand
    }

    /// Art der Zone (für Rust-Code; in Godot über die Eigenschaft `kind`).
    pub fn zone_kind(&self) -> ZoneKind {
        self.kind
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

    /// Ordnet alle Karten neu an (Tween zu ihren Slots).
    #[func]
    pub fn layout(&mut self) {
        self.layout_with_delay(None);
    }

    /// Anzahl der Karten in der Zone.
    #[func]
    pub fn get_card_count(&self) -> i64 {
        self.cards.len() as i64
    }

    /// Oberste/letzte Karte der Zone.
    #[func]
    pub fn get_top_card(&self) -> Option<Gd<Card3D>> {
        self.cards.last().cloned()
    }

    /// Alle Karten der Zone, unterste bzw. erste zuerst.
    #[func]
    pub fn get_cards(&self) -> Array<Gd<Card3D>> {
        self.cards.iter().cloned().collect()
    }

    /// Dürfen hier Karten abgelegt werden?
    #[func]
    pub fn accepts_drops(&self) -> bool {
        self.accepts_drops
    }

    /// Dürfen Karten aus dieser Zone gegriffen werden?
    #[func]
    pub fn cards_draggable(&self) -> bool {
        self.cards_draggable
    }

    /// Zieht ein Klick auf eine Karte dieser Zone eine Karte? (nur ohne Regeln)
    #[func]
    pub fn click_to_draw(&self) -> bool {
        self.click_to_draw
    }

    /// Hebt die Zonenfläche hervor (nur sichtbar, wenn [member show_area] aktiv ist).
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
                // Getappt: leicht schräg gestellt
                local.basis = local.basis * Basis::from_axis_angle(Vector3::UP, -TAP_ANGLE_DEGREES.to_radians());
            }
        };

        // 1) Freie (nicht angelegte) Karten bekommen eigene Slots.
        let base: Vec<usize> = (0..n)
            .filter(|&i| !self.attachments.contains_key(&self.cards[i].instance_id()))
            .collect();
        let nb = base.len();
        // Ressourcen-Spalten nur aus freien Karten (Auren auf Ressourcen hängen darunter)
        let resource_slots = if self.kind == ZoneKind::Resources { self.resource_slots(&base) } else { Vec::new() };
        let auto_slots = if self.auto_arrange && self.kind == ZoneKind::Battlefield { self.auto_slots(&base) } else { Vec::new() };
        let mut targets: HashMap<InstanceId, Transform3D> = HashMap::new();
        for (bi, &i) in base.iter().enumerate() {
            let card = &self.cards[i];
            let mut local = match self.free_positions.get(&card.instance_id()) {
                _ if self.kind == ZoneKind::Resources => resource_slots[bi],
                _ if !auto_slots.is_empty() => auto_slots[bi],
                Some(p) if self.free_placement => {
                    // Später abgelegte Karten liegen minimal höher (kein Z-Fighting bei Überlappung).
                    Transform3D::new(Basis::IDENTITY, Vector3::new(p.x, CARD_THICKNESS * 0.5 + i as f32 * 0.002, p.y))
                }
                _ => {
                    let mut xf = self.slot_transform(bi, nb);
                    xf.basis = xf.basis.scaled(Vector3::splat(self.card_scale));
                    xf
                }
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
            card.bind_mut().set_tile(self.tile_cards);
            card.bind_mut().set_zoom(self.zoom_cards);
            card.bind_mut().move_to(zone_xf * local, delay);
        }
    }

    /// Kartenhöhe in dieser Zone (Kachel oder ganze Karte), ohne Skalierung.
    fn card_length(&self) -> f32 {
        if self.tile_cards { TILE_HEIGHT } else { CARD_HEIGHT }
    }

    /// Größe für `n` Karten nebeneinander auf `avail` Breite: höchstens [member card_scale],
    /// höchstens bis [`MIN_SHRINK`] verkleinert.
    fn fit_scale(&self, n: usize, avail: f32) -> f32 {
        let need = n.max(1) as f32 * CARD_WIDTH * (1.0 + AUTO_GAP);
        (avail / need).clamp(self.card_scale * MIN_SHRINK, self.card_scale)
    }

    /// Slots der automatischen Anordnung (siehe [member auto_arrange]) für die freien Karten
    /// `base`, in deren Reihenfolge – Aufbau wie im Video des Users. Lokal: −z = zur
    /// Tischmitte, +x = rechts (aus Sicht der Seite):
    /// - **vordere Reihe(n)**: Kreaturen mittig an der Mittellinie; passen sie nicht in eine
    ///   Reihe, werden sie kleiner, dann zweireihig;
    /// - **hintere Reihe rechts** (neben dem Medaillon): Planeswalker, Schlachten, Artefakte,
    ///   Verzauberungen … – bei Platzmangel kleiner, zuletzt überlappend.
    ///
    /// Länder liegen in der eigenen Ressourcen-Zone (hinten links).
    fn auto_slots(&self, base: &[usize]) -> Vec<Transform3D> {
        let front: Vec<bool> = base.iter().map(|&i| self.cards[i].bind().kind() == CardKind::Creature).collect();
        let n_front = front.iter().filter(|f| **f).count();
        let n_back = front.len() - n_front;
        let (half_x, half_z) = (self.size.x * 0.5, self.size.z * 0.5);
        let len = self.card_length();

        // vorn: eine Reihe, wenn sie mit höchstens 75 % Verkleinerung passt, sonst zwei
        let avail = 2.0 * half_x - 0.2;
        let one_row = (avail / (n_front.max(1) as f32 * CARD_WIDTH * (1.0 + AUTO_GAP))).min(self.card_scale);
        let (rows, s_front) = if one_row >= self.card_scale * 0.75 || n_front <= 1 {
            (1, one_row.max(self.card_scale * MIN_SHRINK))
        } else {
            (2, self.fit_scale(n_front.div_ceil(2), avail))
        };
        let per_row = n_front.div_ceil(rows).max(1);
        let step_f = CARD_WIDTH * s_front * (1.0 + AUTO_GAP);
        let top = -half_z + len * s_front * 0.5 + 0.05;

        // hinten rechts
        let back_avail = half_x - BACK_ROW_START - 0.1;
        let s_back = self.fit_scale(n_back, back_avail);
        let step_b = (CARD_WIDTH * s_back * (1.0 + AUTO_GAP)).min(if n_back > 1 { (back_avail - CARD_WIDTH * s_back) / (n_back - 1) as f32 } else { f32::MAX });
        let back_z = half_z - len * s_back * 0.5 - 0.05;
        let back_x0 = BACK_ROW_START + CARD_WIDTH * s_back * 0.5;

        let (mut k_front, mut k_back) = (0usize, 0usize);
        front
            .iter()
            .enumerate()
            .map(|(i, is_front)| {
                let (x, z, s) = if *is_front {
                    let k = k_front;
                    k_front += 1;
                    let (row, col) = (k / per_row, k % per_row);
                    let in_row = per_row.min(n_front - row * per_row);
                    let x = (col as f32 - (in_row as f32 - 1.0) * 0.5) * step_f;
                    (x, top + row as f32 * (len * s_front + 0.1), s_front)
                } else {
                    let k = k_back;
                    k_back += 1;
                    (back_x0 + k as f32 * step_b, back_z, s_back)
                };
                let y = CARD_THICKNESS * 0.5 + i as f32 * 0.002;
                Transform3D::new(Basis::IDENTITY.scaled(Vector3::splat(s)), Vector3::new(x, y, z))
            })
            .collect()
    }

    /// Slots der Ressourcen-Reihe (Länder) wie im Video: von links nach rechts, gleichnamige
    /// Länder überlappend übereinander (je [`LAND_STACK_STEP`] Kartenbreite versetzt), Gruppen
    /// in Manafarben-Reihenfolge. Passen sie nicht, werden sie kleiner, zuletzt enger.
    fn resource_slots(&self, base: &[usize]) -> Vec<Transform3D> {
        let keys: Vec<(usize, String)> = base
            .iter()
            .map(|&i| {
                let c = self.cards[i].bind();
                (c.produced_mana().index(), c.card_name().to_string())
            })
            .collect();
        let mut groups: Vec<(usize, String)> = Vec::new();
        for k in &keys {
            if !groups.contains(k) {
                groups.push(k.clone());
            }
        }
        groups.sort();
        let sizes: Vec<usize> = groups.iter().map(|g| keys.iter().filter(|k| *k == g).count()).collect();
        let avail = self.size.x - 0.1;
        // Breite in Kartenbreiten bei Maßstab 1
        let units: f32 = sizes.iter().map(|n| 1.0 + (*n as f32 - 1.0) * LAND_STACK_STEP).sum::<f32>()
            + (groups.len().max(1) - 1) as f32 * LAND_GROUP_GAP;
        let s = (avail / (units * CARD_WIDTH)).clamp(self.card_scale * MIN_SHRINK, self.card_scale);
        let squeeze = (avail / (units * CARD_WIDTH * s)).min(1.0);
        let w = CARD_WIDTH * s;
        let mut starts = Vec::new();
        let mut x = -self.size.x * 0.5 + 0.05 + w * 0.5;
        for n in &sizes {
            starts.push(x);
            x += (w + (*n as f32 - 1.0) * LAND_STACK_STEP * w + LAND_GROUP_GAP * w) * squeeze;
        }
        let mut in_group = vec![0usize; groups.len()];
        keys.iter()
            .enumerate()
            .map(|(i, k)| {
                let g = groups.iter().position(|x| x == k).unwrap_or(0);
                let row = in_group[g];
                in_group[g] += 1;
                let x = starts[g] + row as f32 * LAND_STACK_STEP * w * squeeze;
                let y = CARD_THICKNESS * 0.5 + row as f32 * 0.004 + i as f32 * 0.0005;
                Transform3D::new(Basis::IDENTITY.scaled(Vector3::splat(s)), Vector3::new(x, y, 0.0))
            })
            .collect()
    }

    /// Lokale Slot-Transform für Karte `i` von `n`.
    /// Platz im Cover-Flow: seitlich linear, zur Seite gedreht (Außenkante weiter hinten), nach
    /// hinten versetzt, leicht im Bogen geneigt; Karten jenseits des Rands liegen dicht gestapelt.
    fn coverflow_slot(&self, i: usize, n: usize, flip: Basis) -> Transform3D {
        let x = (i as f32 - (n as f32 - 1.0) * 0.5) * self.coverflow_step + self.coverflow_scroll;
        let side = x.signum();
        let x_shown = if x.abs() > COVERFLOW_EDGE { side * (COVERFLOW_EDGE + (x.abs() - COVERFLOW_EDGE) * COVERFLOW_EDGE_STEP / self.coverflow_step.max(0.01)) } else { x };
        // gerade Mitte eine halbe Kartenbreite weit: die beiden mittleren Karten liegen flach
        // übereinander, statt sich gegeneinander gedreht zu durchdringen
        let off_center = (x_shown.abs() - self.coverflow_step * 0.5).max(0.0);
        let u = side * (off_center / COVERFLOW_REACH).min(1.0);
        let depth = ((x.abs() - self.coverflow_step * 0.5).max(0.0) * COVERFLOW_DEPTH).min(COVERFLOW_MAX_DEPTH) - i as f32 * 0.004;
        // Drehung um die Kartenachse von oben nach unten (Zone: z = Bildschirm unten): +X der
        // Karte rechts von der Mitte nach hinten (−Y), links nach vorn
        let yaw = Basis::from_axis_angle(Vector3::BACK, -u * COVERFLOW_YAW.to_radians());
        // im Bogen: außen etwas tiefer und nach außen geneigt (wie ein gehaltener Fächer)
        let roll = Basis::from_axis_angle(Vector3::UP, -u * 0.5 * COVERFLOW_ROLL.to_radians());
        let pos = Vector3::new(x_shown, CARD_THICKNESS * 0.5 - depth, u * u * COVERFLOW_DROP);
        Transform3D::new(roll * yaw * flip, pos)
    }

    fn slot_transform(&self, i: usize, n: usize) -> Transform3D {
        let flip = if self.face_up {
            Basis::IDENTITY
        } else {
            Basis::from_axis_angle(Vector3::BACK, std::f32::consts::PI)
        };
        let half_t = CARD_THICKNESS * 0.5;

        match self.kind {
            ZoneKind::Hand if self.coverflow => self.coverflow_slot(i, n, flip),
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
                // Stapel wie ein Kartenhaufen: der neueste Eintrag (oben auf dem Stack, wird zuerst
                // abgehandelt) liegt vorn; jeder ältere ein Stück weiter oben links, etwas gedreht
                // und dahinter – so bleibt sichtbar, was darunter liegt und worauf reagiert wird.
                let depth = (n - 1 - i) as f32;
                let shown = depth.min(STACK_MAX_SHOWN);
                let pos = Vector3::new(
                    -shown * STACK_OFFSET.x,
                    half_t + 0.05 + i as f32 * 0.012,
                    -shown * STACK_OFFSET.y,
                );
                let turn = Basis::from_axis_angle(Vector3::UP, shown * STACK_TURN_DEGREES.to_radians());
                Transform3D::new(turn * flip, pos)
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

