//! Manasymbole aus `Assets/Mana/Mana.svg` (Symbolbogen vom User, Quelle slightlymagic.net): ein
//! Raster aus 100-px-Symbolen im Abstand von 105 px, zur Laufzeit einmal gerendert und in einzelne
//! Bilder zerlegt. Alle Kostenanzeigen und Manasymbole der Oberfläche nehmen diese Bilder
//! (Kosten über Handkarten, Knöpfe, Beschriftungen, Kartenbild-Platzhalter, Manapool, Farbrad).
//!
//! Raster (Spalte, Zeile): Zeile 0 = 0–9, Zeile 1 = 10–19, Zeile 2 = 20, X, Y, Z, W, U, B, R, G,
//! Schnee; Zeile 3 = Hybride W/U, W/B, U/B, U/R, B/R, B/G, R/W, R/G, G/W, G/U; Zeile 4 = 2/W … 2/G,
//! W/P … G/P (phyrexianisch); Zeile 5 = Tappen, Enttappen, ∞, ½. Ein farbloses Symbol {C} gibt es
//! im Bogen nicht – dafür bleibt die gezeichnete graue Raute.

use std::cell::RefCell;
use std::collections::HashMap;

use godot::classes::image::Format;
use godot::classes::{Image, ImageTexture, RichTextLabel, Texture2D};
use godot::prelude::*;

/// Der Symbolbogen (in die Bibliothek eingebaut, keine Import-Einstellung nötig).
const SHEET: &[u8] = include_bytes!("../../../Assets/Mana/Mana.svg");
/// Abstand und Größe der Symbole im Bogen (SVG-Pixel) und Render-Maßstab (Symbol = 120 px).
const STEP: f32 = 105.0;
const SIZE: f32 = 100.0;
const SCALE: f32 = 1.2;

thread_local! {
    static SHEET_IMAGE: RefCell<Option<Option<Gd<Image>>>> = const { RefCell::new(None) };
    static TEXTURES: RefCell<HashMap<(i32, i32), Gd<Texture2D>>> = RefCell::new(HashMap::new());
}

/// Lage eines Symbols im Bogen für ein Kostensymbol der Engine („R“, „2“, „X“, „W/U“, „2/G“,
/// „R/P“, „T“, „Q“, „S“), sonst None (z. B. „C“, Hybrid-phyrexianisch).
pub fn cell(token: &str) -> Option<(i32, i32)> {
    let t = token.trim().trim_matches(['{', '}']).to_uppercase();
    if let Ok(n) = t.parse::<i32>() {
        return match n {
            0..=9 => Some((n, 0)),
            10..=19 => Some((n - 10, 1)),
            20 => Some((0, 2)),
            _ => None,
        };
    }
    let single = |c: &str| "WUBRG".find(c).map(|i| i as i32);
    match t.as_str() {
        "X" => return Some((1, 2)),
        "Y" => return Some((2, 2)),
        "Z" => return Some((3, 2)),
        "S" => return Some((9, 2)),
        "T" => return Some((0, 5)),
        "Q" => return Some((1, 5)),
        "½" | "1/2" => return Some((3, 5)),
        "∞" => return Some((2, 5)),
        _ => {}
    }
    if let Some(i) = single(&t) {
        return Some((4 + i, 2));
    }
    let parts: Vec<&str> = t.split('/').collect();
    if parts.len() == 2 {
        let (a, b) = (parts[0], parts[1]);
        if a == "2" {
            return single(b).map(|i| (i, 4));
        }
        if b == "P" {
            return single(a).map(|i| (5 + i, 4));
        }
        const HYBRID: [&str; 10] = ["WU", "WB", "UB", "UR", "BR", "BG", "RW", "RG", "GW", "GU"];
        let ab = format!("{a}{b}");
        let ba = format!("{b}{a}");
        return HYBRID.iter().position(|h| *h == ab || *h == ba).map(|i| (i as i32, 3));
    }
    None
}

fn sheet() -> Option<Gd<Image>> {
    SHEET_IMAGE.with(|s| {
        s.borrow_mut()
            .get_or_insert_with(|| {
                let mut img = Image::new_gd();
                let bytes = PackedByteArray::from(SHEET);
                (img.load_svg_from_buffer_ex(&bytes).scale(SCALE).done() == godot::global::Error::OK).then_some(img)
            })
            .clone()
    })
}

/// Bild eines Symbols (120 × 120 px, RGBA), sonst None.
pub fn image(token: &str) -> Option<Gd<Image>> {
    let (c, r) = cell(token)?;
    let mut img = sheet()?.get_region(Rect2i::new(
        Vector2i::new((c as f32 * STEP * SCALE).round() as i32, (r as f32 * STEP * SCALE).round() as i32),
        Vector2i::splat((SIZE * SCALE).round() as i32),
    ))?;
    if img.get_format() != Format::RGBA8 {
        img.convert(Format::RGBA8);
    }
    Some(img)
}

