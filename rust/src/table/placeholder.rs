//! Erzeugt Platzhalter-Texturen für Demo-Karten (bis echte Kartenbilder existieren).

use godot::classes::image::Format;
use godot::classes::{Image, ImageTexture, Texture2D};
use godot::prelude::*;

use crate::game::cards::{CardDef, CardKind};
use crate::game::mana::ManaType;

const W: i32 = 256;
const H: i32 = 358;

fn finish(mut img: Gd<Image>) -> Gd<Texture2D> {
    img.generate_mipmaps();
    ImageTexture::create_from_image(&img)
        .expect("ImageTexture aus Image")
        .upcast()
}

fn rect(x: i32, y: i32, w: i32, h: i32) -> Rect2i {
    Rect2i::new(Vector2i::new(x, y), Vector2i::new(w, h))
}

/// Durchmesser und Abstand der Kostensymbole in der Titelleiste (Pixel).
const PIP: i32 = 24;
const PIP_STEP: i32 = 26;
const TITLE_Y: i32 = 36;

/// Pixelmitten (x) der Kostensymbole, von links nach rechts, rechtsbündig in der Titelleiste.
pub fn pip_centers(count: usize) -> Vec<f32> {
    (0..count)
        .map(|i| (W - 34 - (count - 1 - i) as i32 * PIP_STEP) as f32)
        .collect()
}

/// Pixelmitte (y) der Titelleiste.
pub fn title_center_y() -> f32 {
    TITLE_Y as f32
}

/// Gefüllter Kreis (zeilenweise aus Rechtecken).
fn circle(img: &mut Gd<Image>, cx: i32, cy: i32, r: i32, color: Color) {
    for dy in -r..=r {
        let half = (((r * r - dy * dy) as f32).sqrt()) as i32;
        img.fill_rect(rect(cx - half, cy + dy, half * 2 + 1, 1), color);
    }
}

/// Vorderseite für eine Kartendefinition: Rahmen in Kartenfarbe, Titelleiste mit
/// Kostensymbolen, Bildfeld, Typzeile, Textfeld und (bei Kreaturen) Wertefeld.
/// Texte kommen per Label3D dazu.
pub fn card_face(def: &CardDef) -> Gd<Texture2D> {
    let base = def.frame_color();
    let frame = base * 0.85;
    let dark = base.darkened(0.6);
    let art = base.lightened(0.45);

    let mut img = Image::create_empty(W, H, false, Format::RGBA8).expect("Image");
    img.fill(Color::from_rgb(0.08, 0.08, 0.1));
    img.fill_rect(rect(10, 10, W - 20, H - 20), frame);
    img.fill_rect(rect(20, 20, W - 40, 32), dark); // Titel
    img.fill_rect(rect(20, 60, W - 40, 140), art); // Bild
    img.fill_rect(rect(20, 208, W - 40, 22), dark); // Typzeile
    img.fill_rect(rect(20, 238, W - 40, 96), Color::from_rgb(0.93, 0.9, 0.82)); // Text

    match def.kind {
        CardKind::Resource => {
            // Großes Farbsymbol im Bildfeld
            let c = def.produces.color();
            circle(&mut img, W / 2, 130, 52, Color::from_rgb(0.1, 0.08, 0.06));
            circle(&mut img, W / 2, 130, 48, c);
            circle(&mut img, W / 2 - 14, 116, 14, c.lightened(0.45));
        }
        kind => {
            // Kostensymbole: zuerst generisch (grau, Zahl per Label3D), dann farbig
            let mut pips: Vec<Color> = Vec::new();
            if def.cost.generic > 0 {
                pips.push(ManaType::Colorless.color());
            }
            for t in ManaType::ALL {
                for _ in 0..def.cost.specific.get(t) {
                    pips.push(t.color());
                }
            }
            for (x, c) in pip_centers(pips.len()).into_iter().zip(pips) {
                circle(&mut img, x as i32, TITLE_Y, PIP / 2 + 1, Color::from_rgb(0.05, 0.04, 0.03));
                circle(&mut img, x as i32, TITLE_Y, PIP / 2 - 1, c);
            }
            // Typsymbol im Bildfeld
            emblem(&mut img, kind, dark);
            // Wertefeld unten rechts (Kreatur, Held, Ausrüstung, Aura)
            if matches!(kind, CardKind::Creature | CardKind::Hero | CardKind::Equipment | CardKind::Aura) {
                img.fill_rect(rect(W - 82, 318, 64, 32), Color::from_rgb(0.12, 0.08, 0.06));
                img.fill_rect(rect(W - 80, 320, 60, 28), dark);
            }
        }
    }
    finish(img)
}

