//! TableController: verbindet Kamera, Zonen und Karten.
//!
//! - Hover: Raycast Maus -> 3D-Bedienobjekte (CONTROL_LAYER, Vorrang), sonst Karten (CARD_LAYER),
//!   liefert Trefferpunkt für den Tilt. Klick auf ein Bedienobjekt ruft dessen `activate()`.
//! - Drag: Karte aus einer Zone greifen; sie folgt der Maus flach auf dem Bildschirm (Ebene
//!   parallel zum Bildschirm durch den Greifpunkt) und behält Ausrichtung und Größe von dort
//!   (Handkarten gerade, ohne Fächer-Drehung, in Handgröße) – kein Kippen, kein Schwung, die
//!   Kamera bleibt stehen. Erst beim Ablegen
//!   gleitet sie auf Tischgröße. Per Raycast (ZONE_LAYER) das Drop-Ziel bestimmen und dort ablegen.
//!   Ungültiger Drop -> Karte gleitet an ihren alten Platz zurück.
//! - Klick: Drücken und ohne Bewegung (< 6 px) loslassen → Signal `card_clicked`; erst bei
//!   Bewegung wird gezogen.
//! - Rechtsklick: Nahaufnahme der Karte unter der Maus (erneut = zurück), Esc = Übersicht.
//!
//! Regeln kennt der Controller nicht: Er meldet jeden Drop per Signal `card_dropped`, der
//! [`ForgeTable`](crate::forge::table::ForgeTable) setzt ihn über Forge um oder legt die Karte zurück.

use godot::classes::{
    INode3D, InputEvent, InputEventKey, InputEventMouseButton, PhysicsRayQueryParameters3D,
};
use godot::global::{Key, MouseButton};
use godot::prelude::*;

use super::card3d::Card3D;
use super::button3d::Button3D;
use super::layers::{CARD_LAYER, CONTROL_LAYER, ZONE_LAYER};
use super::table_camera::TableCamera;
use super::zone::Zone;

/// Mausbewegung in Pixeln, ab der aus einem Klick ein Ziehen wird.
const CLICK_SLOP: f32 = 6.0;

struct DragState {
    card: Gd<Card3D>,
    origin_zone: Gd<Zone>,
    origin_index: i64,
    /// Mauspunkt auf dem Tisch (Höhe der Drag-Ebene) – Ablagepunkt für das Drop-Ziel.
    plane_point: Vector3,
    /// Greifpunkt (Welt): durch ihn läuft die bildschirmparallele Ebene.
    grab_origin: Vector3,
    /// Mauspunkt auf der bildschirmparallelen Ebene durch den Greifpunkt: hier liegt die Karte.
    screen_point: Vector3,
    /// Ausrichtung und Größe der Karte beim Greifen (z. B. Hand): bleibt beim Ziehen erhalten.
    grab_basis: Basis,
}

struct RayHit {
    collider: Gd<Object>,
    position: Vector3,
}

/// Wurzel der Tischszene: Maus-Eingabe (Hover, Drag-and-Drop per Raycast) und Kamera-Fokus
/// (Rechtsklick). Kennt keine Regeln: Jeder Drop wird per [signal card_dropped] gemeldet; der
/// [ForgeTable] spielt die Karte dann über Forge oder legt sie an ihren Platz zurück.
#[derive(GodotClass)]
#[class(base=Node3D)]
pub struct TableController {
    /// Die Tischkamera.
    #[export]
    camera: Option<Gd<TableCamera>>,
    /// Höhe der Drag-Ebene über dem Tisch (Tischoberfläche = y dieses Nodes).
    #[export]
    drag_height: f32,
    /// Wie schnell die gezogene Karte der Maus folgt.
    #[export]
    drag_follow_speed: f32,
    /// Länge der Maus-Raycasts in Einheiten.
    #[export]
    ray_length: f32,
    hovered: Option<Gd<Card3D>>,
    hover_zone: Option<Gd<Zone>>,
    drag: Option<DragState>,
    /// Gedrückte Karte und Mausposition beim Drücken: wird zum Ziehen, sobald sich die Maus mehr
    /// als [`CLICK_SLOP`] Pixel bewegt; sonst ist das Loslassen ein Klick.
    press: Option<(Gd<Card3D>, Vector2)>,
    /// 3D-Bedienobjekt unter der Maus (Vorrang vor Karten).
    hovered_control: Option<Gd<Button3D>>,
    /// Gedrücktes Bedienobjekt: Klick, wenn die Maus beim Loslassen noch darauf ist.
    control_press: Option<Gd<Button3D>>,
    /// Zielen: von dieser Karte (nicht verschiebbar, z. B. auf dem Spielfeld) wird ein Pfeil
    /// gezogen; `aim_point` = Mauspunkt auf der Tischebene.
    aim: Option<Gd<Card3D>>,
    aim_point: Vector3,