/// Textur eines Symbols (zwischengespeichert), sonst None.
pub fn texture(token: &str) -> Option<Gd<Texture2D>> {
    let key = cell(token)?;
    if let Some(t) = TEXTURES.with(|m| m.borrow().get(&key).cloned()) {
        return Some(t);
    }
    let mut img = image(token)?;
    img.generate_mipmaps();
    let tex = ImageTexture::create_from_image(&img)?.upcast::<Texture2D>();
    TEXTURES.with(|m| m.borrow_mut().insert(key, tex.clone()));
    Some(tex)
}

/// Der ganze Symbolbogen als Textur (für Shader, z. B. das Farbrad), sonst None.
pub fn sheet_texture() -> Option<Gd<Texture2D>> {
    const KEY: (i32, i32) = (-1, -1);
    if let Some(t) = TEXTURES.with(|m| m.borrow().get(&KEY).cloned()) {
        return Some(t);
    }
    let mut img = sheet()?;
    if img.get_format() != Format::RGBA8 {
        img.convert(Format::RGBA8);
    }
    img.generate_mipmaps();
    let tex = ImageTexture::create_from_image(&img)?.upcast::<Texture2D>();
    TEXTURES.with(|m| m.borrow_mut().insert(KEY, tex.clone()));
    Some(tex)
}

/// Lage eines Symbols im ganzen Bogen als UV-Rechteck (x, y, Breite, Höhe; 0–1), sonst None.
pub fn uv_rect(token: &str) -> Option<Vector4> {
    let (c, r) = cell(token)?;
    let img = sheet()?;
    let (w, h) = (img.get_width() as f32, img.get_height() as f32);
    let px = SIZE * SCALE;
    Some(Vector4::new(c as f32 * STEP * SCALE / w, r as f32 * STEP * SCALE / h, px / w, px / h))
}

/// Enthält der Text Symbole in geschweiften Klammern, die der Bogen kennt („{2}{R}“, „{T}“)?
pub fn has_symbols(text: &str) -> bool {
    let mut rest = text;
    while let Some(i) = rest.find('{') {
        let Some(j) = rest[i..].find('}') else { return false };
        if cell(&rest[i + 1..i + j]).is_some() || &rest[i + 1..i + j] == "C" {
            return true;
        }
        rest = &rest[i + j + 1..];
    }
    false
}

/// `RichTextLabel` mit `text` füllen: Symbole in geschweiften Klammern („{2}{R}“, „{T}: Add {G}.“)
/// als eingebettete Bilder aus dem Bogen in Schrifthöhe `px`, der Rest als Text. Farblos „{C}“ als
/// gezeichnete Raute (`cost_symbols`), Unbekanntes bleibt als Text stehen.
pub fn fill_rich(label: &mut Gd<RichTextLabel>, text: &str, px: i32) {
    label.clear();
    let mut rest = text;
    while let Some(i) = rest.find('{') {
        label.add_text(&rest[..i]);
        let Some(j) = rest[i..].find('}') else {
            label.add_text(&rest[i..]);
            return;
        };
        let token = &rest[i + 1..i + j];
        match crate::table::cost_symbols::symbol_texture(token) {
            Some(tex) => {
                label.add_image_ex(&tex).width(px as f32).height(px as f32).done();
            }
            None => label.add_text(&rest[i..i + j + 1]),
        }
        rest = &rest[i + j + 1..];
    }
    label.add_text(rest);
}

/// Zwischenspeicher leeren (beim Abbau des Tisches, damit beim Beenden keine Texturen übrig bleiben).
pub fn clear() {
    TEXTURES.with(|m| m.borrow_mut().clear());
    SHEET_IMAGE.with(|s| *s.borrow_mut() = None);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_tokens_to_cells() {
        assert_eq!(cell("3"), Some((3, 0)));
        assert_eq!(cell("{12}"), Some((2, 1)));
        assert_eq!(cell("X"), Some((1, 2)));
        assert_eq!(cell("R"), Some((7, 2)));
        assert_eq!(cell("U/W"), Some((0, 3)));
        assert_eq!(cell("G/U"), Some((9, 3)));
        assert_eq!(cell("2/B"), Some((2, 4)));
        assert_eq!(cell("r/p"), Some((8, 4)));
        assert_eq!(cell("T"), Some((0, 5)));
        assert_eq!(cell("C"), None);
    }
}
