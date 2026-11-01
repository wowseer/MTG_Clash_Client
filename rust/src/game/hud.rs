//! TurnHud: Bildschirm-Overlay für den Rundenablauf.
//!
//! - oben rechts: Phasenliste (aktuelle Phase hervorgehoben)
//! - oben links / unten links: Leben, Ressourcen und Mana von Gegner / Spieler
//! - unten rechts: Hinweis, Protokoll und Button „Nächste Phase“ (auch Leertaste)
//!
//! Das HUD liest den Zustand jedes Frame vom TurnManager (keine Signal-Kopplung nötig).
//! Alle Flächen lassen Mausklicks durch, nur der Button fängt sie ab.

use godot::classes::control::{GrowDirection, MouseFilter};
use godot::classes::text_server::AutowrapMode;
use godot::classes::{
    Button, CanvasLayer, RichTextLabel, Control, ICanvasLayer, InputEvent, InputEventKey, Label, PanelContainer,
    StyleBoxFlat, VBoxContainer,
};
use godot::builtin::Side;
use godot::global::Key;
use godot::prelude::*;

use super::turn_manager::TurnManager;

const GOLD: Color = Color::from_rgb(0.95, 0.78, 0.4);
const PARCHMENT: Color = Color::from_rgb(0.93, 0.87, 0.74);
const DIM: Color = Color::from_rgba(0.93, 0.87, 0.74, 0.45);

/// Bildschirm-Overlay für den Rundenablauf: Phasenliste, Hinweis und Protokoll (oben rechts),
/// Leben, Ressourcen und Mana beider Spieler (links), Button „Nächste Phase“ / „Passen“
/// (unten rechts, auch Leertaste) und das Auswahlfenster für Heldenfähigkeiten.
/// Liest den Zustand jedes Frame vom [TurnManager].
#[derive(GodotClass)]
#[class(base=CanvasLayer)]
pub struct TurnHud {
    /// Der [TurnManager], dessen Zustand angezeigt wird.
    #[export]
    turn_manager: Option<Gd<TurnManager>>,

    phase_labels: Vec<Gd<Label>>,
    opponent_info: Option<Gd<RichTextLabel>>,
    player_info: Option<Gd<RichTextLabel>>,
    turn_label: Option<Gd<Label>>,
    hint_label: Option<Gd<Label>>,
    log_label: Option<Gd<Label>>,
    button: Option<Gd<Button>>,
    hero_panel: Option<Gd<PanelContainer>>,
    hero_title: Option<Gd<Label>>,
    hero_buttons: Vec<Gd<Button>>,

    base: Base<CanvasLayer>,
}

#[godot_api]
impl ICanvasLayer for TurnHud {
    fn init(base: Base<CanvasLayer>) -> Self {
        Self {
            turn_manager: None,
            phase_labels: Vec::new(),
            opponent_info: None,
            player_info: None,
            turn_label: None,
            hint_label: None,
            log_label: None,
            button: None,
            hero_panel: None,
            hero_title: None,
            hero_buttons: Vec::new(),
            base,
        }
    }

    fn ready(&mut self) {
        self.build();
    }

    fn process(&mut self, _delta: f64) {
        self.refresh();
    }

    fn unhandled_input(&mut self, event: Gd<InputEvent>) {
        if let Ok(key) = event.try_cast::<InputEventKey>() {
            if key.is_pressed() && !key.is_echo() && key.get_keycode() == Key::SPACE {
                self.on_next_pressed();
            }
        }
    }
}

#[godot_api]
impl TurnHud {
    /// Knopf der ersten Heldenfähigkeit im Auswahlfenster.
    #[func]
    fn on_hero_ability_0(&mut self) {
        self.activate_hero(0);
    }

    /// Knopf der zweiten Heldenfähigkeit im Auswahlfenster.
    #[func]
    fn on_hero_ability_1(&mut self) {
        self.activate_hero(1);
    }

    /// Knopf „Abbrechen“ im Auswahlfenster für Heldenfähigkeiten.
    #[func]
    fn on_hero_cancel(&mut self) {
        if let Some(tm) = self.turn_manager.as_mut() {
            tm.bind_mut().cancel_hero_choice();
        }
    }

