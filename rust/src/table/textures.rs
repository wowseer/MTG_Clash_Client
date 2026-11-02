//! Prozedurale Texturen für Taverne und Arena (Holz, Stein, Putz, Arena-Spielbrett).
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

/// Symbol für getappte Karten: grauer Kreispfeil im Uhrzeigersinn auf dunkler,
/// halbtransparenter Scheibe (96 × 96, mit Alpha). Wird je Karte erzeugt (klein; ein globaler
/// Zwischenspeicher würde beim Beenden als nicht freigegebene Textur gemeldet).
pub fn tap_icon() -> Gd<Texture2D> {
    const N: i32 = 96;
    let mut data = vec![0u8; (N * N * 4) as usize];
    let c = N as f32 * 0.5;
    // Maße unten sind für 128 px angegeben und werden auf N skaliert
    let k = N as f32 / 128.0;
    let grey = Color::from_rgb(0.78, 0.78, 0.8);
    let disc = Color::from_rgb(0.08, 0.08, 0.1);
    let (ring_r, ring_w) = (34.0 * k, 6.0 * k);
    // Bogen von 40° bis 320° (Lücke oben rechts), Spitze am Ende bei 320°, zeigt im Uhrzeigersinn
    let (a0, a1) = (40f32.to_radians(), 320f32.to_radians());
    let tip_angle = a1;
    let tip_center = Vector2::new(tip_angle.cos(), tip_angle.sin()) * ring_r;
    let along = Vector2::new(-tip_angle.sin(), tip_angle.cos()); // Laufrichtung bei wachsendem Winkel
    let outward = Vector2::new(tip_angle.cos(), tip_angle.sin());
    let (head_len, head_w) = (20.0 * k, 15.0 * k);
    for y in 0..N {
        for x in 0..N {
            let p = Vector2::new(x as f32 + 0.5 - c, y as f32 + 0.5 - c);
            let r = p.length();
            let mut ang = p.y.atan2(p.x);
            if ang < 0.0 {
                ang += std::f32::consts::TAU;
            }
            // Scheibe
            let disc_a = (1.0 - smoothstep(54.0 * k, 57.0 * k, r)) * 0.62;
            // Ring-Bogen
            let in_arc = ang >= a0 && ang <= a1;
            let ring = if in_arc { 1.0 - smoothstep(ring_w * 0.5 - 1.0, ring_w * 0.5 + 1.0, (r - ring_r).abs()) } else { 0.0 };
            // Pfeilspitze: Dreieck ab dem Bogenende in Laufrichtung
            let q = p - tip_center;
            let t = q.dot(along);
            let s = q.dot(outward).abs();
            let head = if (-1.0..head_len).contains(&t) {
                let half = head_w * (1.0 - t / head_len);
                1.0 - smoothstep(half - 1.0, half + 1.0, s)
            } else {
                0.0
            };
            let icon = ring.max(head);
            let col = mix(disc, grey, icon);
            let a = disc_a.max(icon);
            let i = ((y * N + x) * 4) as usize;
            data[i] = (col.r * 255.0) as u8;
            data[i + 1] = (col.g * 255.0) as u8;
            data[i + 2] = (col.b * 255.0) as u8;
            data[i + 3] = (a.clamp(0.0, 1.0) * 255.0) as u8;
        }
    }
    let mut img = Image::create_from_data(N, N, false, Format::RGBA8, &PackedByteArray::from(data)).expect("Tapp-Symbol");
    img.generate_mipmaps();
    ImageTexture::create_from_image(&img).expect("ImageTexture").upcast()
}

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

// ---------------------------------------------------------------- Arena

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Tiefe einer gemeißelten Rille (0 = Oberfläche, 1 = Rillengrund) im Abstand `d`
/// von der Rillenmitte bei der Breite `w` (alles in Welt-Einheiten).
fn groove(d: f32, w: f32) -> f32 {
    1.0 - smoothstep(w * 0.45, w, d)
}

/// Vorzeichenbehafteter Abstand zu einem abgerundeten Rechteck (negativ = innen).
fn sd_round_rect(p: Vector2, center: Vector2, half: Vector2, radius: f32) -> f32 {
    let q = (p - center).abs() - half + Vector2::splat(radius);
    let outside = Vector2::new(q.x.max(0.0), q.y.max(0.0)).length();
    outside + q.x.max(q.y).min(0.0) - radius
}

/// Texturen der Arena-Spielfläche.
pub struct BoardTextures {
    pub albedo: Gd<Texture2D>,
    pub normal: Gd<Texture2D>,
    pub emission: Gd<Texture2D>,
}

