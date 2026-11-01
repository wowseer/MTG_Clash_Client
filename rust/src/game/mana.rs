//! Fünffarbiges Mana-System.
//!
//! - [`ManaType`]: Weiß, Blau, Schwarz, Rot, Grün, Farblos.
//! - [`Mana`] / [`Cost`]: reine Rust-Logik (ohne Godot, per `cargo test` geprüft).
//! - [`ManaCost`]: Godot-Resource für Kartenkosten (im Inspector editierbar).
//! - [`ManaPool`]: Godot-Node pro Spieler, verwaltet den aktuellen Vorrat.
//!
//! Kostenregeln:
//! - Farbige Anteile (z. B. 2× Rot) müssen mit genau dieser Farbe bezahlt werden.
//! - Der Anteil `farblos` muss mit farblosem Mana bezahlt werden.
//! - Der Anteil `generisch` darf mit beliebigem Mana bezahlt werden.
//!
//! Textschreibweise (für `ManaCost.parse`): Zahl = generisch, Buchstaben je Farbe:
//! `W` Weiß, `B` Blau, `S` Schwarz, `R` Rot, `G` Grün, `F` Farblos.
//! Beispiel: `"2RR"` = 2 beliebige + 2 Rote.

use godot::prelude::*;

// ================================================================== ManaType

/// Manafarbe. Als Zahl: 0 = Weiß, 1 = Blau, 2 = Schwarz, 3 = Rot, 4 = Grün, 5 = Farblos.
#[derive(GodotConvert, Var, Export, Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[godot(via = i64)]
pub enum ManaType {
    White,
    Blue,
    Black,
    Red,
    Green,
    Colorless,
}

impl ManaType {
    pub const ALL: [ManaType; 6] = [
        ManaType::White,
        ManaType::Blue,
        ManaType::Black,
        ManaType::Red,
        ManaType::Green,
        ManaType::Colorless,
    ];

    pub const fn index(self) -> usize {
        self as usize
    }

    pub fn from_index(i: usize) -> Option<Self> {
        Self::ALL.get(i).copied()
    }

    pub fn is_colored(self) -> bool {
        self != ManaType::Colorless
    }

    pub fn name(self) -> &'static str {
        match self {
            ManaType::White => "Weiß",
            ManaType::Blue => "Blau",
            ManaType::Black => "Schwarz",
            ManaType::Red => "Rot",
            ManaType::Green => "Grün",
            ManaType::Colorless => "Farblos",
        }
    }

    /// Kürzel für die Textschreibweise.
    pub fn letter(self) -> char {
        match self {
            ManaType::White => 'W',
            ManaType::Blue => 'B',
            ManaType::Black => 'S',
            ManaType::Red => 'R',
            ManaType::Green => 'G',
            ManaType::Colorless => 'F',
        }
    }

    pub fn from_letter(c: char) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.letter() == c.to_ascii_uppercase())
    }

    /// Anzeigefarbe (z. B. für HUD-Symbole).
    pub fn color(self) -> Color {
        match self {
            ManaType::White => Color::from_rgb(0.97, 0.94, 0.8),
            ManaType::Blue => Color::from_rgb(0.3, 0.55, 0.95),
            ManaType::Black => Color::from_rgb(0.35, 0.3, 0.35),
            ManaType::Red => Color::from_rgb(0.92, 0.3, 0.2),
            ManaType::Green => Color::from_rgb(0.3, 0.7, 0.35),
            ManaType::Colorless => Color::from_rgb(0.7, 0.68, 0.65),
        }
    }
}

// ================================================================== Reine Logik

/// Mana-Mengen je Typ (Index = `ManaType::index`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Mana(pub [u32; 6]);

impl Mana {
    pub fn get(&self, t: ManaType) -> u32 {
        self.0[t.index()]
    }

    pub fn add(&mut self, t: ManaType, amount: u32) {
        self.0[t.index()] += amount;
    }

    pub fn total(&self) -> u32 {
        self.0.iter().sum()
    }

    pub fn is_empty(&self) -> bool {
        self.total() == 0
    }
}

/// Kosten: feste Anteile je Typ plus generischer Anteil (beliebiges Mana).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Cost {
    /// Muss mit genau diesem Typ bezahlt werden (inkl. farblos).
    pub specific: Mana,
    /// Darf mit beliebigem Mana bezahlt werden.
    pub generic: u32,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    UnknownSymbol(char),
}

impl Cost {
    /// Gesamter Manawert der Kosten.
    pub fn total(&self) -> u32 {
        self.specific.total() + self.generic
    }

