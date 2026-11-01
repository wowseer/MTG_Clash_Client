//! Debug-Tool (nur in Test-Builds).
//!
//! Dieses Modul wird in `lib.rs` mit `#[cfg(debug_assertions)]` eingebunden und
//! existiert in Release-Builds (`cargo build --release`, Godot-Export) überhaupt
//! nicht. Der TableController erzeugt das Panel per Code, deshalb steht in der
//! Szene kein Node, der im fertigen Spiel fehlen könnte.
//!
//! Oben: Sprachauswahl (Deutsch / English) für das ganze Spiel.
//!
//! Reiter:
//! - Ressourcen: beliebig viele Ressourcen-Karten je Farbe in die eigene Reihe.
//! - Karten:     jede mögliche Karte auf die Hand (Liste mit Suche).
//! - Erstellen:  eigene Karten (Name, Typ, Fähigkeit, Kosten, Angriff, Verteidigung).

use godot::classes::control::{GrowDirection, LayoutPreset, MouseFilter};
use godot::classes::{
    Button, CanvasLayer, Control, GridContainer, HBoxContainer, ICanvasLayer, ItemList, Label, LineEdit,
    OptionButton, PanelContainer, SpinBox, StyleBoxFlat, TabContainer, TextureRect, VBoxContainer,
};
use godot::classes::texture_rect::{ExpandMode, StretchMode};
use godot::global::{HorizontalAlignment, VerticalAlignment};
use godot::builtin::Side;
use godot::prelude::*;

use crate::game::ability::CardAbility;
use crate::game::cards::{
    self, ActivatedAbility, AuraDef, AuraEffect, AuraTarget, CardDef, CardKind, EquipStats, HeroAbility,
    SpellEffect, StaticEffect,
};
use crate::game::mana::{Cost, ManaType};
use crate::game::seat::PlayerSeat;
use crate::game::turn_manager::TurnManager;
use crate::table::table_controller::TableController;
use crate::table::placeholder;
use crate::table::zone::Zone;

/// Ressourcen-Knöpfe: (Beschriftung, Manatyp). Gelb = weißes Mana, Grau = farblos.
const RESOURCE_BUTTONS: [(&str, ManaType); 6] = [
    ("Blau", ManaType::Blue),
    ("Schwarz", ManaType::Black),
    ("Grün", ManaType::Green),
    ("Rot", ManaType::Red),
    ("Gelb", ManaType::White),
    ("Grau", ManaType::Colorless),
];

/// Beschriftung eines Ressourcen-Knopfs in der aktuellen Sprache.
fn resource_label(de: &str) -> &'static str {
    match de {
        "Blau" => tr!("Blau", "Blue"),
        "Schwarz" => tr!("Schwarz", "Black"),
        "Grün" => tr!("Grün", "Green"),
        "Rot" => tr!("Rot", "Red"),
        "Gelb" => tr!("Gelb", "Yellow"),
        _ => tr!("Grau", "Grey"),
    }
}

/// Kartentypen im "Erstellen"-Reiter (Reihenfolge = Index im Auswahlfeld).
const KINDS: [CardKind; 9] = [
    CardKind::Creature,
    CardKind::Resource,
    CardKind::Instant,
    CardKind::Sorcery,
    CardKind::Hero,
    CardKind::Equipment,
    CardKind::Artifact,
    CardKind::Enchantment,
    CardKind::Aura,
];

