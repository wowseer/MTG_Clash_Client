//! `DebugPanel`: Entwickler-Fenster, das es **nur in Debug-Builds** gibt (aus dem Editor bzw.
//! einem Debug-Export gestartet; `OS.is_debug_build()`). Kein Teil der Spiel-Oberfläche – darum
//! ein schlichtes 2D-Fenster.
//!
//! - **F10** blendet es ein und aus.
//! - **Karte auf die Hand:** Suchfeld über alle Kartennamen aus Forges Kartendatenbank (die
//!   ersten Treffer erscheinen in der Liste); „Auf meine Hand“ / „Auf die Hand der KI“ bzw.
//!   Doppelklick (= meine Hand) erzeugt die Karte neu ([method ForgeBridge.debug_add_card]).
//! - **Unendlich Mana:** je ein Schalter für mich und die KI
//!   ([method ForgeBridge.debug_infinite_mana]): Forge füllt den Manapool bei jedem
//!   Phasenwechsel und Zauber auf 50 je Manaart auf.

use godot::classes::control::LayoutPreset;
use godot::classes::{
    Button, CanvasLayer, CheckBox, HBoxContainer, ICanvasLayer, InputEvent, InputEventKey, ItemList, Label, LineEdit, Os,
    PanelContainer, VBoxContainer,
};
use godot::global::Key;
use godot::prelude::*;

use crate::forge::bridge::ForgeBridge;

/// Höchstzahl der angezeigten Treffer der Kartensuche.
const MAX_HITS: usize = 200;
/// Sitzplatz des Spielers unten bzw. der KI (Forge-Sitzplätze).
const ME: i64 = 0;
const AI: i64 = 1;

/// Debug-Fenster (nur in Debug-Builds): Karten auf die Hand holen, unendliches Mana für mich
/// und die KI. F10 blendet es ein und aus.
#[derive(GodotClass)]
#[class(base=CanvasLayer)]
pub struct DebugPanel {
    /// Brücke zu Forge, an die die Debug-Befehle gehen.
    #[export]
    bridge: Option<Gd<ForgeBridge>>,
    names: Vec<String>,
    names_requested: bool,
    panel: Option<Gd<PanelContainer>>,
    search: Option<Gd<LineEdit>>,
    list: Option<Gd<ItemList>>,
    status: Option<Gd<Label>>,
    base: Base<CanvasLayer>,
}

#[godot_api]
impl ICanvasLayer for DebugPanel {
    fn init(base: Base<CanvasLayer>) -> Self {
        Self {
            bridge: None,
            names: Vec::new(),
            names_requested: false,
            panel: None,
            search: None,
            list: None,
            status: None,
            base,
        }
    }

    fn ready(&mut self) {
        // nur in Debug-Builds: sonst sofort wieder weg
        if !Os::singleton().is_debug_build() {
            self.base_mut().queue_free();
            return;
        }
        self.base_mut().set_layer(100);
        self.build();
        if let Some(mut b) = self.bridge.clone() {
            b.connect("debug_card_names_received", &Callable::from_object_method(&self.to_gd(), "on_card_names"));
        }
    }

    fn input(&mut self, event: Gd<InputEvent>) {
        let Ok(key) = event.try_cast::<InputEventKey>() else { return };
        if key.is_pressed() && !key.is_echo() && key.get_keycode() == Key::F10 {
            self.toggle();
            if let Some(mut vp) = self.base().get_viewport() {
                vp.set_input_as_handled();
            }
        }
    }
}

#[godot_api]
impl DebugPanel {
    /// Fenster ein- bzw. ausblenden (F10); beim ersten Öffnen die Kartennamen anfordern.
    #[func]
    pub fn toggle(&mut self) {
        let Some(mut panel) = self.panel.clone() else { return };
        let show = !panel.is_visible();
        panel.set_visible(show);
        if show {
            if !self.names_requested {
                self.names_requested = true;
                self.set_status("Lade Kartennamen …");
                if let Some(mut b) = self.bridge.clone() {
                    b.bind_mut().debug_request_card_names();
                }
            }
            if let Some(mut s) = self.search.clone() {
                s.grab_focus();
            }
        }
    }

    /// Kartennamen von Forge erhalten.
    #[func]
    fn on_card_names(&mut self, names: PackedStringArray) {
        self.names = names.as_slice().iter().map(|n| n.to_string()).collect();
        let n = self.names.len();
        self.set_status(&format!("{n} Karten"));
        let text = self.search.as_ref().map(|s| s.get_text()).unwrap_or_default();
        self.on_search(text);
    }

