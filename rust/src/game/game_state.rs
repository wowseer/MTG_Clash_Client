//! GameState: alle Daten einer laufenden Partie plus die Regel-Operationen,
//! die von den Phasen-Zuständen benutzt werden (Ziehen, Tappen, Kampf, KI …).
//!
//! Hinweis zu Borrows: Methoden binden Karten/Zonen immer nur kurz und nie
//! verschachtelt dasselbe Objekt; der TableController wird hier nie angefasst.

use std::collections::VecDeque;

use godot::prelude::*;

use super::ability::CardAbility;
use super::cards::{AuraDef, AuraEffect, CardKind, HeroAbility, SpellEffect, StaticEffect};
use super::mana::{Cost, Mana, ManaPool, ManaType};
use super::phases::Phase;
use super::timing::{check_aura, check_equip, check_hero_activation, check_play, PlayContext, PlayError};
use super::seat::PlayerSeat;
use super::stack::{StackItem, StackManager};
use crate::table::card3d::{Card3D, CardMarker};
use crate::table::zone::Zone;

const LOG_LINES: usize = 7;

#[derive(Default)]
pub struct Combat {
    pub attackers: Vec<Gd<Card3D>>,
    /// (Blocker, Angreifer)
    pub blocks: Vec<(Gd<Card3D>, Gd<Card3D>)>,
    /// Vom menschlichen Verteidiger angeklickter, noch nicht zugewiesener Blocker.
    pub selected: Option<Gd<Card3D>>,
}

/// Von Hand getappte Ressource und das Mana, das sie in den Pool gelegt hat. Solange dieses
/// Mana unbenutzt im Pool liegt, kann die Ressource wieder enttappt werden (Mana zurück).
pub struct ManaTap {
    pub card: Gd<Card3D>,
    pub mana_type: ManaType,
    pub amount: u32,
}

/// Temporärer Effekt, der in der End-Phase entfernt wird.
pub struct TempEffect {
    pub card: Gd<Card3D>,
    pub attack_bonus: i32,
}

pub struct GameState {
    /// [0] = Spieler (unten), [1] = Gegner (oben)
    pub seats: [Gd<PlayerSeat>; 2],
    pub active: usize,
    pub turn: u32,
    pub draw_count: i32,
    pub combat: Combat,
    pub temp_effects: Vec<TempEffect>,
    pub log: VecDeque<String>,
    pub winner: Option<usize>,
    /// Zauber und Fähigkeiten warten hier auf ihre Auflösung (LIFO, Prioritätsschleife).
    pub stack: Option<Gd<StackManager>>,
    /// Hier liegen Zauberkarten, solange sie auf dem Stack sind.
    pub stack_zone: Option<Gd<Zone>>,
    /// Callable, die ein StackItem auflöst (TurnManager::resolve_stack_item).
    pub resolver: Callable,
    /// Stack-Item, auf das die KI schon geantwortet hat (keine Endlosschleife).
    pub ai_answered: Option<InstanceId>,
    /// Vom Menschen gewählte Ausrüstung, die angelegt werden soll.
    pub pending_equip: Option<Gd<Card3D>>,
    /// Von Hand getappte Ressourcen, deren Mana noch zurückgenommen werden kann.
    pub mana_taps: Vec<ManaTap>,
    /// Held, dessen Fähigkeitenauswahl im HUD offen ist.
    pub pending_hero: Option<Gd<Card3D>>,
}

impl GameState {
    pub fn new(
        player: Gd<PlayerSeat>,
        opponent: Gd<PlayerSeat>,
        draw_count: i32,
        stack: Option<Gd<StackManager>>,
        stack_zone: Option<Gd<Zone>>,
        resolver: Callable,
    ) -> Self {
        Self {
            seats: [player, opponent],
            active: 0,
            turn: 1,
            draw_count,
            combat: Combat::default(),
            temp_effects: Vec::new(),
            log: VecDeque::new(),
            winner: None,
            stack,
            stack_zone,
            resolver,
            ai_answered: None,
            pending_equip: None,
            mana_taps: Vec::new(),
            pending_hero: None,
        }
    }

    // ------------------------------------------------------------ Abfragen

    pub fn defender(&self) -> usize {
        1 - self.active
    }

    pub fn is_human(&self, seat: usize) -> bool {
        !self.seats[seat].bind().is_ai
    }

    /// Anzeigename des Sitzplatzes in der aktuellen Sprache.
    pub fn name(&self, seat: usize) -> String {
        crate::i18n::name(&self.seats[seat].bind().display_name.to_string())
    }

    pub fn hand(&self, seat: usize) -> Option<Gd<Zone>> {
        self.seats[seat].bind().hand.clone()
    }

    pub fn deck(&self, seat: usize) -> Option<Gd<Zone>> {
        self.seats[seat].bind().deck.clone()
    }

    pub fn battlefield(&self, seat: usize) -> Option<Gd<Zone>> {
        self.seats[seat].bind().battlefield.clone()
    }

    pub fn discard(&self, seat: usize) -> Option<Gd<Zone>> {
        self.seats[seat].bind().discard.clone()
    }

    pub fn cards_in(zone: Option<Gd<Zone>>) -> Vec<Gd<Card3D>> {
        zone.map(|z| z.bind().cards()).unwrap_or_default()
    }

    /// Alle bleibenden Karten auf dem Spielfeld (Kreaturen, Helden, Ausrüstung).
    pub fn permanents(&self, seat: usize) -> Vec<Gd<Card3D>> {
        Self::cards_in(self.battlefield(seat))
    }

    /// Nur Kreaturen auf dem Spielfeld.
    pub fn creatures(&self, seat: usize) -> Vec<Gd<Card3D>> {
        self.permanents(seat).into_iter().filter(|c| c.bind().is_creature()).collect()
    }

    pub fn of_kind(&self, seat: usize, kind: CardKind) -> Vec<Gd<Card3D>> {
        self.permanents(seat).into_iter().filter(|c| c.bind().kind() == kind).collect()
    }

    pub fn untapped_creatures(&self, seat: usize) -> Vec<Gd<Card3D>> {
        self.creatures(seat).into_iter().filter(|c| !c.bind().is_tapped()).collect()
    }

    pub fn seat_of_zone(&self, zone: &Gd<Zone>) -> Option<usize> {
        (0..2).find(|&i| self.seats[i].bind().owns_zone(zone.clone()))
    }

    pub fn seat_of_card(&self, card: &Gd<Card3D>) -> Option<usize> {
        let zone = card.bind().get_zone()?;
        self.seat_of_zone(&zone)
    }

    pub fn is_on_battlefield(&self, card: &Gd<Card3D>, seat: usize) -> bool {
        let zone = card.bind().get_zone();
        zone.is_some() && zone == self.battlefield(seat)
    }

    /// Liegt die Karte als bleibende Karte im Spiel (Spielfeld oder Ressourcen-Reihe)?
    pub fn is_on_field(&self, card: &Gd<Card3D>) -> bool {
        let zone = card.bind().get_zone();
        zone.is_some() && (0..2).any(|s| zone == self.battlefield(s) || zone == self.resource_zone(s))
    }

