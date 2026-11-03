//! Auswahlfenster des HUD (Bildschirmmitte) für alle allgemeinen Entscheidungen von Forge.
//!
//! Vier Modi ([`ChoiceMode`]):
//! - **Eine Option**: je Option ein Knopf, Klick = Antwort (Index).
//! - **Mehrere Optionen**: Knöpfe zum An-/Abwählen, „Fertig (n)“ wird aktiv, sobald die Anzahl
//!   zwischen Minimum und Maximum liegt (Antwort: Liste von Indizes).
//! - **Ja/Nein**: zwei Knöpfe mit eigenen Beschriftungen (Antwort: true / false).
//! - **Zahl**: Zahlenfeld mit Minimum/Maximum und „OK“ (Antwort: Zahl).
//!
//! - **Nur Knopf**: Auswahl findet auf dem Tisch statt (Angreifer, Blocker); der Knopf bestätigt.
//!
//! Optional „Abbrechen“ (Antwort: null). Bei vielen Optionen werden die Knöpfe zweispaltig und
//! scrollbar angeordnet, ab [`FILTER_FROM`] Optionen gibt es ein Suchfeld. Es werden höchstens
//! [`MAX_SHOWN`] Knöpfe gebaut (lange Listen wie alle Kartennamen: nur die Suchtreffer); Farb-Optionen bekommen die Farbe der Manaart.
//! Das Fenster wird nur neu aufgebaut, wenn eine neue Anfrage kommt ([`HudChoice::key`]).

use godot::classes::control::{MouseFilter, SizeFlags};
use godot::classes::text_server::AutowrapMode;
use godot::classes::{
    Button, Control, GridContainer, HBoxContainer, LineEdit, PanelContainer, ScrollContainer, SpinBox,
    VBoxContainer,
};
use godot::prelude::*;

/// Art der Auswahl im [`HudChoice`].
#[derive(Clone, Debug, PartialEq)]
pub enum ChoiceMode {
    /// Genau eine Option.
    Single,
    /// Zwischen `min` und `max` Optionen.
    Multi { min: usize, max: usize },
    /// Ja/Nein: erste Option = true, zweite = false.
    Confirm,
    /// Zahl zwischen `min` und `max`.
    Number { min: i64, max: i64 },
    /// Keine Optionen, nur ein Knopf mit `label` (die Auswahl passiert auf dem Tisch, z. B.
    /// Angreifer/Blocker); Antwort `true`.
    Done { label: String },
}

/// Eine wählbare Option: Text und optional eine Farbe für den Knopf.
#[derive(Clone, Debug, PartialEq)]
pub struct ChoiceOption {
    pub text: String,
    pub color: Option<Color>,
}

/// Eine offene Entscheidung für das Auswahlfenster.
#[derive(Clone, Debug, PartialEq)]
pub struct HudChoice {
    /// Kennung der Anfrage: Das Fenster wird nur bei einer neuen Kennung neu aufgebaut.
    pub key: i64,
    pub title: String,
    /// Zusatzzeile (z. B. Quelle und Fähigkeit), leer = keine.
    pub detail: String,
    pub mode: ChoiceMode,
    pub options: Vec<ChoiceOption>,
    pub cancelable: bool,
    /// Fenster am rechten Bildrand statt in der Mitte, damit der Tisch anklickbar bleibt
    /// (z. B. bei der Zielwahl).
    pub side: bool,
}

/// Ab so vielen Optionen werden die Knöpfe zweispaltig angeordnet.
const TWO_COLUMNS_FROM: usize = 5;
/// Ab so vielen Optionen erscheint ein Suchfeld, das die Knöpfe filtert.
const FILTER_FROM: usize = 13;
/// Höchstens so viele Optionsknöpfe gleichzeitig (der Rest ist über das Suchfeld erreichbar).
const MAX_SHOWN: usize = 40;
/// Maximale Höhe der Optionsliste in Pixeln (darüber scrollbar).
const LIST_MAX_HEIGHT: f32 = 380.0;
/// Hervorhebung ausgewählter Optionen (Mehrfachauswahl).
const SELECTED: Color = Color::from_rgb(0.95, 0.78, 0.4);