    /// Liest z. B. `"2RR"`, `"WB"`, `"3"`, `"1F"` (Leerzeichen erlaubt).
    pub fn parse(text: &str) -> Result<Cost, ParseError> {
        let mut cost = Cost::default();
        let mut number = String::new();
        for c in text.chars().filter(|c| !c.is_whitespace()) {
            if c.is_ascii_digit() {
                number.push(c);
                continue;
            }
            match ManaType::from_letter(c) {
                Some(t) => cost.specific.add(t, 1),
                None => return Err(ParseError::UnknownSymbol(c)),
            }
        }
        // Alle Ziffern zusammen bilden den generischen Anteil ("12R" = 12 + Rot)
        if !number.is_empty() {
            cost.generic = number.parse().unwrap_or(0);
        }
        Ok(cost)
    }

    /// Kurzschreibweise, z. B. `"2RR"`; kostenlos = `"0"`.
    pub fn to_text(&self) -> String {
        let mut s = String::new();
        if self.generic > 0 || self.specific.is_empty() {
            s.push_str(&self.generic.to_string());
        }
        for t in ManaType::ALL {
            for _ in 0..self.specific.get(t) {
                s.push(t.letter());
            }
        }
        s
    }

    /// Kann `pool` diese Kosten bezahlen?
    pub fn can_pay(&self, pool: &Mana) -> bool {
        self.payment(pool).is_some()
    }

    /// Berechnet, welches Mana zum Bezahlen verwendet wird (ohne etwas abzuziehen).
    ///
    /// Generische Kosten werden zuerst mit farblosem Mana bezahlt, danach mit
    /// der Farbe, von der am meisten übrig ist – so bleiben seltene Farben
    /// möglichst für spätere Karten erhalten.
    pub fn payment(&self, pool: &Mana) -> Option<Mana> {
        let mut used = Mana::default();
        let mut rest = *pool;
        // 1) Feste Anteile
        for t in ManaType::ALL {
            let need = self.specific.get(t);
            if rest.get(t) < need {
                return None;
            }
            rest.0[t.index()] -= need;
            used.0[t.index()] += need;
        }
        // 2) Generischer Anteil
        let mut generic = self.generic;
        if rest.total() < generic {
            return None;
        }
        let take = rest.get(ManaType::Colorless).min(generic);
        rest.0[ManaType::Colorless.index()] -= take;
        used.0[ManaType::Colorless.index()] += take;
        generic -= take;
        while generic > 0 {
            let t = ManaType::ALL
                .into_iter()
                .filter(|t| t.is_colored())
                .max_by_key(|t| rest.get(*t))
                .expect("mindestens eine Farbe");
            rest.0[t.index()] -= 1;
            used.0[t.index()] += 1;
            generic -= 1;
        }
        Some(used)
    }

    /// Zieht die Kosten von `pool` ab. Gibt das verwendete Mana zurück;
    /// bei zu wenig Mana bleibt der Pool unverändert (alles oder nichts).
    pub fn pay(&self, pool: &mut Mana) -> Option<Mana> {
        let used = self.payment(pool)?;
        for i in 0..6 {
            pool.0[i] -= used.0[i];
        }
        Some(used)
    }
}

// ================================================================== ManaCost (Resource)

/// Kosten einer Karte, als Resource im Inspector editierbar.
#[derive(GodotClass)]
#[class(base=Resource, init)]
pub struct ManaCost {
    /// Beliebiges Mana.
    #[export]
    generic: i32,
    /// Weißes Mana, das mit Weiß bezahlt werden muss.
    #[export]
    white: i32,
    /// Blaues Mana, das mit Blau bezahlt werden muss.
    #[export]
    blue: i32,
    /// Schwarzes Mana, das mit Schwarz bezahlt werden muss.
    #[export]
    black: i32,
    /// Rotes Mana, das mit Rot bezahlt werden muss.
    #[export]
    red: i32,
    /// Grünes Mana, das mit Grün bezahlt werden muss.
    #[export]
    green: i32,
    /// Muss mit farblosem Mana bezahlt werden.
    #[export]
    colorless: i32,
    base: Base<Resource>,
}

#[godot_api]
impl ManaCost {
    /// Erzeugt Kosten aus der Textschreibweise, z. B. `"2RR"`. Ungültig -> null.
    #[func]
    pub fn parse(text: GString) -> Option<Gd<ManaCost>> {
        match Cost::parse(&text.to_string()) {
            Ok(cost) => Some(Self::from_cost(cost)),
            Err(ParseError::UnknownSymbol(c)) => {
                godot_error!("ManaCost.parse: unbekanntes Symbol '{c}' in \"{text}\"");
                None
            }
        }
    }