    /// Alle Karten im Spiel (beide Spielfelder und Ressourcen-Reihen).
    pub fn field_cards(&self) -> Vec<Gd<Card3D>> {
        (0..2)
            .flat_map(|s| [self.battlefield(s), self.resource_zone(s)])
            .flat_map(Self::cards_in)
            .collect()
    }

    pub fn all_auras(&self) -> Vec<Gd<Card3D>> {
        self.field_cards().into_iter().filter(|c| c.bind().kind() == CardKind::Aura).collect()
    }

    /// Auren, die `card` verzaubern.
    pub fn auras_on(&self, card: &Gd<Card3D>) -> Vec<AuraDef> {
        self.all_auras()
            .into_iter()
            .filter(|a| a.bind().enchanted.as_ref() == Some(card))
            .filter_map(|a| a.bind().aura)
            .collect()
    }

    /// Darf `aura` die Karte `target` verzaubern? (im Spiel + passende Zielklasse)
    pub fn legal_aura_target(&self, aura: &Gd<Card3D>, target: &Gd<Card3D>) -> bool {
        if aura == target || !target.is_instance_valid() || !self.is_on_field(target) {
            return false;
        }
        let kind = target.bind().kind();
        aura.bind().aura.is_some_and(|a| a.target.allows(kind))
    }

    pub fn life(&self, seat: usize) -> i32 {
        self.seats[seat].bind().life
    }

    // ------------------------------------------------------------ Grundoperationen

    pub fn log(&mut self, msg: impl Into<String>) {
        let msg = msg.into();
        godot_print!("[Runde {}] {}", self.turn, msg);
        self.log.push_back(msg);
        while self.log.len() > LOG_LINES {
            self.log.pop_front();
        }
    }

    pub fn change_life(&mut self, seat: usize, delta: i32) {
        let life = {
            let mut s = self.seats[seat].bind_mut();
            s.life += delta;
            s.life
        };
        if life <= 0 && self.winner.is_none() {
            self.winner = Some(1 - seat);
            let (w, l) = (self.name(1 - seat), self.name(seat));
            self.log(trf!("{l} hat keine Lebenspunkte mehr – {w} gewinnt!", "{l} has no life left – {w} wins!"));
        }
    }

    pub fn relayout(card: &Gd<Card3D>) {
        let zone = card.bind().get_zone();
        if let Some(mut z) = zone {
            z.bind_mut().layout();
        }
    }

    pub fn set_tapped(card: &mut Gd<Card3D>, tapped: bool) {
        if card.bind().is_tapped() == tapped {
            return;
        }
        card.bind_mut().set_tapped(tapped);
        Self::relayout(card);
    }

    pub fn move_card(card: &Gd<Card3D>, to: &mut Gd<Zone>, delay: f64) {
        let from = card.bind().get_zone();
        if let Some(mut from) = from {
            from.bind_mut().remove_card(card.clone());
        }
        to.bind_mut().add_card(card.clone(), -1, delay);
    }

    /// Zerstört eine Kreatur: kommt auf den Ablagestapel ihres Besitzers.
    pub fn destroy(&mut self, card: &mut Gd<Card3D>) {
        let Some(field_seat) = self.seat_of_card(card) else {
            return;
        };
        // Karten gehen immer auf den Friedhof ihres Besitzers (Auren können
        // auf dem Spielfeld des Gegners liegen).
        let owner = card.bind().owner_seat.clamp(0, 1) as usize;
        // Angelegte Ausrüstung / Aura: Bonus vom Träger entfernen.
        let host = card.bind().equipped_to.clone();
        if let (Some(mut host), Some(e)) = (host, card.bind().equip) {
            if host.is_instance_valid() {
                host.bind_mut().add_equip_bonus(-e.attack, -e.defense);
            }
        }
        let host = card.bind().enchanted.clone();
        let aura = card.bind().aura;
        if let (Some(mut host), Some(AuraDef { effect: AuraEffect::Stats(a, d), .. })) = (host, aura) {
            if host.is_instance_valid() {
                host.bind_mut().add_equip_bonus(-a, -d);
            }
        }
        // Träger verlässt das Spielfeld: seine Ausrüstung bleibt liegen, ist aber frei.
        // (Auren erledigt die zustandsbasierte Aktion.)
        for mut eq in self.of_kind(field_seat, CardKind::Equipment) {
            if eq.bind().equipped_to.as_ref() == Some(card) {
                eq.bind_mut().equipped_to = None;
            }
        }
        {
            let mut c = card.bind_mut();
            c.clear_turn_effects();
            c.set_tapped(false);
            c.reset_on_leave();
        }
        if let Some(mut discard) = self.discard(owner) {
            Self::move_card(card, &mut discard, 0.15);
        }
        let name = card.bind().display_name();
        self.log(trf!("{name} wird zerstört.", "{name} is destroyed."));
    }

    // ------------------------------------------------------------ Phasen-Aktionen

    /// Enttappen: alle Kreaturen und Ressourcen des Spielers enttappen,
    /// Ressourcen-Zähler der Runde zurücksetzen.
    pub fn untap(&mut self, seat: usize) {
        let taps = std::mem::take(&mut self.mana_taps);
        self.mana_taps = taps.into_iter().filter(|t| self.seat_of_card(&t.card) != Some(seat)).collect();
        self.seats[seat].bind_mut().resources_played = 0;
        for mut hero in self.of_kind(seat, CardKind::Hero) {
            hero.bind_mut().hero_used = false;
        }
        for zone in [self.battlefield(seat), self.resource_zone(seat)].into_iter().flatten() {
            for mut card in zone.bind().cards() {
                let locked = self.auras_on(&card).iter().any(|a| a.effect == AuraEffect::NoUntap);
                if !locked {
                    card.bind_mut().set_tapped(false);
                }
            }
            let mut zone = zone;
            zone.bind_mut().layout();
        }
    }

    /// Auffrischung: Fähigkeiten "zu Beginn der Runde" auslösen.
    pub fn trigger_upkeep(&mut self, seat: usize) {
        for card in self.creatures(seat) {
            let (ability, name) = {
                let c = card.bind();
                (c.ability_kind(), c.display_name())
            };
            match ability {
                CardAbility::Regeneration => {
                    self.change_life(seat, 1);
                    self.log(trf!("{name}: {} erhält 1 Leben.", "{name}: {} gains 1 life.", self.name(seat)));
                }
                CardAbility::Firebreath => {
                    let target = 1 - seat;
                    self.change_life(target, -1);
                    self.log(trf!("{name}: 1 Schaden an {}.", "{name}: 1 damage to {}.", self.name(target)));
                }
                _ => {}
            }
        }
    }

    pub fn draw(&mut self, seat: usize, count: i32) {
        let (Some(mut deck), Some(mut hand)) = (self.deck(seat), self.hand(seat)) else {
            return;
        };
        for k in 0..count {
            let top = deck.bind().get_top_card();
            match top {
                Some(card) => {
                    deck.bind_mut().remove_card(card.clone());
                    hand.bind_mut().add_card(card, -1, k as f64 * 0.15);
                }
                None => {
                    self.log(trf!("{}: Deck ist leer.", "{}: deck is empty.", self.name(seat)));
                    return;
                }
            }
        }
    }