/// Sandstein-Spielbrett mit gemeißelten Ornamenten: Runenkreis in der Mitte (leuchtet über die
/// Emissionstextur), große Raute, Mittellinie und doppelter Rahmen mit Eck-Ringen (keine
/// eingelassenen Kartenfelder). `size` = Brettgröße in Welt-Einheiten (x, z), Brettmitte = (0, 0).
/// u läuft entlang +x, v entlang +z.
pub fn arena_board(size: Vector2, px_per_unit: f32) -> BoardTextures {
    let w = (size.x * px_per_unit) as i32;
    let h = (size.y * px_per_unit) as i32;
    let half = size * 0.5;
    let n = (w * h) as usize;
    let mut height = vec![0u8; n];
    let mut albedo = Canvas::new(w, h);
    let mut emission = Canvas::new(w, h);

    let light = Color::from_rgb(0.66, 0.55, 0.4);
    let dark = Color::from_rgb(0.42, 0.33, 0.23);
    let glow_col = Color::from_rgb(0.35, 0.85, 1.0);
    let frame_outer = half - Vector2::splat(0.25);
    let frame_inner = half - Vector2::splat(0.42);
    let tau = std::f32::consts::TAU;
    let rhombus = Vector2::new(half.x - 1.0, half.y - 0.8);
    let rhombus_norm = (1.0 / (rhombus.x * rhombus.x) + 1.0 / (rhombus.y * rhombus.y)).sqrt();

    for py in 0..h {
        for px in 0..w {
            let (u, v) = (px as f32 / w as f32, py as f32 / h as f32);
            let p = Vector2::new(u * size.x - half.x, v * size.y - half.y);
            let r = p.length();
            let ang = p.y.atan2(p.x);

            // Rillen mit Leuchten (Runenkreis) und ohne
            let mut lit = 0.0f32;
            let mut carved = 0.0f32;

            // --- Runenkreis
            for (r0, gw, glows) in [(1.62, 0.04, false), (1.5, 0.026, false), (1.05, 0.032, true), (0.62, 0.026, true), (0.2, 0.024, true)] {
                let g = groove((r - r0).abs(), gw);
                if glows { lit = lit.max(g) } else { carved = carved.max(g) }
            }
            // Speichen innen/außen um den Glyphenring
            if (1.05..1.21).contains(&r) || (1.35..1.5).contains(&r) {
                let step = tau / 36.0;
                let a = (ang / step).round() * step;
                carved = carved.max(groove(r * (ang - a).abs(), 0.014));
            }
            // Glyphen: kleine Ringe auf r = 1.28
            {
                let step = tau / 12.0;
                let a = ((ang - step * 0.5) / step).round() * step + step * 0.5;
                let c = Vector2::new(a.cos(), a.sin()) * 1.28;
                let d = (p - c).length();
                lit = lit.max(groove((d - 0.065).abs(), 0.018));
            }
            // Achtzackiger Stern aus zwei Quadraten im Kreis r = 1.05
            if r < 1.08 {
                let s = 1.05 / std::f32::consts::SQRT_2;
                let sq1 = (p.x.abs().max(p.y.abs()) - s).abs();
                let q = Vector2::new(p.x + p.y, p.y - p.x) / std::f32::consts::SQRT_2;
                let sq2 = (q.x.abs().max(q.y.abs()) - s).abs();
                lit = lit.max(groove(sq1.min(sq2), 0.022));
            }

            // --- Große Raute und Mittellinie (außerhalb des Kreises)
            if r > 1.7 {
                let d = (p.x.abs() / rhombus.x + p.y.abs() / rhombus.y - 1.0).abs() / rhombus_norm;
                carved = carved.max(groove(d, 0.03));
                if p.x.abs() < frame_inner.x {
                    carved = carved.max(groove(p.y.abs(), 0.04));
                }
            }

            // --- Doppelter Rahmen mit Ringen in den Ecken
            carved = carved.max(groove(sd_round_rect(p, Vector2::ZERO, frame_outer, 0.3).abs(), 0.045));
            carved = carved.max(groove(sd_round_rect(p, Vector2::ZERO, frame_inner, 0.2).abs(), 0.026));
            let corner = Vector2::new(frame_inner.x, frame_inner.y);
            let dc = (p.abs() - corner).length();
            carved = carved.max(groove((dc - 0.32).abs(), 0.026));
            carved = carved.max(groove((dc - 0.16).abs(), 0.02));

            // Steinstruktur
            let big = fbm(u, v, 4, 3, 4, 101);
            let fine = fbm(u, v, 48, 34, 3, 202);
            let pits = smoothstep(0.76, 0.84, fbm(u, v, 40, 28, 2, 303)) * 0.6;
            let depth = carved.max(lit);
            let hgt = 0.85 - depth * 0.55 - (fine - 0.5) * 0.12 - pits * 0.12;
            height[(py * w + px) as usize] = (hgt.clamp(0.0, 1.0) * 255.0) as u8;

            let mut col = mix(dark, light, 0.25 + big * 0.9 + (fine - 0.5) * 0.4);
            col = col * (1.0 - depth * 0.5) * (1.0 - pits * 0.25);
            col.a = 1.0;
            albedo.put(px, py, col);
            emission.put(px, py, glow_col * lit);
        }
    }

    let mut himg = Image::create_from_data(w, h, false, Format::L8, &PackedByteArray::from(height))
        .expect("Höhenbild");
    himg.bump_map_to_normal_map_ex().bump_scale(4.0).done();
    himg.generate_mipmaps();
    BoardTextures {
        albedo: albedo.texture(),
        normal: ImageTexture::create_from_image(&himg).expect("Normal-Map").upcast(),
        emission: emission.texture(),
    }
}

/// Bodenplatten aus dunklem Schiefer, 4 × 4 Platten (kachelbar).
pub fn flagstones() -> Gd<Texture2D> {
    let (w, h) = (512, 512);
    let tiles = 4;
    let tile = w / tiles;
    let mut c = Canvas::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let (tx, ty) = (x / tile, y / tile);
            let (ex, ey) = (x % tile, y % tile);
            let edge = ex.min(tile - 1 - ex).min(ey.min(tile - 1 - ey)) as f32;
            let (u, v) = (x as f32 / w as f32, y as f32 / h as f32);
            let n = fbm(u, v, 16, 16, 4, 404);
            let tone = hash(tx, ty, 405);
            let base = mix(Color::from_rgb(0.2, 0.21, 0.23), Color::from_rgb(0.34, 0.34, 0.35), tone);
            let col = if edge < 3.0 + n * 3.0 {
                Color::from_rgb(0.07, 0.07, 0.08)
            } else {
                let bevel = 1.0 - (1.0 - (edge / 10.0).min(1.0)) * 0.3;
                mix(base * 0.8, base * 1.2, n) * bevel
            };
            c.put(x, y, col);
        }
    }
    c.texture()
}
