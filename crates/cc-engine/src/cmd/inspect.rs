//! Čítanie stavu: projekt a postavený model (pre agenta, testy a riadiaci kanál).

use cc_i18n::tr;
use cc_model::Severity;
use serde::Deserialize;
use serde_json::{json, Value};

use super::{has_doc, parse, CommandSpec, Effect};
use crate::{EngineError, Session};

pub static COMMANDS: &[CommandSpec] = &[
    CommandSpec {
        id: "project.inspect",
        menu: false,
        shortcut: None,
        params: r#"{"full"?: false} – full: celé zadanie projektu (ako project.toml)"#,
        effect: Effect::Read,
        enabled: has_doc,
        run: project,
    },
    CommandSpec {
        id: "model.inspect",
        menu: false,
        shortcut: None,
        params: r#"{"parts"?: false, "part"?: "SP1-SIDE-L"} – parts: zoznam dielov; part: celý diel s operáciami"#,
        effect: Effect::Read,
        enabled: has_doc,
        run: model,
    },
    CommandSpec {
        id: "model.hardware",
        menu: false,
        shortcut: None,
        params: r#"{"cabinet"?: "SP1"} – súpis kovania zákazky alebo jednej skrinky [{name, qty}]"#,
        effect: Effect::Read,
        enabled: has_doc,
        run: hardware,
    },
];

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct HardwareParams {
    cabinet: Option<String>,
}

fn hardware(s: &mut Session, params: Value) -> Result<Value, EngineError> {
    let p: HardwareParams = if params.is_null() {
        HardwareParams::default()
    } else {
        parse("model.hardware", params)?
    };
    let standard = s.require_doc()?.standard.clone();
    let built = match s.built() {
        None => return Err(tr!("engine.errors.no_project").into()),
        Some(Err(e)) => return Err(tr!("engine.errors.model_failed_with", error = e).into()),
        Some(Ok(b)) => b.clone(),
    };
    let mut model = built.model.clone();
    if let Some(id) = &p.cabinet {
        model.cabinets.retain(|c| &c.id == id);
        if model.cabinets.is_empty() {
            return Err(tr!("engine.errors.cabinet_missing", id = id).into());
        }
    }
    serde_json::to_value(cc_joints::hardware_bill(&model, &standard))
        .map_err(|e| EngineError::Failed(e.to_string()))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectParams {
    #[serde(default)]
    full: bool,
}

fn project(s: &mut Session, params: Value) -> Result<Value, EngineError> {
    let p: ProjectParams = parse("project.inspect", params)?;
    let doc = s.require_doc()?;
    let cabinets: Vec<Value> = doc
        .project
        .cabinets
        .iter()
        .map(|c| {
            json!({
                "id": c.id,
                "type": c.cabinet_type.as_str(),
                "w": c.w,
                "h": c.h,
                "d": c.d,
                "wall": c.wall,
                "at": c.at,
            })
        })
        .collect();
    let mut out = json!({
        "name": doc.project.project.name,
        "path": doc.path,
        "base": doc.base,
        "title": doc.title(),
        "dirty": doc.dirty,
        "standardDirty": doc.standard_dirty,
        "construction": doc.project.defaults.construction,
        "standards": doc.standards_available(),
        "machine": doc.machine_path,
        "cabinets": cabinets,
        "walls": doc.project.walls.iter().map(|w| &w.id).collect::<Vec<_>>(),
        "selection": s.selection(),
        "undo": s.undo_label(),
        "redo": s.redo_label(),
        "history": { "undo": s.history_len().0, "redo": s.history_len().1 },
        "version": s.version(),
    });
    if p.full {
        if let Value::Object(map) = &mut out {
            map.insert(
                "project".into(),
                serde_json::to_value(&doc.project).map_err(|e| e.to_string())?,
            );
        }
    }
    Ok(out)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ModelParams {
    #[serde(default)]
    parts: bool,
    part: Option<String>,
}

fn model(s: &mut Session, params: Value) -> Result<Value, EngineError> {
    let p: ModelParams = parse("model.inspect", params)?;
    let built = match s.built() {
        None => return Err(tr!("engine.errors.no_project").into()),
        Some(Err(e)) => return Ok(json!({ "ok": false, "error": e })),
        Some(Ok(b)) => b.clone(),
    };
    let issues: Vec<Value> = built
        .issues()
        .map(|i| {
            json!({
                "code": i.code.to_string(),
                "severity": i.severity,
                "part": i.part,
                "message": i.message,
            })
        })
        .collect();
    let errors = built
        .issues()
        .filter(|i| i.severity == Severity::Error)
        .count();
    let mut out = json!({
        "ok": true,
        "cabinets": built.model.cabinets.len(),
        "parts": built.model.parts().count(),
        "operations": built.model.parts().map(|x| x.operations.len()).sum::<usize>(),
        "errors": errors,
        "warnings": issues.len() - errors,
        "issues": issues,
    });
    let Value::Object(map) = &mut out else {
        return Ok(out);
    };
    if p.parts {
        let parts: Vec<Value> = built
            .model
            .parts()
            .map(|x| {
                json!({
                    "id": x.id,
                    "cabinet": x.cabinet,
                    "role": x.role.as_str(),
                    "material": x.material,
                    "length": x.length,
                    "width": x.width,
                    "thickness": x.thickness,
                    "operations": x.operations.len(),
                })
            })
            .collect();
        map.insert("partList".into(), Value::Array(parts));
    }
    if let Some(id) = p.part {
        let part = built
            .model
            .parts()
            .find(|x| x.id == id)
            .ok_or_else(|| EngineError::Failed(tr!("engine.errors.part_missing", id = &id)))?;
        map.insert(
            "part".into(),
            serde_json::to_value(part).map_err(|e| e.to_string())?,
        );
    }
    Ok(out)
}
