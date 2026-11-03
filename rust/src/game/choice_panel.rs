//! Auswahlfenster des HUD (Bildschirmmitte) für alle allgemeinen Entscheidungen von Forge.
//!
//! Fünf Modi ([`ChoiceMode`]):
//! - **Eine Option**: je Option ein Knopf, Klick = Antwort (Index).
//! - **Mehrere Optionen**: Knöpfe zum An-/Abwählen, „Fertig (n)“ wird aktiv, sobald die Anzahl
//!   zwischen Minimum und Maximum liegt (Antwort: Liste von Indizes).
//! - **Ja/Nein**: zwei Knöpfe mit eigenen Beschriftungen (Antwort: true / false).
//! - **Zahl**: Zahlenfeld mit Minimum/Maximum und „OK“ (Antwort: Zahl).
//! - **Text**: Eingabefeld (z. B. Kartenname) mit „OK“ bzw. Eingabetaste, Fehlermeldung darüber.
//!
//! Optional „Abbrechen“ (Antwort: null). Bei vielen Optionen werden die Knöpfe zweispaltig und
//! scrollbar angeordnet, ab [`FILTER_FROM`] Optionen gibt es ein Suchfeld; Farb-Optionen bekommen die Farbe der Manaart.
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
    /// Freie Eingabe; `error` = Hinweis nach einer ungültigen Eingabe.
    Text { error: String },
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
}

/// Ab so vielen Optionen werden die Knöpfe zweispaltig angeordnet.
const TWO_COLUMNS_FROM: usize = 5;
/// Ab so vielen Optionen erscheint ein Suchfeld, das die Knöpfe filtert.
const FILTER_FROM: usize = 13;
/// Maximale Höhe der Optionsliste in Pixeln (darüber scrollbar).
const LIST_MAX_HEIGHT: f32 = 380.0;
/// Hervorhebung ausgewählter Optionen (Mehrfachauswahl).
const SELECTED: Color = Color::from_rgb(0.95, 0.78, 0.4);

