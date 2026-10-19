//! Späť/znova a výber.

use cc_i18n::tr;
use serde::Deserialize;
use serde_json::{json, Value};

use super::{has_doc, parse, CommandSpec, Effect};
use crate::{EngineError, Session};

pub static COMMANDS: &[CommandSpec] = &[
    CommandSpec {
        id: "edit.undo",
        menu: true,
        shortcut: Some("Ctrl+Z"),
        params: "{}",
        effect: Effect::Other,
        enabled: can_undo,
        run: undo,
    },
    CommandSpec {
        id: "edit.redo",
        menu: true,
        shortcut: Some("Ctrl+Y"),
        params: "{}",
        effect: Effect::Other,
        enabled: can_redo,
        run: redo,
    },
    CommandSpec {
        id: "selection.set",
        menu: false,
        shortcut: None,
        params: r#"{"cabinet"?: "SP1"|null, "part"?: "SP1-SIDE-L"|null} – diel vyberie aj jeho skrinku"#,
        effect: Effect::Read,
        enabled: has_doc,
        run: select,
    },
];

fn can_undo(s: &Session) -> Result<(), String> {
    match s.undo_label() {
        Some(_) => Ok(()),
        None => Err(tr!("engine.session.nothing_to_undo")),
    }
}

fn can_redo(s: &Session) -> Result<(), String> {
    match s.redo_label() {
        Some(_) => Ok(()),
        None => Err(tr!("engine.session.nothing_to_redo")),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}

fn undo(s: &mut Session, params: Value) -> Result<Value, EngineError> {
    let _: Empty = parse("edit.undo", params)?;
    s.undo_step(false)
}

fn redo(s: &mut Session, params: Value) -> Result<Value, EngineError> {
    let _: Empty = parse("edit.redo", params)?;
    s.undo_step(true)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Select {
    #[serde(default, deserialize_with = "present")]
    cabinet: Option<Option<String>>,
    #[serde(default, deserialize_with = "present")]
    part: Option<Option<String>>,
}

/// Pole je v JSON uvedené: `null` = `Some(None)` (zrušiť výber), chýbajúce pole = `None`
/// (cez `default`). Bez toho serde číta `null` ako chýbajúce pole.
fn present<'de, D, T>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(d).map(Some)
}

fn select(s: &mut Session, params: Value) -> Result<Value, EngineError> {
    let p: Select = parse("selection.set", params)?;
    let doc = s.require_doc()?;
    let mut sel = s.selection().clone();
    if let Some(cabinet) = p.cabinet {
        if let Some(id) = &cabinet {
            if !doc.project.cabinets.iter().any(|c| &c.id == id) {
                return Err(tr!("engine.errors.cabinet_missing", id = id).into());
            }
        }
        if sel.cabinet != cabinet {
            sel.part = None;
        }
        sel.cabinet = cabinet;
    }
    if let Some(part) = p.part {
        if let Some(id) = &part {
            let model = s
                .model()
                .ok_or_else(|| EngineError::Failed(tr!("engine.errors.model_failed")))?;
            let found =
                model.model.parts().find(|x| &x.id == id).ok_or_else(|| {
                    EngineError::Failed(tr!("engine.errors.part_missing", id = id))
                })?;
            sel.cabinet = Some(found.cabinet.clone());
        }
        sel.part = part;
    }
    s.selection = sel;
    Ok(json!(s.selection()))
}
