//! Tvar požiadaviek a odpovedí.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Jedna požiadavka (jeden riadok JSON).
#[derive(Debug, Clone, Deserialize)]
pub struct Request {
    /// Vráti sa v odpovedi; ľubovoľná hodnota JSON (číslo, text).
    #[serde(default)]
    pub id: Value,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

/// Chyba metódy: text pre človeka a krátky druh pre program.
#[derive(Debug, Clone, PartialEq)]
pub struct ControlError {
    pub message: String,
    pub kind: &'static str,
}

impl ControlError {
    pub fn new(kind: &'static str, message: impl Into<String>) -> Self {
        ControlError {
            message: message.into(),
            kind,
        }
    }

    pub fn unknown_method(method: &str) -> Self {
        ControlError::new(
            "unknown-method",
            format!("neznáma metóda `{method}`; zoznam metód: control.methods"),
        )
    }

    pub fn params(message: impl Into<String>) -> Self {
        ControlError::new("params", message)
    }
}

impl From<cc_engine::EngineError> for ControlError {
    fn from(e: cc_engine::EngineError) -> Self {
        ControlError {
            kind: e.kind(),
            message: e.to_string(),
        }
    }
}

/// Popis metódy pre `control.methods`.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct MethodInfo {
    pub method: &'static str,
    pub params: &'static str,
    pub description: &'static str,
}

pub(crate) fn ok(id: &Value, result: Value) -> Value {
    json!({ "id": id, "ok": true, "result": result })
}

pub(crate) fn error(id: &Value, e: &ControlError) -> Value {
    json!({ "id": id, "ok": false, "kind": e.kind, "error": e.message })
}