    /// Suchtext geändert: Treffer (Teilwort, ohne Groß-/Kleinschreibung) in die Liste.
    #[func]
    fn on_search(&mut self, text: GString) {
        let Some(mut list) = self.list.clone() else { return };
        list.clear();
        let q = text.to_string().to_lowercase();
        // exakte Anfänge zuerst, dann Teilwörter
        let mut starts: Vec<&String> = Vec::new();
        let mut contains: Vec<&String> = Vec::new();
        for n in &self.names {
            let l = n.to_lowercase();
            if l.starts_with(&q) {
                starts.push(n);
            } else if !q.is_empty() && l.contains(&q) {
                contains.push(n);
            }
            if starts.len() >= MAX_HITS {
                break;
            }
        }
        for n in starts.into_iter().chain(contains).take(MAX_HITS) {
            list.add_item(n.as_str());
        }
        if list.get_item_count() > 0 {
            list.select(0);
        }
    }

    /// Ausgewählte Karte auf meine Hand.
    #[func]
    fn on_add_mine(&mut self) {
        self.add_selected(ME);
    }

    /// Ausgewählte Karte auf die Hand der KI.
    #[func]
    fn on_add_ai(&mut self) {
        self.add_selected(AI);
    }

    /// Doppelklick in der Liste: Karte auf meine Hand.
    #[func]
    fn on_item_activated(&mut self, _index: i64) {
        self.add_selected(ME);
    }

    /// Schalter „Unendlich Mana“ für `seat` umgelegt.
    #[func]
    fn on_mana_toggled(&mut self, on: bool, seat: i64) {
        if let Some(mut b) = self.bridge.clone() {
            b.bind_mut().debug_infinite_mana(seat, on);
        }
        let who = if seat == ME { "mich" } else { "die KI" };
        self.set_status(&format!("Unendlich Mana für {who}: {}", if on { "an" } else { "aus" }));
    }
}

impl DebugPanel {
    fn add_selected(&mut self, seat: i64) {
        let Some(list) = self.list.clone() else { return };
        let selected = list.get_selected_items();
        let Some(i) = selected.as_slice().first().copied() else {
            self.set_status("Keine Karte gewählt");
            return;
        };
        let name = list.get_item_text(i);
        if let Some(mut b) = self.bridge.clone() {
            b.bind_mut().debug_add_card(seat, name.clone());
        }
        let who = if seat == ME { "meine Hand" } else { "die Hand der KI" };
        self.set_status(&format!("{name} → {who}"));
    }

    fn set_status(&mut self, text: &str) {
        if let Some(l) = self.status.as_mut() {
            l.set_text(text);
        }
    }

    fn build(&mut self) {
        let me = self.to_gd();
        let mut panel = PanelContainer::new_alloc();
        panel.set_anchors_preset(LayoutPreset::TOP_LEFT);
        panel.set_position(Vector2::new(16.0, 80.0));
        panel.set_custom_minimum_size(Vector2::new(360.0, 460.0));
        panel.set_visible(false);
        let mut col = VBoxContainer::new_alloc();
        panel.add_child(&col);

        let mut title = Label::new_alloc();
        title.set_text("Debug (F10)");
        col.add_child(&title);

        let mut search = LineEdit::new_alloc();
        search.set_placeholder("Karte suchen …");
        search.set_clear_button_enabled(true);
        search.connect("text_changed", &Callable::from_object_method(&me, "on_search"));
        col.add_child(&search);

        let mut list = ItemList::new_alloc();
        list.set_custom_minimum_size(Vector2::new(340.0, 260.0));
        list.set_v_size_flags(godot::classes::control::SizeFlags::EXPAND_FILL);
        list.connect("item_activated", &Callable::from_object_method(&me, "on_item_activated"));
        col.add_child(&list);

        let mut row = HBoxContainer::new_alloc();
        let mut mine = Button::new_alloc();
        mine.set_text("Auf meine Hand");
        mine.connect("pressed", &Callable::from_object_method(&me, "on_add_mine"));
        row.add_child(&mine);
        let mut ai = Button::new_alloc();
        ai.set_text("Auf die Hand der KI");
        ai.connect("pressed", &Callable::from_object_method(&me, "on_add_ai"));
        row.add_child(&ai);
        col.add_child(&row);

        for (text, seat) in [("Unendlich Mana: ich", ME), ("Unendlich Mana: KI", AI)] {
            let mut check = CheckBox::new_alloc();
            check.set_text(text);
            check.connect("toggled", &Callable::from_object_method(&me, "on_mana_toggled").bind(&[seat.to_variant()]));
            col.add_child(&check);
        }

        let mut status = Label::new_alloc();
        status.set_text("");
        col.add_child(&status);

        self.base_mut().add_child(&panel);
        self.panel = Some(panel);
        self.search = Some(search);
        self.list = Some(list);
        self.status = Some(status);
    }
}
