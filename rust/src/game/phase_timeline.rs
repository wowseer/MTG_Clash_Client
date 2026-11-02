//! PhaseTimeline: Zeitstrahl der Phasen für das HUD (unten rechts über dem Button „Nächste Phase“).
//!
//! Waagerechte Linie mit drei Stationen:
//! 1. **Rundenbeginn** – ein Kreis für Enttappen, Auffrischung, Ziehen und Hauptphase 1, der mit
//!    jedem dieser Schritte größer wird (Hauptphase 1 = großer Kreis).
//! 2. **Kampf** – ein mittlerer Punkt; seine drei Schritte (Angreifer, Blocker, Schaden) sind
//!    eingeklappt und klappen erst auf, wenn der Kampf beginnt (danach wieder zu).
//! 3. **Rundenende** – ein Kreis für Hauptphase 2 und Ende: groß in Hauptphase 2, schrumpft
//!    im Endschritt auf einen kleinen Kreis.
//!
//! Die aktuelle Station leuchtet (weiß mit goldenem Rand, pulsierender Schein); erledigte Strecke
//! und Stationen sind golden, kommende hohl. Größe, Aufklappen und Position gleiten weich.
//! Über der Linie steht der Name der aktuellen Phase, darunter Runde und wer am Zug ist.

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

/// Phasen-Indizes des Rundenbeginns (Enttappen, Auffrischung, Ziehen, Hauptphase 1) – ein wachsender Kreis.
const OPENING: RangeInclusive<usize> = 0..=3;
/// Phasen-Indizes der Kampfschritte (Angreifer, Blocker, Schaden) – eingeklappte Unterpunkte.
const COMBAT: RangeInclusive<usize> = 4..=6;
/// Phasen-Indizes des Rundenendes (Hauptphase 2, Ende) – ein schrumpfender Kreis.
const CLOSING: RangeInclusive<usize> = 7..=8;
const END: usize = 8;

const WIDTH: f32 = 270.0;
const HEIGHT: f32 = 78.0;
const LINE_Y: f32 = 40.0;
/// Radien: Rundenbeginn wächst von OPENING_R[0] bis BIG_R; Kampf mittel, Unterpunkte klein;
/// Rundenende groß (BIG_R) und im Endschritt klein (END_R).
const OPENING_R: [f32; 4] = [5.0, 7.0, 9.0, BIG_R];
const BIG_R: f32 = 12.0;
const COMBAT_R: f32 = 7.0;
const SUB_R: f32 = 4.0;
const END_R: f32 = 5.5;
/// Zusätzlicher Abstand (in Stationseinheiten), den die aufgeklappten Kampfschritte einnehmen.
const COMBAT_SPREAD: f32 = 1.5;

/// Zeitstrahl der Phasen (HUD, unten rechts über dem Hauptbutton): ein mit Enttappen, Auffrischung,
/// Ziehen und Hauptphase 1 wachsender Kreis, ein Punkt „Kampf“, der seine drei Schritte (Phasen-Index
/// 4–6) erst während des Kampfs aufklappt, und ein Kreis für Hauptphase 2 und Ende, der im
/// Endschritt kleiner wird. Die aktuelle Station leuchtet; erledigte sind golden, kommende hohl. Darüber steht der Name
/// der aktuellen Phase, darunter Runde und Seite am Zug. Wird vom [TurnHud] jedes Frame über
/// [method set_state] gefüttert.
#[derive(GodotClass)]
#[class(base=Control)]
pub struct PhaseTimeline {
    titles: Vec<String>,
    current: i64,
    turn_text: String,
    /// Animierte Werte (None = noch nicht gesetzt): x der Markierung, Radius des
    /// Rundenbeginn- und Rundenende-Kreises, Aufklappgrad des Kampfs (0 = zu, 1 = offen).
    marker_x: Option<f32>,
    opening_r: Option<f32>,
    closing_r: Option<f32>,
    expand: f32,
    time: f32,
    base: Base<Control>,
}

/// Zustand einer Station relativ zur aktuellen Phase.
#[derive(Clone, Copy, PartialEq)]
enum State {
    Done,
    Current,
    Upcoming,
}

