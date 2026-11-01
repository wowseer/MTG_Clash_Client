use godot::prelude::*;

mod hello;
/// Debug-Tool: existiert nur in Test-Builds, nicht in `cargo build --release`.
#[cfg(debug_assertions)]
mod debug;
mod game;
mod table;

struct MtgClashExtension;

#[gdextension]
unsafe impl ExtensionLibrary for MtgClashExtension {}
