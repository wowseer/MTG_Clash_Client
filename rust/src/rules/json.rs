//! Umwandlung zwischen Godot-Werten (`Variant`) und JSON (`serde_json::Value`) – die Naht
//! zwischen der Godot-UI und den godot-freien Crates `clash-protocol` / `clash-forge-client`.
//! Ganzzahlige Zahlen bleiben `int` (Godots eigenes JSON liefert sonst immer `float`).

use godot::prelude::*;
use serde_json::{Map, Number, Value};

/// JSON → Godot: Objekte als `Dictionary`, Listen als `Array`, ganze Zahlen als `int`.
pub fn to_variant(v: &Value) -> Variant {
    match v {
        Value::Null => Variant::nil(),
        Value::Bool(b) => b.to_variant(),
        Value::Number(n) => match n.as_i64() {
            Some(i) => i.to_variant(),
            None => n.as_f64().unwrap_or(0.0).to_variant(),
        },
        Value::String(s) => s.to_variant(),
        Value::Array(a) => a.iter().map(to_variant).collect::<VarArray>().to_variant(),
        Value::Object(o) => to_dictionary(o).to_variant(),
    }
}

/// JSON-Objekt → `Dictionary`.
pub fn to_dictionary(o: &Map<String, Value>) -> VarDictionary {
    let mut d = VarDictionary::new();
    for (k, val) in o {
        d.set(k.as_str(), &to_variant(val));
    }
    d
}

/// Godot → JSON. Ganzzahlige `float` werden zu ganzen Zahlen (wie im Protokoll gemeint);
/// gepackte Arrays werden Listen; unbekannte Typen ihr Text.
pub fn from_variant(v: &Variant) -> Value {
    match v.get_type() {
        VariantType::NIL => Value::Null,
        VariantType::BOOL => Value::Bool(v.to::<bool>()),
        VariantType::INT => Value::from(v.to::<i64>()),
        VariantType::FLOAT => {
            let f = v.to::<f64>();
            if f.fract() == 0.0 && f.abs() < 9.0e15 {
                Value::from(f as i64)
            } else {
                Number::from_f64(f).map_or(Value::Null, Value::Number)
            }
        }
        VariantType::STRING | VariantType::STRING_NAME | VariantType::NODE_PATH => Value::String(v.to_string()),
        VariantType::DICTIONARY => {
            let d = v.to::<VarDictionary>();
            Value::Object(d.iter_shared().map(|(k, val)| (k.to_string(), from_variant(&val))).collect())
        }
        VariantType::ARRAY => Value::Array(v.to::<VarArray>().iter_shared().map(|e| from_variant(&e)).collect()),
        VariantType::PACKED_STRING_ARRAY => Value::Array(v.to::<PackedStringArray>().as_slice().iter().map(|s| Value::String(s.to_string())).collect()),
        VariantType::PACKED_INT32_ARRAY => Value::Array(v.to::<PackedInt32Array>().as_slice().iter().map(|i| Value::from(*i)).collect()),
        VariantType::PACKED_INT64_ARRAY => Value::Array(v.to::<PackedInt64Array>().as_slice().iter().map(|i| Value::from(*i)).collect()),
        VariantType::PACKED_FLOAT32_ARRAY | VariantType::PACKED_FLOAT64_ARRAY => {
            Value::Array(v.to::<PackedFloat64Array>().as_slice().iter().map(|f| from_variant(&f.to_variant())).collect())
        }
        _ => Value::String(v.to_string()),
    }
}
