//! HourglassIcon: kleines, per `draw()` gezeichnetes Sanduhr-Symbol für das HUD.
//!
//! Zwei goldene Querbalken oben und unten, dazwischen zwei Glaskolben (Dreiecke, die sich in
//! der Mitte treffen). Im unteren Kolben liegt ein Sandhaufen, ein dünner Sandstrahl läuft durch
//! die Engstelle. Keine Bilddateien.

use godot::classes::control::MouseFilter;
use godot::classes::{Control, IControl};
use godot::prelude::*;

const FRAME: Color = Color::from_rgb(0.9, 0.72, 0.36);
const GLASS: Color = Color::from_rgba(0.93, 0.87, 0.74, 0.25);
const SAND: Color = Color::from_rgb(0.95, 0.8, 0.45);

/// Sanduhr-Symbol (HUD), z. B. neben der Rundenzahl am linken Bildrand. Größe = [member icon_size]
/// (Breite × Höhe in Pixeln; die Mindestgröße des Controls).
#[derive(GodotClass)]
#[class(base=Control)]
pub struct HourglassIcon {
    /// Breite × Höhe des Symbols in Pixeln.
    #[export]
    icon_size: Vector2,
    base: Base<Control>,
}

#[godot_api]
impl IControl for HourglassIcon {
    fn init(base: Base<Control>) -> Self {
        Self { icon_size: Vector2::new(14.0, 20.0), base }
    }

    fn ready(&mut self) {
        let size = self.icon_size;
        self.base_mut().set_custom_minimum_size(size);
        self.base_mut().set_mouse_filter(MouseFilter::IGNORE);
    }

    fn draw(&mut self) {
        let Vector2 { x: w, y: h } = self.icon_size;
        let bar = (h * 0.12).max(2.0);
        let (top, bottom) = (bar, h - bar);
        let mid = Vector2::new(w * 0.5, h * 0.5);
        let inset = w * 0.12;
        let (l, r) = (inset, w - inset);
        let pts = |p: &[Vector2]| PackedVector2Array::from(p);

        // Glaskolben (oben und unten)
        self.base_mut().draw_colored_polygon(&pts(&[Vector2::new(l, top), Vector2::new(r, top), mid]), GLASS);
        self.base_mut().draw_colored_polygon(&pts(&[mid, Vector2::new(r, bottom), Vector2::new(l, bottom)]), GLASS);
        // Sandhaufen unten und dünner Strahl durch die Engstelle
        let heap = h * 0.2;
        self.base_mut().draw_colored_polygon(
            &pts(&[Vector2::new(l + 1.0, bottom), Vector2::new(r - 1.0, bottom), Vector2::new(w * 0.5, bottom - heap)]),
            SAND,
        );
        self.base_mut().draw_line_ex(mid, Vector2::new(w * 0.5, bottom - heap), SAND).width(1.0).done();
        // Konturen der Kolben
        for (a, b) in [
            (Vector2::new(l, top), mid),
            (Vector2::new(r, top), mid),
            (mid, Vector2::new(l, bottom)),
            (mid, Vector2::new(r, bottom)),
        ] {
            self.base_mut().draw_line_ex(a, b, FRAME).width(1.5).antialiased(true).done();
        }
        // Querbalken
        self.base_mut().draw_rect(Rect2::new(Vector2::ZERO, Vector2::new(w, bar)), FRAME);
        self.base_mut().draw_rect(Rect2::new(Vector2::new(0.0, h - bar), Vector2::new(w, bar)), FRAME);
    }
}