/// Fähigkeiten je Typ und was Angriff/Verteidigung bedeuten.
fn abilities_for(kind: CardKind) -> (&'static [&'static str], &'static str) {
    match kind {
        CardKind::Creature => (
            tr!(
                &["Keine", "Heiler: Auffrischung +1 Leben", "Feuer: Auffrischung 1 Schaden am Gegner", "Raufbold: +1 Angriff beim Angreifen"],
                &["None", "Healer: upkeep +1 life", "Fire: upkeep 1 damage to the opponent", "Brawler: +1 attack when attacking"]
            ),
            tr!("Angriff / Verteidigung der Kreatur.", "Attack / defense of the creature."),
        ),
        CardKind::Resource => (
            tr!(
                &["Gelb (Weiß)", "Blau", "Schwarz", "Rot", "Grün", "Grau (Farblos)"],
                &["Yellow (white)", "Blue", "Black", "Red", "Green", "Grey (colorless)"]
            ),
            tr!("Ressourcen kosten nichts; Angriff/Verteidigung werden ignoriert.", "Resources are free; attack/defense are ignored."),
        ),
        CardKind::Instant | CardKind::Sorcery => (
            tr!(
                &["Heilen", "Karten ziehen", "Schaden am Gegner", "Schaden an stärkster gegn. Kreatur", "Neutralisieren (Counterspell)"],
                &["Heal", "Draw cards", "Damage to opponent", "Damage to strongest enemy creature", "Counter (counterspell)"]
            ),
            tr!("Angriff = Stärke des Effekts (Leben, Karten, Schaden).", "Attack = strength of the effect (life, cards, damage)."),
        ),
        CardKind::Hero => (
            tr!(
                &["+1: Leben (Angriff)  /  −3: Schaden an Kreatur", "+1: Karte ziehen  /  −3: Schaden am Gegner (Angriff)"],
                &["+1: life (attack)  /  −3: damage to creature", "+1: draw a card  /  −3: damage to opponent (attack)"]
            ),
            tr!("Angriff = Stärke der Fähigkeiten, Verteidigung = Willenskraft.", "Attack = strength of the abilities, defense = willpower."),
        ),
        CardKind::Equipment => (
            tr!(&["Anlegen kostet 1", "Anlegen kostet 2", "Anlegen kostet 3"], &["Equip costs 1", "Equip costs 2", "Equip costs 3"]),
            tr!("Angriff / Verteidigung = Bonus für die ausgerüstete Kreatur.", "Attack / defense = bonus for the equipped creature."),
        ),
        CardKind::Artifact => (
            tr!(
                &["Passiv: deine Kreaturen +A/+V", "Tappen: +Angriff Leben", "Tappen: ziehe Angriff Karten", "Tappen: Angriff Schaden am Gegner"],
                &["Passive: your creatures +A/+D", "Tap: +attack life", "Tap: draw attack cards", "Tap: attack damage to opponent"]
            ),
            tr!("Angriff / Verteidigung = Bonus bzw. Stärke.", "Attack / defense = bonus or strength."),
        ),
        CardKind::Enchantment => (
            tr!(&["Deine Kreaturen +A/+V", "Gegnerische Kreaturen −A/−V"], &["Your creatures +A/+D", "Enemy creatures −A/−D"]),
            tr!("Angriff / Verteidigung = globaler Bonus.", "Attack / defense = global bonus."),
        ),
        CardKind::Aura => (
            tr!(
                &[
                    "Verzaubert Kreatur: ±A/±V",
                    "Verzaubert Ressource: +Angriff Mana",
                    "Verzaubert bleibende Karte: enttappt nicht",
                    "Verzaubert Held: +Verteidigung Willenskraft",
                ],
                &[
                    "Enchant creature: ±A/±D",
                    "Enchant resource: +attack mana",
                    "Enchant permanent: does not untap",
                    "Enchant hero: +defense willpower",
                ]
            ),
            tr!("Angriff / Verteidigung = Wirkung (negativ = Fluch).", "Attack / defense = effect (negative = curse)."),
        ),
    }
}

/// Baut eine Kartendefinition aus den Eingaben der Maske.
fn build_def(name: &str, kind: CardKind, ability: usize, cost: Cost, atk: i32, def: i32) -> CardDef {
    let name = if name.trim().is_empty() { "Eigene Karte" } else { name.trim() };
    let cost = if kind == CardKind::Resource { Cost::default() } else { cost };
    let mut d = CardDef::base(name, kind, cost);
    let n = atk.max(1);
    match kind {
        CardKind::Creature => {
            d.attack = atk.max(0);
            d.defense = def.max(1);
            d.ability = [CardAbility::None, CardAbility::Regeneration, CardAbility::Firebreath, CardAbility::Rage]
                .get(ability)
                .copied()
                .unwrap_or_default();
        }
        CardKind::Resource => {
            d.produces = [
                ManaType::White,
                ManaType::Blue,
                ManaType::Black,
                ManaType::Red,
                ManaType::Green,
                ManaType::Colorless,
            ][ability.min(5)];
        }
        CardKind::Instant | CardKind::Sorcery => {
            d.effect = match ability {
                0 => SpellEffect::Heal(n),
                1 => SpellEffect::Draw(n),
                2 => SpellEffect::DamageOpponent(n),
                3 => SpellEffect::DamageStrongestCreature(n),
                _ => SpellEffect::Counter,
            };
        }
        CardKind::Hero => {
            d.willpower = def.max(1);
            d.hero_abilities = if ability == 0 {
                vec![
                    HeroAbility { delta: 1, effect: SpellEffect::Heal(n) },
                    HeroAbility { delta: -3, effect: SpellEffect::DamageStrongestCreature(n + 2) },
                ]
            } else {
                vec![
                    HeroAbility { delta: 1, effect: SpellEffect::Draw(1) },
                    HeroAbility { delta: -3, effect: SpellEffect::DamageOpponent(n) },
                ]
            };
        }
        CardKind::Equipment => {
            d.equip = Some(EquipStats {
                attack: atk,
                defense: def,
                cost: Cost { generic: ability as u32 + 1, ..Cost::default() },
            });
        }
        CardKind::Artifact => match ability {
            0 => d.static_effect = Some(StaticEffect::OwnCreatures(atk, def)),
            1 => d.activated = Some(ActivatedAbility { cost: Cost::default(), effect: SpellEffect::Heal(n) }),
            2 => d.activated = Some(ActivatedAbility { cost: Cost::default(), effect: SpellEffect::Draw(n) }),
            _ => d.activated = Some(ActivatedAbility { cost: Cost::default(), effect: SpellEffect::DamageOpponent(n) }),
        },
        CardKind::Enchantment => {
            d.static_effect = Some(if ability == 0 {
                StaticEffect::OwnCreatures(atk, def)
            } else {
                StaticEffect::OpponentCreatures(-atk.abs(), -def.abs())
            });
        }
        CardKind::Aura => {
            d.aura = Some(match ability {
                0 => AuraDef { target: AuraTarget::Creature, effect: AuraEffect::Stats(atk, def) },
                1 => AuraDef { target: AuraTarget::Resource, effect: AuraEffect::ExtraMana(n as u32) },
                2 => AuraDef { target: AuraTarget::Permanent, effect: AuraEffect::NoUntap },
                _ => AuraDef { target: AuraTarget::Hero, effect: AuraEffect::Willpower(def.max(1)) },
            });
        }
    }
    d
}

