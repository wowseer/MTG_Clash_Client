//! Kostensymbole in 3D: der gemeinsame Baustein für jede Kostenanzeige der 3D-Oberfläche
//! (Kosten über den Handkarten wie im Video „Spree und Stack“, Knöpfe wie „Bezahlen {1}{U}“,
//! Beschriftungen wie „+ {3}“).
//!
//! Die Motive sind dieselben wie im Manapool ([`draw_mana_symbol`]): Weiß = Sonne, Blau =
//! Wassertropfen, Schwarz = Totenkopf, Rot = Flamme, Grün = Nadelbaum (Wald), Farblos = graue
//! Raute. Generische Kosten (Zahlen), X und andere Symbole (Hybrid, phyrexianisch) stehen als
//! Text in einem grauen Kreis. Jedes Symbol wird einmal in eine kleine `SubViewport` gezeichnet
//! und als Textur zwischengespeichert; in der Szene ist es ein `Sprite3D`.

use std::cell::RefCell;
use std::collections::HashMap;

use godot::classes::base_material_3d::{BillboardMode, TextureFilter};
use godot::classes::control::MouseFilter;
use godot::classes::label_3d::DrawFlags as LabelFlags;
use godot::classes::sprite_base_3d::DrawFlags as SpriteFlags;
use godot::classes::sub_viewport::UpdateMode;
use godot::classes::{
    Control, Engine, IControl, Label3D, SceneTree, Sprite3D, SubViewport, Texture2D, ThemeDb,
};
use godot::global::HorizontalAlignment;
use godot::prelude::*;

use crate::game::mana::ManaType;
use crate::game::mana_view::draw_mana_symbol;

/// Kantenlänge einer Symbol-Textur (Pixel).
const TEX: i32 = 128;
/// Abstand zwischen zwei Symbolen einer Reihe (Anteil der Symbolhöhe).
const SYMBOL_GAP: f32 = 0.08;
/// Abstand zwischen Text und Symbolen (Anteil der Symbolhöhe).
const TEXT_GAP: f32 = 0.22;
/// Höhe eines Symbols im Verhältnis zur Schriftgröße einer Beschriftung.
const SYMBOL_PER_FONT: f32 = 0.82;

/// Zerlegt Kosten in Symbole: „{2}{W}{W}“, „2 W W“, „X U“ → `["2", "W", "W"]`; Hybrid und
/// phyrexianisch bleiben ein Symbol („W/U“, „R/P“).
pub fn cost_tokens(text: &str) -> Vec<String> {
    text.split(|c: char| c.is_whitespace() || c == '{' || c == '}')
        .filter(|t| !t.is_empty())
        .map(|t| t.to_uppercase())
        .collect()
}

/// Zeichenfläche eines einzelnen Kostensymbols (intern: liegt in einer unsichtbaren `SubViewport`, deren Textur die 3D-Kostensymbole nutzen).
#[derive(GodotClass)]
#[class(base=Control, init)]
pub struct CostSymbolCanvas {
    token: String,
    base: Base<Control>,
}

#[godot_api]
impl IControl for CostSymbolCanvas {
    fn draw(&mut self) {
        let size = TEX as f32;
        let c = Vector2::splat(size * 0.5);
        let r = size * 0.44;
        let token = self.token.clone();
        let mana = (token.len() == 1).then(|| token.chars().next().and_then(|ch| if ch == 'C' { Some(ManaType::Colorless) } else { ManaType::from_wubrg(ch) })).flatten();
        let mut b = self.base_mut();
        if let Some(t) = mana {
            draw_mana_symbol(&mut b, t, c, r);
            return;
        }
        // Zahl, X oder sonstiges Symbol: grauer Kreis mit goldenem Rand und dunkler Schrift
        b.draw_circle(c + Vector2::new(0.0, size * 0.02), r + size * 0.02, Color::from_rgba(0.0, 0.0, 0.0, 0.45));
        b.draw_circle(c, r, Color::from_rgb(0.78, 0.76, 0.72));
        b.draw_arc_ex(c, r, 0.0, std::f32::consts::TAU, 48, Color::from_rgb(0.9, 0.72, 0.36)).width(size * 0.035).antialiased(true).done();
        let Some(font) = ThemeDb::singleton().get_fallback_font() else { return };
        let font_size = if token.chars().count() <= 1 { 84 } else if token.chars().count() == 2 { 66 } else { 44 };
        let ascent = font.get_ascent_ex().font_size(font_size).done();
        let descent = font.get_descent_ex().font_size(font_size).done();
        let baseline = c.y + (ascent - descent) * 0.5;
        b.draw_string_ex(&font, Vector2::new(0.0, baseline), &token)
            .alignment(HorizontalAlignment::CENTER)
            .width(size)
            .font_size(font_size)
            .modulate(Color::from_rgb(0.1, 0.09, 0.08))
            .done();
    }
}

