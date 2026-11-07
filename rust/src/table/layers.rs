//! Physik-Layer für Raycasts (Bitmasken), Sicht-Layer der Kulisse und Reihenfolge der Bildebenen.

/// Layer 1: Karten (Hover/Anwählen).
pub const CARD_LAYER: u32 = 1 << 0;
/// Layer 2: Zonen (Drop-Ziele).
pub const ZONE_LAYER: u32 = 1 << 1;
/// Layer 3: 3D-Bedienobjekte (z. B. [`Button3D`](super::button3d::Button3D)); haben beim Hover
/// und Klick Vorrang vor Karten.
pub const CONTROL_LAYER: u32 = 1 << 2;

/// Sicht-Layer 20 (`VisualInstance3D.layers`): Kulisse (Tisch, Wände, Säulen). Die Hauptkamera
/// zeichnet nur sie; alles andere zeichnet die Vordergrund-Kamera
/// ([`TableCamera`](super::table_camera::TableCamera)) über den Medaillons. Lichter liegen auf
/// beiden Layern, damit sie Kulisse und Vordergrund beleuchten.
pub const BACKGROUND_VISUAL_LAYER: u32 = 1 << 19;
/// Alle Sicht-Layer (Godot kennt 20).
pub const ALL_VISUAL_LAYERS: u32 = (1 << 20) - 1;

/// Bildebenen (`CanvasLayer.layer`) von hinten nach vorn: Medaillons, Namen und Manapool liegen
/// über der Kulisse, aber unter dem 3D-Vordergrund (Karten, Bühnen, Knöpfe, Pfeile, Effekte);
/// das übrige HUD liegt ganz vorn.
pub const SEAT_CANVAS_LAYER: i32 = 1;
pub const FOREGROUND_CANVAS_LAYER: i32 = 2;
pub const HUD_CANVAS_LAYER: i32 = 3;

/// Setzt die Kulisse unter `node` auf [`BACKGROUND_VISUAL_LAYER`]: Meshes nur dorthin,
/// Lichter zusätzlich (sie beleuchten weiter auch den Vordergrund).
pub fn mark_background(node: &godot::prelude::Gd<godot::prelude::Node>) {
    use godot::classes::{GeometryInstance3D, Light3D, VisualInstance3D};
    for child in node.get_children().iter_shared() {
        if child.clone().try_cast::<GeometryInstance3D>().is_ok() {
            if let Ok(mut v) = child.clone().try_cast::<VisualInstance3D>() {
                v.set_layer_mask(BACKGROUND_VISUAL_LAYER);
            }
        } else if let Ok(mut l) = child.clone().try_cast::<Light3D>() {
            let mask = l.get_layer_mask();
            l.set_layer_mask(mask | BACKGROUND_VISUAL_LAYER);
        }
        mark_background(&child);
    }
}