/// Debug-Tool (nur in Test-Builds): Sprache umschalten (Deutsch / English), Ressourcen
/// hinzufügen, jede Karte auf die Hand geben und eigene Karten erstellen. Wird vom [TableController] automatisch erzeugt und
/// existiert in Release-Builds (`cargo build --release`) nicht.
#[derive(GodotClass)]
#[class(base=CanvasLayer)]
pub struct DebugPanel {
    controller: Option<Gd<TableController>>,
    turn_manager: Option<Gd<TurnManager>>,

    catalog: Vec<CardDef>,
    /// Indizes in `catalog`, die der Suchfilter gerade zeigt.
    shown: Vec<usize>,

    window: Option<Gd<PanelContainer>>,
    amount: Option<Gd<SpinBox>>,
    status: Option<Gd<Label>>,
    filter: Option<Gd<LineEdit>>,
    list: Option<Gd<ItemList>>,
    name_edit: Option<Gd<LineEdit>>,
    kind_select: Option<Gd<OptionButton>>,
    ability_select: Option<Gd<OptionButton>>,
    cost_edit: Option<Gd<LineEdit>>,
    attack: Option<Gd<SpinBox>>,
    defense: Option<Gd<SpinBox>>,
    hint: Option<Gd<Label>>,
    tabs: Option<Gd<TabContainer>>,
    preview: Option<Preview>,
    /// Oberstes Control der Oberfläche (wird beim Sprachwechsel neu aufgebaut).
    root: Option<Gd<Control>>,

    base: Base<CanvasLayer>,
}

#[godot_api]
impl ICanvasLayer for DebugPanel {
    fn init(base: Base<CanvasLayer>) -> Self {
        Self {
            controller: None,
            turn_manager: None,
            catalog: cards::catalog(),
            shown: Vec::new(),
            window: None,
            amount: None,
            status: None,
            filter: None,
            list: None,
            name_edit: None,
            kind_select: None,
            ability_select: None,
            cost_edit: None,
            attack: None,
            defense: None,
            hint: None,
            tabs: None,
            preview: None,
            root: None,
            base,
        }
    }

    fn ready(&mut self) {
        self.base_mut().set_layer(20);
        self.build();
        self.refresh_list();
        self.on_kind_selected(0);
    }
}

#[godot_api]
impl DebugPanel {
    /// Fenster ein-/ausblenden.
    #[func]
    fn on_toggle(&mut self) {
        if let Some(w) = self.window.as_mut() {
            let visible = w.is_visible();
            w.set_visible(!visible);
        }
        self.update_preview_visibility();
    }

    /// Sprache gewählt (0 = Deutsch, 1 = English): stellt das ganze Spiel um,
    /// beschriftet alle Karten neu und baut das Debug-Fenster in der neuen Sprache auf.
    #[func]
    fn on_language_selected(&mut self, index: i64) {
        if let Some(mut c) = self.controller.clone() {
            c.bind_mut().set_language(if index == 1 { "en" } else { "de" }.into());
        }
        // Oberfläche neu aufbauen, offenen Reiter beibehalten
        let tab = self.tabs.as_ref().map_or(0, |t| t.get_current_tab());
        if let Some(mut old) = self.root.take() {
            old.queue_free();
        }
        self.build();
        if let Some(w) = self.window.as_mut() {
            w.set_visible(true);
        }
        if let Some(t) = self.tabs.as_mut() {
            t.set_current_tab(tab);
        }
        self.refresh_list();
        self.on_kind_selected(0);
        self.update_preview_visibility();
        self.set_status(tr!("Sprache: Deutsch", "Language: English"));
    }