thread_local! {
    /// Je Symbol seine `SubViewport` (Knoten, kein Referenzzähler: beim Beenden räumt Godot sie
    /// mit dem Baum ab, der Zwischenspeicher hält keine Textur fest).
    static CACHE: RefCell<HashMap<String, Gd<SubViewport>>> = RefCell::new(HashMap::new());
}

/// Textur eines Symbols (einmal gezeichnet, danach aus dem Zwischenspeicher). Die
/// `SubViewport` hängt unsichtbar an der Wurzel des Szenenbaums.
pub fn symbol_texture(token: &str) -> Option<Gd<Texture2D>> {
    if let Some(vp) = CACHE.with(|c| c.borrow().get(token).cloned()).filter(|v| v.is_instance_valid()) {
        return vp.get_texture().map(|t| t.upcast::<Texture2D>());
    }
    let tree = Engine::singleton().get_main_loop()?.try_cast::<SceneTree>().ok()?;
    let mut root = tree.get_root();
    let mut vp = SubViewport::new_alloc();
    vp.set_name(&format!("CostSymbol_{}", token.replace('/', "_")));
    vp.set_size(Vector2i::new(TEX, TEX));
    vp.set_transparent_background(true);
    vp.set_disable_3d(true);
    vp.set_update_mode(UpdateMode::ONCE);
    let mut canvas = CostSymbolCanvas::new_alloc();
    canvas.bind_mut().token = token.to_string();
    canvas.set_size(Vector2::splat(TEX as f32));
    canvas.set_mouse_filter(MouseFilter::IGNORE);
    vp.add_child(&canvas);
    root.call_deferred("add_child", &[vp.to_variant()]);
    let tex = vp.get_texture()?.upcast::<Texture2D>();
    CACHE.with(|c| c.borrow_mut().insert(token.to_string(), vp));
    Some(tex)
}

/// Ein Symbol als `Sprite3D` (Fläche in der XY-Ebene, Vorderseite +Z) mit Höhe `height`.
fn symbol_sprite(token: &str, height: f32) -> Gd<Sprite3D> {
    let mut s = Sprite3D::new_alloc();
    if let Some(t) = symbol_texture(token) {
        s.set_texture(&t);
    }
    s.set_pixel_size(height / TEX as f32);
    s.set_billboard_mode(BillboardMode::DISABLED);
    s.set_draw_flag(SpriteFlags::SHADED, false);
    s.set_draw_flag(SpriteFlags::DOUBLE_SIDED, false);
    s.set_texture_filter(TextureFilter::LINEAR_WITH_MIPMAPS);
    s.set_render_priority(3);
    s
}

/// Breite einer Symbolreihe aus `n` Symbolen der Höhe `height`.
pub fn row_width(n: usize, height: f32) -> f32 {
    if n == 0 {
        return 0.0;
    }
    n as f32 * height + (n - 1) as f32 * height * SYMBOL_GAP
}

/// Symbolreihe (Fläche in der XY-Ebene, Vorderseite +Z), um den Ursprung zentriert.
pub fn cost_row(tokens: &[String], height: f32) -> Gd<Node3D> {
    let mut row = Node3D::new_alloc();
    row.set_name("CostRow");
    let width = row_width(tokens.len(), height);
    for (i, t) in tokens.iter().enumerate() {
        let mut s = symbol_sprite(t, height);
        let x = -width * 0.5 + height * 0.5 + i as f32 * height * (1.0 + SYMBOL_GAP);
        s.set_position(Vector3::new(x, 0.0, 0.0));
        row.add_child(&s);
    }
    row
}

/// Enthält der Text Kosten in geschweiften Klammern („Bezahlen {1}{U}“, „+ {3}“)?
pub fn has_cost(text: &str) -> bool {
    text.contains('{') && text.contains('}')
}

/// Teil einer Beschriftung: Text oder Kostensymbole.
enum Part {
    Text(String),
    Symbols(Vec<String>),
}

