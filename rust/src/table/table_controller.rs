//! TableController: verbindet Kamera, Zonen und Karten.
//!
//! - Hover: Raycast Maus -> Karten (CARD_LAYER), liefert Trefferpunkt für den Tilt.
//! - Drag: Karte aus einer Zone greifen, auf einer Ebene über dem Tisch bewegen,
//!   per Raycast (ZONE_LAYER) das Drop-Ziel bestimmen und dort ablegen.
//!   Ungültiger Drop -> Karte gleitet an ihren alten Platz zurück.
//! - Rechtsklick: Nahaufnahme der Karte unter der Maus (erneut = zurück), Esc = Übersicht.
//! - Klick auf das Deck (`click_to_draw`) zieht eine Karte auf die Hand.
//! - Zielpfeile ([`TargetArrows`]): Angriffs- und Blockpfeile aus dem Kampf; bei der Zielwahl
//!   (Blocker zuweisen, Ausrüstung anlegen) wird von der Karte ein Pfeil gezogen und über dem
//!   Ziel losgelassen – oder Karte anklicken, dann folgt ein Pfeil der Maus bis zum Zielklick.
//!   Während der Zielwahl werden Karten nicht gezoomt.

use godot::classes::{
    INode3D, InputEvent, InputEventKey, InputEventMouseButton, PhysicsRayQueryParameters3D,
};
use godot::global::{Key, MouseButton};
use godot::prelude::*;

use super::card3d::{Card3D, CardMarker};
use crate::game::cards::CardKind;
use crate::game::cards;
#[cfg(debug_assertions)]
use crate::game::cards::CardDef;
use crate::game::mana::ManaType;

/// Farben der Demo-Decks.
const PLAYER_COLORS: [ManaType; 2] = [ManaType::Red, ManaType::Green];
const OPPONENT_COLORS: [ManaType; 2] = [ManaType::Blue, ManaType::Black];
use crate::game::turn_manager::TurnManager;
use super::layers::{CARD_LAYER, ZONE_LAYER};
use super::placeholder;
use super::table_camera::TableCamera;
use super::zone::Zone;
use super::target_arrows::{ArrowSpec, TargetArrows};
use super::card3d::SELECT_GLOW;

struct DragState {
    card: Gd<Card3D>,
    origin_zone: Gd<Zone>,
    origin_index: i64,
    /// Aktueller Zielpunkt auf der Drag-Ebene (Welt).
    plane_point: Vector3,
    last_position: Vector3,
    /// Nur für Auren: Kreatur unter dem Mauszeiger (mögliches Ziel).
    aura_target: Option<Gd<Card3D>>,
}

struct RayHit {
    collider: Gd<Object>,
    position: Vector3,
}

/// Wurzel der Tischszene: Maus-Eingabe (Hover, Klick, Drag-and-Drop per Raycast),
/// Kamera-Fokus (Rechtsklick) und Erzeugung der Demo-Decks. Ist [member turn_manager]
/// gesetzt, gelten alle Spielregeln; ohne ihn lassen sich Karten frei bewegen.
/// In Test-Builds erzeugt er außerdem das Debug-Tool.
#[derive(GodotClass)]
#[class(base=Node3D)]
pub struct TableController {
    /// Die Tischkamera.
    #[export]
    camera: Option<Gd<TableCamera>>,
    /// Optional: Regelwerk/Rundenablauf. Ohne ihn ist alles frei spielbar.
    #[export]
    turn_manager: Option<Gd<TurnManager>>,
    /// Zone, in die gezogene Karten (vom Deck) wandern.
    #[export]
    hand_zone: Option<Gd<Zone>>,
    /// Zone, aus der die Demo-Karten erzeugt werden.
    #[export]
    deck_zone: Option<Gd<Zone>>,
    /// Höhe der Drag-Ebene über dem Tisch (Tischoberfläche = y dieses Nodes).
    #[export]
    drag_height: f32,
    /// Wie schnell die gezogene Karte der Maus folgt.
    #[export]
    drag_follow_speed: f32,
    /// Stärke der Schwung-Neigung beim Ziehen.
    #[export]
    drag_sway_amount: f32,
    /// Länge der Maus-Raycasts in Einheiten.
    #[export]
    ray_length: f32,
    /// Anzahl der Demo-Karten im Deck (0 = keine Demo).
    #[export]
    demo_deck_size: i32,
    /// Anzahl der Demo-Karten, die zu Beginn ausgeteilt werden.
    #[export]
    demo_hand_size: i32,

