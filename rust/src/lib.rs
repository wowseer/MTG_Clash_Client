use godot::prelude::*;

mod hello;
#[macro_use]
mod i18n;
mod forge;
mod game;
mod rules;
mod table;

struct MtgClashExtension;

#[gdextension]
unsafe impl ExtensionLibrary for MtgClashExtension {}