    /// Karte in der Liste ausgewählt: große Vorschau zeigen.
    #[func]
    fn on_catalog_selected(&mut self, index: i64) {
        if let Some(&i) = self.shown.get(index.max(0) as usize) {
            let def = self.catalog[i].clone();
            if let Some(p) = self.preview.as_mut() {
                p.show(&def);
            }
        }
        self.update_preview_visibility();
    }

    /// Reiter gewechselt: blendet die große Kartenvorschau ein oder aus.
    #[func]
    fn on_tab_changed(&mut self, _tab: i64) {
        self.update_preview_visibility();
    }

    /// "+"-Knopf: `index` in RESOURCE_BUTTONS.
    #[func]
    fn on_add_resource(&mut self, index: i64) {
        let (label, t) = RESOURCE_BUTTONS[index.clamp(0, 5) as usize];
        let label = resource_label(label);
        let count = self.amount.as_ref().map_or(1, |a| a.get_value() as i32);
        let Some(zone) = self.player_seat().and_then(|s| s.bind().resource_zone.clone()) else {
            return self.set_status(tr!("Keine Ressourcen-Reihe gefunden.", "No resource row found."));
        };
        let mut def = CardDef::base(cards::resource_name(t), CardKind::Resource, Cost::default());
        def.produces = t;
        for _ in 0..count {
            self.spawn(&def, zone.clone());
        }
        self.set_status(&trf!("+{count} Ressource(n) {label}.", "+{count} resource(s) {label}."));
    }

    /// Knopf „Alle Ressourcen enttappen“: enttappt alle Karten der eigenen Ressourcen-Reihe.
    #[func]
    fn on_untap_resources(&mut self) {
        let Some(mut zone) = self.player_seat().and_then(|s| s.bind().resource_zone.clone()) else {
            return;
        };
        for mut c in zone.bind().cards() {
            c.bind_mut().set_tapped(false);
        }
        zone.bind_mut().layout();
        self.set_status(tr!("Alle Ressourcen enttappt.", "All resources untapped."));
    }

    /// Suchfeld geändert: filtert die Kartenliste nach Name oder Typ.
    #[func]
    fn on_filter_changed(&mut self, _text: GString) {
        self.refresh_list();
    }

    /// Doppelklick in der Kartenliste.
    #[func]
    fn on_catalog_activated(&mut self, index: i64) {
        self.give_to_hand(index);
    }

    /// Knopf „Auf die Hand“: legt die in der Liste gewählte Karte auf die Hand.
    #[func]
    fn on_give_selected(&mut self) {
        let selected = self.list.as_ref().map(|l| l.get_selected_items()).unwrap_or_default();
        match selected.as_slice().first() {
            Some(&i) => self.give_to_hand(i as i64),
            None => self.set_status(tr!("Bitte zuerst eine Karte in der Liste wählen.", "Please select a card in the list first.")),
        }
    }

    /// Typ gewechselt: passende Fähigkeiten und Hinweis anzeigen.
    #[func]
    fn on_kind_selected(&mut self, index: i64) {
        let kind = KINDS[index.clamp(0, KINDS.len() as i64 - 1) as usize];
        let (abilities, hint) = abilities_for(kind);
        if let Some(sel) = self.ability_select.as_mut() {
            sel.clear();
            for a in abilities {
                sel.add_item(*a);
            }
            sel.select(0);
        }
        if let Some(h) = self.hint.as_mut() {
            h.set_text(hint);
        }
    }

    /// Knopf „Erstellen“: baut eine Karte aus Name, Typ, Fähigkeit, Kosten, Angriff und
    /// Verteidigung. `to_hand` = true legt sie zusätzlich direkt auf die Hand.
    #[func]
    fn on_create(&mut self, to_hand: bool) {
        let name = self.name_edit.as_ref().map(|e| e.get_text().to_string()).unwrap_or_default();
        let kind = KINDS[self.kind_select.as_ref().map_or(0, |k| k.get_selected().max(0) as usize)];
        let ability = self.ability_select.as_ref().map_or(0, |a| a.get_selected().max(0) as usize);
        let cost_text = self.cost_edit.as_ref().map(|e| e.get_text().to_string()).unwrap_or_default();
        let cost = match Cost::parse(&cost_text) {
            Ok(c) => c,
            Err(_) => return self.set_status(tr!("Kosten ungültig. Beispiel: 2RG (W B S R G F).", "Invalid cost. Example: 2RG (W B S R G F).")),
        };
        let atk = self.attack.as_ref().map_or(0, |s| s.get_value() as i32);
        let def = self.defense.as_ref().map_or(0, |s| s.get_value() as i32);
        let card = build_def(&name, kind, ability, cost, atk, def);
        let label = format!("{} ({})", crate::i18n::name(&card.name), kind.title());
        self.catalog.push(card);
        self.refresh_list();
        if to_hand {
            let index = self.shown.iter().position(|&i| i == self.catalog.len() - 1);
            match index {
                Some(i) => self.give_to_hand(i as i64),
                None => self.give_catalog_entry(self.catalog.len() - 1),
            }
        } else {
            self.set_status(&trf!("{label} erstellt – steht jetzt in der Kartenliste.", "{label} created – now in the card list."));
        }
    }
}