    // ------------------------------------------------------------ Mana & Ressourcen

    pub fn resource_zone(&self, seat: usize) -> Option<Gd<Zone>> {
        self.seats[seat].bind().resource_zone.clone()
    }

    pub fn pool(&self, seat: usize) -> Option<Gd<ManaPool>> {
        self.seats[seat].bind().mana_pool.clone()
    }

    pub fn pool_mana(&self, seat: usize) -> Mana {
        self.pool(seat).map(|p| p.bind().mana()).unwrap_or_default()
    }

    pub fn untapped_resources(&self, seat: usize) -> Vec<Gd<Card3D>> {
        Self::cards_in(self.resource_zone(seat))
            .into_iter()
            .filter(|c| !c.bind().is_tapped())
            .collect()
    }

    /// Mana, das der Spieler jetzt höchstens aufbringen kann (Pool + ungetappte Ressourcen).
    pub fn potential_mana(&self, seat: usize) -> Mana {
        let mut m = self.pool_mana(seat);
        for c in self.untapped_resources(seat) {
            m.add(c.bind().produced_mana(), 1 + self.extra_mana(&c));
        }
        m
    }

    /// Zusätzliches Mana durch Auren (z. B. Goldader) auf einer Ressource.
    pub fn extra_mana(&self, card: &Gd<Card3D>) -> u32 {
        self.auras_on(card)
            .iter()
            .map(|a| if let AuraEffect::ExtraMana(n) = a.effect { n } else { 0 })
            .sum()
    }

    pub fn can_afford(&self, seat: usize, cost: &Cost) -> bool {
        self.has_infinite_mana(seat) || cost.can_pay(&self.potential_mana(seat))
    }

    /// Debug-Modus „unendlich Mana“ für diesen Spieler aktiv?
    pub fn has_infinite_mana(&self, seat: usize) -> bool {
        self.seats[seat].bind().infinite_mana
    }

    /// Tappt eine ungetappte Ressourcen-Karte und legt ihr Mana in den Pool.
    pub fn tap_for_mana(&mut self, card: &mut Gd<Card3D>) -> bool {
        let Some(seat) = self.seat_of_card(card) else {
            return false;
        };
        let in_row = card.bind().get_zone() == self.resource_zone(seat);
        if !in_row || card.bind().is_tapped() || !card.bind().is_resource() {
            return false;
        }
        let t = card.bind().produced_mana();
        let amount = 1 + self.extra_mana(card);
        Self::set_tapped(card, true);
        if let Some(mut pool) = self.pool(seat) {
            pool.bind_mut().add_mana(t, amount as i64);
        }
        true
    }

    /// Tappt eine Ressource von Hand (Klick) und merkt sich das Mana, damit der Tapp
    /// zurückgenommen werden kann, solange das Mana unbenutzt im Pool liegt.
    pub fn tap_for_mana_by_hand(&mut self, card: &mut Gd<Card3D>) -> bool {
        if !self.tap_for_mana(card) {
            return false;
        }
        let amount = 1 + self.extra_mana(card);
        let mana_type = card.bind().produced_mana();
        self.mana_taps.push(ManaTap { card: card.clone(), mana_type, amount });
        true
    }

    /// Nimmt einen Tapp für Mana zurück: Ist `card` von Hand getappt worden und liegt ihr Mana
    /// noch im Pool, wird das Mana abgezogen und die Ressource enttappt. false = nicht möglich
    /// (Mana schon ausgegeben, Pool geleert oder nicht von Hand getappt).
    pub fn untap_unused_mana(&mut self, card: &mut Gd<Card3D>) -> bool {
        let Some(pos) = self.mana_taps.iter().position(|t| &t.card == card) else {
            return false;
        };
        let Some(seat) = self.seat_of_card(card) else { return false };
        let Some(mut pool) = self.pool(seat) else { return false };
        let (mana_type, amount) = (self.mana_taps[pos].mana_type, self.mana_taps[pos].amount);
        if !pool.bind_mut().remove_mana(mana_type, amount as i64) {
            return false;
        }
        self.mana_taps.remove(pos);
        Self::set_tapped(card, false);
        true
    }

    /// Bezahlt Kosten: zuerst aus dem Pool; fehlt etwas, werden passende
    /// Ressourcen automatisch getappt. Alles oder nichts.
    pub fn auto_pay(&mut self, seat: usize, cost: &Cost) -> bool {
        // Debug „unendlich Mana“: bezahlt, ohne zu tappen oder den Pool zu verbrauchen
        if self.has_infinite_mana(seat) {
            return true;
        }
        if !self.can_afford(seat, cost) {
            return false;
        }
        loop {
            let pool = self.pool_mana(seat);
            if cost.can_pay(&pool) {
                break;
            }
            let free = self.untapped_resources(seat);
            let of_type = |t: ManaType| free.iter().find(|c| c.bind().produced_mana() == t).cloned();
            // 1) Fehlende feste Anteile decken
            let mut pick = ManaType::ALL
                .into_iter()
                .find(|t| pool.get(*t) < cost.specific.get(*t))
                .and_then(of_type);
            // 2) Generischer Rest: farblos zuerst, dann die häufigste Farbe
            //    (seltene Farben bleiben für spätere Karten übrig)
            if pick.is_none() {
                pick = of_type(ManaType::Colorless).or_else(|| {
                    ManaType::ALL
                        .into_iter()
                        .filter(|t| t.is_colored())
                        .max_by_key(|t| free.iter().filter(|c| c.bind().produced_mana() == *t).count())
                        .and_then(of_type)
                });
            }
            match pick {
                Some(mut card) => {
                    self.tap_for_mana(&mut card);
                }
                None => return false, // sollte durch can_afford nicht vorkommen
            }
        }
        match self.pool(seat) {
            Some(mut pool) => pool.bind_mut().pay(cost).is_some(),
            None => cost.total() == 0,
        }
    }

    // ------------------------------------------------------------ Timing & Ausspielen

    /// Spielzustand aus Sicht von `seat` für die Timing-Regeln.
    pub fn play_context(&self, seat: usize, phase: Phase, cost: &Cost) -> PlayContext {
        PlayContext {
            own_turn: self.active == seat,
            phase: if self.winner.is_some() { Phase::GameOver } else { phase },
            stack_empty: self.stack_empty(),
            resource_played: self.seats[seat].bind().resources_played > 0,
            can_afford: self.can_afford(seat, cost),
        }
    }

    /// Darf `seat` diese Karte jetzt aus der Hand spielen?
    pub fn check_card(&self, seat: usize, card: &Gd<Card3D>, phase: Phase) -> Result<(), PlayError> {
        let (kind, cost) = {
            let c = card.bind();
            (c.kind(), c.cost())
        };
        // Hinweis: Hier wird bewusst nicht geprüft, ob die Karte noch in der Hand liegt –
        // beim Ziehen mit der Maus hat sie vorübergehend keine Zone. Die Herkunft prüft
        // `TurnManager::resolve_drop` bzw. `can_play_card`.
        check_play(kind, &self.play_context(seat, phase, &cost))?;
        // Counterspell braucht einen gegnerischen Zauber auf dem Stack
        if card.bind().effect == SpellEffect::Counter && self.counter_target(seat).is_none() {
            return Err(PlayError::NoTarget);
        }
        Ok(())
    }

