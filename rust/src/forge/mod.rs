//! Schnittstelle zur Forge-Regel-Engine (Java).
//!
//! Forge läuft als eigener Java-Prozess (`forge-bridge/`, GPL-3.0) und spricht über
//! stdin/stdout ein JSON-Lines-Protokoll (siehe `docs/Forge-Protocol.md`).
//! Auf Godot-Seite kapselt der Node [`bridge::ForgeBridge`] den Prozess und meldet alle
//! Nachrichten als Signale; [`table::ForgeTable`] spielt eine Forge-Partie auf dem 3D-Tisch.

pub mod bridge;
pub mod table;
