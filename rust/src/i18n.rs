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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switch_language() {
        set_lang(Lang::En);
        let n = 3;
        assert_eq!(trf!("{n} Karten", "{n} cards"), "3 cards");
        assert_eq!(tr!("Abbrechen", "Cancel"), "Cancel");
        set_lang(Lang::De);
        assert_eq!(trf!("{} Karten", "{} cards", n), "3 Karten");
    }
}