    /// Erzeugt Kosten aus einem Dictionary `{ ManaType: Menge, "generic": Menge }`.
    #[func]
    pub fn from_dictionary(dict: VarDictionary) -> Gd<ManaCost> {
        let mut cost = Cost::default();
        for (key, value) in dict.iter_shared() {
            let amount = value.try_to::<i64>().unwrap_or(0).max(0) as u32;
            if key.try_to::<GString>().is_ok_and(|k| k == "generic") {
                cost.generic += amount;
            } else if let Some(t) = key.try_to::<i64>().ok().and_then(|i| ManaType::from_index(i as usize)) {
                cost.specific.add(t, amount);
            }
        }
        Self::from_cost(cost)
    }

    /// Prüft, ob der Pool die Kosten bezahlen kann.
    #[func]
    pub fn can_pay(&self, pool: Gd<ManaPool>) -> bool {
        self.cost().can_pay(&pool.bind().mana)
    }

    /// Gesamter Manawert.
    #[func]
    pub fn get_total(&self) -> i64 {
        self.cost().total() as i64
    }

    /// Kurzschreibweise, z. B. "2RR".
    #[func]
    pub fn to_text(&self) -> GString {
        GString::from(self.cost().to_text().as_str())
    }
}

impl ManaCost {
    pub fn cost(&self) -> Cost {
        let n = |v: i32| v.max(0) as u32;
        Cost {
            specific: Mana([n(self.white), n(self.blue), n(self.black), n(self.red), n(self.green), n(self.colorless)]),
            generic: n(self.generic),
        }
    }

    pub fn from_cost(cost: Cost) -> Gd<ManaCost> {
        let s = cost.specific.0.map(|v| v as i32);
        Gd::from_init_fn(|base| ManaCost {
            generic: cost.generic as i32,
            white: s[0],
            blue: s[1],
            black: s[2],
            red: s[3],
            green: s[4],
            colorless: s[5],
            base,
        })
    }
}

// ================================================================== ManaPool (Node)

/// Manavorrat eines Spielers.
#[derive(GodotClass)]
#[class(base=Node, init)]
pub struct ManaPool {
    mana: Mana,
    base: Base<Node>,
}

#[godot_api]
impl ManaPool {
    /// Wird nach jeder Änderung ausgelöst.
    #[signal]
    fn pool_changed();

    /// Fügt Mana hinzu.
    #[func]
    pub fn add_mana(&mut self, mana_type: ManaType, amount: i64) {
        if amount <= 0 {
            return;
        }
        self.mana.add(mana_type, amount as u32);
        self.signals().pool_changed().emit();
    }

    /// Bezahlt die Kosten (alles oder nichts). true = bezahlt.
    #[func]
    pub fn pay_mana(&mut self, cost: Gd<ManaCost>) -> bool {
        let cost = cost.bind().cost();
        self.pay(&cost).is_some()
    }

    /// Wie `pay_mana`, mit Kosten als Dictionary `{ ManaType: Menge, "generic": Menge }`.
    #[func]
    pub fn pay_mana_dict(&mut self, cost: VarDictionary) -> bool {
        self.pay_mana(ManaCost::from_dictionary(cost))
    }

    /// Kann dieser Pool die Kosten `cost` bezahlen?
    #[func]
    pub fn can_pay(&self, cost: Gd<ManaCost>) -> bool {
        cost.bind().cost().can_pay(&self.mana)
    }

    /// Vorrat einer Farbe (0 = Weiß, 1 = Blau, 2 = Schwarz, 3 = Rot, 4 = Grün, 5 = Farblos).
    #[func]
    pub fn get_amount(&self, mana_type: ManaType) -> i64 {
        self.mana.get(mana_type) as i64
    }

    /// Gesamtes Mana im Pool (alle Farben zusammen).
    #[func]
    pub fn get_total(&self) -> i64 {
        self.mana.total() as i64
    }

    /// Leert den Pool (z. B. am Ende jeder Phase).
    #[func]
    pub fn empty_pool(&mut self) {
        if !self.mana.is_empty() {
            self.mana = Mana::default();
            self.signals().pool_changed().emit();
        }
    }

    /// Aktueller Vorrat als Dictionary `{ ManaType: Menge }`.
    #[func]
    pub fn to_dictionary(&self) -> VarDictionary {
        let mut d = VarDictionary::new();
        for t in ManaType::ALL {
            d.set(t.index() as i64, self.mana.get(t) as i64);
        }
        d
    }