    /// Oberster gegnerischer **Zauber** (gewirkte Karte, keine Fähigkeit) auf dem Stack.
    pub fn counter_target(&self, seat: usize) -> Option<Gd<StackItem>> {
        let stack = self.stack.as_ref()?;
        let items: Vec<Gd<StackItem>> = stack.bind().get_items().iter_shared().collect();
        items.into_iter().rev().find(|i| {
            let i = i.bind();
            let is_cast = i.payload.is_nil() || i.payload.try_to::<Vector3>().is_ok();
            i.controller as usize != seat && is_cast
        })
    }

    /// Ausspielen aus der Hand: prüfen, bezahlen, ausführen. Gibt die Zielzone zurück
    /// (None = nicht erlaubt):
    /// - Ressource -> Ressourcen-Reihe (max. 1 pro Runde)
    /// - Sofortzauber / Hexerei -> über den Stack auflösen, danach Ablagestapel
    /// - Kreatur / Held / Ausrüstung -> Spielfeld
    pub fn play_from_hand(&mut self, seat: usize, card: &Gd<Card3D>, phase: Phase) -> Option<Gd<Zone>> {
        self.play_from_hand_at(seat, card, phase, None)
    }

    /// Wie `play_from_hand`; `drop_point` (Welt) = gewünschter Platz für bleibende Karten.
    pub fn play_from_hand_at(
        &mut self,
        seat: usize,
        card: &Gd<Card3D>,
        phase: Phase,
        drop_point: Option<Vector3>,
    ) -> Option<Gd<Zone>> {
        let (kind, name, cost, effect) = {
            let c = card.bind();
            (c.kind(), c.display_name(), c.cost(), c.effect)
        };
        if let Err(e) = self.check_card(seat, card, phase) {
            self.log(format!("{name}: {}", e.message()));
            return None;
        }
        if kind == CardKind::Resource {
            self.seats[seat].bind_mut().resources_played += 1;
            self.log(trf!("{} legt {name} aus.", "{} plays {name}.", self.name(seat)));
            return self.resource_zone(seat);
        }
        if kind == CardKind::Aura {
            self.log(trf!("{name}: Auren auf eine Kreatur ziehen ({}).", "{name}: drag auras onto a target ({}).", PlayError::NoTarget.message()));
            return None;
        }
        if !self.auto_pay(seat, &cost) {
            self.log(format!("{name}: {}", PlayError::NotEnoughMana.message()));
            return None;
        }
        self.log(trf!("{} spielt {name} ({}, {}).", "{} casts {name} ({}, {}).", self.name(seat), kind.title(), cost.to_text()));
        // Alles außer Ressourcen geht auf den Stack; bleibende Karten kommen beim
        // Auflösen aufs Spielfeld (Payload = gewünschte Position).
        let text = match kind {
            CardKind::Instant | CardKind::Sorcery => format!("{name}: {}", effect.description()),
            _ => trf!("{name} ({})", "{name} ({})", kind.title()),
        };
        let payload = drop_point.map_or(Variant::nil(), |p| p.to_variant());
        self.push_item(seat, card, text, Variant::nil(), payload);
        self.stack_zone.clone().or_else(|| self.discard(seat))
    }

    /// Wirkt eine Aura auf `target` (eine Kreatur auf einem Spielfeld).
    pub fn cast_aura(&mut self, seat: usize, card: &Gd<Card3D>, target: &Gd<Card3D>, phase: Phase) -> Option<Gd<Zone>> {
        let (name, cost) = {
            let c = card.bind();
            (c.display_name(), c.cost())
        };
        let legal = self.legal_aura_target(card, target);
        let ctx = self.play_context(seat, phase, &cost);
        if let Err(e) = check_aura(&ctx, legal) {
            self.log(format!("{name}: {}", e.message()));
            return None;
        }
        if !self.auto_pay(seat, &cost) {
            self.log(format!("{name}: {}", PlayError::NotEnoughMana.message()));
            return None;
        }
        let tname = target.bind().display_name();
        self.log(trf!("{} wirkt {name} auf {tname} ({}).", "{} casts {name} on {tname} ({}).", self.name(seat), cost.to_text()));
        self.push_item(seat, card, format!("{name} → {tname}"), target.to_variant(), Variant::nil());
        self.stack_zone.clone().or_else(|| self.discard(seat))
    }

    /// Tapp-Fähigkeit eines eigenen Artefakts (Timing wie Sofortzauber).
    pub fn activate_artifact(&mut self, seat: usize, card: &mut Gd<Card3D>, phase: Phase) -> Result<(), PlayError> {
        let Some(ability) = card.bind().activated else {
            return Err(PlayError::NoTarget);
        };
        if !self.is_on_battlefield(card, seat) || card.bind().is_tapped() {
            return Err(PlayError::NoTarget);
        }
        let ctx = self.play_context(seat, phase, &ability.cost);
        check_play(CardKind::Instant, &ctx)?;
        if !self.auto_pay(seat, &ability.cost) {
            return Err(PlayError::NotEnoughMana);
        }
        Self::set_tapped(card, true);
        let name = card.bind().display_name();
        self.log(format!("{name}: {}", ability.description()));
        self.push_item(seat, card, format!("{name}: {}", ability.effect.description()), Variant::nil(), "ability".to_variant());
        Ok(())
    }

    // ------------------------------------------------------------ Stack

    pub fn stack_empty(&self) -> bool {
        self.stack.as_ref().is_none_or(|s| s.bind().is_empty())
    }

    /// Legt einen Zauber / eine Fähigkeit mit `card` als Quelle auf den Stack.
    ///
    /// `payload`: nil/Vector3 = Karte wird gewirkt (Vector3 = Wunschplatz),
    /// int = Heldenfähigkeit (Index), "ability" = Artefakt-Fähigkeit.
    pub fn push_item(&mut self, seat: usize, card: &Gd<Card3D>, text: String, target: Variant, payload: Variant) {
        let mut item = StackItem::create(
            card.to_variant(),
            target,
            self.resolver.clone(),
            seat as i64,
            GString::from(text.as_str()),
        );
        item.bind_mut().payload = payload;
        match self.stack.clone() {
            Some(mut stack) => stack.bind_mut().push(item),
            None => self.resolve_item(&item), // ohne StackManager sofort auflösen
        }
    }

