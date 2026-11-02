//! ManaPoolView: Manapool als Reihe runder Symbole im HUD (unten am Brett, neben dem Medaillon).
//!
//! Je Manaart mit Mana im Pool ein Kreis in deren Farbe mit einem eigenen, schlicht gezeichneten
//! Symbol: Grün = Nadelbaum, Blau = Wassertropfen, Schwarz = Totenkopf, Weiß = Sonne,
//! Rot = Flamme, Farblos = Raute. Ab 2 Mana steht die Anzahl in einer kleinen Plakette unten
//! rechts. Kommt Mana hinzu (z. B. eine Ressource wird getappt), springt der Kreis kurz auf.
//! Alles per `draw()`, ohne Bilddateien.

use std::f32::consts::{PI, TAU};

use godot::classes::control::MouseFilter;
use godot::classes::{Control, IControl};
use godot::global::HorizontalAlignment;
use godot::prelude::*;

use super::mana::ManaType;

/// Radius eines Mana-Kreises und Abstand zwischen zwei Kreisen (Pixel).
const RADIUS: f32 = 15.0;
const GAP: f32 = 7.0;
const RIM: Color = Color::from_rgb(0.9, 0.72, 0.36);
/// Dauer des Aufspringens (Sekunden).
const POP_TIME: f32 = 0.35;

/// Manapool als Reihe runder Symbole (HUD). Wird vom [TurnHud] jedes Frame über
/// [method set_amounts] gefüttert; die Breite passt sich der Anzahl der Kreise an.
#[derive(GodotClass)]
#[class(base=Control)]
pub struct ManaPoolView {
    amounts: [u32; 6],
    /// Zeit seit dem letzten Zuwachs je Manaart (für das Aufspringen).
    pop: [f32; 6],
    base: Base<Control>,
}

#[godot_api]
impl IControl for ManaPoolView {
    fn init(base: Base<Control>) -> Self {
        Self { amounts: [0; 6], pop: [POP_TIME; 6], base }
    }

    fn ready(&mut self) {
        self.base_mut().set_mouse_filter(MouseFilter::IGNORE);
    }

    fn process(&mut self, delta: f64) {
        let mut animating = false;
        for p in self.pop.iter_mut() {
            if *p < POP_TIME {
                *p = (*p + delta as f32).min(POP_TIME);
                animating = true;
            }
        }
        if animating {
            self.base_mut().queue_redraw();
        }
    }

    fn draw(&mut self) {
        let shown: Vec<ManaType> = ManaType::ALL.into_iter().filter(|t| self.amounts[t.index()] > 0).collect();
        for (k, t) in shown.into_iter().enumerate() {
            let c = Vector2::new(RADIUS + 2.0 + k as f32 * (2.0 * RADIUS + GAP), RADIUS + 2.0);
            // Aufspringen: kurz größer, dann zurück (leichtes Überschwingen)
            let f = self.pop[t.index()] / POP_TIME;
            let scale = if f < 1.0 { 0.55 + 0.45 * f + 0.25 * (f * PI).sin() } else { 1.0 };
            self.draw_mana(t, c, RADIUS * scale);
            let n = self.amounts[t.index()];
            if n > 1 {
                self.draw_count(c + Vector2::new(RADIUS * 0.72, RADIUS * 0.72), n);
            }
        }
    }
}

#[godot_api]
impl ManaPoolView {
    /// Setzt die Mana-Mengen je Art (Reihenfolge Weiß, Blau, Schwarz, Rot, Grün, Farblos).
    /// Leeres Array = nichts anzeigen. Zuwachs lässt den Kreis aufspringen.
    #[func]
    pub fn set_amounts(&mut self, amounts: PackedInt64Array) {
        let mut next = [0u32; 6];
        for (i, a) in amounts.as_slice().iter().take(6).enumerate() {
            next[i] = (*a).max(0) as u32;
        }
        if next == self.amounts {
            return;
        }
        for i in 0..6 {
            if next[i] > self.amounts[i] {
                self.pop[i] = 0.0;
            }
        }
        self.amounts = next;
        let count = next.iter().filter(|a| **a > 0).count() as f32;
        let width = if count > 0.0 { count * 2.0 * RADIUS + (count - 1.0) * GAP + 4.0 } else { 0.0 };
        self.base_mut().set_custom_minimum_size(Vector2::new(width, 2.0 * RADIUS + 4.0));
        self.base_mut().queue_redraw();
    }
}