    /// Anzeigefarbe eines Manatyps (für HUD-Symbole).
    #[func]
    pub fn get_mana_color(mana_type: ManaType) -> Color {
        mana_type.color()
    }

    /// Anzeigename eines Manatyps, z. B. "Rot".
    #[func]
    pub fn get_mana_name(mana_type: ManaType) -> GString {
        GString::from(mana_type.name())
    }

    /// Lesbare Anzeige, z. B. "Rot 2 · Grün 1".
    #[func]
    pub fn to_text(&self) -> GString {
        let parts: Vec<String> = ManaType::ALL
            .into_iter()
            .filter(|t| self.mana.get(*t) > 0)
            .map(|t| format!("{} {}", t.name(), self.mana.get(t)))
            .collect();
        let text = if parts.is_empty() { "leer".to_string() } else { parts.join(" · ") };
        GString::from(text.as_str())
    }
}

impl ManaPool {
    pub fn mana(&self) -> Mana {
        self.mana
    }

    /// Bezahlt Kosten aus Rust; gibt das verwendete Mana zurück.
    pub fn pay(&mut self, cost: &Cost) -> Option<Mana> {
        let used = cost.pay(&mut self.mana)?;
        self.signals().pool_changed().emit();
        Some(used)
    }
}

// ================================================================== Tests

#[cfg(test)]
mod tests {
    use super::*;
    use ManaType::*;

    fn pool(w: u32, b: u32, s: u32, r: u32, g: u32, f: u32) -> Mana {
        Mana([w, b, s, r, g, f])
    }

    #[test]
    fn parse_and_text_roundtrip() {
        let c = Cost::parse("2RR").unwrap();
        assert_eq!(c.generic, 2);
        assert_eq!(c.specific.get(Red), 2);
        assert_eq!(c.total(), 4);
        assert_eq!(c.to_text(), "2RR");
        assert_eq!(Cost::parse("wbg").unwrap().to_text(), "WBG");
        assert_eq!(Cost::parse("").unwrap().to_text(), "0");
        assert_eq!(Cost::parse(" 1 F ").unwrap().to_text(), "1F");
        assert_eq!(Cost::parse("2X"), Err(ParseError::UnknownSymbol('X')));
    }

    #[test]
    fn colored_requirement_must_match() {
        let c = Cost::parse("R").unwrap();
        assert!(!c.can_pay(&pool(5, 5, 5, 0, 5, 5)));
        assert!(c.can_pay(&pool(0, 0, 0, 1, 0, 0)));
    }

    #[test]
    fn generic_uses_any_mana() {
        let c = Cost::parse("3").unwrap();
        assert!(c.can_pay(&pool(1, 1, 0, 0, 1, 0)));
        assert!(!c.can_pay(&pool(1, 1, 0, 0, 0, 0)));
    }

    #[test]
    fn colored_mana_reserved_before_generic() {
        // 2RR mit 3 Rot + 1 Grün: geht (2 Rot fest, Rest 1 Rot + 1 Grün generisch)
        let c = Cost::parse("2RR").unwrap();
        assert!(c.can_pay(&pool(0, 0, 0, 3, 1, 0)));
        // 2RR mit 1 Rot + 5 Grün: geht nicht (nur 1 Rot)
        assert!(!c.can_pay(&pool(0, 0, 0, 1, 5, 0)));
    }

    #[test]
    fn colorless_requirement_only_colorless() {
        let c = Cost::parse("F").unwrap();
        assert!(!c.can_pay(&pool(3, 3, 3, 3, 3, 0)));
        assert!(c.can_pay(&pool(0, 0, 0, 0, 0, 1)));
    }

    #[test]
    fn generic_prefers_colorless_then_most_plentiful() {
        let c = Cost::parse("3G").unwrap();
        let mut p = pool(0, 1, 0, 3, 1, 1);
        let used = c.pay(&mut p).unwrap();
        // G fest; generisch: 1 farblos, dann 2 Rot (größter Vorrat), Blau bleibt
        assert_eq!(used, pool(0, 0, 0, 2, 1, 1));
        assert_eq!(p, pool(0, 1, 0, 1, 0, 0));
    }

    #[test]
    fn failed_payment_leaves_pool_untouched() {
        let c = Cost::parse("2WW").unwrap();
        let mut p = pool(1, 3, 0, 0, 0, 0);
        assert!(c.pay(&mut p).is_none());
        assert_eq!(p, pool(1, 3, 0, 0, 0, 0));
    }

    #[test]
    fn free_cost_always_payable() {
        assert!(Cost::default().can_pay(&Mana::default()));
    }
}
