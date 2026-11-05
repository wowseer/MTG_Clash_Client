//! Dungeons (CR 309): Räume mit Text und Folgeräumen aus den Engine-Daten (`dungeon` im
//! Kartenzustand) und ihre Anordnung auf der Dungeon-Karte – Ebenen von oben (Eingang) nach unten
//! (letzter Raum), Räume einer Ebene nebeneinander. Engine-neutral; gezeichnet wird in
//! `table::dungeon_view`.

use std::cell::RefCell;
use std::collections::HashMap;

use godot::classes::Image;
use godot::prelude::*;

/// Ein Raum: Name, Effekt-Text und die Namen der Räume, zu denen er führt (CR 309.4).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct DungeonRoom {
    pub name: String,
    pub text: String,
    pub next: Vec<String>,
}

/// Ein Dungeon: Name, Räume (erster = Eingang) und aktueller Raum (leer = noch keiner betreten).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Dungeon {
    pub name: String,
    pub rooms: Vec<DungeonRoom>,
    pub current: String,
    /// Kartenbild von Scryfall (leer = keins bzw. Modus „nur eigene Grafik“).
    pub image: String,
}

/// Bildbereich der Karte über den Räumen (Titel), Rand und Abstände (Anteile der Kartengröße).
pub const TITLE_BAND: f32 = 0.1;
const MARGIN_X: f32 = 0.06;
const MARGIN_BOTTOM: f32 = 0.04;
const GAP_X: f32 = 0.035;
/// Anteil der Ebenenhöhe, den ein Raum einnimmt (Rest = Weg zur nächsten Ebene).
const BOX_FILL: f32 = 0.74;

impl Dungeon {
    /// Aus Kartendaten (`{name, dungeon: {rooms: [{name, text, next}], current}}`), sonst None.
    pub fn from_card(card: &VarDictionary) -> Option<Dungeon> {
        let d = card.get("dungeon")?.try_to::<VarDictionary>().ok()?;
        let rooms: Vec<DungeonRoom> = d
            .get("rooms")?
            .try_to::<VarArray>()
            .ok()?
            .iter_shared()
            .filter_map(|r| r.try_to::<VarDictionary>().ok())
            .map(|r| {
                let s = |k: &str| r.get(k).map(|v| v.to_string()).unwrap_or_default();
                let next = r
                    .get("next")
                    .and_then(|v| v.try_to::<VarArray>().ok())
                    .map(|a| a.iter_shared().map(|v| v.to_string()).collect())
                    .unwrap_or_default();
                DungeonRoom { name: s("name"), text: s("text"), next }
            })
            .collect();
        if rooms.is_empty() {
            return None;
        }
        let name = card.get("name").map(|v| v.to_string()).unwrap_or_default();
        let current = d.get("current").map(|v| v.to_string()).unwrap_or_default();
        let image = if crate::assets::protected_enabled() {
            card.get("image").filter(|v| !v.is_nil()).map(|v| v.to_string()).unwrap_or_default()
        } else {
            String::new()
        };
        Some(Dungeon { name, rooms, current, image })
    }