impl ManaPoolView {
    /// Kreis in der Manafarbe mit goldenem Rand und Symbol.
    fn draw_mana(&mut self, t: ManaType, c: Vector2, r: f32) {
        let fill = t.color().lerp(Color::WHITE, 0.15);
        let lum = 0.299 * fill.r + 0.587 * fill.g + 0.114 * fill.b;
        let ink = if lum < 0.45 { Color::from_rgb(0.93, 0.9, 0.85) } else { Color::from_rgb(0.12, 0.1, 0.08) };
        let mut b = self.base_mut();
        b.draw_circle(c + Vector2::new(0.0, 1.5), r + 1.5, Color::from_rgba(0.0, 0.0, 0.0, 0.45));
        b.draw_circle(c, r, fill);
        b.draw_arc_ex(c, r, 0.0, TAU, 32, RIM).width(2.0).antialiased(true).done();
        let s = r * 0.62;
        let w = (s * 0.18).max(1.2);
        let poly = |pts: &[Vector2]| PackedVector2Array::from(pts);
        match t {
            ManaType::Green => {
                // Nadelbaum: drei gestapelte Dreiecke und ein Stamm
                b.draw_rect(Rect2::new(c + Vector2::new(-s * 0.14, s * 0.45), Vector2::new(s * 0.28, s * 0.5)), ink);
                for (top, half, bottom) in [(-0.95f32, 0.45f32, -0.2f32), (-0.6, 0.62, 0.15), (-0.25, 0.8, 0.5)] {
                    b.draw_colored_polygon(
                        &poly(&[c + Vector2::new(0.0, top) * s, c + Vector2::new(half, bottom) * s, c + Vector2::new(-half, bottom) * s]),
                        ink,
                    );
                }
            }
            ManaType::Blue => {
                // Wassertropfen: Spitze oben, unten rund; kleiner Glanzpunkt
                let pts: Vec<Vector2> = (0..28)
                    .map(|i| {
                        let a = i as f32 / 28.0 * TAU;
                        c + Vector2::new(0.72 * a.sin() * (a * 0.5).sin(), -0.95 * a.cos()) * s + Vector2::new(0.0, s * 0.12)
                    })
                    .collect();
                b.draw_colored_polygon(&poly(&pts), ink);
                b.draw_circle(c + Vector2::new(-s * 0.2, s * 0.3), s * 0.12, fill);
            }
            ManaType::Black => {
                // Totenkopf: runder Schädel, Kiefer, zwei Augenhöhlen, Nasenloch, Zahnfugen
                b.draw_circle(c + Vector2::new(0.0, -s * 0.15), s * 0.72, ink);
                b.draw_rect(Rect2::new(c + Vector2::new(-s * 0.42, s * 0.2), Vector2::new(s * 0.84, s * 0.6)), ink);
                for dx in [-0.3f32, 0.3] {
                    b.draw_circle(c + Vector2::new(dx * s, -s * 0.12), s * 0.2, fill);
                }
                b.draw_colored_polygon(
                    &poly(&[c + Vector2::new(0.0, s * 0.12), c + Vector2::new(s * 0.1, s * 0.3), c + Vector2::new(-s * 0.1, s * 0.3)]),
                    fill,
                );
                for dx in [-0.14f32, 0.14] {
                    b.draw_line_ex(c + Vector2::new(dx * s, s * 0.5), c + Vector2::new(dx * s, s * 0.8), fill).width(w * 0.7).done();
                }
            }
            ManaType::White => {
                // Sonne: Scheibe mit zwölf Strahlen
                b.draw_circle(c, s * 0.45, ink);
                for k in 0..12 {
                    let a = k as f32 * TAU / 12.0;
                    let d = Vector2::new(a.cos(), a.sin());
                    let len = if k % 2 == 0 { 0.98 } else { 0.8 };
                    b.draw_line_ex(c + d * (s * 0.6), c + d * (s * len), ink).width(w).antialiased(true).done();
                }
            }
            ManaType::Red => {
                // Flamme: drei Zungen (mittlere am höchsten) auf gemeinsamem rundem Fuß
                let tongue = |x: f32, h: f32, width: f32| -> Vec<Vector2> {
                    (0..20)
                        .map(|i| {
                            let a = i as f32 / 20.0 * TAU;
                            let p = Vector2::new(width * a.sin() * (a * 0.5).sin(), -h * a.cos());
                            c + Vector2::new(x, 0.35) * s + Vector2::new(p.x, if p.y < 0.0 { p.y } else { p.y * 0.45 }) * s
                        })
                        .collect()
                };
                b.draw_colored_polygon(&poly(&tongue(-0.32, 0.75, 0.4)), ink);
                b.draw_colored_polygon(&poly(&tongue(0.32, 0.75, 0.4)), ink);
                b.draw_colored_polygon(&poly(&tongue(0.0, 1.25, 0.55)), ink);
                b.draw_colored_polygon(&poly(&tongue(0.0, 0.55, 0.25)), fill);
            }
            ManaType::Colorless => {
                // Raute mit kleiner Raute innen
                let d = |k: f32| {
                    poly(&[c + Vector2::new(0.0, -k) * s, c + Vector2::new(k * 0.75, 0.0) * s, c + Vector2::new(0.0, k) * s, c + Vector2::new(-k * 0.75, 0.0) * s])
                };
                b.draw_colored_polygon(&d(0.95), ink);
                b.draw_colored_polygon(&d(0.4), fill);
            }
        }
    }

    /// Kleine dunkle Plakette mit der Anzahl.
    fn draw_count(&mut self, c: Vector2, n: u32) {
        let Some(font) = self.base().get_theme_default_font() else { return };
        let r = 8.0;
        self.base_mut().draw_circle(c, r, Color::from_rgba(0.05, 0.05, 0.07, 0.95));
        self.base_mut().draw_arc_ex(c, r, 0.0, TAU, 20, RIM).width(1.2).antialiased(true).done();
        let text = n.to_string();
        let w = 2.0 * r;
        self.base_mut()
            .draw_string_ex(&font, c + Vector2::new(-r, 4.0), &text)
            .alignment(HorizontalAlignment::CENTER)
            .width(w)
            .font_size(11)
            .modulate(Color::WHITE)
            .done();
    }
}
