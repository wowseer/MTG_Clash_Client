//! TableCamera: wechselt weich zwischen Tischübersicht, Nahaufnahme einer Karte
//! und einem leichten Mitführen während des Ziehens.

use godot::classes::{Camera3D, ICamera3D};
use godot::prelude::*;

#[derive(Clone)]
enum CameraMode {
    Overview,
    /// Nahaufnahme: Kamera schaut senkrecht auf die Kartenvorderseite.
    Focus(Gd<Node3D>),
    /// Während des Ziehens: Kamera folgt der Karte ein Stück und zoomt leicht.
    Follow(Gd<Node3D>),
}

/// Tischkamera: Übersicht über den Tisch, Nahaufnahme einer Karte (Rechtsklick) und leichtes
/// Mitführen beim Ziehen einer Karte. Alle Übergänge sind weich.
#[derive(GodotClass)]
#[class(base=Camera3D)]
pub struct TableCamera {
    /// Kameraposition in der Übersicht.
    #[export]
    overview_position: Vector3,
    /// Blickpunkt in der Übersicht.
    #[export]
    overview_target: Vector3,
    /// Abstand zur Karte in der Nahaufnahme.
    #[export]
    focus_distance: f32,
    /// Wie stark die Kamera einer gezogenen Karte folgt (0 = gar nicht, 1 = voll).
    #[export(range = (0.0, 1.0))]
    follow_strength: f32,
    /// Abstand zur gezogenen Karte, wenn follow_strength = 1.
    #[export]
    follow_distance: f32,
    /// Glättung der Kamerabewegung (größer = schneller).
    #[export]
    smoothing: f32,

    mode: CameraMode,
    base: Base<Camera3D>,
}

#[godot_api]
impl ICamera3D for TableCamera {
    fn init(base: Base<Camera3D>) -> Self {
        Self {
            overview_position: Vector3::new(0.0, 6.2, 5.2),
            overview_target: Vector3::new(0.0, 0.0, 0.7),
            focus_distance: 1.25,
            follow_strength: 0.3,
            follow_distance: 3.5,
            smoothing: 6.0,
            mode: CameraMode::Overview,
            base,
        }
    }

    fn ready(&mut self) {
        let xf = self.overview_transform();
        self.base_mut().set_global_transform(xf);
    }

    fn process(&mut self, delta: f64) {
        // Freigegebene Karten sicher behandeln.
        if let CameraMode::Focus(n) | CameraMode::Follow(n) = &self.mode {
            if !n.is_instance_valid() {
                self.mode = CameraMode::Overview;
            }
        }

        let desired = match &self.mode {
            CameraMode::Overview => self.overview_transform(),
            CameraMode::Focus(node) => self.focus_transform(&node.get_global_transform()),
            CameraMode::Follow(node) => self.follow_transform(node.get_global_position()),
        };

        let w = 1.0 - (-self.smoothing * delta as f32).exp();
        let current = self.base().get_global_transform();
        let next = current.interpolate_with(&desired, w);
        self.base_mut().set_global_transform(next);
    }
}

#[godot_api]
impl TableCamera {
    /// Zurück zur Tischübersicht.
    #[func]
    pub fn show_overview(&mut self) {
        self.mode = CameraMode::Overview;
    }

    /// Nahaufnahme einer Karte (oder eines beliebigen Node3D).
    #[func]
    pub fn focus_on(&mut self, node: Gd<Node3D>) {
        self.mode = CameraMode::Focus(node);
    }

    /// Leichtes Mitführen (z. B. während eine Karte gezogen wird).
    #[func]
    pub fn follow(&mut self, node: Gd<Node3D>) {
        self.mode = CameraMode::Follow(node);
    }

    /// Ist die Kamera gerade auf genau diesen Node fokussiert?
    #[func]
    pub fn is_focused_on(&self, node: Gd<Node3D>) -> bool {
        matches!(&self.mode, CameraMode::Focus(n) if n == &node)
    }

    /// Ist die Kamera gerade in der Übersicht?
    #[func]
    pub fn is_overview(&self) -> bool {
        matches!(self.mode, CameraMode::Overview)
    }
}

impl TableCamera {
    fn overview_transform(&self) -> Transform3D {
        look_from(self.overview_position, self.overview_target, Vector3::UP)
    }

    fn focus_transform(&self, card: &Transform3D) -> Transform3D {
        let center = card.origin;
        let mut normal = card.basis.col_b().normalized();
        // Immer von der Seite schauen, auf der die Übersichtskamera steht.
        if normal.dot(self.overview_position - center) < 0.0 {
            normal = -normal;
        }
        // Kartenoberkante zeigt lokal nach -Z -> als "oben" im Bild verwenden.
        let mut up = -card.basis.col_c().normalized();
        if up.cross(normal).length_squared() < 1e-4 {
            up = Vector3::FORWARD;
        }
        // Minimal schräg von unten, wirkt plastischer als exakt senkrecht.
        let dir = (normal * 0.96 - up * 0.28).normalized();
        look_from(center + dir * self.focus_distance, center, up)
    }

    fn follow_transform(&self, card_pos: Vector3) -> Transform3D {
        let k = self.follow_strength;
        let view_dir = (self.overview_position - self.overview_target).normalized();
        let near_pos = card_pos + view_dir * self.follow_distance;
        let pos = self.overview_position.lerp(near_pos, k);
        let target = self.overview_target.lerp(card_pos, k);
        look_from(pos, target, Vector3::UP)
    }
}

fn look_from(eye: Vector3, target: Vector3, up: Vector3) -> Transform3D {
    let dir = target - eye;
    if dir.length_squared() < 1e-6 {
        return Transform3D::new(Basis::IDENTITY, eye);
    }
    Transform3D::new(Basis::looking_at_ex(dir).up(up).done(), eye)
}