/// Das Auswahlfenster. Gehört dem [`TurnHud`](super::hud::TurnHud); Knöpfe rufen dessen
/// Methoden `on_choice_option(index)`, `on_choice_done()`, `on_choice_cancel()`,
/// und `on_choice_filter_changed(text)` auf.
pub struct ChoicePanel {
    panel: Gd<PanelContainer>,
    body: Gd<VBoxContainer>,
    current: Option<HudChoice>,
    selected: Vec<usize>,
    /// Sichtbare Optionsknöpfe mit dem Index ihrer Option.
    option_buttons: Vec<(usize, Gd<Button>)>,
    /// Raster der Optionsknöpfe und Hinweis „… weitere“ (bei langen Listen).
    grid: Option<Gd<GridContainer>>,
    more: Option<Gd<godot::classes::Label>>,
    /// Objekt, dessen Methoden die Knöpfe aufrufen (das HUD).
    hud: Option<Gd<Object>>,
    done: Option<Gd<Button>>,
    spin: Option<Gd<SpinBox>>,
    /// Feld, das beim Öffnen den Tastaturfokus bekommt (Suchfeld der Optionsliste).
    edit_focus: Option<Gd<LineEdit>>,
}

impl ChoicePanel {
    /// Legt das (unsichtbare) Fenster mittig in `root` an.
    pub fn new(root: &mut Gd<Control>) -> Self {
        let mut body = VBoxContainer::new_alloc();
        body.add_theme_constant_override("separation", 8);
        body.set_custom_minimum_size(Vector2::new(460.0, 0.0));
        let mut panel = super::hud::panel();
        // Klicks auf das Fenster nicht an den Tisch dahinter durchreichen
        panel.set_mouse_filter(MouseFilter::STOP);
        panel.add_child(&body);
        panel.set_visible(false);
        super::hud::place(
            &mut panel,
            (0.5, 0.5),
            (0.0, 0.0),
            godot::classes::control::GrowDirection::BOTH,
            godot::classes::control::GrowDirection::BOTH,
        );
        root.add_child(&panel);
        Self {
            panel,
            body,
            current: None,
            selected: Vec::new(),
            option_buttons: Vec::new(),
            grid: None,
            more: None,
            hud: None,
            done: None,
            spin: None,
            edit_focus: None,
        }
    }

    /// Zeigt `choice` an (baut nur bei einer neuen Anfrage neu auf) bzw. blendet das Fenster aus.
    /// `hud` ist das Objekt, dessen Methoden die Knöpfe aufrufen.
    pub fn show(&mut self, choice: Option<&HudChoice>, hud: &Gd<Object>) {
        let Some(choice) = choice else {
            if self.current.take().is_some() {
                self.clear();
                self.panel.set_visible(false);
            }
            return;
        };
        let changed = match &self.current {
            Some(c) => c.key != choice.key || c != choice,
            None => true,
        };
        if !changed {
            return;
        }
        self.current = Some(choice.clone());
        self.selected.clear();
        self.build(choice, hud);
        self.panel.set_visible(true);
        // Mitte bzw. rechter Rand (Tisch bleibt anklickbar)
        let (anchor, offset, grow_h) = if choice.side {
            ((1.0, 0.5), (-14.0, 0.0), godot::classes::control::GrowDirection::BEGIN)
        } else {
            ((0.5, 0.5), (0.0, 0.0), godot::classes::control::GrowDirection::BOTH)
        };
        super::hud::place(&mut self.panel, anchor, offset, grow_h, godot::classes::control::GrowDirection::BOTH);
        // Größe neu berechnen (Inhalt hat sich geändert)
        let size = self.panel.get_combined_minimum_size();
        self.panel.set_size(size);
        if let Some(edit) = self.edit_focus.as_mut() {
            edit.grab_focus();
        }
    }

    /// Klick auf Option `index`. Gibt die Antwort zurück, wenn die Auswahl damit fertig ist
    /// (eine Option, Ja/Nein); bei Mehrfachauswahl wird nur umgeschaltet.
    pub fn on_option(&mut self, index: usize) -> Option<Variant> {
        let choice = self.current.as_ref()?;
        match choice.mode {
            ChoiceMode::Single => Some((index as i64).to_variant()),
            ChoiceMode::Confirm => Some((index == 0).to_variant()),
            ChoiceMode::Multi { max, .. } => {
                if let Some(pos) = self.selected.iter().position(|i| *i == index) {
                    self.selected.remove(pos);
                } else if self.selected.len() < max {
                    self.selected.push(index);
                } else if max == 1 {
                    // Bei höchstens einer Option: Auswahl wechseln
                    self.selected = vec![index];
                }
                self.refresh_multi();
                None
            }
            _ => None,
        }
    }

    /// Suchfeld geändert: nur Optionen anzeigen, deren Text `filter` enthält (ohne Groß/klein),
    /// höchstens [`MAX_SHOWN`].
    pub fn filter(&mut self, filter: &str) {
        self.render_options(filter);
        self.refresh_multi();
    }

