use godot::prelude::*;

mod hello;
#[macro_use]
mod i18n;
/// Debug-Tool: existiert nur in Test-Builds, nicht in `cargo build --release`.
#[cfg(debug_assertions)]
mod debug;
mod game;
mod table;

struct MtgClashExtension;

#[gdextension]
unsafe impl ExtensionLibrary for MtgClashExtension {}
