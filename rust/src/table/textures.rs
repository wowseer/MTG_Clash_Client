//! Prozedurale Texturen für die Taverne (Holz, Spielmatte, Putz).
//! Die Pixel werden in Rust berechnet und in einem Rutsch an Godot übergeben.

use godot::classes::image::Format;
use godot::classes::{Image, ImageTexture, Texture2D};
use godot::prelude::*;

// ---------------------------------------------------------------- Rauschen

fn hash(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(374761393)
        ^ (y as u32).wrapping_mul(668265263)
        ^ seed.wrapping_mul(2246822519);
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    h ^= h >> 16;
    (h & 0xffff) as f32 / 65535.0
}

/// Kachelbares Value-Noise (Periode `px` × `py` Gitterzellen).
fn value_noise(x: f32, y: f32, px: i32, py: i32, seed: u32) -> f32 {
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let (xi, yi) = (x0 as i32, y0 as i32);
    let w = |a: i32, b: i32| hash(a.rem_euclid(px), b.rem_euclid(py), seed);
    let top = w(xi, yi) + (w(xi + 1, yi) - w(xi, yi)) * sx;
    let bot = w(xi, yi + 1) + (w(xi + 1, yi + 1) - w(xi, yi + 1)) * sx;
    top + (bot - top) * sy
}

/// Fraktales Rauschen, kachelbar in u/v ∈ [0,1).
fn fbm(u: f32, v: f32, fx: i32, fy: i32, octaves: u32, seed: u32) -> f32 {
    let (mut sum, mut amp, mut norm, mut f) = (0.0, 0.5, 0.0, 1);
    for o in 0..octaves {
        sum += amp * value_noise(u * (fx * f) as f32, v * (fy * f) as f32, fx * f, fy * f, seed + o);
        norm += amp;
        amp *= 0.5;
        f *= 2;
    }
    sum / norm
}

fn mix(a: Color, b: Color, t: f32) -> Color {
    a.lerp(b, t.clamp(0.0, 1.0) as f64)
}

struct Canvas {
    w: i32,
    h: i32,
    data: Vec<u8>,
}

impl Canvas {
    fn new(w: i32, h: i32) -> Self {
        Self { w, h, data: vec![0; (w * h * 4) as usize] }
    }

    fn put(&mut self, x: i32, y: i32, c: Color) {
        let i = ((y * self.w + x) * 4) as usize;
        self.data[i] = (c.r.clamp(0.0, 1.0) * 255.0) as u8;
        self.data[i + 1] = (c.g.clamp(0.0, 1.0) * 255.0) as u8;
        self.data[i + 2] = (c.b.clamp(0.0, 1.0) * 255.0) as u8;
        self.data[i + 3] = 255;
    }

    fn texture(self) -> Gd<Texture2D> {
        let bytes = PackedByteArray::from(self.data);
        let mut img = Image::create_from_data(self.w, self.h, false, Format::RGBA8, &bytes)
            .expect("Image aus Daten");
        img.generate_mipmaps();
        ImageTexture::create_from_image(&img).expect("ImageTexture").upcast()
    }
}

// ---------------------------------------------------------------- Texturen

/// Holzdielen (kachelbar). `planks` Dielen übereinander, Maserung entlang U.
pub fn wood_planks(planks: i32, light: Color, dark: Color, seed: u32) -> Gd<Texture2D> {
    wood_planks_oriented(planks, light, dark, seed, false)
}

