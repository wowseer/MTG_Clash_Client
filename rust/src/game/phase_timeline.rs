//! PhaseTimeline: Zeitstrahl der Phasen für das HUD.
//!
//! Waagerechte Linie mit Punkten für die Phasen und kleineren Unterpunkten für die
//! Kampfschritte (hinter einem eigenen Punkt „Kampf“). Eine leuchtende Markierung gleitet
//! zur aktuellen Phase; erledigte Strecke und Punkte sind golden, kommende hohl. Über der
//! Markierung steht der Name der aktuellen Phase, darunter Runde und wer am Zug ist.

use std::f32::consts::TAU;
use std::ops::RangeInclusive;

use godot::classes::control::MouseFilter;
use godot::classes::{Control, IControl};
use godot::global::HorizontalAlignment;
use godot::prelude::*;

const GOLD: Color = Color::from_rgb(0.95, 0.78, 0.4);
const TEXT: Color = Color::from_rgb(0.97, 0.94, 0.88);
const DIM: Color = Color::from_rgba(0.93, 0.87, 0.74, 0.55);
const LINE: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.22);
const HOLLOW: Color = Color::from_rgba(0.05, 0.06, 0.08, 0.9);

/// Phasen-Indizes der Kampfschritte (Angreifer, Blocker, Schaden) – als Unterpunkte gezeichnet.
const COMBAT: RangeInclusive<usize> = 4..=6;
const WIDTH: f32 = 330.0;
const HEIGHT: f32 = 64.0;
const LINE_Y: f32 = 34.0;
const MAIN_R: f32 = 6.0;
const SUB_R: f32 = 3.5;

/// Zeitstrahl der Phasen (HUD): Punkte für die Phasen, kleinere Unterpunkte für die Kampfschritte
/// (Phasen-Index 4–6) hinter einem Punkt „Kampf“. Eine leuchtende Markierung gleitet zur aktuellen
/// Phase; erledigte Punkte sind golden, kommende hohl. Darüber steht der Name der aktuellen Phase,
/// darunter Runde und Seite am Zug. Wird vom [TurnHud] jedes Frame über [method set_state] gefüttert.
#[derive(GodotClass)]
#[class(base=Control)]
pub struct PhaseTimeline {
    titles: Vec<String>,
    current: i64,
    turn_text: String,
    /// Aktuelle (animierte) x-Position der Markierung; None = noch nicht gesetzt.
    marker_x: Option<f32>,
    time: f32,
    base: Base<Control>,
}

/// Ein Punkt auf dem Zeitstrahl.
struct Dot {
    x: f32,
    sub: bool,
    /// Phasen-Index; None für den Gruppenpunkt „Kampf“.
    phase: Option<usize>,
}

#[godot_api]
impl IControl for PhaseTimeline {
    fn init(base: Base<Control>) -> Self {
        Self { titles: Vec::new(), current: -1, turn_text: String::new(), marker_x: None, time: 0.0, base }
    }

    fn ready(&mut self) {
        self.base_mut().set_custom_minimum_size(Vector2::new(WIDTH, HEIGHT));
        self.base_mut().set_mouse_filter(MouseFilter::IGNORE);
    }

    fn process(&mut self, delta: f64) {
        self.time += delta as f32;
        let dots = self.dots();
        let target = self.target_x(&dots);
        let x = match self.marker_x {
            // Neue Runde (zurück zum Anfang): springen statt rückwärts gleiten
            Some(x) if target >= x - 0.5 => x + (target - x) * (1.0 - (-8.0 * delta as f32).exp()),
            _ => target,
        };
        self.marker_x = Some(x);
        self.base_mut().queue_redraw();
    }