impl DebugPanel {
    pub fn init_refs(&mut self, controller: Gd<TableController>, turn_manager: Option<Gd<TurnManager>>) {
        self.controller = Some(controller);
        self.turn_manager = turn_manager;
    }

    fn player_seat(&self) -> Option<Gd<PlayerSeat>> {
        let hand = self.controller.as_ref()?.bind().hand()?;
        hand.get_parent()?.try_cast::<PlayerSeat>().ok()
    }

    fn spawn(&mut self, def: &CardDef, zone: Gd<Zone>) {
        if let Some(mut c) = self.controller.clone() {
            c.bind_mut().spawn_card(def, 0, zone);
        }
    }

    fn give_to_hand(&mut self, list_index: i64) {
        if let Some(&i) = self.shown.get(list_index.max(0) as usize) {
            self.give_catalog_entry(i);
        }
    }

    fn give_catalog_entry(&mut self, i: usize) {
        let Some(hand) = self.controller.as_ref().and_then(|c| c.bind().hand()) else {
            return;
        };
        let def = self.catalog[i].clone();
        self.spawn(&def, hand);
        let shown = crate::i18n::name(&def.name);
        self.set_status(&trf!("{shown} auf die Hand gelegt.", "{shown} added to your hand."));
    }

    fn set_status(&mut self, text: &str) {
        if let Some(l) = self.status.as_mut() {
            l.set_text(text);
        }
    }

    fn refresh_list(&mut self) {
        let filter = self.filter.as_ref().map(|f| f.get_text().to_string().to_lowercase()).unwrap_or_default();
        self.shown = (0..self.catalog.len())
            .filter(|&i| {
                let d = &self.catalog[i];
                filter.is_empty()
                    || d.name.to_lowercase().contains(&filter)
                    || crate::i18n::name(&d.name).to_lowercase().contains(&filter)
                    || d.kind.title().to_lowercase().contains(&filter)
            })
            .collect();
        let entries: Vec<String> = self
            .shown
            .iter()
            .map(|&i| {
                let d = &self.catalog[i];
                let stats = match d.kind {
                    CardKind::Creature => format!("  {}/{}", d.attack, d.defense),
                    _ => String::new(),
                };
                let cost = if d.kind == CardKind::Resource { d.produces.name().to_string() } else { d.cost.to_text() };
                format!("{}  [{}, {}]{}", crate::i18n::name(&d.name), d.kind.title(), cost, stats)
            })
            .collect();
        if let Some(list) = self.list.as_mut() {
            list.clear();
            for e in entries {
                list.add_item(&e);
            }
        }
    }

    /// Vorschau nur bei offenem Fenster, Reiter "Karten" und gewählter Karte.
    fn update_preview_visibility(&mut self) {
        let open = self.window.as_ref().is_some_and(|w| w.is_visible());
        let cards_tab = self.tabs.as_ref().is_some_and(|t| t.get_current_tab() == 1);
        if let Some(p) = self.preview.as_mut() {
            let has = p.has_card;
            p.panel.set_visible(open && cards_tab && has);
        }
    }

    fn method(&self, name: &str) -> Callable {
        Callable::from_object_method(&self.to_gd(), name)
    }