/// Das Auswahlfenster. Gehört dem [`TurnHud`](super::hud::TurnHud); Knöpfe rufen dessen
/// Methoden `on_choice_option(index)`, `on_choice_done()`, `on_choice_cancel()`,
/// `on_choice_text_submitted(text)` und `on_choice_filter_changed(text)` auf.
pub struct ChoicePanel {
    panel: Gd<PanelContainer>,
    body: Gd<VBoxContainer>,
    current: Option<HudChoice>,
    selected: Vec<usize>,
    option_buttons: Vec<Gd<Button>>,
    done: Option<Gd<Button>>,
    spin: Option<Gd<SpinBox>>,
    edit: Option<Gd<LineEdit>>,
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
            done: None,
            spin: None,
            edit: None,
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
        // Größe neu berechnen (Inhalt hat sich geändert)
        let size = self.panel.get_combined_minimum_size();
        self.panel.set_size(size);
        if let Some(edit) = self.edit.as_mut().or(self.edit_focus.as_mut()) {
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

    /// Suchfeld geändert: nur Optionen anzeigen, deren Text `filter` enthält (ohne Groß/klein).
    pub fn filter(&mut self, filter: &str) {
        let needle = filter.trim().to_lowercase();
        let Some(choice) = self.current.as_ref() else { return };
        for (b, option) in self.option_buttons.iter_mut().zip(&choice.options) {
            b.set_visible(needle.is_empty() || option.text.to_lowercase().contains(&needle));
        }
    }

    /// „Fertig“ / „OK“ / Eingabetaste: Antwort aus Mehrfachauswahl, Zahl oder Text.
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
            ChoiceMode::Text { .. } => {
                let text = self.edit.as_ref()?.get_text().to_string();
                let text = text.trim();
                (!text.is_empty()).then(|| text.to_variant())
            }
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
        self.done = None;
        self.spin = None;
        self.edit = None;
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
        if let ChoiceMode::Text { error } = &choice.mode {
            if !error.is_empty() {
                let mut e = super::hud::label(14, Color::from_rgb(1.0, 0.45, 0.4));
                e.set_text(error);
                e.set_autowrap_mode(AutowrapMode::WORD_SMART);
                self.body.add_child(&e);
            }
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
                for (i, option) in choice.options.iter().enumerate() {
                    let mut b = super::hud::styled_button(16);
                    b.set_text(&option.text);
                    b.set_h_size_flags(SizeFlags::EXPAND_FILL);
                    b.set_text_overrun_behavior(godot::classes::text_server::OverrunBehavior::TRIM_ELLIPSIS);
                    b.set_tooltip_text(&option.text);
                    if let Some(c) = option.color {
                        b.add_theme_stylebox_override("normal", &super::hud::style(c.darkened(0.35), 6.0));
                        b.add_theme_stylebox_override("hover", &super::hud::style(c.darkened(0.15), 6.0));
                    }
                    b.connect("pressed", &method("on_choice_option").bind(&[(i as i64).to_variant()]));
                    grid.add_child(&b);
                    self.option_buttons.push(b);
                }
                let rows = choice.options.len().div_ceil(columns as usize);
                if rows as f32 * 44.0 > LIST_MAX_HEIGHT {
                    let mut scroll = ScrollContainer::new_alloc();
                    scroll.set_custom_minimum_size(Vector2::new(460.0, LIST_MAX_HEIGHT));
                    scroll.add_child(&grid);
                    self.body.add_child(&scroll);
                } else {
                    self.body.add_child(&grid);
                }
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
            ChoiceMode::Text { .. } => {
                let mut edit = LineEdit::new_alloc();
                edit.set_placeholder(tr!("Name eingeben …", "Enter a name …"));
                edit.add_theme_font_size_override("font_size", 18);
                edit.connect("text_submitted", &method("on_choice_text_submitted"));
                self.body.add_child(&edit);
                self.edit = Some(edit);
            }
        }

        // Untere Zeile: Fertig/OK und Abbrechen
        let needs_done = matches!(choice.mode, ChoiceMode::Multi { .. } | ChoiceMode::Number { .. } | ChoiceMode::Text { .. });
        if needs_done || choice.cancelable {
            let mut row = HBoxContainer::new_alloc();
            row.add_theme_constant_override("separation", 8);
            row.set_alignment(godot::classes::box_container::AlignmentMode::END);
            if needs_done {
                let mut done = super::hud::styled_button(16);
                done.set_text(tr!("OK", "OK"));
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
        for (i, b) in self.option_buttons.iter_mut().enumerate() {
            let on = self.selected.contains(&i);
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

/// Erzeugt einen [`HudChoice`] aus einer Forge-Anfrage `kind = "choice"`
/// (Aufbau siehe `docs/Forge-Schnittstelle.md`). `None` bei unbekanntem Modus.
pub fn from_forge_request(req: &VarDictionary) -> Option<HudChoice> {
    let get = |k: &str| req.get(k).filter(|v| !v.is_nil());
    let int = |k: &str| get(k).and_then(|v| v.try_to::<i64>().ok()).unwrap_or(0);
    let text = |k: &str| get(k).map(|v| v.to_string()).unwrap_or_default();
    let options: Vec<ChoiceOption> = get("options")
        .and_then(|v| v.try_to::<VarArray>().ok())
        .map(|arr| {
            arr.iter_shared()
                .filter_map(|o| o.try_to::<VarDictionary>().ok())
                .map(|o| ChoiceOption {
                    text: o.get("text").map(|v| v.to_string()).unwrap_or_default(),
                    color: o.get("color").map(|v| v.to_string()).and_then(|c| forge_color(&c)),
                })
                .collect()
        })
        .unwrap_or_default();
    let mode = match text("mode").as_str() {
        "single" => ChoiceMode::Single,
        "multi" => ChoiceMode::Multi { min: int("min").max(0) as usize, max: int("max").max(0) as usize },
        "confirm" => ChoiceMode::Confirm,
        "number" => ChoiceMode::Number { min: int("min"), max: int("max") },
        "text" => ChoiceMode::Text { error: text("error") },
        _ => return None,
    };
    let source = get("source")
        .and_then(|v| v.try_to::<VarDictionary>().ok())
        .and_then(|s| s.get("name"))
        .map(|v| v.to_string())
        .unwrap_or_default();
    let ability = text("ability");
    let detail = match (source.is_empty(), ability.is_empty() || ability == source) {
        (true, true) => String::new(),
        (false, true) => source,
        (true, false) => ability,
        (false, false) => format!("{source}: {ability}"),
    };
    let mut title = text("title");
    if title.is_empty() {
        title = tr!("Auswahl", "Choice").to_string();
    }
    Some(HudChoice { key: int("id"), title, detail, mode, options, cancelable: get("cancelable").is_some_and(|v| v.booleanize()) })
}

/// Forge-Farbkürzel (W, U, B, R, G, C) → Knopffarbe.
fn forge_color(code: &str) -> Option<Color> {
    use crate::game::mana::ManaType;
    let t = match code {
        "W" => ManaType::White,
        "U" => ManaType::Blue,
        "B" => ManaType::Black,
        "R" => ManaType::Red,
        "G" => ManaType::Green,
        "C" => ManaType::Colorless,
        _ => return None,
    };
    Some(t.color())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_forge_colors() {
        assert!(forge_color("U").is_some());
        assert!(forge_color("X").is_none());
    }
}
