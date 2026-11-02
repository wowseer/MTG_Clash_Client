//! CardCutout: Maske, die HUD-Elemente dort ausspart, wo eine 3D-Karte davor schwebt.
//!
//! Das HUD (CanvasLayer) wird immer über der 3D-Szene gezeichnet. Damit eine gezoomte
//! oder gezogene Karte trotzdem **über** den Spieler-Medaillons liegt, hängen die
//! Medaillon-Zeilen unter dieser Maske (`clip_children = CLIP_CHILDREN_ONLY`): Die Maske
//! zeichnet den ganzen Bildschirm außer dem Kartenviereck – Kinder sind nur dort sichtbar.
//! Die Aussparung ist das auf den Bildschirm projizierte Viereck der Karte
//! ([`Card3D::screen_quad`](crate::table::card3d::Card3D::screen_quad)).

use godot::classes::canvas_item::ClipChildrenMode;
use godot::classes::control::{LayoutPreset, MouseFilter};
use godot::classes::{Control, IControl};
use godot::prelude::*;

/// Ausdehnung der Halbebenen, aus denen das Komplement des Vierecks zusammengesetzt wird (Pixel).
const FAR: f32 = 100_000.0;

/// Vollbild-Maske für HUD-Elemente: Ihre Kinder werden überall gezeichnet, außer innerhalb des
/// konvexen Vierecks [member cutout] (Bildschirmkoordinaten, 4 Punkte). So wirkt eine 3D-Karte,
/// die vor einem Medaillon schwebt, als läge sie darüber. Leeres [member cutout] = nichts ausgespart.
/// Der [TurnHud] setzt die Aussparung jedes Frame auf die gehoverte bzw. gezogene Karte.
#[derive(GodotClass)]
#[class(base=Control)]
pub struct CardCutout {
    /// Konvexes Viereck (4 Punkte, Bildschirmkoordinaten), in dem die Kinder unsichtbar sind.
    /// Leer = keine Aussparung.
    #[var(set = set_cutout)]
    cutout: PackedVector2Array,
    base: Base<Control>,
}

#[godot_api]
impl IControl for CardCutout {
    fn init(base: Base<Control>) -> Self {
        Self { cutout: PackedVector2Array::new(), base }
    }

    fn ready(&mut self) {
        self.base_mut().set_anchors_preset(LayoutPreset::FULL_RECT);
        self.base_mut().set_mouse_filter(MouseFilter::IGNORE);
        self.base_mut().set_clip_children_mode(ClipChildrenMode::ONLY);
    }

    fn draw(&mut self) {
        let quad: Vec<Vector2> = self.cutout.as_slice().to_vec();
        if quad.len() != 4 {
            let rect = Rect2::new(Vector2::splat(-FAR), Vector2::splat(2.0 * FAR));
            self.base_mut().draw_rect(rect, Color::WHITE);
            return;
        }
        // Komplement eines konvexen Vierecks = Vereinigung der Halbebenen außerhalb jeder Kante.
        let center = quad.iter().fold(Vector2::ZERO, |a, p| a + *p) / 4.0;
        for i in 0..4 {
            let (a, b) = (quad[i], quad[(i + 1) % 4]);
            let dir = (b - a).normalized_or_zero();
            if dir == Vector2::ZERO {
                continue;
            }
            let mut n = Vector2::new(-dir.y, dir.x);
            if n.dot(center - a) > 0.0 {
                n = -n;
            }
            let poly = [a - dir * FAR, b + dir * FAR, b + dir * FAR + n * FAR, a - dir * FAR + n * FAR];
            self.base_mut().draw_colored_polygon(&PackedVector2Array::from(&poly[..]), Color::WHITE);
        }
    }
}

#[godot_api]
impl CardCutout {
    /// Setzt die Aussparung (4 Punkte oder leer) und zeichnet die Maske bei Änderung neu.
    #[func]
    pub fn set_cutout(&mut self, quad: PackedVector2Array) {
        if quad != self.cutout {
            self.cutout = quad;
            self.base_mut().queue_redraw();
        }
    }
}