    /// Baut die Optionsknöpfe für alle Optionen, die `filter` enthalten (höchstens [`MAX_SHOWN`];
    /// Treffer, die mit dem Suchtext beginnen, zuerst).
    fn render_options(&mut self, filter: &str) {
        let (Some(choice), Some(mut grid), Some(hud)) = (self.current.clone(), self.grid.clone(), self.hud.clone()) else {
            return;
        };
        for child in grid.get_children().iter_shared() {
            child.clone().queue_free();
            grid.remove_child(&child);
        }
        self.option_buttons.clear();
        let needle = filter.trim().to_lowercase();
        let mut matches: Vec<usize> = (0..choice.options.len())
            .filter(|i| needle.is_empty() || choice.options[*i].text.to_lowercase().contains(&needle))
            .collect();
        if !needle.is_empty() {
            matches.sort_by_key(|i| !choice.options[*i].text.to_lowercase().starts_with(&needle));
        }
        let total = matches.len();
        for i in matches.into_iter().take(MAX_SHOWN) {
            let option = &choice.options[i];
            let mut b = super::hud::styled_button(16);
            b.set_text(&option.text);
            b.set_h_size_flags(SizeFlags::EXPAND_FILL);
            b.set_text_overrun_behavior(godot::classes::text_server::OverrunBehavior::TRIM_ELLIPSIS);
            b.set_tooltip_text(&option.text);
            if let Some(c) = option.color {
                b.add_theme_stylebox_override("normal", &super::hud::style(c.darkened(0.35), 6.0));
                b.add_theme_stylebox_override("hover", &super::hud::style(c.darkened(0.15), 6.0));
            }
            b.connect("pressed", &Callable::from_object_method(&hud, "on_choice_option").bind(&[(i as i64).to_variant()]));
            grid.add_child(&b);
            self.option_buttons.push((i, b));
        }
        if let Some(more) = self.more.as_mut() {
            let rest = total.saturating_sub(MAX_SHOWN);
            more.set_visible(rest > 0 || total == 0);
            more.set_text(&if total == 0 {
                tr!("Keine Treffer", "No matches").to_string()
            } else {
                trf!("… {rest} weitere – Suchbegriff eingeben", "… {rest} more – type to search")
            });
        }
    }

    /// Aktuell gewählte Optionen (Mehrfachauswahl; sonst leer).
    pub fn selected(&self) -> &[usize] {
        &self.selected
    }

    /// „Fertig“ / „OK“: Antwort aus Mehrfachauswahl oder Zahl.
    pub fn on_done(&self) -> Option<Variant> {
        let choice = self.current.as_ref()?;
        match &choice.mode {
            ChoiceMode::Multi { min, max } => {
                let n = self.selected.len();
                (n >= *min && n <= *max).then(|| {
                    let arr: VarArray = self.selected.iter().map(|i| (*i as i64).to_variant()).collect();
                    arr.to_variant()
                })
            }
            ChoiceMode::Number { .. } => self.spin.as_ref().map(|s| (s.get_value().round() as i64).to_variant()),
            ChoiceMode::Done { .. } => Some(true.to_variant()),
            _ => None,
        }
    }

    fn clear(&mut self) {
        for child in self.body.get_children().iter_shared() {
            // queue_free: der Aufruf kann aus dem pressed-Signal eines dieser Knöpfe kommen
            child.clone().queue_free();
            self.body.remove_child(&child);
        }
        self.option_buttons.clear();
        self.grid = None;
        self.more = None;
        self.done = None;
        self.spin = None;
        self.edit_focus = None;
    }

