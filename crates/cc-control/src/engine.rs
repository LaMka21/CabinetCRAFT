//! Metódy nad jadrom príkazov: rovnaké v okne aj v `cabinetcraft serve`.

use cc_engine::Session;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{ControlError, MethodInfo};

/// Metódy, ktoré vybaví [`handle`].
pub const METHODS: &[MethodInfo] = &[
    MethodInfo {
        method: "ping",
        params: "",
        description: "Odpovie \"pong\" (aj keď je aplikácia zaneprázdnená).",
    },
    MethodInfo {
        method: "control.methods",
        params: "",
        description: "Zoznam metód, ktoré tento server vybaví.",
    },
    MethodInfo {
        method: "engine.commands",
        params: "",
        description:
            "Register príkazov: id, popis, ponuka, skratka, parametre, či je teraz dostupný.",
    },
    MethodInfo {
        method: "engine.execute",
        params: r#"{"command": "cabinet.add", "params": {...}}"#,
        description: "Vykoná príkaz z registra. Nikdy neotvorí dialóg. Alias: command.",
    },
    MethodInfo {
        method: "project.inspect",
        params: r#"{"full"?: false}"#,
        description: "Stav projektu: skrinky, steny, neuložené zmeny, výber, história.",
    },
    MethodInfo {
        method: "model.inspect",
        params: r#"{"parts"?: false, "part"?: "ID"}"#,
        description: "Postavený model: počty, zistenia kontroly, diely, jeden diel s operáciami.",
    },
    MethodInfo {
        method: "app.open",
        params: r#"{"path": "project.toml"}"#,
        description: "Otvorí projekt (ako príkaz project.open).",
    },
    MethodInfo {
        method: "app.save",
        params: r#"{"path"?: "nový project.toml"}"#,
        description: "Uloží projekt; s `path` ako Uložiť ako.",
    },
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Execute {
    command: String,
    #[serde(default)]
    params: Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Save {
    path: Option<String>,
}

/// Vybaví metódu nad jadrom. `None`: metóda nepatrí jadru (napr. `ui.*`).
pub fn handle(
    session: &mut Session,
    method: &str,
    params: Value,
) -> Option<Result<Value, ControlError>> {
    let result = match method {
        "ping" => Ok(json!("pong")),
        "control.methods" => Ok(json!(METHODS)),
        "engine.commands" => Ok(Value::Array(
            cc_engine::commands().map(|c| c.describe(session)).collect(),
        )),
        "engine.execute" | "command" => match serde_json::from_value::<Execute>(params) {
            Ok(e) => session.run(&e.command, e.params).map_err(Into::into),
            Err(e) => Err(ControlError::params(format!("{method}: {e}"))),
        },
        "project.inspect" | "model.inspect" => session.run(method, params).map_err(Into::into),
        "app.open" => session.run("project.open", params).map_err(Into::into),
        "app.save" => match serde_json::from_value::<Save>(params) {
            Ok(Save { path: Some(path) }) => session
                .run("project.save_as", json!({ "path": path }))
                .map_err(Into::into),
            Ok(Save { path: None }) => session.run("project.save", json!({})).map_err(Into::into),
            Err(e) => Err(ControlError::params(format!("app.save: {e}"))),
        },
        _ => return None,
    };
    Some(result)
}