    /// Index des Raums `name`.
    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.rooms.iter().position(|r| r.name == name)
    }

    /// Ebene je Raum: längster Weg vom Eingang (Eingang = 0). Räume ohne Weg dorthin stehen eine
    /// Ebene unter ihrem tiefsten Vorgänger bzw. auf Ebene 0.
    pub fn levels(&self) -> Vec<usize> {
        let n = self.rooms.len();
        let mut level = vec![0usize; n];
        // höchstens n Durchgänge (Dungeons sind kreisfrei)
        for _ in 0..n {
            let mut changed = false;
            for (i, r) in self.rooms.iter().enumerate() {
                for next in &r.next {
                    if let Some(j) = self.index_of(next) {
                        if level[j] < level[i] + 1 {
                            level[j] = level[i] + 1;
                            changed = true;
                        }
                    }
                }
            }
            if !changed {
                break;
            }
        }
        level
    }

    /// Lage jedes Raums auf der Karte als (x, y, Breite, Höhe), Anteile von Kartenbreite/-höhe
    /// ab links oben; Reihenfolge wie `rooms`.
    pub fn layout(&self) -> Vec<[f32; 4]> {
        let level = self.levels();
        let depth = level.iter().copied().max().unwrap_or(0) + 1;
        let level_h = (1.0 - TITLE_BAND - MARGIN_BOTTOM) / depth as f32;
        let mut out = vec![[0.0; 4]; self.rooms.len()];
        for l in 0..depth {
            let members: Vec<usize> = (0..self.rooms.len()).filter(|&i| level[i] == l).collect();
            let k = members.len().max(1) as f32;
            let w = (1.0 - 2.0 * MARGIN_X - GAP_X * (k - 1.0)) / k;
            let h = level_h * BOX_FILL;
            let y = TITLE_BAND + l as f32 * level_h + (level_h - h) * 0.5;
            for (pos, &i) in members.iter().enumerate() {
                out[i] = [MARGIN_X + pos as f32 * (w + GAP_X), y, w, h];
            }
        }
        out
    }
}

thread_local! {
    /// Erkannte Räume je Kartenbild (None = Erkennung passte nicht zur Raumstruktur).
    static DETECTED: RefCell<HashMap<String, Option<Vec<[f32; 4]>>>> = RefCell::new(HashMap::new());
}

impl Dungeon {
    /// Lage der Räume für die Anzeige: auf dem echten Kartenbild die dort erkannten Räume
    /// ([`detect_rooms`]), sonst die berechnete Anordnung ([`Dungeon::layout`]).
    pub fn room_rects(&self) -> Vec<[f32; 4]> {
        if self.image.is_empty() {
            return self.layout();
        }
        let key = format!("{}|{}", self.image, self.rooms.len());
        let found = DETECTED.with(|m| m.borrow().get(&key).cloned());
        let found = match found {
            Some(f) => f,
            None => {
                let f = detect_rooms(&self.image, self);
                DETECTED.with(|m| m.borrow_mut().insert(key, f.clone()));
                f
            }
        };
        found.unwrap_or_else(|| self.layout())
    }
}

