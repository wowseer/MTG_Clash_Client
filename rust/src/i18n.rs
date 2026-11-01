//! Zweisprachigkeit (Deutsch / Englisch).
//!
//! - [`lang`] / [`set_lang`]: aktuelle Sprache (global, Standard Deutsch).
//! - `tr!("deutsch", "english")`: fester Text in der aktuellen Sprache.
//! - `trf!("Hallo {name}", "Hello {name}")`: Text mit Werten (wie `format!`, auch
//!   mit eingebetteten Variablen).
//! - [`name`]: übersetzt Eigennamen (Karten, Sitzplätze). Unbekannte Namen – z. B.
//!   selbst erstellte Karten – bleiben unverändert.
//!
//! Neue Texte immer mit beiden Sprachen über `tr!`/`trf!` schreiben, neue Kartennamen
//! in [`name`] eintragen.

use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    De,
    En,
}

static LANG: AtomicU8 = AtomicU8::new(0);

pub fn lang() -> Lang {
    if LANG.load(Ordering::Relaxed) == 1 { Lang::En } else { Lang::De }
}

pub fn set_lang(lang: Lang) {
    LANG.store(if lang == Lang::En { 1 } else { 0 }, Ordering::Relaxed);
}

/// Fester Text in der aktuellen Sprache: `tr!("Abbrechen", "Cancel")`.
#[macro_export]
macro_rules! tr {
    ($de:expr, $en:expr $(,)?) => {
        match $crate::i18n::lang() {
            $crate::i18n::Lang::De => $de,
            $crate::i18n::Lang::En => $en,
        }
    };
}

/// Text mit Werten (wie `format!`): `trf!("{n} Karten", "{n} cards")`.
#[macro_export]
macro_rules! trf {
    ($de:literal, $en:literal $(, $arg:expr)* $(,)?) => {
        match $crate::i18n::lang() {
            $crate::i18n::Lang::De => format!($de $(, $arg)*),
            $crate::i18n::Lang::En => format!($en $(, $arg)*),
        }
    };
}

/// Übersetzt einen Eigennamen (deutscher Originalname -> Anzeige in aktueller Sprache).
pub fn name(de: &str) -> String {
    if lang() == Lang::De {
        return de.to_string();
    }
    let en = match de {
        // Sitzplätze
        "Spieler" => "Player",
        "Gegner" => "Opponent",
        // Ressourcen
        "Kapellenglocke" => "Chapel Bell",
        "Brunnenquelle" => "Wellspring",
        "Kellergewölbe" => "Cellar Vault",
        "Glutherd" => "Ember Hearth",
        "Kräutergarten" => "Herb Garden",
        "Alter Steinkreis" => "Old Stone Circle",
        // Kreaturen
        "Schankmaid" => "Barmaid",
        "Taverndieb" => "Tavern Thief",
        "Söldnerin" => "Mercenary",
        "Wanderbarde" => "Wandering Bard",
        "Trunkenbold" => "Drunkard",
        "Hofhund" => "Yard Dog",
        "Feuerschlucker" => "Fire Eater",
        "Kräuterhexe" => "Herb Witch",
        "Zwergenschmied" => "Dwarf Smith",
        "Würfelspieler" => "Dice Player",
        "Rausschmeißer" => "Bouncer",
        "Rattenfänger" => "Rat Catcher",
        "Pilgerin" => "Pilgrim",
        "Kesselflicker" => "Tinker",
        "Nachtwächter" => "Night Watchman",
        // Helden
        "Wirtin Brunhild" => "Innkeeper Brunhild",
        "Schmuggler Veit" => "Smuggler Veit",
        "Barde Lianne" => "Bard Lianne",
        "Hauptmann Ruk" => "Captain Ruk",
        // Zauber
        "Kneipenschlägerei" => "Bar Brawl",
        "Kräutertrank" => "Herbal Draught",
        "Klarer Kopf" => "Clear Head",
        "Giftiger Schnaps" => "Poisoned Schnapps",
        "Stoßgebet" => "Quick Prayer",
        "Brandfass" => "Fire Barrel",
        "Erntedank" => "Harvest Blessing",
        "Ausschank" => "Last Round",
        "Zechprellerei" => "Dine and Dash",
        "Sammelbüchse" => "Collection Box",
        "Hausverbot" => "Banned from the Inn",
        // Ausrüstung / Artefakte / Verzauberungen / Auren
        "Schankmesser" => "Bar Knife",
        "Eisenkrughelm" => "Iron Tankard Helm",
        "Kriegstrommel" => "War Drum",
        "Bierfass" => "Ale Barrel",
        "Gemeinsames Gelage" => "Shared Feast",
        "Dichter Tabakrauch" => "Thick Pipe Smoke",
        "Wirtshaussegen" => "Inn's Blessing",
        "Fluch des Katers" => "Hangover Curse",
        "Goldader" => "Gold Vein",
        "Rostfluch" => "Rust Curse",
        // Debug-Standardname
        "Eigene Karte" => "Custom Card",
        "Karte" => "Card",
        other => other,
    };
    en.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switch_language() {
        set_lang(Lang::En);
        let n = 3;
        assert_eq!(trf!("{n} Karten", "{n} cards"), "3 cards");
        assert_eq!(tr!("Abbrechen", "Cancel"), "Cancel");
        assert_eq!(name("Hausverbot"), "Banned from the Inn");
        assert_eq!(name("Selbst erfunden"), "Selbst erfunden");
        set_lang(Lang::De);
        assert_eq!(trf!("{} Karten", "{} cards", n), "3 Karten");
        assert_eq!(name("Hausverbot"), "Hausverbot");
    }
}
