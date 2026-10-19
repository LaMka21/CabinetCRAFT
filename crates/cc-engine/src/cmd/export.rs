//! Export pre stroj: rozpis, `model.json`, programy `.xcs`/`.mpr`.

use std::path::PathBuf;

use cc_i18n::tr;
use cc_pipeline::CsvOptions;
use serde::Deserialize;
use serde_json::{json, Value};

use super::{parse, CommandSpec, Effect};
use crate::{EngineError, Session};

pub static COMMANDS: &[CommandSpec] = &[CommandSpec {
    id: "export.machine",
    menu: true,
    shortcut: Some("Ctrl+E"),
    params: r#"{"dir"?: "priečinok (predvolene out vedľa projektu)", "merge"?: false, "decimal_comma"?: false, "mode"?: "cx100"|"nesting" (predvolene [production] v machine.toml)} – pri chybách kontroly sa nevykoná"#,
    effect: Effect::Read,
    enabled: model_ok,
    run: export,
}];

fn model_ok(s: &Session) -> Result<(), String> {
    match s.built() {
        None => Err(tr!("engine.errors.no_project")),
        Some(Err(e)) => Err(tr!("engine.errors.model_failed_with", error = e)),
        Some(Ok(b)) if b.has_errors() => Err(tr!("engine.errors.has_issues")),
        Some(Ok(_)) => Ok(()),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Export {
    dir: Option<PathBuf>,
    #[serde(default)]
    merge: bool,
    #[serde(default)]
    decimal_comma: bool,
    /// Prepíše `[production] mode` profilu stroja pre tento export.
    mode: Option<cc_model::ProductionMode>,
}

fn export(s: &mut Session, params: Value) -> Result<Value, EngineError> {
    let p: Export = parse("export.machine", params)?;
    // poistka aj pri volaní mimo `Session::run`
    model_ok(s)?;
    let doc = s.require_doc()?;
    let built = s
        .model()
        .ok_or_else(|| EngineError::Failed(tr!("engine.errors.model_failed")))?;
    let dir = p.dir.unwrap_or_else(|| doc.base.join("out"));
    let csv = CsvOptions {
        merge: p.merge,
        decimal_comma: p.decimal_comma,
    };
    let mut machine = doc.machine.clone();
    if let (Some(mode), Some(m)) = (p.mode, machine.as_mut()) {
        if mode == cc_model::ProductionMode::Nesting && m.nesting.is_none() {
            return Err(tr!("engine.errors.no_nesting").into());
        }
        m.production.mode = mode;
    }
    let w = cc_pipeline::write_outputs(&built.model, machine.as_ref(), &dir, csv).map_err(|e| {
        let dir = dir.display().to_string();
        tr!("engine.export.failed", dir = dir, error = e.to_string())
    })?;
    Ok(json!({
        "dir": dir,
        "parts": w.parts,
        "operations": w.operations,
        "xcs": w.xcs,
        "mpr": w.mpr,
        "sheets": w.sheets,
        "labels": w.labels,
        "nesting": w.nesting,
        "notices": w.notices,
    }))
}