    /// Button „Nächste Phase“ / „Passen“ bzw. Leertaste: ruft [method TurnManager.next_phase] auf.
    #[func]
    fn on_next_pressed(&mut self) {
        if let Some(tm) = self.turn_manager.as_mut() {
            tm.bind_mut().next_phase();
        }
    }
}

impl TurnHud {
    fn activate_hero(&mut self, index: i64) {
        if let Some(tm) = self.turn_manager.as_mut() {
            tm.bind_mut().activate_hero_ability(index);
        }
    }

    /// Auswahlfenster für Heldenfähigkeiten (unten Mitte, über der Hand).
    fn build_hero_panel(&mut self, root: &mut Gd<Control>) {
        let mut col = VBoxContainer::new_alloc();
        col.add_theme_constant_override("separation", 6);
        col.set_custom_minimum_size(Vector2::new(420.0, 0.0));
        let title = label(17, GOLD);
        col.add_child(&title);
        for (i, method) in ["on_hero_ability_0", "on_hero_ability_1", "on_hero_cancel"].into_iter().enumerate() {
            let mut b = styled_button(if i == 2 { 15 } else { 16 });
            if i == 2 {
                b.set_text(tr!("Abbrechen", "Cancel"));
            }
            b.connect("pressed", &Callable::from_object_method(&self.to_gd(), method));
            col.add_child(&b);
            self.hero_buttons.push(b);
        }
        let mut p = panel();
        p.add_child(&col);
        p.set_visible(false);
        place(&mut p, (0.5, 1.0), (0.0, -190.0), GrowDirection::BOTH, GrowDirection::BEGIN);
        root.add_child(&p);
        self.hero_panel = Some(p);
        self.hero_title = Some(title);
    }

    fn build(&mut self) {
        let mut root = Control::new_alloc();
        root.set_name("Root");
        root.set_anchors_preset(godot::classes::control::LayoutPreset::FULL_RECT);
        root.set_mouse_filter(MouseFilter::IGNORE);

        // Phasenliste oben rechts
        let mut phases = VBoxContainer::new_alloc();
        phases.add_theme_constant_override("separation", 2);
        let mut turn = label(18, GOLD);
        phases.add_child(&turn);
        let titles = self
            .turn_manager
            .as_ref()
            .map(|tm| tm.bind().get_phase_titles())
            .unwrap_or_default();
        for (i, t) in titles.as_slice().iter().enumerate() {
            // Kampf-Unterphasen eingerückt
            let indent = if (4..=6).contains(&i) { "    " } else { "" };
            let mut l = label(15, DIM);
            l.set_text(&format!("{indent}{t}"));
            phases.add_child(&l);
            self.phase_labels.push(l);
        }
        // Hinweis und Protokoll unter der Phasenliste
        phases.set_custom_minimum_size(Vector2::new(270.0, 0.0));
        let mut sep = label(6, DIM);
        sep.set_text(" ");
        phases.add_child(&sep);
        let mut hint = label(16, GOLD);
        hint.set_autowrap_mode(AutowrapMode::WORD_SMART);
        phases.add_child(&hint);
        let mut log = label(13, PARCHMENT);
        log.set_autowrap_mode(AutowrapMode::WORD_SMART);
        phases.add_child(&log);

        let mut phase_panel = panel();
        phase_panel.add_child(&phases);
        place(&mut phase_panel, (1.0, 0.0), (-14.0, 14.0), GrowDirection::BEGIN, GrowDirection::END);
        root.add_child(&phase_panel);
        turn.set_text(tr!("Runde –", "Turn –"));

        // Anzeigen für Leben, Ressourcen und Mana
        let opp = rich(18);
        let mut opp_panel = panel();
        opp_panel.add_child(&opp);
        place(&mut opp_panel, (0.0, 0.0), (14.0, 14.0), GrowDirection::END, GrowDirection::END);
        root.add_child(&opp_panel);

        let me = rich(18);
        let mut me_panel = panel();
        me_panel.add_child(&me);
        place(&mut me_panel, (0.0, 1.0), (14.0, -14.0), GrowDirection::END, GrowDirection::BEGIN);
        root.add_child(&me_panel);

        // Nur der Button unten rechts (Deck und Ablage bleiben sichtbar)
        let mut button = styled_button(18);
        button.set_text(tr!("Nächste Phase  »", "Next phase  »"));
        button.set_custom_minimum_size(Vector2::new(240.0, 46.0));
        let callable = Callable::from_object_method(&self.to_gd(), "on_next_pressed");
        button.connect("pressed", &callable);
        place(&mut button, (1.0, 1.0), (-14.0, -14.0), GrowDirection::BEGIN, GrowDirection::BEGIN);
        root.add_child(&button);

        self.build_hero_panel(&mut root);
        self.base_mut().add_child(&root);
        self.turn_label = Some(turn);
        self.opponent_info = Some(opp);
        self.player_info = Some(me);
        self.hint_label = Some(hint);
        self.log_label = Some(log);
        self.button = Some(button);
    }

