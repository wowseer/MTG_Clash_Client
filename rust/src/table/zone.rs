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
/// Automatische Anordnung (Aufteilung wie `mtg_arena_spielfeld.png`): Abstand zwischen Karten
/// (Anteil der Kartenbreite), am weitesten links mögliche Lage der hinteren Reihe (Artefakte,
/// Verzauberungen …) rechts vom Medaillon (lokal x) und kleinste Verkleinerung (Anteil von
/// [member card_scale]), bevor Karten sich überlappen.
const AUTO_GAP: f32 = 0.12;
const BACK_ROW_START: f32 = 1.75;
/// Die hintere Reihe liegt rechtsbündig und reicht so weit über den rechten Rand der Zone
/// hinaus bis an den Tischrand (lokal x).
const BACK_ROW_OVERHANG: f32 = 1.9;
/// Abstand zwischen Kreaturen-Reihe und hinterer Reihe.
const BACK_ROW_GAP: f32 = 0.12;
/// Spielfeld: Abstand der Kreaturen und Planeswalker zur Kante zur Tischmitte (lokal z). Beide
/// Seiten zusammen ergeben die Lücke zwischen den Kreaturenreihen (etwa 0,6 Kachelhöhen wie im
/// Video „Blocken“), damit sich Angreifer und Blocker gegenüber nicht fast berühren.
const FRONT_GAP: f32 = 0.35;
/// Kleinste Größe der Planeswalker (Anteil von [member card_scale]) bei vielen Planeswalkern.
const PW_MIN_SCALE: f32 = 0.6;
/// Ab so vielen Planeswalkern liegen sie in zwei Reihen.
const PW_TWO_ROWS_FROM: usize = 3;
/// Breite des Planeswalker-Bereichs links (lokal, ab dem linken Rand) – rechts davon beginnen
/// die Länder (Zone `Resources`).
const PW_REGION_WIDTH: f32 = 2.9;
/// Tiefe der unteren Reihe (Länder in ihrer Zone, Artefakte im Spielfeld) – Kreaturen und
/// Planeswalker bleiben darüber.
const BACK_ROW_DEPTH: f32 = 1.7;
/// Platz in der unteren Reihe zur Kreaturen-Reihe hin, den getappte (schräg gestellte)
/// Artefakte brauchen, ohne die Kreaturen zu berühren.
const BACK_ROW_TAP_ROOM: f32 = 0.3;
/// Zweite Kreaturen-Reihe, sobald eine Reihe unter diesen Anteil von [member card_scale] schrumpfen müsste.
const TWO_ROWS_BELOW: f32 = 0.6;
const MIN_SHRINK: f32 = 0.32;
/// Ressourcen-Reihe: gleiche Länder überlappen um diesen Anteil der Kartenbreite versetzt;
/// Abstand zwischen den Gruppen.
const LAND_STACK_STEP: f32 = 0.22;
const LAND_GROUP_GAP: f32 = 0.12;
/// Spielfeld: Versatz gleicher Karten auf einem Stapel (Kartenbreiten).
const BATTLE_STACK_STEP: f32 = 0.3;
/// Handfächer: Lücke vor der Endgruppe (in Kartenabständen).
const TAIL_GAP: f32 = 1.5;
/// Spielfeld: höchstens so viele gleiche Karten auf einem Stapel (Kreaturen wachsen sonst bis
/// über Manapool und Hand), danach beginnt daneben ein neuer Stapel.
const STACK_MAX: usize = 6;
/// Spielfeld: Versatz gleicher Kreaturen untereinander (Anteil der Kartenlänge).
const CREATURE_STACK_STEP: f32 = 0.2;
/// Höchste Zusatzhöhe eines Kreaturen-Stapels (Anteil der Kartenlänge).
const CREATURE_STACK_MAX_EXTRA: f32 = 0.4;
/// Größe verschluckter Karten ([member Zone.swallow_cards]).
const SWALLOWED_SCALE: f32 = 0.02;
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
    /// Karten zeigen ihre Manakosten als Symbolreihe über der Oberkante (eigene Hand, wie im
    /// Video „Spree und Stack“; `table::cost_symbols`).
    #[export]
    show_costs: bool,
    /// Handfächer: so viele Karten am rechten Ende bilden eine eigene Gruppe mit Lücke davor
    /// (Karten aus anderen Zonen, [method add_tail_card]).
    tail: usize,

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
    /// Hand: Verschiebung der Endgruppe ([method add_tail_card], lokal) – z. B. damit der
    /// Commander in der fast verdeckten Hand des Gegners ganz zu sehen ist.
    #[export]
    tail_offset: Vector3,
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
    /// Automatische Anordnung seitenverkehrt (lokal x gespiegelt): für die um 180° gedrehte Zone des
    /// Gegners, damit seine Artefakte von uns aus wie unsere rechts und Planeswalker links liegen.
    #[export]
    mirror_x: bool,
    /// Größe der Karten in dieser Zone (1 = normal). Spielfeld und Ressourcen verkleinern sie
    /// automatisch, wenn sonst nicht alle in eine Reihe passen.
    #[export]
    card_scale: f32,
    /// Karten als Kachel zeigen (nur Name und Bild, Stärke/Widerstand unten rechts; siehe
    /// [method Card3D.set_tile]) – Form der Karten auf dem Spielfeld.
    #[export]
    tile_cards: bool,
    /// Kartenbilder um 180° gedreht zeigen ([method Card3D.set_turned]): für die um 180° gedrehten
    /// Zonen des Gegners, damit seine Karten von unserer Seite aus lesbar sind.
    #[export]
    turn_cards: bool,
    /// Karten dieser Zone beim Hovern selbst groß vor die Kamera holen (eigene Hand); sonst zeigt
    /// die Kartenansicht eine Kopie.
    #[export]
    zoom_cards: bool,
    /// Karten verschwinden in der Zone (Exil-Strudel): sie gleiten zur Mitte, schrumpfen dabei
    /// auf fast nichts und sinken unter den Tisch – unsichtbar und nicht anklickbar. Verlassen
    /// sie die Zone, wachsen sie auf dem Weg zum neuen Platz wieder.
    #[export]
    swallow_cards: bool,

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
            turn_cards: false,
            swallow_cards: false,
            size: Vector3::new(3.0, 0.2, 2.0),
            accepts_drops: true,
            cards_draggable: true,
            click_to_draw: false,
            face_up: true,
            hand_radius: 5.0,
            hand_card_angle_degrees: 6.0,
            tail_offset: Vector3::ZERO,
            coverflow: false,
            coverflow_step: 0.3,
            coverflow_scroll: 0.0,
            hand_max_spread_degrees: 40.0,
            pile_spacing: 0.004,
            grid_columns: 6,
            grid_spacing: Vector2::new(0.78, 1.02),
            free_placement: false,
            auto_arrange: false,
            mirror_x: false,
            show_area: false,
            show_costs: false,
            tail: 0,
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
        if !card.is_instance_valid() {
            return;
        }
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
        self.prune_freed();
        if !card.is_instance_valid() {
            return;
        }
        if let Some(old) = self.cards.iter().position(|c| c == &card) {
            if old >= self.cards.len() - self.tail {
                self.tail -= 1;
            }
            self.cards.remove(old);
        }
        // vor der Endgruppe einfügen (sie bleibt immer rechts außen)
        let end = self.cards.len() - self.tail;
        let idx = if index < 0 { end } else { (index as usize).min(end) };
        self.cards.insert(idx, card.clone());
        card.bind_mut().set_zone(Some(self.to_gd()));
        self.layout_with_delay(Some((&card, delay)));
    }

    /// Handfächer: Karte ans rechte Ende in die eigene Gruppe mit Lücke davor legen (z. B. Karten,
    /// die aus einer anderen Zone spielbar sind); neue Handkarten kommen immer davor.
    #[func]
    pub fn add_tail_card(&mut self, mut card: Gd<Card3D>) {
        self.prune_freed();
        if !card.is_instance_valid() {
            return;
        }
        if let Some(old) = self.cards.iter().position(|c| c == &card) {
            if old >= self.cards.len() - self.tail {
                self.tail -= 1;
            }
            self.cards.remove(old);
        }
        self.cards.push(card.clone());
        self.tail += 1;
        card.bind_mut().set_zone(Some(self.to_gd()));
        self.layout_with_delay(Some((&card, 0.0)));
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

    /// Zeigen Karten in dieser Zone ihre Manakosten über der Oberkante ([member show_costs])?
    pub fn shows_costs(&self) -> bool {
        self.show_costs
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
        if !card.is_instance_valid() {
            // schon freigegeben: nur aufräumen
            self.prune_freed();
            self.layout();
            return -1;
        }
        let Some(idx) = self.cards.iter().position(|c| c == &card) else {
            return -1;
        };
        if idx >= self.cards.len() - self.tail {
            self.tail -= 1;
        }
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
        self.cards.iter().rev().find(|c| c.is_instance_valid()).cloned()
    }

    /// Alle Karten der Zone, unterste bzw. erste zuerst.
    #[func]
    pub fn get_cards(&self) -> Array<Gd<Card3D>> {
        self.cards.iter().filter(|c| c.is_instance_valid()).cloned().collect()
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

    /// Karten, die freigegeben wurden, ohne die Zone zu verlassen (z. B. Bühnen- oder
    /// Handkopien beim schnellen Schließen), aus Liste, Endgruppe und Anlagen entfernen – sonst
    /// griffe das Anordnen auf freigegebene Objekte zu.
    fn prune_freed(&mut self) {
        if self.cards.iter().all(|c| c.is_instance_valid()) && self.attachments.values().all(|h| h.is_instance_valid()) {
            return;
        }
        let tail_start = self.cards.len() - self.tail;
        let mut kept = Vec::with_capacity(self.cards.len());
        let mut tail = 0;
        for (i, c) in self.cards.drain(..).enumerate() {
            if c.is_instance_valid() {
                if i >= tail_start {
                    tail += 1;
                }
                kept.push(c);
            }
        }
        self.cards = kept;
        self.tail = tail;
        let live: std::collections::HashSet<InstanceId> = self.cards.iter().map(|c| c.instance_id()).collect();
        self.attachments.retain(|id, host| live.contains(id) && host.is_instance_valid());
        self.free_positions.retain(|id, _| live.contains(id));
    }

    fn layout_with_delay(&mut self, delayed: Option<(&Gd<Card3D>, f64)>) {
        self.prune_freed();
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
        let mut auto_slots = if self.auto_arrange && self.kind == ZoneKind::Battlefield { self.auto_slots(&base) } else { Vec::new() };
        if self.mirror_x {
            // gespiegelt (Gegner): Artefakte bleiben von uns aus rechts, Planeswalker links
            for xf in auto_slots.iter_mut() {
                xf.origin.x = -xf.origin.x;
            }
        }
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
                _ if self.swallow_cards => {
                    let mut xf = self.slot_transform(bi, nb);
                    xf.basis = xf.basis.scaled(Vector3::splat(SWALLOWED_SCALE));
                    xf.origin.y -= 0.05;
                    xf
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
            card.bind_mut().set_turned(self.turn_cards);
            card.bind_mut().set_zoom(self.zoom_cards);
            card.bind_mut().move_to(zone_xf * local, delay);
        }
    }

    /// Kartenhöhe in dieser Zone (Kachel oder ganze Karte), ohne Skalierung.
    fn card_length(&self) -> f32 {
        if self.tile_cards { TILE_HEIGHT } else { CARD_HEIGHT }
    }

    /// Slots der automatischen Anordnung (siehe [member auto_arrange]) für die freien Karten
    /// `base`, in deren Reihenfolge – Aufteilung wie `mtg_arena_spielfeld.png`. Lokal: −z = zur
    /// Tischmitte, +x = rechts (aus Sicht der Seite):
    /// - **untere Reihe** ([`BACK_ROW_DEPTH`] tief): links die Länder (eigene Zone), rechts
    ///   Artefakte, Verzauberungen, Schlachten … in eigener Größe, nach Kategorie geordnet und
    ///   rechtsbündig bis an den Tischrand ([`BACK_ROW_OVERHANG`]), höchstens bis
    ///   [`BACK_ROW_START`] nach links; bei vielen zwei Reihen (bevorzugt je Kategorie eine);
    /// - **darüber** Planeswalker ganz links (Größe nur nach ihrer Zahl, ab [`PW_TWO_ROWS_FROM`] in zwei Reihen) und
    ///   die Kreaturen mittig, rechts neben den Planeswalkern; gleiche Kreaturen untereinander
    ///   gestapelt (hohe Stapel enger, [`stack_step`]). Passen sie nicht, werden sie kleiner; eine
    ///   zweite Kreaturen-Reihe kommt, sobald eine Reihe unter [`TWO_ROWS_BELOW`] schrumpfen müsste
    ///   und zwei Reihen größere Karten erlauben. Nichts reicht in die untere Reihe hinein.
    fn auto_slots(&self, base: &[usize]) -> Vec<Transform3D> {
        // Reihe je Karte: 0 = Kreaturen vorn, 1 = hinten rechts, 2 = Planeswalker vorn links
        let front: Vec<u8> = base
            .iter()
            .map(|&i| match self.cards[i].bind().kind() {
                CardKind::Creature => 0,
                CardKind::Planeswalker => 2,
                _ => 1,
            })
            .collect();
        let (half_x, half_z) = (self.size.x * 0.5, self.size.z * 0.5);
        let len = self.card_length();

        // Gleiche Karten stapeln wie gleichnamige Länder (Stapel = Gruppe gleicher stack_key,
        // je Karte BATTLE_STACK_STEP Kartenbreite versetzt); Träger angelegter Karten bleiben allein
        let hosts: Vec<Gd<Card3D>> = self.attachments.values().cloned().collect();
        // höchstens STACK_MAX Karten je Stapel, danach beginnt daneben ein neuer
        let mut per_key: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        let keys: Vec<String> = base
            .iter()
            .enumerate()
            .map(|(n, &i)| {
                let c = &self.cards[i];
                if hosts.contains(c) {
                    return format!("#host{n}");
                }
                let k = c.bind().stack_key();
                let count = per_key.entry(k.clone()).or_insert(0);
                let chunk = *count / STACK_MAX;
                *count += 1;
                format!("{k}#{chunk}")
            })
            .collect();
        // Gruppen je Reihe in der Reihenfolge des ersten Auftretens: (Schlüssel, vorn?, Anzahl)
        let mut groups: Vec<(String, u8, usize)> = Vec::new();
        for (k, f) in keys.iter().zip(&front) {
            match groups.iter_mut().find(|(gk, gf, _)| gk == k && gf == f) {
                Some(g) => g.2 += 1,
                None => groups.push((k.clone(), *f, 1)),
            }
        }
        // gleichnamige Stapel nebeneinander (wie bei Ländern)
        let name_of = |k: &str| k.split('|').next().unwrap_or("").to_string();
        let first: Vec<String> = groups.iter().map(|g| name_of(&g.0)).collect();
        // gleichnamige Stapel: bereit, dann noch nicht einsatzbereit, angreifend, getappt (rechts)
        let rank = |g: &(String, u8, usize)| {
            let n = name_of(&g.0);
            let flag = |i: usize| g.0.split('|').nth(i) == Some("true");
            (first.iter().position(|f| *f == n).unwrap_or(0), flag(2), flag(1), flag(3))
        };
        groups.sort_by_key(|g| rank(g));
        // Breite einer Gruppe in Kartenbreiten (ohne Abstand): Kreaturen-Stapel liegen
        // untereinander (eine Kartenbreite), die anderen nebeneinander
        let group_units = |n: usize, row: u8| if row == 0 { 1.0 } else { 1.0 + (n as f32 - 1.0) * BATTLE_STACK_STEP };
        let wall = half_x - 0.1;
        // Die untere Reihe der Zone gehört Ländern (eigene Zone daneben) und Artefakten:
        // Kreaturen und Planeswalker liegen immer darüber, auch mit hohen Stapeln
        let area_top = -half_z + FRONT_GAP;
        let area_h = (2.0 * half_z - BACK_ROW_DEPTH - FRONT_GAP).max(len * self.card_scale * MIN_SHRINK);
        let min_s = self.card_scale * MIN_SHRINK;

        // Planeswalker: Größe nur nach ihrer Zahl (bis zwei voll, danach sanft kleiner), ganz
        // links; ab PW_TWO_ROWS_FROM in zwei Reihen. Ihr Bereich links reicht über die ganze
        // Tiefe (die Länder beginnen erst rechts davon), höchstens PW_REGION_WIDTH breit
        let pw: Vec<usize> = (0..groups.len()).filter(|&g| groups[g].1 == 2).collect();
        let pw_rows = if pw.len() >= PW_TWO_ROWS_FROM { 2 } else { 1 };
        let pw_cols = pw.len().div_ceil(pw_rows).max(1);
        let pw_s = {
            let by_count = if pw.len() <= 2 { 1.0 } else { (2.0 / pw.len() as f32).sqrt().max(PW_MIN_SCALE) };
            let by_height = (2.0 * half_z - 0.1 - BACK_ROW_GAP * (pw_rows as f32 - 1.0)) / (pw_rows as f32 * len);
            let by_width = PW_REGION_WIDTH / (pw_cols as f32 * CARD_WIDTH * (1.0 + AUTO_GAP));
            (self.card_scale * by_count).min(by_height).min(by_width).max(min_s)
        };
        let pw_w = CARD_WIDTH * pw_s;
        let left_w = if pw.is_empty() { 0.0 } else { pw_cols as f32 * pw_w * (1.0 + AUTO_GAP) };

        // Kreaturen: eine Reihe, bei vielen zwei – rechts neben den Planeswalkern, über der unteren Reihe
        let cre: Vec<usize> = (0..groups.len()).filter(|&g| groups[g].1 == 0).collect();
        let avail_w = (2.0 * wall - left_w).max(CARD_WIDTH * min_s);
        let split = |rows: usize| -> Vec<Vec<usize>> {
            if rows == 1 {
                return vec![cre.clone()];
            }
            let first = cre.len().div_ceil(2);
            vec![cre[..first].to_vec(), cre[first..].to_vec()]
        };
        let row_h = |r: &[usize], sc: f32| -> f32 {
            let tall = r.iter().map(|&g| groups[g].2.min(STACK_MAX)).max().unwrap_or(1);
            len * sc * (1.0 + (tall as f32 - 1.0) * stack_step(tall))
        };
        let fits = |rows: &[Vec<usize>], sc: f32| -> bool {
            let step = CARD_WIDTH * sc * (1.0 + AUTO_GAP);
            let widest = rows.iter().map(|r| r.len()).max().unwrap_or(0) as f32 * step;
            let h: f32 = rows.iter().map(|r| row_h(r, sc)).sum::<f32>() + BACK_ROW_GAP * (rows.len() as f32 - 1.0);
            widest <= avail_w && h <= area_h
        };
        let best = |rows: usize| -> (Vec<Vec<usize>>, f32) {
            let rs = split(rows);
            let mut sc = self.card_scale;
            while sc > min_s && !fits(&rs, sc) {
                sc -= self.card_scale * 0.02;
            }
            (rs, sc.max(min_s))
        };
        let (one, s1) = best(1);
        let (rows, s) = if cre.len() >= 2 && s1 < self.card_scale * TWO_ROWS_BELOW {
            let (two, s2) = best(2);
            if s2 > s1 { (two, s2) } else { (one, s1) }
        } else {
            (one, s1)
        };
        let w = CARD_WIDTH * s;

        // hintere Reihe (Artefakte, Verzauberungen …): nach Kategorie geordnet (Artefakte, dann
        // Verzauberungen, dann der Rest), rechtsbündig bis an den Tischrand; eine zweite Reihe, sobald
        // sie größere Karten erlaubt – bevorzugt an einer Kategorie-Grenze geteilt
        let category = |g: usize| -> u8 {
            let i = keys.iter().zip(&front).position(|(k, f)| *k == groups[g].0 && *f == groups[g].1).unwrap_or(0);
            // nach der Typzeile: Embleme, Dungeons, „The Ring“ (CR 408) gelten nicht als Artefakt
            let card = self.cards[base[i]].bind();
            let has = |w: &str| card.def().type_line.split(|c: char| !c.is_alphanumeric()).any(|t| t == w);
            if has("Artifact") {
                0
            } else if has("Enchantment") {
                1
            } else {
                2
            }
        };
        let mut back: Vec<usize> = (0..groups.len()).filter(|&g| groups[g].1 == 1).collect();
        back.sort_by_key(|&g| category(g)); // stabil: innerhalb der Kategorie wie bisher
        let back_right = wall + BACK_ROW_OVERHANG;
        let back_avail = (back_right - BACK_ROW_START).max(CARD_WIDTH * min_s);
        let units_of = |r: &[usize]| -> f32 {
            (r.iter().map(|&g| group_units(groups[g].2, 1) + AUTO_GAP).sum::<f32>() - AUTO_GAP).max(0.0)
        };
        let back_scale = |rows: &[Vec<usize>]| -> f32 {
            let n = rows.len() as f32;
            let by_depth = (BACK_ROW_DEPTH - 0.1 - BACK_ROW_TAP_ROOM - BACK_ROW_GAP * (n - 1.0)) / (n * len);
            let widest = rows.iter().map(|r| units_of(r)).fold(0.0f32, f32::max).max(0.001);
            self.card_scale.min(by_depth).min(back_avail / (widest * CARD_WIDTH)).max(min_s)
        };
        // Kandidaten: eine Reihe, zwei Reihen an jeder Kategorie-Grenze, zwei gleich breite Reihen
        let mut layouts: Vec<Vec<Vec<usize>>> = vec![vec![back.clone()]];
        for cut in 1..back.len() {
            if category(back[cut]) != category(back[cut - 1]) {
                layouts.push(vec![back[..cut].to_vec(), back[cut..].to_vec()]);
            }
        }
        if back.len() >= 2 {
            let total = units_of(&back);
            let cut = (1..back.len())
                .min_by(|&a, &b| {
                    let d = |c: usize| (units_of(&back[..c]) - total * 0.5).abs();
                    d(a).total_cmp(&d(b))
                })
                .unwrap_or(1);
            layouts.push(vec![back[..cut].to_vec(), back[cut..].to_vec()]);
        }
        let mut back_rows = layouts[0].clone();
        let mut back_s = back_scale(&back_rows);
        for l in &layouts[1..] {
            let sc = back_scale(l);
            // zweite Reihe nur, wenn die Karten dadurch spürbar größer werden
            if sc > back_s * 1.05 {
                back_rows = l.clone();
                back_s = sc;
            }
        }
        let back_w = CARD_WIDTH * back_s;

        // Lage (x, z, Größe) der ersten Karte jeder Gruppe
        let mut slot = vec![(0.0f32, 0.0f32, s); groups.len()];
        for (j, &g) in pw.iter().enumerate() {
            let (r, c) = (j / pw_cols, j % pw_cols);
            let x = -wall + pw_w * 0.5 + c as f32 * pw_w * (1.0 + AUTO_GAP);
            let z = area_top + len * pw_s * 0.5 + r as f32 * (len * pw_s + BACK_ROW_GAP);
            slot[g] = (x, z, pw_s);
        }
        let left_end = -wall + left_w;
        // Kampf: geblockte Angreifer liegen ihrem Blocker genau gegenüber ([method Card3D.set_combat_x],
        // Weltposition → lokal vor dem Spiegeln), die übrigen Angreifer direkt daneben
        let zone_xf = self.base().get_global_transform();
        let zone_inv = zone_xf.affine_inverse();
        let mirror = self.mirror_x;
        let first_card = |g: usize| -> Option<&Gd<Card3D>> {
            keys.iter().zip(&front).position(|(k, f)| *k == groups[g].0 && *f == groups[g].1).map(|i| &self.cards[base[i]])
        };
        let pinned = |g: usize| -> Option<f32> {
            let card = first_card(g)?;
            let wx = card.bind().combat_x()?;
            let lx = (zone_inv * Vector3::new(wx, zone_xf.origin.y, zone_xf.origin.z)).x;
            Some(if mirror { -lx } else { lx })
        };
        let attacking = |g: usize| first_card(g).is_some_and(|c| c.bind().is_attacking());
        let mut z_row = area_top + len * s * 0.5;
        for r in &rows {
            let full = w * (1.0 + AUTO_GAP);
            let step = if r.len() as f32 * full <= avail_w { full } else { avail_w / r.len().max(1) as f32 };
            let width = r.len() as f32 * step;
            let pins: Vec<(usize, f32)> = r.iter().filter_map(|&g| pinned(g).map(|x| (g, x))).collect();
            if pins.is_empty() {
                // mittig; reicht der Platz nicht, rechts neben den Planeswalkern beginnen
                let mut x = (-width * 0.5).max(left_end).min(wall - width) + step * 0.5;
                for &g in r {
                    slot[g] = (x, z_row, s);
                    x += step;
                }
            } else {
                let xs = combat_row(r, &pins, &attacking, step, left_end, wall);
                for (&g, x) in r.iter().zip(xs) {
                    slot[g] = (x, z_row, s);
                }
            }
            z_row += row_h(r, s) + BACK_ROW_GAP;
        }
        // jede Reihe endet am Tischrand; die untere Reihe liegt an der Kante zum Spieler
        let mut squeeze_of = vec![1.0f32; groups.len()];
        let n_back = back_rows.len();
        for (r, row) in back_rows.iter().enumerate() {
            let squeeze = (back_avail / (units_of(row) * back_w).max(0.001)).min(1.0);
            let z = half_z - 0.05 - len * back_s * 0.5 - (n_back - 1 - r) as f32 * (len * back_s + BACK_ROW_GAP);
            let mut xb = back_right - units_of(row) * back_w * squeeze + back_w * 0.5;
            for &g in row {
                slot[g] = (xb, z, back_s);
                squeeze_of[g] = squeeze;
                xb += (group_units(groups[g].2, 1) + AUTO_GAP) * back_w * squeeze;
            }
        }

        let mut in_group = vec![0usize; groups.len()];
        keys.iter()
            .zip(&front)
            .enumerate()
            .map(|(i, (k, f))| {
                let g = groups.iter().position(|(gk, gf, _)| gk == k && gf == f).unwrap_or(0);
                let row = in_group[g];
                in_group[g] += 1;
                let (x0, z0, sc) = slot[g];
                // Kreaturen: jede weitere etwas weiter zum Spieler hin und obenauf, die Kopfzeilen
                // darunter bleiben sichtbar; andere: nach rechts versetzt
                let (x, z) = if *f == 0 {
                    (x0, z0 + row as f32 * stack_step(groups[g].2.min(STACK_MAX)) * len * sc)
                } else {
                    let squeeze = squeeze_of[g];
                    (x0 + row as f32 * BATTLE_STACK_STEP * CARD_WIDTH * sc * squeeze, z0)
                };
                let y = CARD_THICKNESS * 0.5 + row as f32 * 0.004 + i as f32 * 0.0005;
                Transform3D::new(Basis::IDENTITY.scaled(Vector3::splat(sc)), Vector3::new(x, y, z))
            })
            .collect()
    }

    /// Slots der Ressourcen-Reihe (Länder) wie im Video – bei vielen Ländergruppen in zwei Reihen
    /// innerhalb der Zone (darunter liegt das Exil, die Zone wird nie verlassen): von links nach rechts, gleichnamige
    /// Länder überlappend übereinander (je [`LAND_STACK_STEP`] Kartenbreite versetzt), Gruppen
    /// in Manafarben-Reihenfolge. Getappte Länder bilden einen eigenen Stapel direkt rechts neben
    /// den ungetappten gleichen Namens (sie liegen nie zwischen ungetappten). Passen sie nicht,
    /// werden sie kleiner, zuletzt enger.
    fn resource_slots(&self, base: &[usize]) -> Vec<Transform3D> {
        let keys: Vec<(usize, String, bool)> = base
            .iter()
            .map(|&i| {
                let c = self.cards[i].bind();
                (c.produced_mana().index(), c.card_name().to_string(), c.is_tapped())
            })
            .collect();
        let mut groups: Vec<(usize, String, bool)> = Vec::new();
        for k in &keys {
            if !groups.contains(k) {
                groups.push(k.clone());
            }
        }
        groups.sort();
        let sizes: Vec<usize> = groups.iter().map(|g| keys.iter().filter(|k| *k == g).count()).collect();
        let avail = self.size.x - 0.1;
        let len = self.card_length();
        // Breite in Kartenbreiten bei Maßstab 1 für die Gruppen `range`
        let units = |range: std::ops::Range<usize>| -> f32 {
            let n = range.len();
            sizes[range].iter().map(|n| 1.0 + (*n as f32 - 1.0) * LAND_STACK_STEP).sum::<f32>() + n.max(1).saturating_sub(1) as f32 * LAND_GROUP_GAP
        };
        let one_s = (avail / (units(0..groups.len()) * CARD_WIDTH)).clamp(self.card_scale * MIN_SHRINK, self.card_scale);
        // zweite Reihe innerhalb der Zone (darunter liegt das Exil – die Zone wird nie verlassen),
        // sobald eine Reihe deutlich kleiner würde
        let split = groups.len().div_ceil(2);
        let two_s = (avail / (units(0..split).max(units(split..groups.len())) * CARD_WIDTH))
            .min((self.size.z - 0.1) * 0.5 / len)
            .clamp(self.card_scale * MIN_SHRINK, self.card_scale);
        let two_rows = groups.len() >= 2 && one_s < self.card_scale * TWO_ROWS_BELOW && two_s > one_s;
        let s = if two_rows { two_s } else { one_s };
        let w = CARD_WIDTH * s;
        let row_ranges = if two_rows { vec![0..split, split..groups.len()] } else { vec![0..groups.len()] };
        let mut starts = vec![(0.0f32, 0.0f32, 1.0f32); groups.len()];
        for (r, range) in row_ranges.iter().enumerate() {
            let squeeze = (avail / (units(range.clone()) * CARD_WIDTH * s)).min(1.0);
            let z = if two_rows { (r as f32 - 0.5) * (len * s + 0.05) } else { 0.0 };
            let mut x = -self.size.x * 0.5 + 0.05 + w * 0.5;
            for g in range.clone() {
                starts[g] = (x, z, squeeze);
                x += (w + (sizes[g] as f32 - 1.0) * LAND_STACK_STEP * w + LAND_GROUP_GAP * w) * squeeze;
            }
        }
        let mut in_group = vec![0usize; groups.len()];
        keys.iter()
            .enumerate()
            .map(|(i, k)| {
                let g = groups.iter().position(|x| x == k).unwrap_or(0);
                let row = in_group[g];
                in_group[g] += 1;
                let (x0, z, squeeze) = starts[g];
                let x = x0 + row as f32 * LAND_STACK_STEP * w * squeeze;
                let y = CARD_THICKNESS * 0.5 + row as f32 * 0.004 + i as f32 * 0.0005;
                Transform3D::new(Basis::IDENTITY.scaled(Vector3::splat(s)), Vector3::new(x, y, z))
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
                // Endgruppe (Karten aus anderen Zonen): nach einer Lücke von TAIL_GAP Kartenabständen
                let tail = if n == self.cards.len() { self.tail.min(n) } else { 0 };
                let gap = if tail > 0 && tail < n { TAIL_GAP } else { 0.0 };
                let slot = i as f32 + if i >= n - tail { gap } else { 0.0 };
                let a = (slot - (n as f32 - 1.0 + gap) * 0.5) * step;
                let r = self.hand_radius;
                // Mittlere Karte oben (-Z), äußere Karten sinken ab und drehen nach außen.
                let pos = Vector3::new(r * a.sin(), half_t + i as f32 * 0.004, r * (1.0 - a.cos()));
                // Endgruppe immer offen (öffentliche Karten wie der Commander, auch in der
                // verdeckten Hand des Gegners)
                let (flip, pos) = if i >= n - tail { (Basis::IDENTITY, pos + self.tail_offset) } else { (flip, pos) };
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

/// Versatz je Karte in einem Kreaturen-Stapel aus `n` Karten (Anteil der Kartenlänge): normal
/// [`CREATURE_STACK_STEP`], bei hohen Stapeln enger, damit ein Stapel höchstens
/// 1 + [`CREATURE_STACK_MAX_EXTRA`] Kartenlängen hoch ist (die Kopfzeilen bleiben sichtbar).
/// Lage (lokal x) der Gruppen einer Kreaturenreihe im Kampf, in der Reihenfolge von `row`:
/// geblockte Angreifer (`pins`: Gruppe → x ihres Blockers) liegen dort, die übrigen Angreifer
/// rechts direkt daneben, alle anderen Kreaturen links davor; mindestens `step` Abstand, innerhalb
/// `left_end` … `wall` (reicht der Platz nicht, rücken auch die ausgerichteten Gruppen).
fn combat_row(row: &[usize], pins: &[(usize, f32)], attacking: &dyn Fn(usize) -> bool, step: f32, left_end: f32, wall: f32) -> Vec<f32> {
    let pin_of = |g: usize| pins.iter().find(|(p, _)| *p == g).map(|(_, x)| *x);
    // Reihenfolge: andere Kreaturen, ausgerichtete Angreifer (nach x), übrige Angreifer
    let mut sorted_pins: Vec<(usize, f32)> = pins.to_vec();
    sorted_pins.sort_by(|a, b| a.1.total_cmp(&b.1));
    let others: Vec<usize> = row.iter().copied().filter(|&g| pin_of(g).is_none() && !attacking(g)).collect();
    let free_attackers: Vec<usize> = row.iter().copied().filter(|&g| pin_of(g).is_none() && attacking(g)).collect();
    let order: Vec<usize> = others.iter().copied().chain(sorted_pins.iter().map(|p| p.0)).chain(free_attackers.iter().copied()).collect();
    let first_pin = others.len();
    let last_pin = first_pin + sorted_pins.len() - 1;
    let mut xs: Vec<f32> = vec![0.0; order.len()];
    for (i, &g) in order.iter().enumerate() {
        xs[i] = pin_of(g).unwrap_or(0.0);
    }
    // ausgerichtete Gruppen nicht überlappen lassen, dann die anderen links bzw. rechts anreihen
    for i in first_pin + 1..=last_pin {
        xs[i] = xs[i].max(xs[i - 1] + step);
    }
    for i in last_pin + 1..order.len() {
        xs[i] = xs[i - 1] + step;
    }
    for i in (0..first_pin).rev() {
        xs[i] = xs[i + 1] - step;
    }
    // in die Reihe schieben
    let (lo, hi) = (left_end + step * 0.5, wall - step * 0.5);
    if let (Some(&first), Some(&last)) = (xs.first(), xs.last()) {
        let shift = if last > hi { hi - last } else if first < lo { lo - first } else { 0.0 };
        for x in xs.iter_mut() {
            *x += shift;
        }
    }
    row.iter().map(|g| xs[order.iter().position(|o| o == g).unwrap_or(0)]).collect()
}

fn stack_step(n: usize) -> f32 {
    if n <= 1 {
        return CREATURE_STACK_STEP;
    }
    CREATURE_STACK_STEP.min(CREATURE_STACK_MAX_EXTRA / (n - 1) as f32)
}