    /// Handzone des Gegners (Demo: verdeckte Starthand).
    #[export_group(name = "Gegner")]
    #[export]
    opponent_hand_zone: Option<Gd<Zone>>,
    /// Deckzone des Gegners (Demo: wird mit Karten gefüllt).
    #[export]
    opponent_deck_zone: Option<Gd<Zone>>,
    /// Spielfeld des Gegners (Demo: optionale Startkarten).
    #[export]
    opponent_battlefield_zone: Option<Gd<Zone>>,
    /// Demo: verdeckte Handkarten des Gegners.
    #[export]
    demo_opponent_hand_size: i32,
    /// Demo: offene Karten, die der Gegner schon ausgespielt hat.
    #[export]
    demo_opponent_battlefield_size: i32,

    back_texture: Option<Gd<godot::classes::Texture2D>>,
    hovered: Option<Gd<Card3D>>,
    hover_zone: Option<Gd<Zone>>,
    drag: Option<DragState>,
    /// Pfeil-Ebene über dem Tisch.
    arrows: Option<Gd<TargetArrows>>,
    /// Karte, von der gerade ein Zielpfeil gezogen wird (Maustaste gedrückt).
    aim_drag: Option<Gd<Card3D>>,
    /// Schnittpunkt des Mausstrahls mit der Tischebene.
    mouse_point: Option<Vector3>,
    /// Lief im letzten Physik-Tick eine Zielwahl? (danach Hover neu aufbauen)
    was_aiming: bool,

    base: Base<Node3D>,
}

#[godot_api]
impl INode3D for TableController {
    fn init(base: Base<Node3D>) -> Self {
        Self {
            camera: None,
            turn_manager: None,
            hand_zone: None,
            deck_zone: None,
            drag_height: 0.35,
            drag_follow_speed: 20.0,
            drag_sway_amount: 0.035,
            ray_length: 100.0,
            demo_deck_size: 30,
            demo_hand_size: 5,
            opponent_hand_zone: None,
            opponent_deck_zone: None,
            opponent_battlefield_zone: None,
            demo_opponent_hand_size: 5,
            demo_opponent_battlefield_size: 3,
            back_texture: None,
            hovered: None,
            hover_zone: None,
            drag: None,
            arrows: None,
            aim_drag: None,
            mouse_point: None,
            was_aiming: false,
            base,
        }
    }

    fn ready(&mut self) {
        let arrows = TargetArrows::new_alloc();
        self.base_mut().add_child(&arrows);
        self.arrows = Some(arrows);
        if self.demo_deck_size > 0 {
            self.spawn_demo_cards();
        }
        // Debug-Tool nur in Test-Builds (cargo build ohne --release)
        #[cfg(debug_assertions)]
        {
            let mut panel = crate::debug::DebugPanel::new_alloc();
            panel.bind_mut().init_refs(self.to_gd(), self.turn_manager.clone());
            self.base_mut().add_child(&panel);
        }
    }

    /// Kartenbewegung beim Ziehen: jedes Frame für flüssige Bewegung.
    fn process(&mut self, delta: f64) {
        self.update_arrows();
        let Some(drag) = self.drag.as_mut() else {
            return;
        };
        let dt = (delta as f32).max(1e-4);
        let mut card = drag.card.clone();
        let current = card.get_global_transform();
        let target = Transform3D::new(Basis::IDENTITY, drag.plane_point);
        let w = 1.0 - (-self.drag_follow_speed * dt).exp();
        let next = current.interpolate_with(&target, w);

        // Schwung: Bewegung in X kippt um Z, Bewegung in Z kippt um X.
        let vel = (next.origin - drag.last_position) / dt;
        drag.last_position = next.origin;
        let s = self.drag_sway_amount;
        let sway = Vector2::new((-vel.x * s).clamp(-0.5, 0.5), (vel.z * s).clamp(-0.5, 0.5));

        card.set_global_transform(next);
        card.bind_mut().set_drag_sway(sway);
    }