    fn build(&mut self, choice: &HudChoice, hud: &Gd<Object>) {
        self.clear();
        let gold = Color::from_rgb(0.95, 0.78, 0.4);
        let parchment = Color::from_rgb(0.93, 0.87, 0.74);

        let mut title = super::hud::label(18, gold);
        title.set_text(&choice.title);
        title.set_autowrap_mode(AutowrapMode::WORD_SMART);
        title.set_custom_minimum_size(Vector2::new(460.0, 0.0));
        self.body.add_child(&title);
        if !choice.detail.is_empty() {
            let mut detail = super::hud::label(14, Color::from_rgba(parchment.r, parchment.g, parchment.b, 0.75));
            detail.set_text(&choice.detail);
            detail.set_autowrap_mode(AutowrapMode::WORD_SMART);
            detail.set_custom_minimum_size(Vector2::new(460.0, 0.0));
            self.body.add_child(&detail);
        }

        let method = |name: &str| Callable::from_object_method(hud, name);
        match &choice.mode {
            ChoiceMode::Single | ChoiceMode::Confirm | ChoiceMode::Multi { .. } => {
                if choice.options.len() >= FILTER_FROM {
                    let mut search = LineEdit::new_alloc();
                    search.set_placeholder(tr!("Suchen …", "Search …"));
                    search.set_clear_button_enabled(true);
                    search.add_theme_font_size_override("font_size", 16);
                    search.connect("text_changed", &method("on_choice_filter_changed"));
                    self.body.add_child(&search);
                    self.edit_focus = Some(search);
                }
                let columns = if choice.options.len() >= TWO_COLUMNS_FROM { 2 } else { 1 };
                let mut grid = GridContainer::new_alloc();
                grid.set_columns(columns);
                grid.add_theme_constant_override("h_separation", 6);
                grid.add_theme_constant_override("v_separation", 6);
                grid.set_h_size_flags(SizeFlags::EXPAND_FILL);
                let shown = choice.options.len().min(MAX_SHOWN);
                if shown.div_ceil(columns as usize) as f32 * 44.0 > LIST_MAX_HEIGHT {
                    let mut scroll = ScrollContainer::new_alloc();
                    scroll.set_custom_minimum_size(Vector2::new(460.0, LIST_MAX_HEIGHT));
                    scroll.add_child(&grid);
                    self.body.add_child(&scroll);
                } else {
                    self.body.add_child(&grid);
                }
                let mut more = super::hud::label(13, parchment);
                more.set_visible(false);
                self.body.add_child(&more);
                self.grid = Some(grid);
                self.more = Some(more);
                self.hud = Some(hud.clone());
                self.render_options("");
            }
            ChoiceMode::Number { min, max } => {
                let mut spin = SpinBox::new_alloc();
                spin.set_min(*min as f64);
                spin.set_max(*max as f64);
                spin.set_step(1.0);
                spin.set_value(*min as f64);
                spin.set_h_size_flags(SizeFlags::EXPAND_FILL);
                spin.add_theme_font_size_override("font_size", 18);
                self.body.add_child(&spin);
                let mut hint = super::hud::label(13, parchment);
                hint.set_text(&format!("{min} – {max}"));
                self.body.add_child(&hint);
                self.spin = Some(spin);
            }
            ChoiceMode::Done { .. } => {}
        }

        // Untere Zeile: Fertig/OK und Abbrechen
        let needs_done = matches!(choice.mode, ChoiceMode::Multi { .. } | ChoiceMode::Number { .. } | ChoiceMode::Done { .. });
        if needs_done || choice.cancelable {
            let mut row = HBoxContainer::new_alloc();
            row.add_theme_constant_override("separation", 8);
            row.set_alignment(godot::classes::box_container::AlignmentMode::END);
            if needs_done {
                let mut done = super::hud::styled_button(16);
                match &choice.mode {
                    ChoiceMode::Done { label } => done.set_text(label),
                    _ => done.set_text(tr!("OK", "OK")),
                }
                done.set_custom_minimum_size(Vector2::new(140.0, 38.0));
                done.connect("pressed", &method("on_choice_done"));
                row.add_child(&done);
                self.done = Some(done);
            }
            if choice.cancelable {
                let mut cancel = super::hud::styled_button(15);
                cancel.set_text(tr!("Abbrechen", "Cancel"));
                cancel.connect("pressed", &method("on_choice_cancel"));
                row.add_child(&cancel);
            }
            self.body.add_child(&row);
        }
        self.refresh_multi();
    }

    /// Mehrfachauswahl: ausgewählte Knöpfe golden umranden, „Fertig (n)“ nur bei gültiger Anzahl.
    fn refresh_multi(&mut self) {
        let Some(ChoiceMode::Multi { min, max }) = self.current.as_ref().map(|c| c.mode.clone()) else { return };
        for (i, b) in self.option_buttons.iter_mut() {
            let on = self.selected.contains(i);
            b.add_theme_color_override("font_color", if on { SELECTED } else { Color::from_rgb(0.93, 0.87, 0.74) });
            b.set_modulate(if on { Color::WHITE } else { Color::from_rgba(1.0, 1.0, 1.0, 0.8) });
        }
        if let Some(done) = self.done.as_mut() {
            let n = self.selected.len();
            done.set_disabled(n < min || n > max);
            let range = if min == max { format!("{max}") } else { format!("{min}–{max}") };
            done.set_text(&trf!("Fertig ({n}/{range})", "Done ({n}/{range})"));
        }
    }
}

