//! Anzeige von Marken (CR 122) auf Karten und Spielern: Markenname der Engine (forge-rules-rs /
//! Forge, z. B. `P1P1`, `LOYALTY`, `POISON`) → kurzer, zweisprachiger Anzeigetext.

/// Anzeigetext einer Marke mit Anzahl, z. B. „+1/+1 ×2“, „Loyalität 3“, „Gift 4“.
pub fn counter_text(name: &str, count: u32) -> String {
    match name {
        "P1P1" => format!("+1/+1 ×{count}"),
        "M1M1" => format!("−1/−1 ×{count}"),
        _ => format!("{} {count}", counter_name(name)),
    }
}

/// Name einer Marke ohne Anzahl.
pub fn counter_name(name: &str) -> String {
    let known = match name {
        "LOYALTY" => Some(tr!("Loyalität", "Loyalty")),
        "DEFENSE" => Some(tr!("Verteidigung", "Defense")),
        "LORE" => Some(tr!("Kunde", "Lore")),
        "POISON" => Some(tr!("Gift", "Poison")),
        "ENERGY" => Some(tr!("Energie", "Energy")),
        "EXPERIENCE" => Some(tr!("Erfahrung", "Experience")),
        "TIME" => Some(tr!("Zeit", "Time")),
        "CHARGE" => Some(tr!("Ladung", "Charge")),
        "SHIELD" => Some(tr!("Schild", "Shield")),
        "STUN" => Some(tr!("Betäubung", "Stun")),
        "OIL" => Some(tr!("Öl", "Oil")),
        "RAD" => Some(tr!("Strahlung", "Rad")),
        "TICKET" => Some(tr!("Ticket", "Ticket")),
        _ => None,
    };
    match known {
        Some(t) => t.to_string(),
        // Unbekannt: Engine-Name in Kleinbuchstaben mit großem Anfangsbuchstaben
        None => {
            let lower = name.to_lowercase().replace('_', " ");
            let mut chars = lower.chars();
            chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
        }
    }
}

/// Plakette einer Karte: „Spielstein“ (bei Spielsteinen), sonst leer. Marken zeigt die Karte
/// als Münzen (`Card3D::set_counters`).
pub fn card_badges(token: bool) -> String {
    if token { tr!("Spielstein", "Token").to_string() } else { String::new() }
}

/// Marken eines Spielers in einer Zeile, z. B. „Gift 3 · Energie 2“ (leer = keine).
pub fn player_counters(counters: &[(String, u32)]) -> String {
    counters.iter().filter(|(_, n)| *n > 0).map(|(name, n)| counter_text(name, *n)).collect::<Vec<_>>().join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_counters() {
        assert_eq!(counter_text("P1P1", 2), "+1/+1 ×2");
        assert_eq!(counter_text("M1M1", 1), "−1/−1 ×1");
        assert_eq!(counter_name("SOME_THING"), "Some thing");
        assert_eq!(card_badges(false), "");
        assert_eq!(card_badges(true), "Spielstein");
    }
}