    /// Raycasts gehören in den Physik-Tick (Zugriff auf den Physics-Space).
    fn physics_process(&mut self, _delta: f64) {
        let Some((origin, dir)) = self.mouse_ray() else {
            return;
        };
        // Mauspunkt auf der Tischebene (Spitze des Zielpfeils)
        let table_y = self.base().get_global_position().y;
        self.mouse_point = (dir.y.abs() > 1e-4)
            .then(|| (table_y - origin.y) / dir.y)
            .filter(|t| *t > 0.0)
            .map(|t| origin + dir * t);
        // Zielwahl beendet: Hover neu aufbauen (war während der Wahl nur unsichtbar gemerkt)
        let aiming = self.is_aiming();
        if self.was_aiming && !aiming {
            self.hovered = None;
        }
        self.was_aiming = aiming;

        if self.drag.is_some() {
            self.update_drag_target(origin, dir);
        } else {
            self.update_hover(origin, dir);
        }
    }

    fn unhandled_input(&mut self, event: Gd<InputEvent>) {
        if let Ok(mb) = event.clone().try_cast::<InputEventMouseButton>() {
            match (mb.get_button_index(), mb.is_pressed()) {
                (MouseButton::LEFT, true) => self.on_left_pressed(),
                (MouseButton::LEFT, false) => self.on_left_released(),
                (MouseButton::RIGHT, true) => self.on_right_pressed(),
                _ => return,
            }
            self.base().get_viewport().unwrap().set_input_as_handled();
        } else if let Ok(key) = event.try_cast::<InputEventKey>() {
            if key.is_pressed() && key.get_keycode() == Key::ESCAPE {
                if let Some(cam) = self.camera.as_mut() {
                    cam.bind_mut().show_overview();
                }
            }
        }
    }
}

#[godot_api]
impl TableController {
    /// Wird ausgelöst, wenn eine Karte per Drag-and-Drop in eine Zone gelegt wurde.
    #[signal]
    fn card_dropped(card: Gd<Card3D>, from_zone: Gd<Zone>, to_zone: Gd<Zone>);

    /// Wird ausgelöst, wenn eine Karte vom Deck auf die Hand gezogen wurde.
    #[signal]
    fn card_drawn(card: Gd<Card3D>);

    /// Zieht die oberste Karte des Decks auf die Hand.
    #[func]
    pub fn draw_card(&mut self, delay: f64) -> Option<Gd<Card3D>> {
        let deck = self.deck_zone.clone()?;
        let hand = self.hand_zone.clone()?;
        let card = Self::move_top_card(deck, hand, delay)?;
        self.signals().card_drawn().emit(&card);
        Some(card)
    }

    /// Bewegt die oberste Karte von `from` nach `to` (z. B. Gegner zieht / spielt aus).
    #[func]
    pub fn move_top_card(mut from: Gd<Zone>, mut to: Gd<Zone>, delay: f64) -> Option<Gd<Card3D>> {
        let card = from.bind().get_top_card()?;
        from.bind_mut().remove_card(card.clone());
        to.bind_mut().add_card(card.clone(), -1, delay);
        Some(card)
    }

    /// Stellt die Sprache des Spiels um: `"de"` = Deutsch, `"en"` = Englisch.
    /// HUD, Hinweise und neue Protokolleinträge wechseln sofort; alle Karten werden
    /// neu beschriftet. Bereits geschriebene Protokolleinträge bleiben unverändert.
    #[func]
    pub fn set_language(&mut self, code: GString) {
        let lang = if code.to_string().to_lowercase().starts_with("en") {
            crate::i18n::Lang::En
        } else {
            crate::i18n::Lang::De
        };
        crate::i18n::set_lang(lang);
        self.refresh_all_cards();
    }

    /// Aktuelle Sprache: `"de"` oder `"en"`.
    #[func]
    pub fn get_language(&self) -> GString {
        if crate::i18n::lang() == crate::i18n::Lang::En { "en".into() } else { "de".into() }
    }

    /// Die Karte, die gerade unter dem Mauszeiger liegt.
    #[func]
    /// Während einer Zielwahl (Zielpfeil) null: Karten werden dann nicht gezoomt.
    pub fn get_hovered_card(&self) -> Option<Gd<Card3D>> {
        if self.is_aiming() {
            return None;
        }
        self.hovered.clone()
    }

