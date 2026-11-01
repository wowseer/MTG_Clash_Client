//! LIFO-Stack für Zauber und Fähigkeiten mit Prioritätsschleife.
//!
//! - [`StackItem`] (RefCounted): Quelle, Ziel, Controller und eine `Callable`,
//!   die beim Auflösen mit dem Item als Argument aufgerufen wird: `func(item): ...`.
//! - [`StackManager`] (Node): `push`, `resolve_next`, `pass_priority`.
//! - [`PriorityCore`]: reine Logik der Prioritätsschleife (per `cargo test` geprüft).
//!
//! Prioritätsregeln:
//! 1. Wer etwas auf den Stack legt, behält die Priorität (kann weiter reagieren).
//! 2. `pass_priority(spieler)` gibt die Priorität an den anderen Spieler weiter.
//! 3. Passen **beide nacheinander** (ohne neues Item dazwischen), wird das oberste
//!    Item aufgelöst. Danach erhält der aktive Spieler wieder die Priorität.
//! 4. Passen beide bei leerem Stack, meldet der Manager das (die Phase darf enden).
//!
//! Ein Item "verpufft" (wird ohne Wirkung entfernt), wenn sein Ziel ein
//! freigegebenes Objekt ist oder seine optionale `condition` false liefert.

use godot::prelude::*;

// ================================================================== Reine Logik

/// Ergebnis von `pass_priority`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PassResult {
    /// Der Spieler war nicht an der Reihe.
    NotYourPriority,
    /// Priorität ging an den anderen Spieler.
    Passed,
    /// Beide haben gepasst: oberstes Item muss jetzt aufgelöst werden.
    ResolveTop,
    /// Beide haben bei leerem Stack gepasst.
    BothPassedEmpty,
}

impl PassResult {
    pub fn code(self) -> i64 {
        match self {
            PassResult::NotYourPriority => -1,
            PassResult::Passed => 0,
            PassResult::ResolveTop => 1,
            PassResult::BothPassedEmpty => 2,
        }
    }
}

/// Stack + Prioritätszustand, generisch über den Item-Typ (für Tests ohne Godot).
#[derive(Debug)]
pub struct PriorityCore<T> {
    pub items: Vec<T>,
    /// Spieler, der gerade Priorität hat (0/1).
    pub priority: usize,
    /// Spieler, der am Zug ist.
    pub active: usize,
    /// Aufeinanderfolgende Pässe seit der letzten Änderung.
    pub passes: u32,
}

impl<T> Default for PriorityCore<T> {
    fn default() -> Self {
        Self { items: Vec::new(), priority: 0, active: 0, passes: 0 }
    }
}

impl<T> PriorityCore<T> {
    /// Neuer Zug / neue Phase: aktiver Spieler erhält Priorität.
    pub fn set_active(&mut self, player: usize) {
        self.active = player % 2;
        self.priority = self.active;
        self.passes = 0;
    }

    /// Item oben auflegen; der Controller behält die Priorität.
    pub fn push(&mut self, item: T, controller: usize) {
        self.items.push(item);
        self.priority = controller % 2;
        self.passes = 0;
    }

    pub fn pass(&mut self, player: usize) -> PassResult {
        if player % 2 != self.priority {
            return PassResult::NotYourPriority;
        }
        self.passes += 1;
        self.priority = 1 - self.priority;
        if self.passes < 2 {
            return PassResult::Passed;
        }
        if self.items.is_empty() {
            self.passes = 0;
            self.priority = self.active;
            PassResult::BothPassedEmpty
        } else {
            PassResult::ResolveTop
        }
    }

    /// Oberstes Item entnehmen (LIFO). Danach hat der aktive Spieler Priorität.
    pub fn pop(&mut self) -> Option<T> {
        let item = self.items.pop();
        self.priority = self.active;
        self.passes = 0;
        item
    }
}

// ================================================================== StackItem

/// Ein Zauber oder eine Fähigkeit auf dem Stack.
#[derive(GodotClass)]
#[class(base=RefCounted, init)]
pub struct StackItem {
    /// Ausführende Karte oder Fähigkeit.
    #[var]
    pub(crate) source: Variant,
    /// Ziel des Effekts (Karte, Spieler-Index, … oder null).
    #[var]
    pub(crate) target: Variant,
    /// Wird beim Auflösen aufgerufen: `func(item: StackItem)`.
    #[var]
    #[init(val = Callable::invalid())]
    pub(crate) effect: Callable,
    /// Optional: Prüfung direkt vor dem Auflösen, `func(item) -> bool`.
    /// false = Item verpufft ohne Wirkung (z. B. Ziel hat das Spielfeld verlassen).
    #[var]
    #[init(val = Callable::invalid())]
    pub(crate) condition: Callable,
    /// Spieler, der das Item kontrolliert (0/1).
    #[var]
    pub(crate) controller: i64,
    /// Text für Anzeige und Protokoll.
    #[var]
    pub(crate) description: GString,
    /// Freie Zusatzdaten (z. B. Index einer Heldenfähigkeit).
    #[var]
    pub(crate) payload: Variant,
    base: Base<RefCounted>,
}

