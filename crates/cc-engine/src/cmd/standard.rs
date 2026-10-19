//! Konštrukčný štandard: zmena hodnoty, výber štandardu, uloženie, načítanie.

use cc_i18n::tr;
use cc_model::Standard;
use serde::Deserialize;
use serde_json::{json, Value};

use super::{has_doc, parse, CommandSpec, Effect};
use crate::{EngineError, Session};

pub static COMMANDS: &[CommandSpec] = &[
    CommandSpec {
        id: "standard.set",
        menu: false,
        shortcut: None,
        params: r#"{"path": "dowel.diameter" (bodky medzi kľúčmi, číslo = index v zozname), "value": 8}"#,
        effect: Effect::Edit,
        enabled: has_doc,
        run: set,
    },
    CommandSpec {
        id: "standard.use",
        menu: true,
        shortcut: None,
        params: r#"{"name": "dielna-cx100"} – súbor standards/<name>.toml"#,
        effect: Effect::Edit,
        enabled: has_doc,
        run: use_standard,
    },
    CommandSpec {
        id: "standard.save",
        menu: true,
        shortcut: None,
        params: "{}",
        effect: Effect::Other,
        enabled: has_doc,
        run: save,
    },
    CommandSpec {
        id: "standard.reload",
        menu: true,
        shortcut: None,
        params: "{} – zahodí neuložené zmeny štandardu (dá sa vrátiť krokom späť)",
        effect: Effect::Edit,
        enabled: has_doc,
        run: reload,
    },
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Set {
    path: String,
    value: Value,
}

fn set(s: &mut Session, params: Value) -> Result<Value, EngineError> {
    const C: &str = "standard.set";
    let p: Set = parse(C, params)?;
    let doc = s.doc_mut()?;
    let mut tree = serde_json::to_value(&doc.standard).map_err(|e| e.to_string())?;
    let slot = lookup(&mut tree, &p.path).map_err(|message| EngineError::Params {
        command: C.into(),
        message,
    })?;
    let old = std::mem::replace(slot, p.value);
    let updated: Standard = serde_json::from_value(tree).map_err(|e| EngineError::Params {
        command: C.into(),
        message: format!("`{}`: {e}", p.path),
    })?;
    doc.standard = updated;
    Ok(json!({ "old": old }))
}

/// Hodnota v strome podľa cesty `a.b.0.c`. Cesta musí existovať.
fn lookup<'a>(tree: &'a mut Value, path: &str) -> Result<&'a mut Value, String> {
    if path.is_empty() {
        return Err(tr!("engine.standard.empty_path"));
    }
    let mut node = tree;
    for key in path.split('.') {
        node = match node {
            Value::Object(map) => map
                .get_mut(key)
                .ok_or_else(|| tr!("engine.standard.no_key", path = path, key = key))?,
            Value::Array(items) => {
                let i: usize = key
                    .parse()
                    .map_err(|_| tr!("engine.standard.not_index", path = path, key = key))?;
                items
                    .get_mut(i)
                    .ok_or_else(|| tr!("engine.standard.out_of_range", path = path, index = i))?
            }
            _ => return Err(tr!("engine.standard.not_container", path = path, key = key)),
        };
    }
    Ok(node)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Name {
    name: String,
}

fn use_standard(s: &mut Session, params: Value) -> Result<Value, EngineError> {
    let p: Name = parse("standard.use", params)?;
    s.doc_mut()?.set_construction(&p.name)?;
    s.flags_set = true;
    Ok(Value::Null)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}

fn save(s: &mut Session, params: Value) -> Result<Value, EngineError> {
    let _: Empty = parse("standard.save", params)?;
    let doc = s.doc_mut()?;
    doc.save_standard()?;
    Ok(json!({ "path": doc.standard_path() }))
}

fn reload(s: &mut Session, params: Value) -> Result<Value, EngineError> {
    let _: Empty = parse("standard.reload", params)?;
    s.doc_mut()?.reload_standard()?;
    s.flags_set = true;
    Ok(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_walks_tables_and_lists() {
        let mut v = json!({"a": {"b": [1, {"c": 2}]}});
        assert_eq!(*lookup(&mut v, "a.b.1.c").unwrap(), json!(2));
        assert!(lookup(&mut v, "a.x").is_err());
        assert!(lookup(&mut v, "a.b.5").is_err());
        assert!(lookup(&mut v, "a.b.1.c.d").is_err());
        assert!(lookup(&mut v, "").is_err());
    }
}
