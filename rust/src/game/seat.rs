//! PlayerSeat: ein Sitzplatz am Tisch (Spieler oder KI) mit seinen Zonen,
//! Lebenspunkten, Ressourcen-Reihe und Manapool.

use godot::prelude::*;

use super::mana::ManaPool;
use crate::table::zone::Zone;

#[derive(GodotClass)]
#[class(base=Node3D)]
pub struct PlayerSeat {
    #[export]
    pub(crate) display_name: GString,
    /// Wird dieser Platz von der KI gespielt?
    #[export]
    pub(crate) is_ai: bool,
    #[export]
    pub(crate) starting_life: i32,
    #[export]
    pub(crate) hand: Option<Gd<Zone>>,
    #[export]
    pub(crate) deck: Option<Gd<Zone>>,
    #[export]
    pub(crate) battlefield: Option<Gd<Zone>>,
    #[export]
    pub(crate) discard: Option<Gd<Zone>>,
    /// Reihe für ausgespielte Ressourcen-Karten.
    #[export]
    pub(crate) resource_zone: Option<Gd<Zone>>,
    #[export]
    pub(crate) mana_pool: Option<Gd<ManaPool>>,

    #[var]
    pub(crate) life: i32,
    /// In dieser Runde bereits ausgespielte Ressourcen-Karten.
    #[var]
    pub(crate) resources_played: i32,

    base: Base<Node3D>,
}

#[godot_api]
impl INode3D for PlayerSeat {
    fn init(base: Base<Node3D>) -> Self {
        Self {
            display_name: "Spieler".into(),
            is_ai: false,
            starting_life: 20,
            hand: None,
            deck: None,
            battlefield: None,
            discard: None,
            resource_zone: None,
            mana_pool: None,
            life: 20,
            resources_played: 0,
            base,
        }
    }

    fn ready(&mut self) {
        self.life = self.starting_life;
    }
}

#[godot_api]
impl PlayerSeat {
    /// Gehört die Zone zu diesem Sitzplatz?
    #[func]
    pub fn owns_zone(&self, zone: Gd<Zone>) -> bool {
        [&self.hand, &self.deck, &self.battlefield, &self.discard, &self.resource_zone]
            .into_iter()
            .any(|z| z.as_ref() == Some(&zone))
    }
}