#[godot_api]
impl StackItem {
    /// Bequemer Konstruktor: `StackItem.create(quelle, ziel, effekt, controller, text)`.
    #[func]
    pub fn create(source: Variant, target: Variant, effect: Callable, controller: i64, description: GString) -> Gd<StackItem> {
        let mut item = StackItem::new_gd();
        {
            let mut i = item.bind_mut();
            i.source = source;
            i.target = target;
            i.effect = effect;
            i.controller = controller;
            i.description = description;
        }
        item
    }

    /// Ist das Ziel noch gültig (nicht freigegeben) und die Bedingung erfüllt?
    #[func]
    pub fn is_still_valid(&self) -> bool {
        if self.target.get_type() == VariantType::OBJECT {
            let alive = self.target.try_to::<Gd<Object>>().is_ok_and(|o| o.is_instance_valid());
            if !alive {
                return false;
            }
        }
        if self.condition.is_valid() {
            let me = self.to_gd().to_variant();
            return self.condition.call(&[me]).booleanize();
        }
        true
    }
}

// ================================================================== StackManager

/// Verwaltet den Stack und die Prioritätsschleife.
#[derive(GodotClass)]
#[class(base=Node, init)]
pub struct StackManager {
    core: PriorityCore<Gd<StackItem>>,
    base: Base<Node>,
}

#[godot_api]
impl StackManager {
    #[signal]
    fn item_pushed(item: Gd<StackItem>);
    #[signal]
    fn item_resolved(item: Gd<StackItem>);
    /// Item wurde ohne Wirkung entfernt (Ziel ungültig / Bedingung false).
    #[signal]
    fn item_fizzled(item: Gd<StackItem>);
    #[signal]
    fn priority_changed(player: i64);
    /// Beide Spieler haben bei leerem Stack gepasst.
    #[signal]
    fn all_passed();
    /// Item wurde von einem anderen Effekt neutralisiert (Counterspell).
    #[signal]
    fn item_countered(item: Gd<StackItem>);

    /// Entfernt ein bestimmtes Item ohne Auflösung (Counterspell). true = gefunden.
    #[func]
    pub fn counter_item(&mut self, item: Gd<StackItem>) -> bool {
        let Some(pos) = self.core.items.iter().position(|i| i == &item) else {
            return false;
        };
        self.core.items.remove(pos);
        self.core.passes = 0;
        self.signals().item_countered().emit(&item);
        true
    }

    /// Legt ein Item oben auf den Stack. Sein Controller behält die Priorität.
    #[func]
    pub fn push(&mut self, item: Gd<StackItem>) {
        let controller = item.bind().controller.clamp(0, 1) as usize;
        self.core.push(item.clone(), controller);
        self.signals().item_pushed().emit(&item);
        self.signals().priority_changed().emit(controller as i64);
    }

    /// Löst das oberste Item sofort auf (ohne Prioritätsrunde).
    /// Gibt das aufgelöste Item zurück (null bei leerem Stack).
    ///
    /// Die Callable darf selbst wieder `push` aufrufen (z. B. ausgelöste Fähigkeiten).
    #[func]
    pub fn resolve_next(&mut self) -> Option<Gd<StackItem>> {
        let item = self.core.pop()?;
        let valid = item.bind().is_still_valid();
        {
            // Während Callable/Signale laufen, sind Rückrufe in diesen Manager erlaubt.
            let _guard = self.base_mut();
            if valid {
                let effect = item.bind().effect.clone();
                if effect.is_valid() {
                    effect.call(&[item.to_variant()]);
                }
            }
        }
        if valid {
            self.signals().item_resolved().emit(&item);
        } else {
            self.signals().item_fizzled().emit(&item);
        }
        let p = self.core.priority as i64;
        self.signals().priority_changed().emit(p);
        Some(item)
    }