/// Wie `wood_planks`, mit `vertical = true` laufen die Dielen entlang V statt U.
pub fn wood_planks_oriented(planks: i32, light: Color, dark: Color, seed: u32, vertical: bool) -> Gd<Texture2D> {
    let (w, h) = (512, 512);
    let mut c = Canvas::new(w, h);
    let plank_h = h / planks;
    for y in 0..h {
        let p = y / plank_h;
        let py = (y % plank_h) as f32 / plank_h as f32;
        let pseed = seed + p as u32 * 17;
        let tone = hash(p, 3, seed) * 0.25;
        let joint = (hash(p, 7, seed) * w as f32) as i32;
        for x in 0..w {
            let u = x as f32 / w as f32;
            let v = y as f32 / h as f32;
            // Maserung: in U gestreckte Linien, durch Rauschen verwellt
            let warp = fbm(u, v, 3, 6, 3, pseed) * 6.0;
            let grain = ((py * 9.0 + warp) * std::f32::consts::TAU).sin() * 0.5 + 0.5;
            let fine = fbm(u, v, 64, 4, 2, pseed + 5);
            let t = 0.35 + tone + grain * 0.25 + (fine - 0.5) * 0.35;
            let mut col = mix(dark, light, t);
            // Fugen zwischen den Dielen und Stoß-Fugen
            let edge = py.min(1.0 - py) * plank_h as f32;
            if edge < 2.0 || (x - joint).abs() < 2 {
                col = mix(col, Color::from_rgb(0.03, 0.02, 0.01), 0.8);
            } else if edge < 4.0 {
                col = mix(col, Color::from_rgb(0.05, 0.03, 0.02), 0.35);
            }
            if vertical {
                c.put(y, x, col);
            } else {
                c.put(x, y, col);
            }
        }
    }
    c.texture()
}

/// Unregelmäßiges Bruchsteinmauerwerk mit Mörtelfugen (kachelbar).
pub fn stone_wall() -> Gd<Texture2D> {
    let (w, h) = (512, 512);
    let rows = 6;
    let row_h = h / rows;
    let mortar = Color::from_rgb(0.16, 0.14, 0.12);
    let mut c = Canvas::new(w, h);
    for y in 0..h {
        let r = y / row_h;
        let ry = y % row_h;
        // Jede Reihe hat eigene Steinbreiten, versetzt
        let offset = (hash(r, 1, 77) * w as f32) as i32;
        for x in 0..w {
            let xs = (x + offset).rem_euclid(w);
            // Steingrenzen: 4 Steine pro Reihe, Breite variiert
            let mut start = 0;
            let mut idx = 0;
            let mut end = 0;
            for k in 0..4 {
                let width = w / 4 + ((hash(r, k + 10, 77) - 0.5) * 50.0) as i32;
                end = if k == 3 { w } else { start + width };
                if xs < end {
                    idx = k;
                    break;
                }
                start = end;
            }
            let (u, v) = (x as f32 / w as f32, y as f32 / h as f32);
            let n = fbm(u, v, 16, 16, 4, 31);
            let edge_x = (xs - start).min(end - 1 - xs);
            let edge_y = ry.min(row_h - 1 - ry);
            // Leicht gerundete Ecken durch Rauschen in der Fugenbreite
            let gap = 3.0 + n * 4.0;
            let col = if (edge_x as f32) < gap || (edge_y as f32) < gap {
                mix(mortar, mortar * 1.4, n)
            } else {
                let tone = hash(r, idx + 40, 77);
                let base = mix(Color::from_rgb(0.32, 0.29, 0.26), Color::from_rgb(0.52, 0.47, 0.4), tone);
                let shade = 1.0 - (1.0 - (edge_x.min(edge_y) as f32 / 12.0).min(1.0)) * 0.25;
                mix(base * 0.8, base * 1.15, n) * shade
            };
            c.put(x, y, col);
        }
    }
    c.texture()
}

/// Rauer Putz für die Wände.
pub fn plaster() -> Gd<Texture2D> {
    let (w, h) = (256, 256);
    let mut c = Canvas::new(w, h);
    let a = Color::from_rgb(0.55, 0.47, 0.36);
    let b = Color::from_rgb(0.75, 0.68, 0.55);
    for y in 0..h {
        for x in 0..w {
            let (u, v) = (x as f32 / w as f32, y as f32 / h as f32);
            let n = fbm(u, v, 8, 8, 5, 3);
            c.put(x, y, mix(a, b, n * 1.4 - 0.2));
        }
    }
    c.texture()
}