    /// Die Karte, die gerade gezogen wird.
    #[func]
    pub fn get_dragged_card(&self) -> Option<Gd<Card3D>> {
        self.drag.as_ref().map(|d| d.card.clone())
    }
}

impl TableController {
    // ------------------------------------------------------------------ Zielpfeile

    /// Läuft eine Zielwahl? (Pfeil wird gezogen oder eine Karte wartet auf ihr Ziel)
    fn is_aiming(&self) -> bool {
        self.aim_drag.is_some() || self.turn_manager.as_ref().is_some_and(|tm| tm.bind().aim_source().is_some())
    }

    /// Pfeile neu zusammenstellen: Kampfpfeile plus ggf. Zielpfeil zur Maus bzw. zur Karte darunter.
    fn update_arrows(&mut self) {
        let Some(mut layer) = self.arrows.clone() else { return };
        let mut specs: Vec<ArrowSpec> = Vec::new();
        let mut source = self.aim_drag.clone();
        if let Some(tm) = self.turn_manager.as_ref() {
            let tm = tm.bind();
            specs.extend(tm.combat_arrows());
            source = source.or_else(|| tm.aim_source());
        }
        if let Some(src) = source.filter(|c| c.is_instance_valid()) {
            let from = src.get_global_position();
            // Über einer anderen Karte: Pfeil rastet auf deren Mitte ein
            let target = self
                .hovered
                .as_ref()
                .filter(|c| c.is_instance_valid() && **c != src)
                .map(|c| c.get_global_position())
                .or(self.mouse_point);
            if let Some(to) = target {
                let dir = (to - from).normalized_or_zero();
                specs.push(ArrowSpec { from: from + dir * 0.3, to, color: SELECT_GLOW });
            }
        }
        layer.bind_mut().set_arrows(&specs);
    }

    // ------------------------------------------------------------------ Raycasting

    fn mouse_ray(&self) -> Option<(Vector3, Vector3)> {
        let cam = self.camera.as_ref()?;
        let mouse = self.base().get_viewport()?.get_mouse_position();
        Some((cam.project_ray_origin(mouse), cam.project_ray_normal(mouse)))
    }

    fn raycast(&self, origin: Vector3, dir: Vector3, mask: u32) -> Option<RayHit> {
        self.raycast_excluding(origin, dir, mask, None)
    }

    /// Raycast, der optional ein Kollisionsobjekt ignoriert (z. B. die gezogene Karte).
    fn raycast_excluding(&self, origin: Vector3, dir: Vector3, mask: u32, exclude: Option<Rid>) -> Option<RayHit> {
        let mut space = self.base().get_world_3d()?.get_direct_space_state()?;
        let mut query = PhysicsRayQueryParameters3D::create(origin, origin + dir * self.ray_length)?;
        if let Some(rid) = exclude {
            let mut list: Array<Rid> = Array::new();
            list.push(rid);
            query.set_exclude(&list);
        }
        query.set_collision_mask(mask);
        query.set_collide_with_areas(true);
        query.set_collide_with_bodies(false);
        let hit = space.intersect_ray(&query);
        let collider = hit.get("collider")?.try_to::<Gd<Object>>().ok()?;
        let position = hit.get("position")?.try_to::<Vector3>().ok()?;
        Some(RayHit { collider, position })
    }

    // ------------------------------------------------------------------ Hover

    fn update_hover(&mut self, origin: Vector3, dir: Vector3) {
        // Liegt die Maus über einer Bildschirmfläche, die Mausereignisse annimmt
        // (Stack-Anzeige, Buttons, Debug-Fenster …), wird nichts dahinter gehovert.
        let over_gui = self
            .base()
            .get_viewport()
            .is_some_and(|v| v.gui_get_hovered_control().is_some());
        let hit = if over_gui {
            None
        } else {
            self.raycast(origin, dir, CARD_LAYER).and_then(|h| {
                let card = h.collider.try_cast::<Card3D>().ok()?;
                Some((card, h.position))
            })
        };

        let new_card = hit.as_ref().map(|(c, _)| c.clone());
        // Während der Zielwahl nur merken (Pfeilziel), nicht zoomen
        let aiming = self.is_aiming();
        if new_card != self.hovered {
            if let Some(mut old) = self.hovered.take() {
                if old.is_instance_valid() {
                    old.bind_mut().set_hovered(false);
                }
            }
            if let Some(mut new) = new_card.clone().filter(|_| !aiming) {
                new.bind_mut().set_hovered(true);
            }
            self.hovered = new_card;
        }
        if aiming {
            return;
        }
        if let Some((mut card, point)) = hit {
            card.bind_mut().update_hover_point(point);
        }
    }