    fn refresh(&mut self) {
        let Some(tm) = self.turn_manager.clone() else {
            return;
        };
        let tm = tm.bind();
        let current = tm.get_phase_index();
        let active = tm.get_active_seat();

        if let Some(l) = self.turn_label.as_mut() {
            let who = if active >= 0 { tm.get_seat_name(active).to_string() } else { "—".into() };
            l.set_text(&trf!("Runde {}  ·  {who}", "Turn {}  ·  {who}", tm.get_turn()));
        }
        let titles = tm.get_phase_titles();
        for (i, l) in self.phase_labels.iter_mut().enumerate() {
            let indent = if (4..=6).contains(&i) { "    " } else { "" };
            if let Some(t) = titles.as_slice().get(i) {
                l.set_text(&format!("{indent}{t}"));
            }
            let (col, size) = match (i as i64).cmp(&current) {
                std::cmp::Ordering::Equal => (GOLD, 17),
                std::cmp::Ordering::Less => (DIM, 15),
                std::cmp::Ordering::Greater => (PARCHMENT, 15),
            };
            l.add_theme_color_override("font_color", col);
            l.add_theme_font_size_override("font_size", size);
        }

        let info = |seat: i64| {
            let r = tm.get_resource_counts(seat);
            let marker = if seat == active { tr!("  [color=#f2c766](am Zug)[/color]", "  [color=#f2c766](to play)[/color]") } else { "" };
            trf!(
                "{}{marker}\nLeben {}   ·   Ressourcen {}/{}\nMana: {}",
                "{}{marker}\nLife {}   ·   Resources {}/{}\nMana: {}",
                tm.get_seat_name(seat),
                tm.get_life(seat),
                r.x,
                r.y,
                tm.get_pool_bbcode(seat)
            )
        };
        if let Some(l) = self.opponent_info.as_mut() {
            l.set_text(&info(1));
        }
        if let Some(l) = self.player_info.as_mut() {
            l.set_text(&info(0));
        }

        let winner = tm.get_winner_name();
        if let Some(l) = self.hint_label.as_mut() {
            let text = if winner.is_empty() {
                let mut t = format!("{}\n{}", tm.get_phase_title(), tm.get_hint());
                let stack = tm.get_stack_lines();
                if !stack.is_empty() {
                    t.push_str(tr!("\nStack (oben zuerst):", "\nStack (top first):"));
                    for line in stack.as_slice() {
                        t.push_str(&format!("\n• {line}"));
                    }
                }
                t
            } else {
                trf!("Spielende\n{winner} gewinnt!", "Game over\n{winner} wins!")
            };
            l.set_text(&text);
        }
        if let Some(l) = self.log_label.as_mut() {
            let log = tm.get_log();
            // Nur die letzten Einträge, damit das Panel nicht über den Button wächst
            let all = log.as_slice();
            let lines: Vec<String> = all[all.len().saturating_sub(4)..].iter().map(|s| s.to_string()).collect();
            l.set_text(&lines.join("\n"));
        }
        if let Some(b) = self.button.as_mut() {
            b.set_disabled(!tm.is_waiting_for_input());
            b.set_text(&tm.get_button_text());
        }

        // Heldenfähigkeiten
        let hero = tm.get_hero_choice_name();
        if let Some(p) = self.hero_panel.as_mut() {
            p.set_visible(!hero.is_empty());
        }
        if !hero.is_empty() {
            if let Some(t) = self.hero_title.as_mut() {
                t.set_text(&trf!("{hero} – Fähigkeit wählen (1× pro Runde)", "{hero} – choose an ability (once per turn)"));
            }
            if let Some(b) = self.hero_buttons.get_mut(2) {
                b.set_text(tr!("Abbrechen", "Cancel"));
            }
            let choices = tm.get_hero_choices();
            for (i, b) in self.hero_buttons.iter_mut().take(2).enumerate() {
                let text = choices.as_slice().get(i).map(|t| t.to_string()).unwrap_or_default();
                b.set_visible(!text.is_empty());
                b.set_text(&text);
                b.set_disabled(!tm.is_hero_choice_available(i as i64));
            }
        }
    }
}