/// Räume auf einem gedruckten Dungeon-Bild finden: Die Räume sind durch dunkle Mauern getrennt.
/// Waagerechte Mauerlinien (Zeilen, die im Raumbereich überwiegend dunkel sind) teilen die
/// Ebenen, senkrechte innerhalb einer Ebene die Räume. Passt die Zahl der gefundenen Ebenen und
/// Räume je Ebene zur Raumstruktur der Engine, kommen die Rechtecke daraus, sonst None.
fn detect_rooms(path: &str, d: &Dungeon) -> Option<Vec<[f32; 4]>> {
    let mut img = Image::load_from_file(path)?;
    let (w, h) = (200i32, 280i32);
    img.resize(w, h);
    let lum = |x: i32, y: i32| {
        let c = img.get_pixel(x, y);
        0.299 * c.r + 0.587 * c.g + 0.114 * c.b
    };
    const DARK: f32 = 0.3;
    let (x0, x1) = ((w as f32 * 0.07) as i32, (w as f32 * 0.93) as i32);
    let (y0, y1) = ((h as f32 * 0.11) as i32, (h as f32 * 0.9) as i32);
    let dark_row = |y: i32| (x0..x1).filter(|&x| lum(x, y) < DARK).count() as f32 / (x1 - x0) as f32;
    // Ebenen: Zeilenbereiche zwischen Mauerzeilen
    let mut bands: Vec<(i32, i32)> = Vec::new();
    let mut start: Option<i32> = None;
    for y in y0..=y1 {
        let wall = y == y1 || dark_row(y) > 0.5;
        match (wall, start) {
            (false, None) => start = Some(y),
            (true, Some(s)) => {
                if y - s >= (h as f32 * 0.035) as i32 {
                    bands.push((s, y - 1));
                }
                start = None;
            }
            _ => {}
        }
    }
    let level = d.levels();
    let depth = level.iter().copied().max().unwrap_or(0) + 1;
    if bands.len() != depth {
        return None;
    }
    // Räume, die über mehrere Ebenen reichen: ihr nächster Raum liegt mehr als eine Ebene tiefer
    // (z. B. Oubliette in Tomb of Annihilation) – sie belegen in den Ebenen dazwischen eine Spalte
    let reach: Vec<usize> = (0..d.rooms.len())
        .map(|i| {
            d.rooms[i].next.iter().filter_map(|n| d.index_of(n)).map(|j| level[j]).min().map_or(level[i], |m| m.saturating_sub(1).max(level[i]))
        })
        .collect();
    let mut out = vec![[0.0f32; 4]; d.rooms.len()];
    for (l, &(by0, by1)) in bands.iter().enumerate() {
        let members: Vec<usize> = (0..d.rooms.len()).filter(|&i| level[i] == l).collect();
        let spanning: Vec<usize> = (0..d.rooms.len()).filter(|&i| level[i] < l && reach[i] >= l).collect();
        // Raumgrenzen: Spalten, die über die Ebene überwiegend dunkel sind
        let dark_col = |x: i32| (by0..=by1).filter(|&y| lum(x, y) < DARK).count() as f32 / (by1 - by0 + 1) as f32;
        let mut cols: Vec<(i32, i32)> = Vec::new();
        let mut cs: Option<i32> = None;
        for x in x0..=x1 {
            let wall = x == x1 || dark_col(x) > 0.6;
            match (wall, cs) {
                (false, None) => cs = Some(x),
                (true, Some(s)) => {
                    if x - s >= (w as f32 * 0.08) as i32 {
                        cols.push((s, x - 1));
                    }
                    cs = None;
                }
                _ => {}
            }
        }
        if cols.len() != members.len() + spanning.len() {
            return None;
        }
        let mut free = members.iter();
        for &(cx0, cx1) in &cols {
            let mid = (cx0 + cx1) as f32 * 0.5 / w as f32;
            // Spalte unter einem durchgehenden Raum: dessen Rechteck nach unten verlängern
            if let Some(&i) = spanning.iter().find(|&&i| out[i][0] <= mid && mid <= out[i][0] + out[i][2]) {
                out[i][3] = (by1 + 1) as f32 / h as f32 - out[i][1];
                continue;
            }
            let Some(&i) = free.next() else { return None };
            out[i] = [
                cx0 as f32 / w as f32,
                by0 as f32 / h as f32,
                (cx1 - cx0 + 1) as f32 / w as f32,
                (by1 - by0 + 1) as f32 / h as f32,
            ];
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn room(name: &str, next: &[&str]) -> DungeonRoom {
        DungeonRoom { name: name.into(), text: String::new(), next: next.iter().map(|s| s.to_string()).collect() }
    }

    #[test]
    fn levels_follow_longest_path() {
        // wie Lost Mine of Phandelver: Eingang → 2 → 3 → Tempel
        let d = Dungeon {
            name: "Mine".into(),
            rooms: vec![
                room("Entrance", &["Lair", "Tunnels"]),
                room("Lair", &["Store", "Pool"]),
                room("Tunnels", &["Pool", "Fungi"]),
                room("Store", &["Temple"]),
                room("Pool", &["Temple"]),
                room("Fungi", &["Temple"]),
                room("Temple", &[]),
            ],
            current: String::new(),
            image: String::new(),
        };
        assert_eq!(d.levels(), vec![0, 1, 1, 2, 2, 2, 3]);
        let l = d.layout();
        // Ebene 1 nebeneinander, gleich hoch; Tempel unten
        assert!(l[1][0] < l[2][0] && (l[1][1] - l[2][1]).abs() < 1e-6);
        assert!(l[6][1] > l[3][1]);
    }
}
