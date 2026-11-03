//! SeatBadge: rautenförmiges Spieler-Medaillon für das HUD.
//!
//! Goldgerahmte Raute mit Wappen (Ring, sechszackiger Stern, Anfangsbuchstabe) in der
//! Farbe der Seite, darunter ein roter Edelstein mit dem Lebenspunktestand. Wer am Zug
//! ist, bekommt einen pulsierenden Schein; Lebensänderungen blitzen kurz auf
//! (rot = Schaden, grün = Heilung). Ist der Spieler ein gültiges Ziel, pulsiert ein goldener
//! Ring und das Medaillon ist anklickbar (Signal `clicked`). Alles wird per `draw()` gezeichnet,
//! ohne Bilddateien.

use std::f32::consts::{PI, TAU};

use godot::classes::control::MouseFilter;
use godot::classes::{Control, IControl, InputEvent, InputEventMouseButton};
use godot::global::MouseButton;
use godot::global::HorizontalAlignment;
use godot::prelude::*;

const GOLD: Color = Color::from_rgb(0.9, 0.72, 0.36);
const FRAME_BG: Color = Color::from_rgba(0.05, 0.05, 0.07, 0.95);
const GEM: Color = Color::from_rgb(0.62, 0.07, 0.1);
const TEXT: Color = Color::from_rgb(0.97, 0.94, 0.88);

/// Halbe Diagonale der großen Raute und des Lebens-Edelsteins (Pixel).
const HALF: f32 = 36.0;
const GEM_HALF: f32 = 16.0;

/// Rautenförmiges Spieler-Medaillon (HUD): Wappen in der Seitenfarbe [member accent], darunter ein
/// roter Edelstein mit den Lebenspunkten [member life]. Ist [member active] gesetzt (Spieler am Zug),
/// pulsiert ein Schein um die Raute; Lebensänderungen blitzen kurz rot (Schaden) bzw. grün (Heilung).
/// Wird vom [TurnHud] jedes Frame über [method set_state] aktualisiert.
#[derive(GodotClass)]
#[class(base=Control)]
pub struct SeatBadge {
    /// Name der Seite; sein Anfangsbuchstabe steht im Wappen.
    #[var]
    seat_name: GString,
    /// Angezeigte Lebenspunkte.
    #[var]
    life: i64,
    /// Ist diese Seite am Zug? (pulsierender Schein)
    #[var]
    active: bool,
    /// Farbe des Wappens (Seitenfarbe, z. B. blau für den Spieler, rot für den Gegner).
    #[export]
    accent: Color,
    /// Ist dieser Spieler gerade ein gültiges Ziel? Dann pulsiert ein goldener Ring und ein Klick
    /// löst [signal clicked] aus (sonst lässt das Medaillon Mausklicks durch).
    #[var(get = is_targetable, set = set_targetable)]
    targetable: bool,
    /// Ist dieser Spieler als Ziel gewählt? (heller, ruhiger Ring)
    #[var(pub)]
    target_selected: bool,

    time: f32,
    /// Restdauer des Aufblitzens nach einer Lebensänderung (Sekunden) und dessen Farbe.
    flash: f32,
    flash_color: Color,
    base: Base<Control>,
}

#[godot_api]
impl IControl for SeatBadge {
    fn init(base: Base<Control>) -> Self {
        Self {
            seat_name: GString::new(),
            life: 0,
            active: false,
            accent: Color::from_rgb(0.25, 0.55, 0.95),
            targetable: false,
            target_selected: false,
            time: 0.0,
            flash: 0.0,
            flash_color: Color::WHITE,
            base,
        }
    }

    fn ready(&mut self) {
        let size = Vector2::new(HALF * 2.0 + 24.0, HALF * 2.0 + GEM_HALF + 14.0);
        self.base_mut().set_custom_minimum_size(size);
        self.base_mut().set_mouse_filter(MouseFilter::IGNORE);
    }

    fn gui_input(&mut self, event: Gd<InputEvent>) {
        let Ok(mb) = event.try_cast::<InputEventMouseButton>() else { return };
        if self.targetable && mb.is_pressed() && mb.get_button_index() == MouseButton::LEFT {
            self.signals().clicked().emit();
            self.base_mut().accept_event();
        }
    }

    fn process(&mut self, delta: f64) {
        self.time += delta as f32;
        self.flash = (self.flash - delta as f32).max(0.0);
        self.base_mut().queue_redraw();
    }