    /// Löst ein Stack-Item auf (aufgerufen über `resolver` aus dem StackManager).
    pub fn resolve_item(&mut self, item: &Gd<StackItem>) {
        let (source, target, controller, payload) = {
            let i = item.bind();
            (i.source.clone(), i.target.clone(), i.controller.clamp(0, 1) as usize, i.payload.clone())
        };
        let Ok(mut card) = source.try_to::<Gd<Card3D>>() else {
            return;
        };
        let (kind, name, effect) = {
            let c = card.bind();
            (c.kind(), c.display_name(), c.effect)
        };

        // Fähigkeiten (Karte bleibt, wo sie ist)
        if let Ok(index) = payload.try_to::<i64>() {
            let ability = card.bind().hero_abilities.get(index.max(0) as usize).copied();
            if let Some(a) = ability {
                self.apply_effect(controller, &name, a.effect);
            }
            return;
        }
        if payload.try_to::<GString>().is_ok_and(|p| p == "ability") {
            let ability = card.bind().activated;
            if let Some(a) = ability {
                self.apply_effect(controller, &name, a.effect);
            }
            return;
        }

        // Gewirkte Karten
        match kind {
            CardKind::Instant | CardKind::Sorcery => {
                self.apply_effect(controller, &name, effect);
                if let Some(mut discard) = self.discard(controller) {
                    Self::move_card(&card, &mut discard, 0.1);
                }
            }
            CardKind::Aura => {
                // Ziel muss noch ein gültiges Ziel im Spiel sein, sonst verpufft die Aura.
                let host = target.try_to::<Gd<Card3D>>().ok().filter(|h| self.legal_aura_target(&card, h));
                match host {
                    Some(mut host) => {
                        let host_zone = host.bind().get_zone();
                        if let Some(mut zone) = host_zone {
                            Self::move_card(&card, &mut zone, 0.0);
                            zone.bind_mut().attach(card.clone(), host.clone());
                        }
                        card.bind_mut().enchanted = Some(host.clone());
                        let effect = card.bind().aura.map(|a| a.effect);
                        match effect {
                            Some(AuraEffect::Stats(a, d)) if host.bind().is_creature() => {
                                host.bind_mut().add_equip_bonus(a, d);
                            }
                            Some(AuraEffect::NoUntap) => Self::set_tapped(&mut host, true),
                            Some(AuraEffect::Willpower(n)) => {
                                host.bind_mut().change_willpower(n);
                            }
                            _ => {}
                        }
                        let hn = host.bind().display_name();
                        self.log(trf!("{name} verzaubert {hn}.", "{name} enchants {hn}."));
                    }
                    None => {
                        self.log(trf!("{name} verpufft: kein gültiges Ziel mehr.", "{name} fizzles: no valid target left."));
                        let owner = card.bind().owner_seat.clamp(0, 1) as usize;
                        if let Some(mut discard) = self.discard(owner) {
                            Self::move_card(&card, &mut discard, 0.1);
                        }
                    }
                }
            }
            _ => {
                // Bleibende Karte betritt das Spielfeld (Wunschplatz oder freier Platz)
                if let Some(mut bf) = self.battlefield(controller) {
                    let from = card.bind().get_zone();
                    if let Some(mut from) = from {
                        from.bind_mut().remove_card(card.clone());
                    }
                    match payload.try_to::<Vector3>() {
                        Ok(point) => bf.bind_mut().add_card_at(card.clone(), point, 0.0),
                        Err(_) => bf.bind_mut().add_card_free(card.clone(), 0.0),
                    }
                    card.bind_mut().set_tapped(false);
                    self.log(trf!("{name} betritt das Spielfeld.", "{name} enters the battlefield."));
                }
            }
        }
    }

    // ------------------------------------------------------------ Zustandsbasierte Aktionen

    /// Wird laufend geprüft (jeder Frame), solange gerade nichts aufgelöst wird:
    /// 1. Globale Boni aus Artefakten/Verzauberungen neu berechnen.
    /// 2. Kreaturen mit Verteidigung ≤ 0 sterben, Helden ohne Willenskraft ebenso.
    /// 3. Auren, deren Kreatur das Spielfeld verlassen hat, gehen auf den Friedhof.
    pub fn check_state_based_actions(&mut self) {
        // 1) Statische Effekte
        let mut bonus = [(0, 0); 2];
        for seat in 0..2 {
            for perm in self.permanents(seat) {
                let effect = perm.bind().static_effect;
                match effect {
                    Some(StaticEffect::OwnCreatures(a, d)) => {
                        bonus[seat].0 += a;
                        bonus[seat].1 += d;
                    }
                    Some(StaticEffect::KeepMana) => {}
                    Some(StaticEffect::OpponentCreatures(a, d)) => {
                        bonus[1 - seat].0 += a;
                        bonus[1 - seat].1 += d;
                    }
                    None => {}
                }
            }
        }
        for seat in 0..2 {
            for mut c in self.creatures(seat) {
                c.bind_mut().set_global_bonus(bonus[seat].0, bonus[seat].1);
            }
        }

        // 2) Tote Kreaturen / Helden ohne Willenskraft
        for seat in 0..2 {
            for mut c in self.permanents(seat) {
                let dead = {
                    let b = c.bind();
                    (b.is_creature() && b.get_remaining_defense() <= 0) || (b.kind() == CardKind::Hero && b.get_willpower() <= 0)
                };
                if dead {
                    self.destroy(&mut c);
                }
            }
        }

        // 3) Auren, deren verzauberte Karte nicht mehr (gültig) im Spiel ist
        {
            for mut aura in self.all_auras() {
                let host = aura.bind().enchanted.clone();
                let host_ok = host.as_ref().is_some_and(|h| self.legal_aura_target(&aura, h));
                if !host_ok {
                    let (an, hn) = (
                        aura.bind().display_name(),
                        host.map(|h| h.bind().display_name()).unwrap_or_default(),
                    );
                    aura.bind_mut().enchanted = None; // Bonus ist mit dem Träger schon verschwunden
                    {
                        let mut a = aura.bind_mut();
                        a.reset_on_leave();
                        a.set_tapped(false);
                    }
                    let owner = aura.bind().owner_seat.clamp(0, 1) as usize;
                    if let Some(mut discard) = self.discard(owner) {
                        Self::move_card(&aura, &mut discard, 0.25);
                    }
                    self.log(trf!("{an} geht mit {hn} auf den Friedhof.", "{an} goes to the graveyard together with {hn}."));
                }
            }
        }
    }

    /// Lohnt sich der Effekt für `seat` gerade? (KI: keine Zauber ohne Ziel)
    pub fn effect_useful(&self, seat: usize, effect: SpellEffect) -> bool {
        match effect {
            SpellEffect::DamageStrongestCreature(_) => !self.creatures(1 - seat).is_empty(),
            SpellEffect::Counter => self.counter_target(seat).is_some(),
            _ => true,
        }
    }

    fn spell_useful(&self, seat: usize, card: &Gd<Card3D>) -> bool {
        let effect = card.bind().effect;
        self.effect_useful(seat, effect)
    }