    fn clear_hover(&mut self) {
        if let Some(mut old) = self.hovered.take() {
            if old.is_instance_valid() {
                old.bind_mut().set_hovered(false);
            }
        }
    }

    // ------------------------------------------------------------------ Drag & Drop

    fn on_left_pressed(&mut self) {
        let Some(card) = self.hovered.clone() else {
            return;
        };
        let Some(zone) = card.bind().get_zone() else {
            return;
        };
        let (draw, draggable) = {
            let z = zone.bind();
            (z.click_to_draw(), z.cards_draggable())
        };
        // Mit Regeln: Kampfauswahl per Klick, Ziehen nur in der Zieh-Phase,
        // greifen nur eigene Karten.
        if let Some(mut tm) = self.turn_manager.clone() {
            // Zielwahl: Pfeil von dieser Karte ziehen (Loslassen auf ihr selbst = normaler Klick)
            if tm.bind().can_aim_from(card.clone()) {
                card.clone().bind_mut().set_hovered(false);
                self.aim_drag = Some(card);
                return;
            }
            if tm.bind_mut().on_card_clicked(card.clone()) {
                return;
            }
            if draggable && !draw && tm.bind().can_drag(card.clone(), zone.clone()) {
                self.start_drag(card, zone);
            }
            return;
        }
        if draw {
            self.draw_card(0.0);
        } else if draggable {
            self.start_drag(card, zone);
        }
    }

    fn start_drag(&mut self, mut card: Gd<Card3D>, mut zone: Gd<Zone>) {
        self.clear_hover();
        let origin_index = zone.bind_mut().remove_card(card.clone());
        card.bind_mut().set_dragging(true);
        let pos = card.get_global_position();
        if let Some(cam) = self.camera.as_mut() {
            cam.bind_mut().follow(card.clone().upcast());
        }
        self.drag = Some(DragState {
            card,
            origin_zone: zone,
            origin_index,
            plane_point: pos,
            last_position: pos,
            aura_target: None,
        });
    }

    fn update_drag_target(&mut self, origin: Vector3, dir: Vector3) {
        // 1) Schnittpunkt Mausstrahl mit der horizontalen Drag-Ebene.
        let plane_y = self.base().get_global_position().y + self.drag_height;
        if dir.y.abs() > 1e-4 {
            let t = (plane_y - origin.y) / dir.y;
            if t > 0.0 {
                if let Some(drag) = self.drag.as_mut() {
                    drag.plane_point = origin + dir * t;
                }
            }
        }

        // 2) Kollisionsabfrage gegen die Zonen -> mögliches Drop-Ziel.
        let zone = self
            .raycast(origin, dir, ZONE_LAYER)
            .and_then(|h| h.collider.try_cast::<Zone>().ok())
            .filter(|z| z.bind().accepts_drops());

        if zone != self.hover_zone {
            if let Some(mut old) = self.hover_zone.take() {
                old.bind_mut().set_highlighted(false);
            }
            if let Some(mut new) = zone.clone() {
                new.bind_mut().set_highlighted(true);
            }
            self.hover_zone = zone;
        }

        // 3) Aura: gültige bleibende Karte unter dem Mauszeiger als Ziel (gelb markieren).
        let Some(drag) = self.drag.as_ref() else {
            return;
        };
        if drag.card.bind().kind() != CardKind::Aura {
            return;
        }
        let rid = drag.card.get_rid();
        let target = self
            .raycast_excluding(origin, dir, CARD_LAYER, Some(rid))
            .and_then(|h| h.collider.try_cast::<Card3D>().ok())
            .filter(|c| match self.turn_manager.as_ref() {
                Some(tm) => tm.bind().is_legal_aura_target(drag.card.clone(), c.clone()),
                None => c.bind().get_zone().is_some(),
            });
        let drag = self.drag.as_mut().unwrap();
        if target != drag.aura_target {
            if let Some(mut old) = drag.aura_target.take() {
                old.bind_mut().set_marker(CardMarker::None);
            }
            if let Some(mut new) = target.clone() {
                new.bind_mut().set_marker(CardMarker::Selected);
            }
            drag.aura_target = target;
        }
    }

