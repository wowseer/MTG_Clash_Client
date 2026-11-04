//! Physik-Layer für Raycasts (Bitmasken).

/// Layer 1: Karten (Hover/Anwählen).
pub const CARD_LAYER: u32 = 1 << 0;
/// Layer 2: Zonen (Drop-Ziele).
pub const ZONE_LAYER: u32 = 1 << 1;
/// Layer 3: 3D-Bedienobjekte (z. B. [`Button3D`](super::button3d::Button3D)); haben beim Hover
/// und Klick Vorrang vor Karten.
pub const CONTROL_LAYER: u32 = 1 << 2;