    fn build(&mut self) {
        let mut root = Control::new_alloc();
        self.root = Some(root.clone());
        root.set_anchors_preset(LayoutPreset::FULL_RECT);
        root.set_mouse_filter(MouseFilter::IGNORE);

        // Seitlicher Button
        let mut toggle = Button::new_alloc();
        toggle.set_text("DEBUG");
        toggle.set_focus_mode(godot::classes::control::FocusMode::NONE);
        toggle.add_theme_color_override("font_color", Color::from_rgb(1.0, 0.85, 0.3));
        toggle.add_theme_stylebox_override("normal", &style(Color::from_rgba(0.35, 0.05, 0.05, 0.9)));
        toggle.add_theme_stylebox_override("hover", &style(Color::from_rgba(0.5, 0.08, 0.08, 0.95)));
        toggle.set_custom_minimum_size(Vector2::new(84.0, 36.0));
        toggle.connect("pressed", &self.method("on_toggle"));
        place(&mut toggle, (0.0, 0.5), (14.0, -150.0), GrowDirection::END, GrowDirection::END);
        root.add_child(&toggle);

        // Fenster mit Reitern
        let mut tabs = TabContainer::new_alloc();
        tabs.set_custom_minimum_size(Vector2::new(420.0, 470.0));
        tabs.add_child(&self.build_resources_tab());
        tabs.add_child(&self.build_cards_tab());
        tabs.add_child(&self.build_create_tab());
        tabs.connect("tab_changed", &self.method("on_tab_changed"));
        self.tabs = Some(tabs.clone());

        let mut col = VBoxContainer::new_alloc();
        let mut lang_row = HBoxContainer::new_alloc();
        lang_row.add_child(&text_label("Sprache / Language:"));
        let mut lang = OptionButton::new_alloc();
        lang.add_item("Deutsch");
        lang.add_item("English");
        lang.select(if crate::i18n::lang() == crate::i18n::Lang::En { 1 } else { 0 });
        lang.connect("item_selected", &self.method("on_language_selected"));
        lang_row.add_child(&lang);
        col.add_child(&lang_row);
        col.add_child(&tabs);
        let mut status = Label::new_alloc();
        status.set_text(tr!("Debug-Tool (nur Test-Build)", "Debug tool (test build only)"));
        status.add_theme_color_override("font_color", Color::from_rgb(1.0, 0.85, 0.3));
        col.add_child(&status);
        self.status = Some(status);

        let mut window = PanelContainer::new_alloc();
        window.add_theme_stylebox_override("panel", &style(Color::from_rgba(0.08, 0.05, 0.04, 0.95)));
        window.add_child(&col);
        window.set_visible(false);
        place(&mut window, (0.0, 0.5), (106.0, -250.0), GrowDirection::END, GrowDirection::END);
        root.add_child(&window);
        self.window = Some(window);

        // Große Kartenvorschau rechts neben dem Fenster
        let preview = Preview::build();
        let mut panel = preview.panel.clone();
        place(&mut panel, (0.0, 0.5), (556.0, -250.0), GrowDirection::END, GrowDirection::END);
        root.add_child(&panel);
        self.preview = Some(preview);

        self.base_mut().add_child(&root);
    }

    fn build_resources_tab(&mut self) -> Gd<VBoxContainer> {
        let mut tab = VBoxContainer::new_alloc();
        tab.set_name(tr!("Ressourcen", "Resources"));
        tab.add_theme_constant_override("separation", 8);

        let mut row = HBoxContainer::new_alloc();
        row.add_child(&text_label(tr!("Anzahl pro Klick:", "Amount per click:")));
        let mut amount = SpinBox::new_alloc();
        amount.set_min(1.0);
        amount.set_max(20.0);
        amount.set_value(1.0);
        row.add_child(&amount);
        tab.add_child(&row);
        self.amount = Some(amount);

        let mut grid = GridContainer::new_alloc();
        grid.set_columns(2);
        for (i, (label, t)) in RESOURCE_BUTTONS.into_iter().enumerate() {
            let mut b = Button::new_alloc();
            b.set_text(&format!("+  {}", resource_label(label)));
            b.set_custom_minimum_size(Vector2::new(190.0, 40.0));
            b.add_theme_stylebox_override("normal", &style(t.color().darkened(0.35)));
            b.add_theme_stylebox_override("hover", &style(t.color().darkened(0.15)));
            let text = if t == ManaType::White { Color::from_rgb(0.1, 0.1, 0.1) } else { Color::WHITE };
            b.add_theme_color_override("font_color", text);
            b.add_theme_color_override("font_hover_color", text);
            b.connect("pressed", &self.method("on_add_resource").bind(&[(i as i64).to_variant()]));
            grid.add_child(&b);
        }
        tab.add_child(&grid);

        let mut untap = Button::new_alloc();
        untap.set_text(tr!("Alle Ressourcen enttappen", "Untap all resources"));
        untap.connect("pressed", &self.method("on_untap_resources"));
        tab.add_child(&untap);
        tab.add_child(&text_label(tr!(
            "Gelb = weißes Mana, Grau = farblos.\nDie Karten landen direkt in deiner Ressourcen-Reihe\n(zählen nicht zum Limit 1 pro Runde).",
            "Yellow = white mana, grey = colorless.\nThe cards go straight into your resource row\n(they do not count towards the limit of 1 per turn)."
        )));
        tab
    }