/// Ein Punkt auf dem Zeitstrahl.
struct Dot {
    x: f32,
    r: f32,
    alpha: f32,
    state: State,
}

#[godot_api]
impl IControl for PhaseTimeline {
    fn init(base: Base<Control>) -> Self {
        Self {
            titles: Vec::new(),
            current: -1,
            turn_text: String::new(),
            marker_x: None,
            opening_r: None,
            closing_r: None,
            expand: 0.0,
            time: 0.0,
            base,
        }
    }

    fn ready(&mut self) {
        self.base_mut().set_custom_minimum_size(Vector2::new(WIDTH, HEIGHT));
        self.base_mut().set_mouse_filter(MouseFilter::IGNORE);
    }

    fn process(&mut self, delta: f64) {
        let k = 1.0 - (-8.0 * delta as f32).exp();
        self.time += delta as f32;
        // Neue Runde: Größe springt zurück statt zu schrumpfen
        let target_r = self.opening_target_r();
        self.opening_r = Some(match self.opening_r {
            Some(r) if target_r >= r - 0.1 => r + (target_r - r) * k,
            _ => target_r,
        });
        // Rundenende: schrumpft im Endschritt, springt bei neuer Runde zurück auf groß
        let target_c = self.closing_target_r();
        self.closing_r = Some(match self.closing_r {
            Some(r) if target_c <= r + 0.1 => r + (target_c - r) * k,
            _ => target_c,
        });
        let target_e = if self.combat_step().is_some() { 1.0 } else { 0.0 };
        self.expand += (target_e - self.expand) * k;

        let target = self.target_x(&self.dots());
        self.marker_x = Some(match self.marker_x {
            // Neue Runde (zurück zum Anfang): springen statt rückwärts gleiten
            Some(x) if target >= x - 0.5 => x + (target - x) * k,
            _ => target,
        });
        self.base_mut().queue_redraw();
    }