    /// KI-Ziel für eine Aura: gute Wirkung -> eigene Karte, schlechte -> gegnerische.
    /// Unter den gültigen Zielen wird die wertvollste Karte gewählt.
    fn ai_aura_target(&self, seat: usize, card: &Gd<Card3D>) -> Option<Gd<Card3D>> {
        let aura = card.bind().aura?;
        let side = if aura.effect.is_beneficial() { seat } else { 1 - seat };
        let value = |c: &Gd<Card3D>| -> i32 {
            let c = c.bind();
            let tapped_penalty = if aura.effect == AuraEffect::NoUntap && c.is_tapped() { -20 } else { 0 };
            tapped_penalty
                + match c.kind() {
                    CardKind::Creature => 10 + c.get_current_attack() * 2,
                    CardKind::Hero => 12,
                    CardKind::Artifact | CardKind::Equipment => 6,
                    CardKind::Enchantment | CardKind::Aura => 4,
                    CardKind::Resource => 3,
                    _ => 0,
                }
        };
        [self.battlefield(side), self.resource_zone(side)]
            .into_iter()
            .flat_map(Self::cards_in)
            .filter(|t| self.legal_aura_target(card, t))
            .max_by_key(|t| value(t))
    }

    /// Texte der Stack-Items, oberstes zuerst.
    pub fn stack_lines(&self) -> Vec<String> {
        let Some(stack) = &self.stack else {
            return Vec::new();
        };
        let items: Vec<Gd<StackItem>> = stack.bind().get_items().iter_shared().collect();
        items
            .iter()
            .rev()
            .map(|i| {
                let i = i.bind();
                format!("{} ({})", i.description, self.name(i.controller.clamp(0, 1) as usize))
            })
            .collect()
    }

    /// KI hat Priorität bei vollem Stack: mit einem Sofortzauber auf einen
    /// gegnerischen Zauber antworten (einmal je Item). true = hat geantwortet.
    pub fn ai_respond(&mut self, seat: usize, phase: Phase) -> bool {
        let Some(top) = self.stack.as_ref().and_then(|s| s.bind().top()) else {
            return false;
        };
        let top_id = top.instance_id();
        if top.bind().controller as usize == seat || self.ai_answered == Some(top_id) {
            return false;
        }
        self.ai_answered = Some(top_id);
        let instant = Self::cards_in(self.hand(seat))
            .into_iter()
            .find(|c| {
                c.bind().kind() == CardKind::Instant && self.spell_useful(seat, c) && self.check_card(seat, c, phase).is_ok()
            });
        if let Some(card) = instant {
            if let Some(mut zone) = self.play_from_hand(seat, &card, phase) {
                Self::move_card(&card, &mut zone, 0.0);
                return true;
            }
        }
        false
    }

    /// Führt einen Effekt für `seat` aus.
    pub fn apply_effect(&mut self, seat: usize, source: &str, effect: SpellEffect) {
        let foe = 1 - seat;
        match effect {
            SpellEffect::None => {}
            SpellEffect::Heal(n) => {
                self.change_life(seat, n);
                self.log(trf!("{source}: {} erhält {n} Leben.", "{source}: {} gains {n} life.", self.name(seat)));
            }
            SpellEffect::Draw(n) => {
                self.draw(seat, n);
                self.log(trf!("{source}: {} zieht {n} Karte(n).", "{source}: {} draws {n} card(s).", self.name(seat)));
            }
            SpellEffect::DamageOpponent(n) => {
                self.change_life(foe, -n);
                self.log(trf!("{source}: {n} Schaden an {}.", "{source}: {n} damage to {}.", self.name(foe)));
            }
            SpellEffect::DamageStrongestCreature(n) => {
                let target = self.creatures(foe).into_iter().max_by_key(|c| c.bind().get_current_attack());
                match target {
                    Some(mut t) => {
                        t.bind_mut().add_damage(n);
                        let tname = t.bind().display_name();
                        self.log(trf!("{source}: {n} Schaden an {tname}.", "{source}: {n} damage to {tname}."));
                        if t.bind().get_remaining_defense() <= 0 {
                            self.destroy(&mut t);
                        }
                    }
                    None => self.log(trf!("{source}: kein Ziel.", "{source}: no target.")),
                }
            }
            SpellEffect::Counter => match (self.counter_target(seat), self.stack.clone()) {
                (Some(item), Some(mut stack)) => {
                    stack.bind_mut().counter_item(item.clone());
                    let (spell, controller) = {
                        let i = item.bind();
                        (i.source.clone(), i.controller.clamp(0, 1) as usize)
                    };
                    if let Ok(card) = spell.try_to::<Gd<Card3D>>() {
                        let name = card.bind().display_name();
                        let owner = card.bind().owner_seat.clamp(0, 1) as usize;
                        if let Some(mut discard) = self.discard(owner) {
                            Self::move_card(&card, &mut discard, 0.1);
                        }
                        self.log(trf!("{source}: {name} von {} wird neutralisiert.", "{source}: {}'s {name} is countered.", self.name(controller)));
                    }
                }
                _ => self.log(trf!("{source}: kein Zauber zum Neutralisieren.", "{source}: no spell to counter.")),
            },
        }
    }

    // ------------------------------------------------------------ Helden

    pub fn check_hero(&self, seat: usize, hero: &Gd<Card3D>, index: usize, phase: Phase) -> Result<HeroAbility, PlayError> {
        let h = hero.bind();
        let ability = *h.hero_abilities.get(index).ok_or(PlayError::NoTarget)?;
        if h.kind() != CardKind::Hero || !self.is_on_battlefield(hero, seat) {
            return Err(PlayError::NoTarget);
        }
        let ctx = self.play_context(seat, phase, &Cost::default());
        check_hero_activation(&ctx, h.hero_used, h.willpower, ability.delta)?;
        Ok(ability)
    }

    /// Aktiviert Fähigkeit `index` eines Helden (1× pro Runde).
    pub fn activate_hero(&mut self, seat: usize, hero: &mut Gd<Card3D>, index: usize, phase: Phase) -> Result<(), PlayError> {
        let ability = self.check_hero(seat, hero, index, phase)?;
        let name = hero.bind().display_name();
        let willpower = {
            let mut h = hero.bind_mut();
            h.hero_used = true;
            h.change_willpower(ability.delta)
        };
        self.log(format!("{name}: {}", ability.description()));
        // Kosten (Willenskraft) sind bezahlt, die Wirkung geht auf den Stack.
        let text = format!("{name}: {}", ability.effect.description());
        self.push_item(seat, hero, text, Variant::nil(), (index as i64).to_variant());
        if willpower <= 0 {
            self.log(trf!("{name} hat keine Willenskraft mehr.", "{name} has no willpower left."));
            self.destroy(hero);
        }
        Ok(())
    }

    // ------------------------------------------------------------ Ausrüstung

