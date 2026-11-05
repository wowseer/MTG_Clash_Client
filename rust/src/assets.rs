//! Schalter „nur eigene Grafik“: zeigt der Tisch Material, das urheberrechtlich geschützt sein
//! kann – Kartenbilder (Scryfall-JPGs mit Artworks) und die Manasymbole aus dem Symbolbogen
//! `Assets/Mana/Mana.svg` / `Colorless.svg`?
//!
//! Aus mit der Umgebungsvariable `CLASH_FREE_ASSETS=1` oder dem Programmargument
//! `--free-assets` (nach `--`). Dann nimmt der Tisch nur selbst erstellte Grafik: eigene
//! Kartenrahmen mit Typsymbolen und die selbst gezeichneten Manasymbole (Sonne, Tropfen, Schädel,
//! Flamme, Baum, Raute) – z. B. für Bilder in der Projektbeschreibung.

use std::sync::OnceLock;

use godot::obj::Singleton;

/// true = Kartenbilder und Symbolbogen verwenden (Standard), false = nur eigene Grafik.
pub fn protected_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        let env_off = std::env::var("CLASH_FREE_ASSETS").is_ok_and(|v| !v.is_empty() && v != "0");
        let arg_off = godot::classes::Os::singleton()
            .get_cmdline_user_args()
            .as_slice()
            .iter()
            .any(|a| a.to_string() == "--free-assets");
        !(env_off || arg_off)
    })
}
