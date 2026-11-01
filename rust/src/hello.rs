use godot::classes::{INode2D, Node2D};
use godot::prelude::*;

/// Beispiel-Node: zeigt, dass die Rust-Extension geladen wird.
#[derive(GodotClass)]
#[class(base=Node2D)]
pub struct Hello {
    /// Text, der beim Start ausgegeben wird.
    #[export]
    greeting: GString,
    base: Base<Node2D>,
}

#[godot_api]
impl INode2D for Hello {
    fn init(base: Base<Node2D>) -> Self {
        Self {
            greeting: "Hallo aus Rust!".into(),
            base,
        }
    }

    fn ready(&mut self) {
        godot_print!("{}", self.greeting);
    }
}

#[godot_api]
impl Hello {
    /// Beispielfunktion: gibt `a + b` zurück.
    #[func]
    fn add(&self, a: i64, b: i64) -> i64 {
        a + b
    }
}