/// Einfaches Symbol je Kartentyp in der Mitte des Bildfelds (Mitte 128/130).
fn emblem(img: &mut Gd<Image>, kind: CardKind, c: Color) {
    let (cx, cy) = (W / 2, 130);
    match kind {
        CardKind::Instant => {
            // Blitz: versetzte Balken
            for (i, dx) in [8, 0, -8, 4, -4, -12].into_iter().enumerate() {
                img.fill_rect(rect(cx + dx - 6, cy - 48 + i as i32 * 16, 14, 18), c);
            }
        }
        CardKind::Sorcery => {
            // Zauberkreis: Ring mit Punkt
            circle(img, cx, cy, 46, c);
            circle(img, cx, cy, 38, c.lightened(0.85));
            circle(img, cx, cy, 10, c);
        }
        CardKind::Hero => {
            // Krone: Sockel und drei Zacken
            img.fill_rect(rect(cx - 44, cy + 10, 88, 22), c);
            for (dx, h) in [(-38, 40), (-8, 56), (22, 40)] {
                img.fill_rect(rect(cx + dx, cy + 10 - h, 16, h), c);
                circle(img, cx + dx + 8, cy + 6 - h, 8, c);
            }
        }
        CardKind::Equipment => {
            // Schwert: Klinge, Parierstange, Griff
            img.fill_rect(rect(cx - 6, cy - 54, 12, 74), c);
            img.fill_rect(rect(cx - 30, cy + 20, 60, 10), c);
            img.fill_rect(rect(cx - 5, cy + 30, 10, 22), c.darkened(0.3));
            circle(img, cx, cy + 56, 7, c);
        }
        CardKind::Artifact => {
            // Zahnrad: Ring mit Zähnen
            for (dx, dy) in [(0, -46), (0, 46), (-46, 0), (46, 0), (-33, -33), (33, 33), (-33, 33), (33, -33)] {
                img.fill_rect(rect(cx + dx - 9, cy + dy - 9, 18, 18), c);
            }
            circle(img, cx, cy, 40, c);
            circle(img, cx, cy, 16, c.lightened(0.85));
        }
        CardKind::Enchantment => {
            // Stern: zwei gekreuzte Balken + Mitte
            img.fill_rect(rect(cx - 6, cy - 52, 12, 104), c);
            img.fill_rect(rect(cx - 52, cy - 6, 104, 12), c);
            circle(img, cx, cy, 20, c);
        }
        CardKind::Aura => {
            // Halo über einer Figur
            circle(img, cx, cy - 34, 26, c);
            circle(img, cx, cy - 34, 18, c.lightened(0.85));
            circle(img, cx, cy + 4, 14, c);
            img.fill_rect(rect(cx - 20, cy + 18, 40, 38), c);
        }
        CardKind::Creature | CardKind::Resource => {}
    }
}

/// Gemeinsame Rückseite.
pub fn card_back() -> Gd<Texture2D> {
    let mut img = Image::create_empty(W, H, false, Format::RGBA8).expect("Image");
    img.fill(Color::from_rgb(0.08, 0.08, 0.1));
    let colors = [
        Color::from_rgb(0.12, 0.16, 0.38),
        Color::from_rgb(0.75, 0.6, 0.25),
        Color::from_rgb(0.18, 0.24, 0.52),
        Color::from_rgb(0.75, 0.6, 0.25),
        Color::from_rgb(0.12, 0.16, 0.38),
    ];
    for (k, c) in colors.iter().enumerate() {
        let m = 10 + k as i32 * 18;
        img.fill_rect(rect(m, m, W - 2 * m, H - 2 * m), *c);
    }
    finish(img)
}