    fn draw(&mut self) {
        let dots = self.dots();
        if dots.is_empty() {
            return;
        }
        let marker = self.marker_x.unwrap_or(dots[0].x);
        let (x0, x1) = (dots[0].x, dots[dots.len() - 1].x);

        // Linie: gesamt gedimmt, erledigte Strecke golden
        self.line(x0, x1, LINE, 2.0);
        self.line(x0, marker, GOLD, 3.0);

        for d in &dots {
            let r = if d.sub { SUB_R } else { MAIN_R };
            let c = Vector2::new(d.x, LINE_Y);
            if d.x <= marker + 0.5 {
                self.base_mut().draw_circle(c, r, GOLD);
            } else {
                self.base_mut().draw_circle(c, r, HOLLOW);
                self.base_mut().draw_arc_ex(c, r, 0.0, TAU, 20, DIM).width(1.5).antialiased(true).done();
            }
        }

        // Markierung mit pulsierendem Schein
        if self.current >= 0 {
            let c = Vector2::new(marker, LINE_Y);
            let pulse = 0.5 + 0.5 * (self.time * 3.0).sin();
            self.base_mut().draw_circle(c, 11.0 + 2.0 * pulse, Color::from_rgba(GOLD.r, GOLD.g, GOLD.b, 0.18 + 0.15 * pulse));
            self.base_mut().draw_circle(c, 7.5, Color::WHITE);
            self.base_mut().draw_arc_ex(c, 7.5, 0.0, TAU, 24, GOLD).width(2.5).antialiased(true).done();
        }

        // Name der aktuellen Phase über der Markierung (im Rahmen gehalten)
        let title = self.current_title();
        let tx = marker.clamp(70.0, WIDTH - 70.0);
        self.text(&title, Vector2::new(tx, LINE_Y - 15.0), 16, GOLD, HorizontalAlignment::CENTER);
        let turn = self.turn_text.clone();
        self.text(&turn, Vector2::new(WIDTH * 0.5, LINE_Y + 26.0), 13, TEXT, HorizontalAlignment::CENTER);
    }
}

#[godot_api]
impl PhaseTimeline {
    /// Setzt Phasennamen (in Spielreihenfolge), Index der aktuellen Phase (-1 = vor Spielbeginn)
    /// und den Text unter dem Zeitstrahl (z. B. „Runde 3 · Spieler“).
    #[func]
    pub fn set_state(&mut self, titles: PackedStringArray, current: i64, turn_text: GString) {
        if titles.len() != self.titles.len() || titles.as_slice().iter().zip(&self.titles).any(|(a, b)| a.to_string() != *b) {
            self.titles = titles.as_slice().iter().map(|t| t.to_string()).collect();
        }
        self.current = current;
        self.turn_text = turn_text.to_string();
    }
}

impl PhaseTimeline {
    /// Punkte mit x-Position: Hauptphasen im Abstand 1, Kampf-Unterpunkte im Abstand 0,5.
    fn dots(&self) -> Vec<Dot> {
        let mut units = Vec::new();
        let mut u = 0.0f32;
        for i in 0..self.titles.len() {
            let sub = COMBAT.contains(&i);
            if i > 0 {
                let first_sub = i == *COMBAT.start();
                u += if sub && !first_sub { 0.5 } else if COMBAT.contains(&(i - 1)) { 0.75 } else { 1.0 };
            }
            if i == *COMBAT.start() {
                // Gruppenpunkt „Kampf“ vor den Unterpunkten
                units.push((u, false, None));
                u += 0.5;
            }
            units.push((u, sub, Some(i)));
        }
        let total = units.last().map_or(1.0, |d| d.0).max(1.0);
        let (left, right) = (14.0, WIDTH - 14.0);
        units
            .into_iter()
            .map(|(u, sub, phase)| Dot { x: left + (right - left) * u / total, sub, phase })
            .collect()
    }

    fn target_x(&self, dots: &[Dot]) -> f32 {
        let cur = usize::try_from(self.current).ok();
        dots.iter()
            .find(|d| d.phase.is_some() && d.phase == cur)
            .or(dots.first())
            .map_or(0.0, |d| d.x)
    }

    fn current_title(&self) -> String {
        let Ok(i) = usize::try_from(self.current) else {
            return tr!("Austeilen …", "Dealing …").to_string();
        };
        let title = self.titles.get(i).cloned().unwrap_or_default();
        if COMBAT.contains(&i) {
            trf!("Kampf · {title}", "Combat · {title}")
        } else {
            title
        }
    }

    fn line(&mut self, from: f32, to: f32, color: Color, width: f32) {
        if to <= from {
            return;
        }
        self.base_mut()
            .draw_line_ex(Vector2::new(from, LINE_Y), Vector2::new(to, LINE_Y), color)
            .width(width)
            .antialiased(true)
            .done();
    }

    /// Text mit dunkler Kontur; `pos` = Grundlinie, bei CENTER mittig.
    fn text(&mut self, text: &str, pos: Vector2, size: i32, color: Color, align: HorizontalAlignment) {
        let Some(font) = self.base().get_theme_default_font() else { return };
        let width = WIDTH;
        let p = pos - Vector2::new(width * 0.5, 0.0);
        self.base_mut()
            .draw_string_outline_ex(&font, p, text)
            .alignment(align)
            .width(width)
            .font_size(size)
            .size(5)
            .modulate(Color::from_rgba(0.0, 0.0, 0.0, 0.85))
            .done();
        self.base_mut()
            .draw_string_ex(&font, p, text)
            .alignment(align)
            .width(width)
            .font_size(size)
            .modulate(color)
            .done();
    }
}