    fn on_left_released(&mut self) {
        if let Some(source) = self.aim_drag.take() {
            let target = self.hovered.clone().filter(|c| c.is_instance_valid());
            if let Some(mut tm) = self.turn_manager.clone() {
                match target {
                    // Auf der Karte selbst losgelassen: wie ein Klick (auswählen, Pfeil folgt der Maus)
                    Some(t) if t == source => {
                        tm.bind_mut().on_card_clicked(source);
                    }
                    Some(t) => {
                        tm.bind_mut().aim_at(source, t);
                    }
                    None => {}
                }
            }
            return;
        }
        let Some(drag) = self.drag.take() else {
            return;
        };
        let DragState { mut card, origin_zone, origin_index, plane_point, aura_target, .. } = drag;
        if let Some(mut t) = aura_target.clone() {
            t.bind_mut().set_marker(CardMarker::None);
        }

        let mut target = self.hover_zone.take();
        if let Some(mut zone) = target.clone() {
            zone.bind_mut().set_highlighted(false);
        }
        // Regeln fragen: darf die Karte dort hin (Phase, Mana, 1 Ressource pro Runde)?
        // Die Regeln können die Zielzone auch umlenken (Ressource -> Ressourcen-Reihe).
        if let Some(mut tm) = self.turn_manager.clone() {
            let is_aura = card.bind().kind() == CardKind::Aura;
            target = match (is_aura, aura_target, target.clone()) {
                // Aura auf eine Kreatur gezogen
                (true, Some(host), _) => tm.bind_mut().resolve_aura_drop(card.clone(), origin_zone.clone(), host),
                (_, _, Some(zone)) => {
                    tm.bind_mut().resolve_drop_at(card.clone(), origin_zone.clone(), zone, plane_point)
                }
                _ => None,
            };
        }
        let (mut dest, index) = match target {
            Some(mut zone) => {
                zone.bind_mut().set_highlighted(false);
                if zone.bind().is_free_placement() {
                    // Freie Fläche: Karte bleibt dort liegen, wo sie losgelassen wurde.
                    card.bind_mut().set_dragging(false);
                    zone.bind_mut().add_card_at(card.clone(), plane_point, 0.0);
                    if let Some(cam) = self.camera.as_mut() {
                        cam.bind_mut().show_overview();
                    }
                    self.signals().card_dropped().emit(&card, &origin_zone, &zone);
                    return;
                }
                let idx = zone.bind().insertion_index(plane_point);
                (zone, idx)
            }
            // Kein gültiges Ziel: zurück an den Ursprungsplatz.
            None => (origin_zone.clone(), origin_index),
        };

        card.bind_mut().set_dragging(false);
        dest.bind_mut().add_card(card.clone(), index, 0.0);

        if let Some(cam) = self.camera.as_mut() {
            cam.bind_mut().show_overview();
        }
        self.signals().card_dropped().emit(&card, &origin_zone, &dest);
    }

    // ------------------------------------------------------------------ Fokus

    fn on_right_pressed(&mut self) {
        if self.drag.is_some() {
            return;
        }
        let hovered = self.hovered.clone();
        let Some(cam) = self.camera.as_mut() else {
            return;
        };
        let mut cam = cam.bind_mut();
        match hovered {
            Some(card) => {
                let node: Gd<Node3D> = card.upcast();
                if cam.is_focused_on(node.clone()) {
                    cam.show_overview();
                } else {
                    cam.focus_on(node);
                }
            }
            None => cam.show_overview(),
        }
    }

    // ------------------------------------------------------------------ Demo

    /// Gemeinsame Kartenrückseite (einmal erzeugt).
    fn back_texture(&mut self) -> Gd<godot::classes::Texture2D> {
        if self.back_texture.is_none() {
            self.back_texture = Some(placeholder::card_back());
        }
        self.back_texture.clone().unwrap()
    }