    base: Base<Node3D>,
}

#[godot_api]
impl INode3D for TableController {
    fn init(base: Base<Node3D>) -> Self {
        Self {
            camera: None,
            drag_height: 0.35,
            drag_follow_speed: 20.0,
            ray_length: 100.0,
            hovered: None,
            hover_zone: None,
            drag: None,
            press: None,
            hovered_control: None,
            control_press: None,
            aim: None,
            aim_point: Vector3::ZERO,
            base,
        }
    }

    /// Kartenbewegung beim Ziehen: jedes Frame für flüssige Bewegung.
    fn process(&mut self, delta: f64) {
        let Some(drag) = self.drag.as_mut() else {
            return;
        };
        let dt = (delta as f32).max(1e-4);
        let mut card = drag.card.clone();
        let current = card.get_global_transform();
        // Flach auf dem Bildschirm: Ausrichtung/Größe vom Greifen, Position auf der Bildschirmebene
        let target = Transform3D::new(drag.grab_basis, drag.screen_point);
        let w = 1.0 - (-self.drag_follow_speed * dt).exp();
        let next = current.interpolate_with(&target, w);
        // Kein Schwung/Kippen: die Karte bleibt flach zum Bildschirm
        card.set_global_transform(next);
    }

    /// Raycasts gehören in den Physik-Tick (Zugriff auf den Physics-Space).
    fn physics_process(&mut self, _delta: f64) {
        let Some((origin, dir)) = self.mouse_ray() else {
            return;
        };
        // Gedrückt gehaltene Karte: ab CLICK_SLOP Pixel Bewegung wird gezogen
        if let Some((card, at)) = self.press.clone() {
            let moved = self.base().get_viewport().map_or(0.0, |v| (v.get_mouse_position() - at).length());
            if moved > CLICK_SLOP {
                self.press = None;
                let zone = card.bind().get_zone().filter(|z| z.bind().cards_draggable());
                if let Some(zone) = zone.filter(|_| card.is_instance_valid()) {
                    self.start_drag(card, zone);
                } else if card.is_instance_valid() {
                    // Nicht verschiebbar: Pfeil ziehen (zielen)
                    self.aim = Some(card.clone());
                    self.signals().aim_started().emit(&card);
                }
            }
        }
        if self.drag.is_some() {
            self.update_drag_target(origin, dir);
        } else {
            self.update_hover(origin, dir);
        }
        if self.aim.is_some() && dir.y.abs() > 1e-4 {
            let t = (self.base().get_global_position().y - origin.y) / dir.y;
            if t > 0.0 {
                self.aim_point = origin + dir * t;
            }
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

    /// Eine Karte wurde angeklickt (gedrückt und ohne nennenswerte Bewegung losgelassen) –
    /// z. B. als Ziel wählen oder Fähigkeit aktivieren. Funktioniert in allen Zonen, auch bei
    /// Karten, die man nicht ziehen kann.
    #[signal]
    fn card_clicked(card: Gd<Card3D>);

    /// Von einer nicht verschiebbaren Karte (z. B. Kreatur auf dem Spielfeld) wird gezogen: der
    /// Spieler zieht einen Pfeil. Wohin er zeigt, liefert [method get_aim_point]; worauf die Maus
    /// liegt, [method get_hovered_card].
    #[signal]
    fn aim_started(card: Gd<Card3D>);

    /// Pfeil losgelassen: `target` = Karte unter der Maus (null = keine). Was das bedeutet
    /// (Angriff, Block, Ziel), entscheidet der [ForgeTable].
    #[signal]
    fn aim_released(card: Gd<Card3D>, target: Option<Gd<Card3D>>);


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
    pub fn get_hovered_card(&self) -> Option<Gd<Card3D>> {
        self.hovered.clone()
    }

    /// Karte, von der gerade ein Pfeil gezogen wird (null = keine).
    #[func]
    pub fn get_aiming_card(&self) -> Option<Gd<Card3D>> {
        self.aim.clone().filter(|c| c.is_instance_valid())
    }

    /// Mauspunkt auf der Tischebene beim Pfeilziehen (Welt).
    #[func]
    pub fn get_aim_point(&self) -> Vector3 {
        self.aim_point
    }

    /// Die Karte, die gerade gezogen wird.
    #[func]
    pub fn get_dragged_card(&self) -> Option<Gd<Card3D>> {
        self.drag.as_ref().map(|d| d.card.clone())
    }
}

impl TableController {
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
        // 3D-Bedienobjekte zuerst: liegt die Maus auf einem, wird keine Karte gehovert
        let control = if over_gui {
            None
        } else {
            self.raycast(origin, dir, CONTROL_LAYER).and_then(|h| h.collider.try_cast::<Button3D>().ok())
        };
        if control != self.hovered_control {
            if let Some(mut old) = self.hovered_control.take() {
                if old.is_instance_valid() {
                    old.bind_mut().set_hovered(false);
                }
            }
            if let Some(mut new) = control.clone() {
                new.bind_mut().set_hovered(true);
            }
            self.hovered_control = control.clone();
        }
        let hit = if over_gui || control.is_some() {
            None
        } else {
            self.raycast(origin, dir, CARD_LAYER).and_then(|h| {
                let card = h.collider.try_cast::<Card3D>().ok()?;
                Some((card, h.position))
            })
        };

        let new_card = hit.as_ref().map(|(c, _)| c.clone());
        if new_card != self.hovered {
            if let Some(mut old) = self.hovered.take() {
                if old.is_instance_valid() {
                    old.bind_mut().set_hovered(false);
                }
            }
            if let Some(mut new) = new_card.clone() {
                new.bind_mut().set_hovered(true);
            }
            self.hovered = new_card;
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

    /// Drücken auf eine Karte: merken. Ob es ein Klick oder ein Ziehen wird, entscheidet die
    /// Mausbewegung (siehe `physics_process`).
    fn on_left_pressed(&mut self) {
        if let Some(control) = self.hovered_control.clone().filter(|c| c.is_instance_valid()) {
            self.control_press = Some(control);
            return;
        }
        let Some(card) = self.hovered.clone() else {
            return;
        };
        let at = self.base().get_viewport().map_or(Vector2::ZERO, |v| v.get_mouse_position());
        self.press = Some((card, at));
    }

    fn start_drag(&mut self, mut card: Gd<Card3D>, mut zone: Gd<Zone>) {
        self.clear_hover();
        let grab = card.get_global_transform();
        // Aus der Hand: gerade halten (Neigung und Größe der Hand, aber ohne Fächer-Drehung)
        let grab_basis = if zone.bind().zone_kind() == super::zone::ZoneKind::Hand {
            zone.get_global_transform().basis
        } else {
            grab.basis
        };
        let origin_index = zone.bind_mut().remove_card(card.clone());
        card.bind_mut().set_dragging(true);
        let pos = card.get_global_position();
        // Kamera bleibt stehen (kein Mitführen), damit die Karte rein auf dem Bildschirm wandert
        self.drag = Some(DragState {
            card,
            origin_zone: zone,
            origin_index,
            plane_point: pos,
            grab_origin: pos,
            screen_point: pos,
            grab_basis,
        });
    }

    fn update_drag_target(&mut self, origin: Vector3, dir: Vector3) {
        // 1) Schnittpunkte des Mausstrahls: mit der waagerechten Tischebene (Ablagepunkt) und mit
        //    der bildschirmparallelen Ebene durch den Greifpunkt (dort wird die Karte gezeigt).
        let plane_y = self.base().get_global_position().y + self.drag_height;
        let cam_forward = self.camera.as_ref().map(|c| -c.get_global_transform().basis.col_c());
        if let Some(drag) = self.drag.as_mut() {
            if dir.y.abs() > 1e-4 {
                let t = (plane_y - origin.y) / dir.y;
                if t > 0.0 {
                    drag.plane_point = origin + dir * t;
                }
            }
            if let Some(n) = cam_forward {
                let denom = dir.dot(n);
                if denom.abs() > 1e-4 {
                    let t = (drag.grab_origin.dot(n) - origin.dot(n)) / denom;
                    if t > 0.0 {
                        drag.screen_point = origin + dir * t;
                    }
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
    }

    fn on_left_released(&mut self) {
        // Bedienobjekt: Klick, wenn die Maus noch darauf liegt
        if let Some(mut control) = self.control_press.take() {
            if control.is_instance_valid() && self.hovered_control.as_ref() == Some(&control) {
                control.bind_mut().activate();
            }
            return;
        }
        if let Some((card, _)) = self.press.take() {
            if card.is_instance_valid() {
                self.signals().card_clicked().emit(&card);
            }
            return;
        }
        if let Some(card) = self.aim.take() {
            if card.is_instance_valid() {
                let target = self.hovered.clone().filter(|h| h.is_instance_valid() && *h != card);
                self.signals().aim_released().emit(&card, target.as_ref());
            }
            return;
        }
        let Some(drag) = self.drag.take() else {
            return;
        };
        let DragState { mut card, origin_zone, origin_index, plane_point, .. } = drag;

        let target = self.hover_zone.take();
        if let Some(mut zone) = target.clone() {
            zone.bind_mut().set_highlighted(false);
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

    // ------------------------------------------------------------------ Karten

    /// Beschriftet alle Karten neu (z. B. nach einem Sprachwechsel).
    pub fn refresh_all_cards(&mut self) {
        for child in self.base().get_children().iter_shared() {
            if let Ok(mut card) = child.try_cast::<Card3D>() {
                card.bind_mut().refresh_labels();
            }
        }
    }
}