    /// Spieler passt. Rückgabe: -1 = nicht an der Reihe, 0 = weitergegeben,
    /// 1 = beide gepasst -> oberstes Item wurde aufgelöst,
    /// 2 = beide gepasst bei leerem Stack.
    #[func]
    pub fn pass_priority(&mut self, player: i64) -> i64 {
        let result = self.core.pass(player.clamp(0, 1) as usize);
        match result {
            PassResult::NotYourPriority => {}
            PassResult::Passed => {
                let p = self.core.priority as i64;
                self.signals().priority_changed().emit(p);
            }
            PassResult::ResolveTop => {
                self.resolve_next();
            }
            PassResult::BothPassedEmpty => {
                self.signals().all_passed().emit();
                let p = self.core.priority as i64;
                self.signals().priority_changed().emit(p);
            }
        }
        result.code()
    }

    /// Neuer Zug / neue Phase: aktiver Spieler erhält Priorität.
    #[func]
    pub fn set_active_player(&mut self, player: i64) {
        self.core.set_active(player.clamp(0, 1) as usize);
        let p = self.core.priority as i64;
        self.signals().priority_changed().emit(p);
    }

    #[func]
    pub fn get_priority_player(&self) -> i64 {
        self.core.priority as i64
    }

    #[func]
    pub fn is_empty(&self) -> bool {
        self.core.items.is_empty()
    }

    #[func]
    pub fn size(&self) -> i64 {
        self.core.items.len() as i64
    }

    /// Oberstes Item (null bei leerem Stack).
    #[func]
    pub fn top(&self) -> Option<Gd<StackItem>> {
        self.core.items.last().cloned()
    }

    /// Alle Items, unterstes zuerst.
    #[func]
    pub fn get_items(&self) -> Array<Gd<StackItem>> {
        self.core.items.iter().cloned().collect()
    }

    /// Entfernt alles ohne Auflösung.
    #[func]
    pub fn clear(&mut self) {
        self.core.items.clear();
        self.core.passes = 0;
    }
}

// ================================================================== Tests

#[cfg(test)]
mod tests {
    use super::*;

    fn core() -> PriorityCore<&'static str> {
        let mut c = PriorityCore::default();
        c.set_active(0);
        c
    }

    #[test]
    fn lifo_order() {
        let mut c = core();
        c.push("Zauber A", 0);
        c.push("Antwort B", 1);
        c.push("Antwort C", 0);
        assert_eq!(c.pop(), Some("Antwort C"));
        assert_eq!(c.pop(), Some("Antwort B"));
        assert_eq!(c.pop(), Some("Zauber A"));
        assert_eq!(c.pop(), None);
    }

    #[test]
    fn caster_keeps_priority_after_push() {
        let mut c = core();
        c.push("Zauber", 0);
        assert_eq!(c.priority, 0);
        c.push("Antwort", 1);
        assert_eq!(c.priority, 1);
    }

    #[test]
    fn both_must_pass_in_succession() {
        let mut c = core();
        c.push("Zauber", 0);
        assert_eq!(c.pass(0), PassResult::Passed);
        assert_eq!(c.priority, 1);
        assert_eq!(c.pass(1), PassResult::ResolveTop);
        assert_eq!(c.pop(), Some("Zauber"));
        assert_eq!(c.priority, 0, "nach dem Auflösen hat der aktive Spieler Priorität");
    }

    #[test]
    fn response_resets_pass_count() {
        let mut c = core();
        c.push("Zauber", 0);
        assert_eq!(c.pass(0), PassResult::Passed);
        c.push("Antwort", 1); // Gegner antwortet statt zu passen
        assert_eq!(c.pass(1), PassResult::Passed);
        assert_eq!(c.pass(0), PassResult::ResolveTop);
        assert_eq!(c.pop(), Some("Antwort"));
        // Der ursprüngliche Zauber braucht erneut zwei Pässe
        assert_eq!(c.pass(0), PassResult::Passed);
        assert_eq!(c.pass(1), PassResult::ResolveTop);
        assert_eq!(c.pop(), Some("Zauber"));
    }

    #[test]
    fn only_priority_holder_may_pass() {
        let mut c = core();
        assert_eq!(c.pass(1), PassResult::NotYourPriority);
        assert_eq!(c.passes, 0);
    }

    #[test]
    fn empty_stack_both_pass() {
        let mut c = core();
        assert_eq!(c.pass(0), PassResult::Passed);
        assert_eq!(c.pass(1), PassResult::BothPassedEmpty);
        assert_eq!(c.priority, 0);
    }

    #[test]
    fn active_player_change() {
        let mut c = core();
        c.set_active(1);
        assert_eq!(c.priority, 1);
        assert_eq!(c.pass(0), PassResult::NotYourPriority);
    }
}
