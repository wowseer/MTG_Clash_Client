//! Regeltext für die Anzeige: Symbole der Engine-Schreibweise (`{T}`, `{2}{R}`, CR 107) durch
//! einzelne Unicode-Zeichen ersetzen und eine Schriftgröße wählen, mit der ein Text in ein
//! Feld passt.

/// Ersetzt Symbole in geschweiften Klammern: Tappen `{T}` → „↷“, Enttappen `{Q}` → „↶“,
/// Mana `{W}{U}{B}{R}{G}{C}{X}{S}` → umkreiste Buchstaben (Ⓦ Ⓤ Ⓑ Ⓡ Ⓖ Ⓒ Ⓧ Ⓢ), Zahlen 0–20 →
/// umkreiste Zahlen (⓪ ① …), Energie `{E}` → „⚡“. Hybride und phyrexianische Symbole
/// (`{R/G}`, `{R/P}`) werden als „(R/G)“ geschrieben, Unbekanntes bleibt stehen.
pub fn with_symbols(text: &str) -> String {
    tidy(&replace_symbols(text))
}

/// Leerzeilen und Leerzeichen am Zeilenende entfernen (Absätze = einfacher Zeilenumbruch).
pub fn tidy(text: &str) -> String {
    text.lines().map(str::trim_end).filter(|l| !l.trim().is_empty()).collect::<Vec<_>>().join("\n")
}

fn replace_symbols(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let Some(end) = after.find('}') else {
            out.push_str(&rest[start..]);
            return out;
        };
        out.push_str(&symbol(&after[..end]));
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

fn symbol(inner: &str) -> String {
    let circled = |c: char| -> Option<char> {
        // Ⓐ = U+24B6 … Ⓩ = U+24CF
        c.is_ascii_uppercase().then(|| char::from_u32(0x24B6 + (c as u32 - 'A' as u32))).flatten()
    };
    match inner {
        "T" => "↷".into(),
        "Q" => "↶".into(),
        "E" => "⚡".into(),
        "W" | "U" | "B" | "R" | "G" | "C" | "X" | "Y" | "S" => {
            circled(inner.chars().next().unwrap_or('?')).map(String::from).unwrap_or_else(|| inner.into())
        }
        _ => match inner.parse::<u32>() {
            Ok(0) => "⓪".into(),
            // ① = U+2460 … ⑳ = U+2473
            Ok(n @ 1..=20) => char::from_u32(0x2460 + n - 1).map(String::from).unwrap_or_default(),
            Ok(n) => format!("({n})"),
            Err(_) if inner.contains('/') => format!("({inner})"),
            Err(_) => format!("{{{inner}}}"),
        },
    }
}

/// Größte Schriftgröße (höchstens `max`, mindestens `min`), bei der `text` in ein Feld der
/// Breite × Höhe `area` (Quadrat-Schriftpunkte bei Schriftgröße 1, siehe Aufrufer) mit
/// Zeilenumbruch etwa passt. Schätzung: Zeichen sind 0,55 Schriftgrößen breit, Zeilen
/// 1,25 Schriftgrößen hoch, 40 % Verschnitt (Zeilenenden, Ränder).
pub fn fit_font(text: &str, area: f32, min: i32, max: i32) -> i32 {
    let chars = text.chars().count().max(1) as f32;
    let size = (area * 0.6 / (chars * 0.55 * 1.25)).sqrt();
    (size as i32).clamp(min, max)
}

/// Größte Schriftgröße (höchstens `max`, mindestens `min`), bei der eine Zeile `text` in die
/// Breite `width` (in Schriftpunkten bei Größe 1) passt.
pub fn fit_line(text: &str, width: f32, min: i32, max: i32) -> i32 {
    let chars = text.chars().count().max(1) as f32;
    ((width / (chars * 0.55)) as i32).clamp(min, max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_symbols() {
        assert_eq!(with_symbols("{T}: Add {R}."), "↷: Add Ⓡ.");
        assert_eq!(with_symbols("{2}{G}{G}"), "②ⒼⒼ");
        assert_eq!(with_symbols("Pay {R/P} or {0}"), "Pay (R/P) or ⓪");
        assert_eq!(with_symbols("open { brace"), "open { brace");
        assert_eq!(with_symbols("{25}"), "(25)");
        assert_eq!(with_symbols("a.   \n\nb."), "a.\nb.");
    }

    #[test]
    fn fits_fonts() {
        assert_eq!(fit_font("short", 100_000.0, 10, 30), 30);
        assert!(fit_font(&"x".repeat(600), 100_000.0, 10, 30) < fit_font(&"x".repeat(100), 100_000.0, 10, 30));
        assert_eq!(fit_line("abc", 1000.0, 8, 36), 36);
        assert!(fit_line("Chandra, Torch of Defiance", 400.0, 8, 36) < 36);
    }
}
