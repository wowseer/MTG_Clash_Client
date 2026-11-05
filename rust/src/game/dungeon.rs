//! Dungeons (CR 309): Räume mit Text und Folgeräumen aus den Engine-Daten (`dungeon` im
//! Kartenzustand) und ihre Anordnung auf der Dungeon-Karte – Ebenen von oben (Eingang) nach unten
//! (letzter Raum), Räume einer Ebene nebeneinander. Engine-neutral; gezeichnet wird in
//! `table::dungeon_view`.

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
        Some(Dungeon { name, rooms, current })
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
        };
        assert_eq!(d.levels(), vec![0, 1, 1, 2, 2, 2, 3]);
        let l = d.layout();
        // Ebene 1 nebeneinander, gleich hoch; Tempel unten
        assert!(l[1][0] < l[2][0] && (l[1][1] - l[2][1]).abs() < 1e-6);
        assert!(l[6][1] > l[3][1]);
    }
}
