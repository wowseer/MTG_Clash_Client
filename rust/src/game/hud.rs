//! TurnHud: Bildschirm-Overlay für den Rundenablauf.
//!
//! - unten links: Zeitstrahl der Phasen ([`PhaseTimeline`])
//! - Bildmitte (Mittellinie): Hinweis bzw. Spielende; oben links: letzte Protokollzeilen
//! - Medaillon ([`SeatBadge`]) mit Leben, daneben Name, Ressourcen und Mana: folgt je einem
//!   3D-Ankerpunkt (Spieler: direkt über der Hand, Gegner: direkt unter dessen Hand); ohne
//!   Anker oben links (Gegner) bzw. unten links (Spieler)
//! - unten rechts: großer Button „Nächste Phase“ / „Passen“ (auch Leertaste); blau, bei
//!   belegtem Stack orange
//! - Bildschirmmitte: Auswahlfenster für Heldenfähigkeiten
//! - obere Bildschirmhälfte: große Stack-Anzeige ([`StackView`]), solange der Stack belegt ist
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

use super::phase_timeline::PhaseTimeline;
use super::seat_badge::SeatBadge;
use super::stack_view::StackView;
use super::turn_manager::TurnManager;

const GOLD: Color = Color::from_rgb(0.95, 0.78, 0.4);
const PARCHMENT: Color = Color::from_rgb(0.93, 0.87, 0.74);
const DIM: Color = Color::from_rgba(0.93, 0.87, 0.74, 0.45);
/// Seitenfarben (passen zu den Bannern der Arena).
const PLAYER_ACCENT: Color = Color::from_rgb(0.25, 0.55, 0.95);
const OPPONENT_ACCENT: Color = Color::from_rgb(0.9, 0.25, 0.22);
/// Hauptbutton: blau (Phase beenden) bzw. orange (Priorität abgeben, Stack belegt).
const BUTTON_BLUE: Color = Color::from_rgb(0.16, 0.36, 0.78);
const BUTTON_ORANGE: Color = Color::from_rgb(0.85, 0.42, 0.1);

/// Bildschirm-Overlay für den Rundenablauf: Zeitstrahl der Phasen ([PhaseTimeline], unten links), Hinweis in der Bildmitte, letzte Protokollzeilen oben links,
/// je ein [SeatBadge]-Medaillon mit Leben plus Ressourcen und Mana beider Spieler (an
/// [member player_hero_anchor] / [member opponent_hero_anchor], sonst links oben / unten), großer Button „Nächste Phase“ / „Passen“ (unten rechts, auch Leertaste;
/// blau, bei belegtem Stack orange) und das Auswahlfenster für Heldenfähigkeiten.
/// Liest den Zustand jedes Frame vom [TurnManager].
#[derive(GodotClass)]
#[class(base=CanvasLayer)]
pub struct TurnHud {
    /// Der [TurnManager], dessen Zustand angezeigt wird.
    #[export]
    turn_manager: Option<Gd<TurnManager>>,
    /// 3D-Punkt, an dem das Spieler-Medaillon sitzt (Unterkante mittig, z. B. direkt über der Hand).
    /// Leer = Medaillon unten links. Liegt die Maus unterhalb des Medaillons im Handbereich, wird es
    /// durchscheinend, damit gezoomte Handkarten lesbar bleiben.
    #[export]
    player_hero_anchor: Option<Gd<Node3D>>,
    /// 3D-Punkt, an dem das Gegner-Medaillon sitzt (Oberkante mittig, z. B. direkt unter dessen Hand).
    /// Leer = Medaillon oben links.
    #[export]
    opponent_hero_anchor: Option<Gd<Node3D>>,