    fn build_cards_tab(&mut self) -> Gd<VBoxContainer> {
        let mut tab = VBoxContainer::new_alloc();
        tab.set_name(tr!("Karten", "Cards"));
        let mut filter = LineEdit::new_alloc();
        filter.set_placeholder(tr!("Suchen (Name oder Typ) …", "Search (name or type) …"));
        filter.connect("text_changed", &self.method("on_filter_changed"));
        tab.add_child(&filter);
        self.filter = Some(filter);

        let mut list = ItemList::new_alloc();
        list.set_custom_minimum_size(Vector2::new(0.0, 330.0));
        list.set_v_size_flags(godot::classes::control::SizeFlags::EXPAND_FILL);
        list.connect("item_activated", &self.method("on_catalog_activated"));
        list.connect("item_selected", &self.method("on_catalog_selected"));
        tab.add_child(&list);
        self.list = Some(list);

        let mut give = Button::new_alloc();
        give.set_text(tr!("Auf die Hand  (oder Doppelklick)", "To hand  (or double-click)"));
        give.connect("pressed", &self.method("on_give_selected"));
        tab.add_child(&give);
        tab
    }

    fn build_create_tab(&mut self) -> Gd<VBoxContainer> {
        let mut tab = VBoxContainer::new_alloc();
        tab.set_name(tr!("Erstellen", "Create"));
        let mut grid = GridContainer::new_alloc();
        grid.set_columns(2);

        grid.add_child(&text_label(tr!("Name:", "Name:")));
        let mut name = LineEdit::new_alloc();
        name.set_placeholder(tr!("z. B. Taverntroll", "e.g. Tavern Troll"));
        name.set_custom_minimum_size(Vector2::new(260.0, 0.0));
        grid.add_child(&name);
        self.name_edit = Some(name);

        grid.add_child(&text_label(tr!("Typ:", "Type:")));
        let mut kind = OptionButton::new_alloc();
        for k in KINDS {
            kind.add_item(k.title());
        }
        kind.connect("item_selected", &self.method("on_kind_selected"));
        grid.add_child(&kind);
        self.kind_select = Some(kind);

        grid.add_child(&text_label(tr!("Fähigkeit:", "Ability:")));
        let ability = OptionButton::new_alloc();
        grid.add_child(&ability);
        self.ability_select = Some(ability);

        grid.add_child(&text_label(tr!("Kosten:", "Cost:")));
        let mut cost = LineEdit::new_alloc();
        cost.set_placeholder(tr!("z. B. 2RG  (W B S R G F)", "e.g. 2RG  (W B S R G F)"));
        grid.add_child(&cost);
        self.cost_edit = Some(cost);

        grid.add_child(&text_label(tr!("Angriff:", "Attack:")));
        let attack = spin(-10.0, 20.0, 2.0);
        grid.add_child(&attack);
        self.attack = Some(attack);

        grid.add_child(&text_label(tr!("Verteidigung:", "Defense:")));
        let defense = spin(-10.0, 20.0, 2.0);
        grid.add_child(&defense);
        self.defense = Some(defense);
        tab.add_child(&grid);

        let mut hint = text_label("");
        hint.set_autowrap_mode(godot::classes::text_server::AutowrapMode::WORD_SMART);
        tab.add_child(&hint);
        self.hint = Some(hint);

        let mut row = HBoxContainer::new_alloc();
        let mut create = Button::new_alloc();
        create.set_text(tr!("Erstellen", "Create"));
        create.connect("pressed", &self.method("on_create").bind(&[false.to_variant()]));
        row.add_child(&create);
        let mut create_hand = Button::new_alloc();
        create_hand.set_text(tr!("Erstellen + auf die Hand", "Create + to hand"));
        create_hand.connect("pressed", &self.method("on_create").bind(&[true.to_variant()]));
        row.add_child(&create_hand);
        tab.add_child(&row);
        tab
    }
}

/// Große Kartenansicht: Kartenbild (256 × 358 px) mit Beschriftung an denselben
/// Stellen wie auf der 3D-Karte, darunter die Regel-Erklärung.
struct Preview {
    panel: Gd<PanelContainer>,
    texture: Gd<TextureRect>,
    name: Gd<Label>,
    cost: Gd<Label>,
    kind: Gd<Label>,
    body: Gd<Label>,
    stats: Gd<Label>,
    rules: Gd<Label>,
    has_card: bool,
}