    /// Erzeugt eine Karte aus einer Definition und legt sie in `zone`
    /// (z. B. Hand oder Ressourcen-Reihe). Wird vom Debug-Tool benutzt.
    #[cfg(debug_assertions)]
    pub fn spawn_card(&mut self, def: &CardDef, owner: usize, mut zone: Gd<Zone>) -> Gd<Card3D> {
        let back = self.back_texture();
        let mut card = Card3D::new_alloc();
        let id = card.instance_id();
        card.set_name(&format!("Debug{id}"));
        {
            let mut c = card.bind_mut();
            c.owner_seat = owner as i64;
            c.set_face_texture(Some(placeholder::card_face(def)));
            c.apply_def(def);
            c.set_back_texture(Some(back));
        }
        self.base_mut().add_child(&card);
        // Startet sichtbar über dem Deck des Besitzers und gleitet dann in die Zielzone
        let deck = if owner == 1 { self.opponent_deck_zone.as_ref() } else { self.deck_zone.as_ref() };
        let start = deck.map_or(zone.get_global_transform(), |d| d.get_global_transform());
        card.bind_mut().snap_to(start);
        zone.bind_mut().add_card(card.clone(), -1, 0.0);
        card
    }

    /// Beschriftet alle Karten neu (z. B. nach einem Sprachwechsel).
    pub fn refresh_all_cards(&mut self) {
        for child in self.base().get_children().iter_shared() {
            if let Ok(mut card) = child.try_cast::<Card3D>() {
                card.bind_mut().refresh_labels();
            }
        }
    }

    /// Zonen-Zugriff für das Debug-Tool.
    #[cfg(debug_assertions)]
    pub fn hand(&self) -> Option<Gd<Zone>> {
        self.hand_zone.clone()
    }

    /// Handzone des Gegners (für das Debug-Tool).
    #[cfg(debug_assertions)]
    pub fn opponent_hand(&self) -> Option<Gd<Zone>> {
        self.opponent_hand_zone.clone()
    }

    fn spawn_demo_cards(&mut self) {
        let back = self.back_texture();
        let Some(deck) = self.deck_zone.clone() else {
            godot_warn!("TableController: deck_zone nicht gesetzt, keine Demo-Karten");
            return;
        };
        self.fill_deck(deck, 0, PLAYER_COLORS, &back);
        for k in 0..self.demo_hand_size {
            self.draw_card(0.3 + k as f64 * 0.12);
        }

        if let Some(opp_deck) = self.opponent_deck_zone.clone() {
            self.fill_deck(opp_deck.clone(), 1000, OPPONENT_COLORS, &back);
            if let Some(opp_hand) = self.opponent_hand_zone.clone() {
                for k in 0..self.demo_opponent_hand_size {
                    Self::move_top_card(opp_deck.clone(), opp_hand.clone(), 0.35 + k as f64 * 0.12);
                }
            }
            if let Some(opp_bf) = self.opponent_battlefield_zone.clone() {
                for k in 0..self.demo_opponent_battlefield_size {
                    Self::move_top_card(opp_deck.clone(), opp_bf.clone(), 1.1 + k as f64 * 0.2);
                }
            }
        }
    }

    fn fill_deck(
        &mut self,
        mut deck: Gd<Zone>,
        first_index: usize,
        colors: [ManaType; 2],
        back: &Gd<godot::classes::Texture2D>,
    ) {
        let start = deck.get_global_transform();
        for i in 0..self.demo_deck_size as usize {
            let id = first_index + i;
            let mut card = Card3D::new_alloc();
            card.set_name(&format!("Card{id}"));
            {
                let mut c = card.bind_mut();
                let def = cards::demo_card(id, colors);
                c.owner_seat = if first_index >= 1000 { 1 } else { 0 };
                c.set_face_texture(Some(placeholder::card_face(&def)));
                c.apply_def(&def);
                c.set_back_texture(Some(back.clone()));
            }
            self.base_mut().add_child(&card);
            card.bind_mut().snap_to(start);
            deck.bind_mut().add_card(card, -1, 0.0);
        }
    }
}