    timeline: Option<Gd<PhaseTimeline>>,
    opponent_info: Option<Gd<RichTextLabel>>,
    player_info: Option<Gd<RichTextLabel>>,
    opponent_badge: Option<Gd<SeatBadge>>,
    player_badge: Option<Gd<SeatBadge>>,
    /// Zeilen (Medaillon + Text) von Gegner und Spieler, für die Positionierung am Anker.
    opponent_row: Option<Gd<Control>>,
    player_row: Option<Gd<Control>>,
    /// Farbe, mit der der Hauptbutton zuletzt gestaltet wurde (nur bei Wechsel neu setzen).
    button_color: Option<Color>,
    hint_label: Option<Gd<Label>>,
    log_label: Option<Gd<Label>>,
    button: Option<Gd<Button>>,
    /// Große Stack-Anzeige in der oberen Bildschirmhälfte.
    stack_view: Option<StackView>,
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
            timeline: None,
            opponent_info: None,
            player_info: None,
            opponent_badge: None,
            player_badge: None,
            opponent_row: None,
            player_row: None,
            player_hero_anchor: None,
            opponent_hero_anchor: None,
            button_color: None,
            hint_label: None,
            log_label: None,
            button: None,
            stack_view: None,
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

    /// Auswahlfenster für Heldenfähigkeiten (Bildschirmmitte).
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
        // Bildschirmmitte (unten Mitte sitzen Hand und Spieler-Medaillon)
        place(&mut p, (0.5, 0.5), (0.0, 0.0), GrowDirection::BOTH, GrowDirection::BOTH);
        root.add_child(&p);
        self.hero_panel = Some(p);
        self.hero_title = Some(title);
    }

    fn build(&mut self) {
        let mut root = Control::new_alloc();
        root.set_name("Root");
        root.set_anchors_preset(godot::classes::control::LayoutPreset::FULL_RECT);
        root.set_mouse_filter(MouseFilter::IGNORE);

        // Zeitstrahl der Phasen unten links (gegenüber dem Button, unter dem Spielbrett)
        let mut timeline = PhaseTimeline::new_alloc();
        place(&mut timeline, (0.0, 1.0), (14.0, -4.0), GrowDirection::END, GrowDirection::BEGIN);
        root.add_child(&timeline);

        // Hinweis in der Bildmitte (auf der Mittellinie zwischen den Spielfeldern)
        let mut hint = label(18, GOLD);
        hint.set_horizontal_alignment(godot::global::HorizontalAlignment::CENTER);
        hint.set_custom_minimum_size(Vector2::new(520.0, 0.0));
        hint.set_autowrap_mode(AutowrapMode::WORD_SMART);
        hint.add_theme_constant_override("outline_size", 6);
        place(&mut hint, (0.5, 0.5), (0.0, 0.0), GrowDirection::BOTH, GrowDirection::BOTH);
        root.add_child(&hint);

        // Letzte Protokollzeilen oben links, dezent
        let mut log = label(13, DIM);
        log.set_custom_minimum_size(Vector2::new(360.0, 0.0));
        log.set_autowrap_mode(AutowrapMode::WORD_SMART);
        place(&mut log, (0.0, 0.0), (14.0, 12.0), GrowDirection::END, GrowDirection::END);
        root.add_child(&log);

        // Medaillons mit Leben + Text für Ressourcen und Mana
        let (opp_row, opp_badge, opp) = seat_row(&mut root, OPPONENT_ACCENT, (0.0, 0.0), (8.0, 8.0), GrowDirection::END);
        let (me_row, me_badge, me) = seat_row(&mut root, PLAYER_ACCENT, (0.0, 1.0), (8.0, -8.0), GrowDirection::BEGIN);

        // Nur der Button unten rechts (Deck und Ablage bleiben sichtbar)
        let mut button = styled_button(20);
        button.set_text(tr!("Nächste Phase  »", "Next phase  »"));
        button.set_custom_minimum_size(Vector2::new(270.0, 56.0));
        let callable = Callable::from_object_method(&self.to_gd(), "on_next_pressed");
        button.connect("pressed", &callable);
        place(&mut button, (1.0, 1.0), (-14.0, -14.0), GrowDirection::BEGIN, GrowDirection::BEGIN);
        root.add_child(&button);

        self.build_hero_panel(&mut root);
        // Stack-Anzeige zuletzt: liegt über den übrigen Panels
        self.stack_view = Some(StackView::new(&mut root));
        self.base_mut().add_child(&root);
        self.timeline = Some(timeline);
        self.opponent_info = Some(opp);
        self.player_info = Some(me);
        self.opponent_badge = Some(opp_badge);
        self.player_badge = Some(me_badge);
        self.opponent_row = Some(opp_row);
        self.player_row = Some(me_row);
        self.hint_label = Some(hint);
        self.log_label = Some(log);
        self.button = Some(button);
    }

    /// Setzt die Medaillon-Zeilen an ihre 3D-Anker (Medaillon mittig über/unter dem Ankerpunkt).
    fn place_seat_rows(&mut self) {
        let Some(viewport) = self.base().get_viewport() else { return };
        let Some(cam) = viewport.get_camera_3d() else { return };
        let mouse = viewport.get_mouse_position();
        for (anchor, row, is_player) in [
            (self.player_hero_anchor.clone(), self.player_row.clone(), true),
            (self.opponent_hero_anchor.clone(), self.opponent_row.clone(), false),
        ] {
            let (Some(anchor), Some(mut row)) = (anchor, row) else { continue };
            let world = anchor.get_global_position();
            if cam.is_position_behind(world) {
                continue;
            }
            let p = cam.unproject_position(world);
            // Zeile auf Mindestgröße bringen; erstes Kind ist das Medaillon
            let size = row.get_combined_minimum_size();
            row.set_size(size);
            let badge_w = row.get_child(0).map(|b| b.cast::<Control>().get_combined_minimum_size().x).unwrap_or(0.0);
            let top = if is_player { p.y - size.y } else { p.y };
            row.set_position(Vector2::new(p.x - badge_w * 0.5, top));
            // Spieler: durchscheinend, solange die Maus im Handbereich darunter ist
            let alpha = if is_player && mouse.y > top + size.y * 0.5 && (mouse.x - p.x).abs() < 380.0 { 0.3 } else { 1.0 };
            row.set_modulate(Color::from_rgba(1.0, 1.0, 1.0, alpha));
        }
    }

    fn refresh(&mut self) {
        self.place_seat_rows();
        if let (Some(tm), Some(view)) = (self.turn_manager.clone(), self.stack_view.as_mut()) {
            let items: Vec<_> = tm.bind().get_stack_items().iter_shared().collect();
            view.update(&items);
        }
        let Some(tm) = self.turn_manager.clone() else {
            return;
        };
        let tm = tm.bind();
        let current = tm.get_phase_index();
        let active = tm.get_active_seat();

        if let Some(t) = self.timeline.as_mut() {
            let who = if active >= 0 { tm.get_seat_name(active).to_string() } else { "—".into() };
            let turn = trf!("Runde {}  ·  {who}", "Turn {}  ·  {who}", tm.get_turn());
            t.bind_mut().set_state(tm.get_phase_titles(), current, GString::from(&turn));
        }

        let info = |seat: i64| {
            let r = tm.get_resource_counts(seat);
            let marker = if seat == active { tr!("  [color=#f2c766](am Zug)[/color]", "  [color=#f2c766](to play)[/color]") } else { "" };
            trf!(
                "[font_size=17]{}[/font_size]{marker}\nRessourcen {}/{}\nMana: {}",
                "[font_size=17]{}[/font_size]{marker}\nResources {}/{}\nMana: {}",
                tm.get_seat_name(seat),
                r.x,
                r.y,
                tm.get_pool_bbcode(seat)
            )
        };
        for (seat, badge) in [(1, self.opponent_badge.as_mut()), (0, self.player_badge.as_mut())] {
            if let Some(b) = badge {
                b.bind_mut().set_state(tm.get_seat_name(seat), tm.get_life(seat), seat == active);
            }
        }
        if let Some(l) = self.opponent_info.as_mut() {
            l.set_text(&info(1));
        }
        if let Some(l) = self.player_info.as_mut() {
            l.set_text(&info(0));
        }

        let winner = tm.get_winner_name();
        if let Some(l) = self.hint_label.as_mut() {
            let text = if winner.is_empty() {
                // Phase steht im Zeitstrahl, der Stack groß in der StackView; hier nur Hinweis + Anzahl
                let mut t = tm.get_hint().to_string();
                let n = tm.get_stack_items().len();
                if n > 0 {
                    t.push_str(&trf!("\nAuf dem Stack: {n}", "\nOn the stack: {n}"));
                }
                t
            } else {
                trf!("Spielende\n{winner} gewinnt!", "Game over\n{winner} wins!")
            };
            l.set_text(&text);
        }
        if let Some(l) = self.log_label.as_mut() {
            let log = tm.get_log();
            // Nur die letzten Einträge
            let all = log.as_slice();
            let lines: Vec<String> = all[all.len().saturating_sub(3)..].iter().map(|s| s.to_string()).collect();
            l.set_text(&lines.join("\n"));
        }
        if let Some(b) = self.button.as_mut() {
            b.set_disabled(!tm.is_waiting_for_input());
            b.set_text(&tm.get_button_text());
            let color = if tm.get_stack_items().is_empty() { BUTTON_BLUE } else { BUTTON_ORANGE };
            if self.button_color != Some(color) {
                paint_main_button(b, color);
                self.button_color = Some(color);
            }
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
        ("normal", Color::from_rgba(0.14, 0.17, 0.24, 0.95)),
        ("hover", Color::from_rgba(0.2, 0.25, 0.36, 0.95)),
        ("pressed", Color::from_rgba(0.09, 0.11, 0.16, 0.95)),
        ("disabled", Color::from_rgba(0.08, 0.09, 0.12, 0.7)),
    ] {
        button.add_theme_stylebox_override(state, &style(bg, 6.0));
    }
    button
}