    fn draw(&mut self) {
        let dots = self.dots();
        let marker = self.marker_x.unwrap_or(dots[0].x);
        let (x0, x1) = (dots[0].x, dots[dots.len() - 1].x);

        // Linie: gesamt gedimmt, erledigte Strecke golden
        self.line(x0, x1, LINE, 2.0);
        if self.current >= 0 {
            self.line(x0, marker, GOLD, 3.0);
        }

        let pulse = 0.5 + 0.5 * (self.time * 3.0).sin();
        for d in &dots {
            if d.alpha < 0.02 {
                continue;
            }
            let c = Vector2::new(d.x, LINE_Y);
            let fade = |col: Color| Color::from_rgba(col.r, col.g, col.b, col.a * d.alpha);
            match d.state {
                State::Done => self.base_mut().draw_circle(c, d.r, fade(GOLD)),
                State::Upcoming => {
                    self.base_mut().draw_circle(c, d.r, fade(HOLLOW));
                    self.base_mut().draw_arc_ex(c, d.r, 0.0, TAU, 28, fade(DIM)).width(1.5).antialiased(true).done();
                }
                State::Current => {
                    let glow = Color::from_rgba(GOLD.r, GOLD.g, GOLD.b, (0.18 + 0.15 * pulse) * d.alpha);
                    self.base_mut().draw_circle(c, d.r + 4.0 + 2.0 * pulse, glow);
                    self.base_mut().draw_circle(c, d.r, fade(Color::WHITE));
                    self.base_mut().draw_arc_ex(c, d.r, 0.0, TAU, 28, fade(GOLD)).width(2.5).antialiased(true).done();
                }
            }
        }

        // Name der aktuellen Phase über der Linie (im Rahmen gehalten)
        let title = self.current_title();
        // Über der Markierung, aber anhand der echten Textbreite im Rahmen gehalten
        let half = self.base().get_theme_default_font().map_or(100.0, |f| {
            f.get_string_size_ex(&title).font_size(16).done().x * 0.5 + 4.0
        });
        let tx = marker.clamp(half.min(WIDTH * 0.5), (WIDTH - half).max(WIDTH * 0.5));
        self.text(&title, Vector2::new(tx, LINE_Y - BIG_R - 8.0), 16, GOLD, HorizontalAlignment::CENTER);
        let turn = self.turn_text.clone();
        self.text(&turn, Vector2::new(WIDTH * 0.5, LINE_Y + BIG_R + 20.0), 13, TEXT, HorizontalAlignment::CENTER);
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
    fn current_index(&self) -> Option<usize> {
        usize::try_from(self.current).ok()
    }

    /// Index des aktuellen Kampfschritts (0–2) oder None außerhalb des Kampfs.
    fn combat_step(&self) -> Option<usize> {
        self.current_index().filter(|i| COMBAT.contains(i)).map(|i| i - COMBAT.start())
    }

    /// Zielradius des Rundenbeginn-Kreises: wächst mit Enttappen → Hauptphase 1, danach groß.
    fn opening_target_r(&self) -> f32 {
        match self.current_index() {
            None => OPENING_R[0],
            Some(i) if OPENING.contains(&i) => OPENING_R[i],
            Some(_) => BIG_R,
        }
    }

    /// Zielradius des Rundenende-Kreises: groß, im Endschritt klein.
    fn closing_target_r(&self) -> f32 {
        if self.current_index() == Some(END) { END_R } else { BIG_R }
    }

    /// Zustand einer Station, die die Phasen `range` umfasst.
    fn state(&self, range: RangeInclusive<usize>) -> State {
        match self.current_index() {
            None => State::Upcoming,
            Some(i) if range.contains(&i) => State::Current,
            Some(i) if i > *range.end() => State::Done,
            Some(_) => State::Upcoming,
        }
    }

    /// Stationen mit (animierter) Position und Größe. Einheiten: Rundenbeginn 0, Kampf 1,
    /// Rundenende 2 – aufgeklappte Kampfschritte schieben das Rundenende um bis zu
    /// [COMBAT_SPREAD] nach rechts.
    fn dots(&self) -> Vec<Dot> {
        let e = self.expand;
        let total = 2.0 + COMBAT_SPREAD * e;
        let (left, right) = (BIG_R + 4.0, WIDTH - BIG_R - 4.0);
        let x = |u: f32| left + (right - left) * u / total;
        let opening_r = self.opening_r.unwrap_or_else(|| self.opening_target_r());

        let mut dots = vec![Dot { x: x(0.0), r: opening_r, alpha: 1.0, state: self.state(OPENING) }];
        // Kampf: Gruppenpunkt; ist er aufgeklappt, übernimmt der aktuelle Schritt die Hervorhebung
        let mut combat_state = self.state(COMBAT);
        if combat_state == State::Current && e > 0.5 {
            combat_state = State::Done;
        }
        dots.push(Dot { x: x(1.0), r: COMBAT_R, alpha: 1.0, state: combat_state });
        let step = self.combat_step();
        for k in 0..3 {
            let state = match step {
                Some(s) if s == k => State::Current,
                Some(s) if s > k => State::Done,
                _ => State::Upcoming,
            };
            let u = 1.0 + (k as f32 + 1.0) * 0.5 * e;
            dots.push(Dot { x: x(u), r: SUB_R, alpha: e, state });
        }
        let closing_r = self.closing_r.unwrap_or_else(|| self.closing_target_r());
        dots.push(Dot { x: x(2.0 + COMBAT_SPREAD * e), r: closing_r, alpha: 1.0, state: self.state(CLOSING) });
        dots
    }

    /// x-Position der aktuellen Station (vor Spielbeginn: erste Station; nach dem Ende: letzte).
    fn target_x(&self, dots: &[Dot]) -> f32 {
        let current = dots.iter().filter(|d| d.state == State::Current).max_by(|a, b| a.alpha.total_cmp(&b.alpha));
        match (current, self.current_index()) {
            (Some(d), _) => d.x,
            (None, Some(_)) => dots[dots.len() - 1].x,
            (None, None) => dots[0].x,
        }
    }

    fn current_title(&self) -> String {
        let Some(i) = self.current_index() else {
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