fn styled_button(font_size: i32) -> Gd<Button> {
    let mut button = Button::new_alloc();
    button.set_focus_mode(godot::classes::control::FocusMode::NONE);
    button.set_custom_minimum_size(Vector2::new(0.0, 38.0));
    button.add_theme_font_size_override("font_size", font_size);
    button.add_theme_color_override("font_color", PARCHMENT);
    button.add_theme_color_override("font_hover_color", GOLD);
    button.add_theme_color_override("font_disabled_color", DIM);
    for (state, bg) in [
        ("normal", Color::from_rgba(0.35, 0.2, 0.08, 0.95)),
        ("hover", Color::from_rgba(0.48, 0.28, 0.1, 0.95)),
        ("pressed", Color::from_rgba(0.25, 0.14, 0.05, 0.95)),
        ("disabled", Color::from_rgba(0.15, 0.1, 0.06, 0.7)),
    ] {
        button.add_theme_stylebox_override(state, &style(bg, 6.0));
    }
    button
}

/// Mehrfarbiger Text (BBCode), z. B. für den Manapool.
fn rich(size: i32) -> Gd<RichTextLabel> {
    let mut l = RichTextLabel::new_alloc();
    l.set_use_bbcode(true);
    l.set_fit_content(true);
    l.set_scroll_active(false);
    l.set_autowrap_mode(AutowrapMode::OFF);
    l.set_custom_minimum_size(Vector2::new(260.0, 0.0));
    l.add_theme_font_size_override("normal_font_size", size);
    l.add_theme_color_override("default_color", PARCHMENT);
    l.add_theme_color_override("font_outline_color", Color::from_rgba(0.0, 0.0, 0.0, 0.8));
    l.add_theme_constant_override("outline_size", 4);
    l.set_mouse_filter(MouseFilter::IGNORE);
    l
}

fn label(size: i32, color: Color) -> Gd<Label> {
    let mut l = Label::new_alloc();
    l.add_theme_font_size_override("font_size", size);
    l.add_theme_color_override("font_color", color);
    l.add_theme_color_override("font_outline_color", Color::from_rgba(0.0, 0.0, 0.0, 0.8));
    l.add_theme_constant_override("outline_size", 4);
    l.set_mouse_filter(MouseFilter::IGNORE);
    l
}

fn style(bg: Color, radius: f32) -> Gd<StyleBoxFlat> {
    let mut s = StyleBoxFlat::new_gd();
    s.set_bg_color(bg);
    s.set_border_color(Color::from_rgba(0.78, 0.6, 0.3, 0.9));
    s.set_border_width_all(2);
    s.set_corner_radius_all(radius as i32);
    s.set_content_margin_all(10.0);
    s
}

fn panel() -> Gd<PanelContainer> {
    let mut p = PanelContainer::new_alloc();
    p.add_theme_stylebox_override("panel", &style(Color::from_rgba(0.09, 0.05, 0.025, 0.8), 8.0));
    p.set_mouse_filter(MouseFilter::IGNORE);
    p
}

/// Verankert ein Control an einer Bildschirmecke (anchor 0/1) mit Abstand `offset`.
fn place<T: Inherits<Control>>(c: &mut Gd<T>, anchor: (f32, f32), offset: (f32, f32), grow_h: GrowDirection, grow_v: GrowDirection) {
    for (side, a, o) in [
        (Side::LEFT, anchor.0, offset.0),
        (Side::RIGHT, anchor.0, offset.0),
        (Side::TOP, anchor.1, offset.1),
        (Side::BOTTOM, anchor.1, offset.1),
    ] {
        let mut c = c.clone().upcast::<Control>();
        c.set_anchor(side, a);
        c.set_offset(side, o);
    }
    let mut c = c.clone().upcast::<Control>();
    c.set_h_grow_direction(grow_h);
    c.set_v_grow_direction(grow_v);
}