    /// Legt eine Ausrüstung für ihre Anlegekosten an eine eigene Kreatur an.
    pub fn equip(&mut self, seat: usize, equipment: &mut Gd<Card3D>, host: &mut Gd<Card3D>, phase: Phase) -> Result<(), PlayError> {
        let Some(stats) = equipment.bind().equip else {
            return Err(PlayError::NoTarget);
        };
        let valid_target = host.bind().is_creature()
            && self.is_on_battlefield(host, seat)
            && self.is_on_battlefield(equipment, seat)
            && equipment.bind().equipped_to.as_ref() != Some(host);
        let ctx = self.play_context(seat, phase, &stats.cost);
        check_equip(&ctx, valid_target)?;
        if !self.auto_pay(seat, &stats.cost) {
            return Err(PlayError::NotEnoughMana);
        }
        // Von einer evtl. vorherigen Kreatur lösen
        let old = equipment.bind_mut().equipped_to.take();
        if let Some(mut old) = old {
            if old.is_instance_valid() {
                old.bind_mut().add_equip_bonus(-stats.attack, -stats.defense);
            }
        }
        host.bind_mut().add_equip_bonus(stats.attack, stats.defense);
        equipment.bind_mut().equipped_to = Some(host.clone());
        if let Some(mut bf) = self.battlefield(seat) {
            bf.bind_mut().attach(equipment.clone(), host.clone());
        }
        let (en, hn) = (equipment.bind().display_name(), host.bind().display_name());
        self.log(trf!("{en} wird an {hn} angelegt (+{}/+{}).", "{en} is attached to {hn} (+{}/+{}).", stats.attack, stats.defense));
        Ok(())
    }

    /// Offene Auswahlen (Ausrüstung/Held) schließen.
    pub fn clear_pending(&mut self) {
        if let Some(mut e) = self.pending_equip.take() {
            if e.is_instance_valid() {
                e.bind_mut().set_marker(CardMarker::None);
            }
        }
        self.pending_hero = None;
    }

    /// Leert die Manapools beider Spieler (Mana verfällt am Ende jeder Phase).
    /// Ende des Zuges: Manapools beider Spieler auf null setzen – außer bei einem
    /// Spieler, der eine Karte mit „Mana bleibt erhalten“ im Spiel hat.
    pub fn empty_pools(&mut self) {
        // Geleertes Mana lässt sich nicht mehr zurücknehmen
        self.mana_taps.clear();
        for seat in 0..2 {
            let Some(mut pool) = self.pool(seat) else {
                continue;
            };
            let keeper = self
                .permanents(seat)
                .into_iter()
                .find(|c| c.bind().static_effect == Some(StaticEffect::KeepMana));
            let total = pool.bind().get_total();
            match keeper {
                Some(card) if total > 0 => {
                    let name = card.bind().display_name();
                    self.log(trf!(
                        "{} behält {total} Mana ({name}).",
                        "{} keeps {total} mana ({name}).",
                        self.name(seat)
                    ));
                }
                Some(_) => {}
                None => pool.bind_mut().empty_pool(),
            }
        }
    }

    // ------------------------------------------------------------ Kampf

    /// Angreifer festlegen: tappen, Raufbold-Bonus als temporärer Effekt.
    pub fn commit_attackers(&mut self) {
        let attackers = self.combat.attackers.clone();
        for mut card in attackers {
            Self::set_tapped(&mut card, true);
            // Hat angegriffen: leicht schräg (getappt) und ausgegraut
            card.bind_mut().set_attacked(true);
            let (ability, name) = {
                let c = card.bind();
                (c.ability_kind(), c.display_name())
            };
            if ability == CardAbility::Rage {
                card.bind_mut().add_attack_bonus(1);
                self.temp_effects.push(TempEffect { card: card.clone(), attack_bonus: 1 });
                self.log(trf!("{name} gerät in Rage (+1 Angriff).", "{name} becomes enraged (+1 attack)."));
            }
        }
        let n = self.combat.attackers.len();
        if n > 0 {
            self.log(trf!("{} greift mit {n} Kreatur(en) an.", "{} attacks with {n} creature(s).", self.name(self.active)));
        }
    }

    pub fn toggle_attacker(&mut self, mut card: Gd<Card3D>) {
        if let Some(pos) = self.combat.attackers.iter().position(|c| c == &card) {
            self.combat.attackers.remove(pos);
            card.bind_mut().set_marker(CardMarker::None);
        } else if card.bind().is_creature() && !card.bind().is_tapped() && card.bind().get_current_attack() > 0 {
            self.combat.attackers.push(card.clone());
            card.bind_mut().set_marker(CardMarker::Attacking);
        }
    }

    /// Klick des menschlichen Verteidigers: eigene Kreatur wählen, dann Angreifer.
    pub fn blocker_click(&mut self, mut card: Gd<Card3D>) -> bool {
        let defender = self.defender();
        if self.is_on_battlefield(&card, defender) {
            if !card.bind().is_creature() {
                return true;
            }
            // Bereits zugewiesen? -> Zuweisung lösen
            if let Some(pos) = self.combat.blocks.iter().position(|(b, _)| b == &card) {
                self.combat.blocks.remove(pos);
                card.bind_mut().set_marker(CardMarker::None);
                return true;
            }
            if card.bind().is_tapped() {
                return true;
            }
            if let Some(mut old) = self.combat.selected.take() {
                old.bind_mut().set_marker(CardMarker::None);
            }
            card.bind_mut().set_marker(CardMarker::Selected);
            self.combat.selected = Some(card);
            return true;
        }
        if self.combat.attackers.contains(&card) {
            if let Some(blocker) = self.combat.selected.clone() {
                self.assign_block(blocker, card);
            }
            return true;
        }
        false
    }

    /// Weist `blocker` (ungetappte Kreatur des Verteidigers) dem Angreifer `attacker` zu – per
    /// Klickfolge oder gezogenem Pfeil. Ein Blocker pro Angreifer: eine bestehende Zuweisung
    /// des Blockers oder des Angreifers wird ersetzt. false = ungültig.
    pub fn assign_block(&mut self, mut blocker: Gd<Card3D>, attacker: Gd<Card3D>) -> bool {
        let valid = self.combat.attackers.contains(&attacker)
            && self.is_on_battlefield(&blocker, self.defender())
            && blocker.bind().is_creature()
            && !blocker.bind().is_tapped();
        if !valid {
            return false;
        }
        if let Some(mut sel) = self.combat.selected.take() {
            sel.bind_mut().set_marker(CardMarker::None);
        }
        self.combat.blocks.retain(|(b, _)| b != &blocker);
        if let Some(pos) = self.combat.blocks.iter().position(|(_, a)| a == &attacker) {
            let (mut old, _) = self.combat.blocks.remove(pos);
            old.bind_mut().set_marker(CardMarker::None);
        }
        blocker.bind_mut().set_marker(CardMarker::Blocking);
        self.combat.blocks.push((blocker, attacker));
        true
    }