/// Großer, pillenförmiger Hauptbutton in der Farbe `color` (heller Rand, Schatten, Hover heller).
fn paint_main_button(button: &mut Gd<Button>, color: Color) {
    let light = color.lerp(Color::WHITE, 0.45);
    for (state, bg, border) in [
        ("normal", color, light),
        ("hover", color.lerp(Color::WHITE, 0.15), Color::WHITE),
        ("pressed", color.darkened(0.25), light),
        ("disabled", Color::from_rgba(0.12, 0.13, 0.16, 0.8), Color::from_rgba(0.4, 0.42, 0.48, 0.8)),
    ] {
        let mut s = style(bg, 28.0);
        s.set_border_color(border);
        s.set_border_width_all(3);
        s.set_shadow_color(Color::from_rgba(color.r, color.g, color.b, if state == "disabled" { 0.0 } else { 0.45 }));
        s.set_shadow_size(10);
        button.add_theme_stylebox_override(state, &s);
    }
    button.add_theme_color_override("font_color", Color::WHITE);
    button.add_theme_color_override("font_hover_color", Color::WHITE);
    button.add_theme_color_override("font_pressed_color", Color::WHITE);
    button.add_theme_color_override("font_outline_color", Color::from_rgba(0.0, 0.0, 0.0, 0.6));
    button.add_theme_constant_override("outline_size", 4);
}