impl Preview {
    fn build() -> Self {
        let light = Color::from_rgb(0.97, 0.93, 0.85);
        let dark = Color::from_rgb(0.18, 0.12, 0.08);

        let mut card = Control::new_alloc();
        card.set_custom_minimum_size(Vector2::new(256.0, 358.0));
        card.set_mouse_filter(MouseFilter::IGNORE);
        let mut texture = TextureRect::new_alloc();
        texture.set_expand_mode(ExpandMode::IGNORE_SIZE);
        texture.set_stretch_mode(StretchMode::SCALE);
        texture.set_size(Vector2::new(256.0, 358.0));
        card.add_child(&texture);

        let mut overlay = |pos: Vector2, size: Vector2, font: i32, color: Color, wrap: bool| -> Gd<Label> {
            let mut l = Label::new_alloc();
            l.set_horizontal_alignment(HorizontalAlignment::CENTER);
            l.set_vertical_alignment(VerticalAlignment::CENTER);
            l.add_theme_font_size_override("font_size", font);
            l.add_theme_color_override("font_color", color);
            if wrap {
                l.set_autowrap_mode(godot::classes::text_server::AutowrapMode::WORD_SMART);
            }
            l.set_position(pos - size * 0.5);
            l.set_size(size);
            card.add_child(&l);
            l
        };
        let name = overlay(Vector2::new(110.0, 36.0), Vector2::new(170.0, 28.0), 15, light, false);
        let cost = overlay(Vector2::new(222.0, 36.0), Vector2::new(24.0, 24.0), 13, dark, false);
        let kind = overlay(Vector2::new(128.0, 219.0), Vector2::new(216.0, 22.0), 12, light, false);
        let body = overlay(Vector2::new(128.0, 280.0), Vector2::new(206.0, 84.0), 12, dark, true);
        let stats = overlay(Vector2::new(206.0, 334.0), Vector2::new(60.0, 28.0), 16, Color::WHITE, false);

        let mut rules = Label::new_alloc();
        rules.set_autowrap_mode(godot::classes::text_server::AutowrapMode::WORD_SMART);
        rules.set_custom_minimum_size(Vector2::new(300.0, 0.0));
        rules.add_theme_font_size_override("font_size", 13);

        let mut col = VBoxContainer::new_alloc();
        col.add_theme_constant_override("separation", 10);
        let mut center = HBoxContainer::new_alloc();
        center.set_alignment(godot::classes::box_container::AlignmentMode::CENTER);
        center.add_child(&card);
        col.add_child(&center);
        col.add_child(&rules);

        let mut panel = PanelContainer::new_alloc();
        panel.add_theme_stylebox_override("panel", &style(Color::from_rgba(0.08, 0.05, 0.04, 0.95)));
        panel.add_child(&col);
        panel.set_visible(false);
        Self { panel, texture, name, cost, kind, body, stats, rules, has_card: false }
    }

    fn show(&mut self, def: &CardDef) {
        self.has_card = true;
        self.texture.set_texture(&placeholder::card_face(def));
        let (kind, body, stats) = def.face_texts();
        let ty = placeholder::title_center_y();
        // Kostensymbole wie auf der Karte: Zahl im grauen Symbol ganz links
        let pips = def.cost.specific.total() as usize + usize::from(def.cost.generic > 0);
        let centers = placeholder::pip_centers(pips);
        let show_cost = def.kind != CardKind::Resource && def.cost.generic > 0;
        self.cost.set_text(&if show_cost { def.cost.generic.to_string() } else { String::new() });
        if show_cost {
            self.cost.set_position(Vector2::new(centers[0] - 12.0, ty - 12.0));
        }
        let left = if def.kind == CardKind::Resource { 236.0 } else { centers.first().copied().unwrap_or(236.0) };
        let width = left - 16.0 - 26.0;
        self.name.set_size(Vector2::new(width, 28.0));
        self.name.set_position(Vector2::new(26.0, ty - 14.0));
        self.name.set_text(&crate::i18n::name(&def.name));
        self.kind.set_text(&kind);
        self.body.set_text(&body);
        self.stats.set_text(&stats);
        self.rules.set_text(&def.rules_text());
    }
}

fn text_label(text: &str) -> Gd<Label> {
    let mut l = Label::new_alloc();
    l.set_text(text);
    l
}

fn spin(min: f64, max: f64, value: f64) -> Gd<SpinBox> {
    let mut s = SpinBox::new_alloc();
    s.set_min(min);
    s.set_max(max);
    s.set_value(value);
    s
}

fn style(bg: Color) -> Gd<StyleBoxFlat> {
    let mut s = StyleBoxFlat::new_gd();
    s.set_bg_color(bg);
    s.set_border_color(Color::from_rgba(0.9, 0.7, 0.3, 0.9));
    s.set_border_width_all(2);
    s.set_corner_radius_all(6);
    s.set_content_margin_all(8.0);
    s
}

/// Verankert ein Control am Bildschirmrand (anchor 0/0.5/1) mit Abstand `offset`.
fn place<T: Inherits<Control>>(c: &mut Gd<T>, anchor: (f32, f32), offset: (f32, f32), grow_h: GrowDirection, grow_v: GrowDirection) {
    let mut c = c.clone().upcast::<Control>();
    for (side, a, o) in [
        (Side::LEFT, anchor.0, offset.0),
        (Side::RIGHT, anchor.0, offset.0),
        (Side::TOP, anchor.1, offset.1),
        (Side::BOTTOM, anchor.1, offset.1),
    ] {
        c.set_anchor(side, a);
        c.set_offset(side, o);
    }
    c.set_h_grow_direction(grow_h);
    c.set_v_grow_direction(grow_v);
}