    /// Schadensberechnung: Blocker und Angreifer tauschen Schaden aus,
    /// ungeblockte Angreifer treffen den Verteidiger.
    pub fn resolve_combat(&mut self) {
        let defender = self.defender();
        let attackers = self.combat.attackers.clone();
        let mut dead: Vec<Gd<Card3D>> = Vec::new();
        for mut attacker in attackers {
            let blocker = self
                .combat
                .blocks
                .iter()
                .find(|(_, a)| a == &attacker)
                .map(|(b, _)| b.clone());
            let (atk, aname) = {
                let a = attacker.bind();
                (a.get_current_attack(), a.display_name())
            };
            match blocker {
                Some(mut blocker) => {
                    let (batk, bname) = {
                        let b = blocker.bind();
                        (b.get_current_attack(), b.display_name())
                    };
                    blocker.bind_mut().add_damage(atk);
                    attacker.bind_mut().add_damage(batk);
                    self.log(trf!("{aname} ({atk}) kämpft gegen {bname} ({batk}).", "{aname} ({atk}) fights {bname} ({batk})."));
                    for c in [&attacker, &blocker] {
                        if c.bind().get_remaining_defense() <= 0 && !dead.contains(c) {
                            dead.push(c.clone());
                        }
                    }
                }
                None => {
                    self.change_life(defender, -atk);
                    self.log(trf!("{aname} trifft {} für {atk}.", "{aname} hits {} for {atk}.", self.name(defender)));
                }
            }
        }
        for mut card in dead {
            self.destroy(&mut card);
        }
    }

    pub fn clear_combat(&mut self) {
        let combat = std::mem::take(&mut self.combat);
        let marked = combat
            .attackers
            .into_iter()
            .chain(combat.blocks.into_iter().map(|(b, _)| b))
            .chain(combat.selected);
        for mut card in marked {
            if card.is_instance_valid() {
                card.bind_mut().set_marker(CardMarker::None);
            }
        }
    }

    /// End-Phase: temporäre Effekte und Schaden entfernen.
    pub fn cleanup(&mut self) {
        for effect in std::mem::take(&mut self.temp_effects) {
            let mut card = effect.card;
            if card.is_instance_valid() {
                card.bind_mut().add_attack_bonus(-effect.attack_bonus);
            }
        }
        for seat in 0..2 {
            for mut card in self.creatures(seat) {
                card.bind_mut().clear_turn_effects();
            }
        }
        self.clear_combat();
    }

    // ------------------------------------------------------------ KI

    /// KI-Hauptphase, schrittweise: Ressource legen und Ausrüstung anlegen
    /// (beides ohne Stack), danach **genau eine** Aktion, die auf den Stack geht
    /// (Karte wirken, Heldenfähigkeit, Artefakt-Fähigkeit). Gibt true zurück, wenn
    /// etwas auf den Stack kam – der TurnManager ruft die Funktion erneut auf,
    /// sobald der Stack wieder leer ist.
    pub fn ai_play_cards(&mut self, seat: usize, phase: Phase) -> bool {
        let hand = Self::cards_in(self.hand(seat));

        if self.seats[seat].bind().resources_played == 0 {
            let needed = |t: ManaType| -> u32 { hand.iter().map(|c| c.bind().cost().specific.get(t)).sum() };
            let have = self.potential_mana(seat);
            let resource = hand
                .iter()
                .filter(|c| c.bind().is_resource())
                .max_by_key(|c| {
                    let t = c.bind().produced_mana();
                    needed(t).saturating_sub(have.get(t))
                })
                .cloned();
            if let Some(card) = resource {
                if let Some(mut zone) = self.play_from_hand(seat, &card, phase) {
                    Self::move_card(&card, &mut zone, 0.0);
                }
            }
        }

        // Freie Ausrüstung an die stärkste Kreatur anlegen
        for mut eq in self.of_kind(seat, CardKind::Equipment) {
            if eq.bind().equipped_to.is_some() {
                continue;
            }
            if let Some(mut host) = self.creatures(seat).into_iter().max_by_key(|c| c.bind().get_current_attack()) {
                let _ = self.equip(seat, &mut eq, &mut host, phase);
            }
        }

        // Karten aus der Hand: bleibende zuerst, dann Zauber; jeweils teuerste zuerst
        let mut cards: Vec<Gd<Card3D>> = Self::cards_in(self.hand(seat))
            .into_iter()
            .filter(|c| !c.bind().is_resource())
            .collect();
        cards.sort_by_key(|c| {
            let c = c.bind();
            (!c.kind().is_permanent(), std::cmp::Reverse(c.cost().total()))
        });
        for card in cards {
            let kind = card.bind().kind();
            if self.check_card(seat, &card, phase).is_err() {
                continue;
            }
            let zone = match kind {
                CardKind::Aura => match self.ai_aura_target(seat, &card) {
                    Some(target) => self.cast_aura(seat, &card, &target, phase),
                    None => None,
                },
                CardKind::Instant | CardKind::Sorcery if !self.spell_useful(seat, &card) => None,
                _ => self.play_from_hand(seat, &card, phase),
            };
            if let Some(mut zone) = zone {
                Self::move_card(&card, &mut zone, 0.0);
                return true;
            }
        }

        // Heldenfähigkeit
        for mut hero in self.of_kind(seat, CardKind::Hero) {
            let has_target = !self.creatures(1 - seat).is_empty();
            let order = if has_target { [1, 0] } else { [0, 1] };
            for idx in order {
                let keeps_alive = hero.bind().hero_abilities.get(idx).is_some_and(|a| hero.bind().willpower + a.delta > 0);
                if keeps_alive && self.activate_hero(seat, &mut hero, idx, phase).is_ok() {
                    return true;
                }
            }
        }

        // Artefakt-Fähigkeiten
        for mut art in self.of_kind(seat, CardKind::Artifact) {
            if art.bind().activated.is_some() && self.activate_artifact(seat, &mut art, phase).is_ok() {
                return true;
            }
        }
        false
    }

    /// Greift mit allen Kreaturen an, die kein Verteidiger gefahrlos abfangen kann.
    pub fn ai_declare_attackers(&mut self) {
        let defender = self.defender();
        let blockers = self.untapped_creatures(defender);
        for card in self.untapped_creatures(self.active) {
            let (atk, def) = {
                let c = card.bind();
                (c.get_current_attack(), c.get_remaining_defense())
            };
            let dies_to_block = blockers.iter().any(|b| {
                let b = b.bind();
                b.get_current_attack() >= def && b.get_remaining_defense() > atk
            });
            if atk > 0 && !dies_to_block {
                self.toggle_attacker(card);
            }
        }
    }

    /// Blockt, wenn der Blocker überlebt – bei wenig Leben auch als Opfer.
    pub fn ai_declare_blockers(&mut self) {
        let defender = self.defender();
        let mut free = self.untapped_creatures(defender);
        let mut attackers = self.combat.attackers.clone();
        attackers.sort_by_key(|a| -a.bind().get_current_attack());
        let incoming: i32 = attackers.iter().map(|a| a.bind().get_current_attack()).sum();
        let desperate = self.life(defender) - incoming <= 5;
        for attacker in attackers {
            let atk = attacker.bind().get_current_attack();
            let pick = free
                .iter()
                .position(|b| b.bind().get_remaining_defense() > atk)
                .or(if desperate && !free.is_empty() { Some(0) } else { None });
            if let Some(i) = pick {
                let mut blocker = free.remove(i);
                blocker.bind_mut().set_marker(CardMarker::Blocking);
                let (bn, an) = (blocker.bind().display_name(), attacker.bind().display_name());
                self.log(trf!("{bn} blockt {an}.", "{bn} blocks {an}."));
                self.combat.blocks.push((blocker, attacker));
            }
        }
    }
}