/// Medaillon + Infotext einer Seite, an einer Bildschirmecke verankert (Rückfall, falls kein
/// 3D-Anker gesetzt ist). Gibt Zeile, Medaillon und Text zurück.
fn seat_row(
    root: &mut Gd<Control>,
    accent: Color,
    anchor: (f32, f32),
    offset: (f32, f32),
    grow_v: GrowDirection,
) -> (Gd<Control>, Gd<SeatBadge>, Gd<RichTextLabel>) {
    let mut row = godot::classes::HBoxContainer::new_alloc();
    row.add_theme_constant_override("separation", 4);
    row.set_mouse_filter(MouseFilter::IGNORE);
    let mut badge = SeatBadge::new_alloc();
    badge.set("accent", &accent.to_variant());
    row.add_child(&badge);
    let mut text = rich(14);
    text.set_v_size_flags(godot::classes::control::SizeFlags::SHRINK_CENTER);
    text.add_theme_constant_override("outline_size", 6);
    row.add_child(&text);
    place(&mut row, anchor, offset, GrowDirection::END, grow_v);
    root.add_child(&row);
    (row.upcast(), badge, text)
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
    p.add_theme_stylebox_override("panel", &style(Color::from_rgba(0.05, 0.06, 0.08, 0.82), 8.0));
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