fn parts(text: &str) -> Vec<Part> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        let before = rest[..open].trim();
        if !before.is_empty() {
            out.push(Part::Text(before.to_string()));
        }
        // zusammenhängende {…}{…} bilden eine Symbolreihe
        let mut end = open;
        let mut tokens = Vec::new();
        while rest[end..].starts_with('{') {
            let Some(close) = rest[end..].find('}') else { break };
            tokens.push(rest[end + 1..end + close].trim().to_uppercase());
            end += close + 1;
        }
        if tokens.is_empty() {
            out.push(Part::Text(rest[open..].to_string()));
            return out;
        }
        out.push(Part::Symbols(tokens));
        rest = &rest[end..];
    }
    if !rest.trim().is_empty() {
        out.push(Part::Text(rest.trim().to_string()));
    }
    out
}

/// Breite von `text` als `Label3D` mit `font_size` und `pixel_size` (Welteinheiten).
fn text_width(text: &str, font_size: i32, pixel_size: f32) -> f32 {
    ThemeDb::singleton()
        .get_fallback_font()
        .map_or(text.chars().count() as f32 * font_size as f32 * 0.55, |f| {
            f.get_string_size_ex(text).alignment(HorizontalAlignment::LEFT).font_size(font_size).done().x
        })
        * pixel_size
}

/// Breite einer Beschriftung mit Kostensymbolen bei Schriftgröße `font_size` und `pixel_size`.
pub fn rich_width(text: &str, font_size: i32, pixel_size: f32) -> f32 {
    let sym = font_size as f32 * pixel_size * SYMBOL_PER_FONT;
    let ps = parts(text);
    let gaps = ps.len().saturating_sub(1) as f32 * sym * TEXT_GAP;
    ps.iter()
        .map(|p| match p {
            Part::Text(t) => text_width(t, font_size, pixel_size),
            Part::Symbols(s) => row_width(s.len(), sym),
        })
        .sum::<f32>()
        + gaps
}

/// Beschriftung mit Kostensymbolen statt „{…}“ (Fläche in der XY-Ebene, Vorderseite +Z, um den
/// Ursprung zentriert): Textteile als `Label3D` im Stil von `style` (Schrift, Farbe, Umriss werden
/// übernommen), Kosten als Symbolreihe in Schrifthöhe.
pub fn rich_label(text: &str, style: &Gd<Label3D>) -> Gd<Node3D> {
    let font_size = style.get_font_size();
    let pixel_size = style.get_pixel_size();
    let sym = font_size as f32 * pixel_size * SYMBOL_PER_FONT;
    let mut node = Node3D::new_alloc();
    node.set_name("RichLabel");
    let total = rich_width(text, font_size, pixel_size);
    let mut x = -total * 0.5;
    for p in parts(text) {
        match p {
            Part::Text(t) => {
                let w = text_width(&t, font_size, pixel_size);
                let mut l = style.duplicate_node().try_cast::<Label3D>().unwrap_or_else(|_| Label3D::new_alloc());
                l.set_name("Text");
                l.set_text(&t);
                l.set_draw_flag(LabelFlags::SHADED, false);
                l.set_position(Vector3::new(x + w * 0.5, 0.0, 0.0));
                node.add_child(&l);
                x += w + sym * TEXT_GAP;
            }
            Part::Symbols(tokens) => {
                let w = row_width(tokens.len(), sym);
                let mut row = cost_row(&tokens, sym);
                // Symbole sitzen etwas tiefer als die Schriftmitte (wie Großbuchstaben)
                row.set_position(Vector3::new(x + w * 0.5, -sym * 0.04, 0.002));
                node.add_child(&row);
                x += w + sym * TEXT_GAP;
            }
        }
    }
    node
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_costs_into_tokens() {
        assert_eq!(cost_tokens("{2}{W}{W}"), vec!["2", "W", "W"]);
        assert_eq!(cost_tokens("1 U"), vec!["1", "U"]);
        assert_eq!(cost_tokens("X r/p"), vec!["X", "R/P"]);
        assert!(cost_tokens("").is_empty());
    }

    #[test]
    fn splits_labels_into_text_and_symbols() {
        let p = parts("Bezahlen {1}{U}");
        assert!(matches!(&p[0], Part::Text(t) if t == "Bezahlen"));
        assert!(matches!(&p[1], Part::Symbols(s) if s == &vec!["1".to_string(), "U".to_string()]));
        let p = parts("Kicker – {R} extra");
        assert_eq!(p.len(), 3);
        assert!(has_cost("+ {3}"));
        assert!(!has_cost("Normal"));
    }
}