    fn draw(&mut self) {
        let size = self.base().get_size();
        let c = Vector2::new(size.x * 0.5, 6.0 + HALF);
        let accent = self.accent;

        // Zielwahl: goldener Ring (pulsierend = wählbar, ruhig und hell = gewählt)
        if self.targetable || self.target_selected {
            let pulse = 0.5 + 0.5 * (self.time * 4.0).sin();
            let (alpha, grow) = if self.target_selected { (0.95, 9.0) } else { (0.45 + 0.4 * pulse, 7.0 + 3.0 * pulse) };
            let ring = Color::from_rgba(1.0, 0.82, 0.35, alpha);
            self.outline(diamond(c, HALF + grow), ring, 4.0);
        }

        // Schein, wenn am Zug
        if self.active {
            let pulse = 0.5 + 0.5 * (self.time * 3.0).sin();
            let glow = Color::from_rgba(accent.r, accent.g, accent.b, 0.25 + 0.3 * pulse);
            self.fill(diamond(c, HALF + 6.0 + 2.0 * pulse), glow);
        }

        // Rahmen und Wappenfeld
        self.fill(diamond(c, HALF), FRAME_BG);
        self.outline(diamond(c, HALF), GOLD, 3.0);
        let field = Color::from_rgba(accent.r * 0.35, accent.g * 0.35, accent.b * 0.35, 1.0);
        self.fill(diamond(c, HALF - 6.0), field);
        self.outline(diamond(c, HALF - 6.0), Color::from_rgba(GOLD.r, GOLD.g, GOLD.b, 0.5), 1.5);

        // Wappen: Ring, sechszackiger Stern, Anfangsbuchstabe
        let light = accent.lerp(Color::WHITE, 0.35);
        self.base_mut().draw_arc_ex(c, 16.5, 0.0, TAU, 40, light).width(2.0).antialiased(true).done();
        for start in [-PI * 0.5, PI * 0.5] {
            let tri: Vec<Vector2> = (0..=3)
                .map(|i| {
                    let a = start + i as f32 * TAU / 3.0;
                    c + Vector2::new(a.cos(), a.sin()) * 15.0
                })
                .collect();
            self.base_mut()
                .draw_polyline_ex(&PackedVector2Array::from(tri), Color::from_rgba(light.r, light.g, light.b, 0.6))
                .width(1.5)
                .antialiased(true)
                .done();
        }
        let initial: String = self.seat_name.to_string().chars().next().map(|ch| ch.to_uppercase().collect()).unwrap_or_default();
        self.text(&initial, c + Vector2::new(0.0, 7.0), 20, TEXT);

        // Lebens-Edelstein an der unteren Spitze; blitzt bei Änderungen auf und wird kurz größer
        let t = self.flash / 0.6;
        let gc = c + Vector2::new(0.0, HALF);
        let gh = GEM_HALF + 4.0 * t;
        let gem = GEM.lerp(self.flash_color, (t * 0.8) as f64);
        self.fill(diamond(gc, gh), gem);
        self.fill(diamond(gc + Vector2::new(0.0, -gh * 0.35), gh * 0.45), Color::from_rgba(1.0, 1.0, 1.0, 0.12));
        self.outline(diamond(gc, gh), GOLD, 2.0);
        let life = self.life.to_string();
        let fs = if life.len() > 2 { 15 } else { 18 };
        self.text(&life, gc + Vector2::new(0.0, fs as f32 * 0.36), fs, TEXT);
    }
}

#[godot_api]
impl SeatBadge {
    /// Das Medaillon wurde angeklickt, während [member targetable] gesetzt war.
    #[signal]
    fn clicked();

    /// Getter für [member targetable].
    #[func]
    pub fn is_targetable(&self) -> bool {
        self.targetable
    }

    /// Setter für [member targetable]: nur ein wählbares Medaillon fängt Mausklicks ab.
    #[func]
    pub fn set_targetable(&mut self, value: bool) {
        self.targetable = value;
        self.base_mut().set_mouse_filter(if value { MouseFilter::STOP } else { MouseFilter::IGNORE });
    }

    /// Setzt Name, Lebenspunkte und „am Zug“ auf einmal. Weicht `life` vom bisherigen Wert ab,
    /// blitzt der Edelstein kurz auf (rot bei Schaden, grün bei Heilung).
    #[func]
    pub fn set_state(&mut self, seat_name: GString, life: i64, active: bool) {
        if life != self.life && !self.seat_name.is_empty() {
            self.flash = 0.6;
            self.flash_color = if life < self.life {
                Color::from_rgb(1.0, 0.25, 0.2)
            } else {
                Color::from_rgb(0.3, 0.9, 0.4)
            };
        }
        self.seat_name = seat_name;
        self.life = life;
        self.active = active;
    }
}

impl SeatBadge {
    fn fill(&mut self, points: Vec<Vector2>, color: Color) {
        self.base_mut().draw_colored_polygon(&PackedVector2Array::from(points), color);
    }

    fn outline(&mut self, mut points: Vec<Vector2>, color: Color, width: f32) {
        points.push(points[0]);
        self.base_mut()
            .draw_polyline_ex(&PackedVector2Array::from(points), color)
            .width(width)
            .antialiased(true)
            .done();
    }

    /// Zentrierter Text mit dunkler Kontur; `pos` = Grundlinie in der Mitte.
    fn text(&mut self, text: &str, pos: Vector2, size: i32, color: Color) {
        let Some(font) = self.base().get_theme_default_font() else {
            return;
        };
        let width = 120.0;
        let p = pos - Vector2::new(width * 0.5, 0.0);
        self.base_mut()
            .draw_string_outline_ex(&font, p, text)
            .alignment(HorizontalAlignment::CENTER)
            .width(width)
            .font_size(size)
            .size(5)
            .modulate(Color::from_rgba(0.0, 0.0, 0.0, 0.85))
            .done();
        self.base_mut()
            .draw_string_ex(&font, p, text)
            .alignment(HorizontalAlignment::CENTER)
            .width(width)
            .font_size(size)
            .modulate(color)
            .done();
    }
}

/// Eckpunkte einer auf der Spitze stehenden Raute (oben, rechts, unten, links).
fn diamond(c: Vector2, half: f32) -> Vec<Vector2> {
    vec![
        c + Vector2::new(0.0, -half),
        c + Vector2::new(half, 0.0),
        c + Vector2::new(0.0, half),
        c + Vector2::new(-half, 0.0),
    ]
}
