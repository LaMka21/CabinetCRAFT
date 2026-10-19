//! Štítky dielov po tabuliach ako program +Label v CV (findings 14 §8): pohľad na tabule
//! a tlač štítku dielu, celej tabule alebo všetkých štítkov do PDF.

use std::path::PathBuf;

use cc_i18n::tr;
use serde::Deserialize;
use serde_json::{json, Value};

use super::{parse, CommandSpec, Effect};
use crate::{EngineError, Session};

pub static COMMANDS: &[CommandSpec] = &[
    CommandSpec {
        id: "labels.sheets",
        menu: false,
        shortcut: None,
        params: r#"{"mode"?: "cx100"|"nesting" (predvolene [production] v machine.toml)} – tabule nestingu s dielmi (poloha, číslo na štítku) v poradí tlače"#,
        effect: Effect::Read,
        enabled: model_ok,
        run: sheets,
    },
    CommandSpec {
        id: "labels.print",
        menu: false,
        shortcut: None,
        params: r#"{"sheet"?: "program tabule, napr. 0101N", "parts"?: ["ID dielu"], "path"?: "súbor .pdf (predvolene out/labels/tlac.pdf)", "a4"?: false, "mode"?: "cx100"|"nesting"} – bez sheet a parts všetky štítky; strana = štítok (rozmer z [labels] v machine.toml) alebo A4"#,
        effect: Effect::Read,
        enabled: model_ok,
        run: print,
    },
];

fn model_ok(s: &Session) -> Result<(), String> {
    match s.built() {
        None => Err(tr!("engine.errors.no_project")),
        Some(Err(e)) => Err(tr!("engine.errors.model_failed_with", error = e)),
        Some(Ok(b)) if b.has_errors() => Err(tr!("engine.errors.has_issues")),
        Some(Ok(_)) => Ok(()),
    }
}

/// Štítky zákazky s profilom stroja (voliteľne iný režim výroby).
fn job(
    s: &Session,
    mode: Option<cc_model::ProductionMode>,
) -> Result<cc_pipeline::labels::LabelJob, EngineError> {
    model_ok(s)?;
    let doc = s.require_doc()?;
    let built = s
        .model()
        .ok_or_else(|| EngineError::Failed(tr!("engine.errors.model_failed")))?;
    let mut machine = doc.machine.clone();
    if let (Some(mode), Some(m)) = (mode, machine.as_mut()) {
        if mode == cc_model::ProductionMode::Nesting && m.nesting.is_none() {
            return Err(tr!("engine.errors.no_nesting").into());
        }
        m.production.mode = mode;
    }
    Ok(cc_pipeline::labels::label_job(
        &built.model,
        machine.as_ref(),
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SheetsParams {
    mode: Option<cc_model::ProductionMode>,
}

fn sheets(s: &mut Session, params: Value) -> Result<Value, EngineError> {
    let p: SheetsParams = parse("labels.sheets", params)?;
    let job = job(s, p.mode)?;
    let sheets: Vec<Value> = job
        .sheets
        .iter()
        .map(|sh| {
            let parts: Vec<Value> = sh
                .parts
                .iter()
                .map(|pp| {
                    let label = job.labels.iter().find(|l| l.part.id == pp.part_id);
                    json!({
                        "id": pp.part_id,
                        "number": pp.number,
                        "x": pp.x,
                        "y": pp.y,
                        "dx": pp.dx,
                        "dy": pp.dy,
                        "rotated": pp.rotated,
                        "face_up": format!("{:?}", pp.face_up),
                        "cabinet": label.map(|l| l.part.cabinet.clone()),
                        "name": label.map(|l| cc_out_pdf::role_name(l.part.role)),
                        "length": label.map(|l| l.part.length),
                        "width": label.map(|l| l.part.width),
                        "programs": label.map(|l| l.programs.clone()),
                        "up_down": label.and_then(|l| l.up_down()),
                    })
                })
                .collect();
            json!({
                "name": sh.name,
                "material": sh.material,
                "thickness": sh.thickness,
                "length": sh.length,
                "width": sh.width,
                "index": sh.index,
                "count": sh.count,
                "parts": parts,
            })
        })
        .collect();
    let on_sheets: std::collections::BTreeSet<&str> = job
        .sheets
        .iter()
        .flat_map(|sh| sh.parts.iter().map(|p| p.part_id.as_str()))
        .collect();
    let other: Vec<&str> = job
        .labels
        .iter()
        .map(|l| l.part.id.as_str())
        .filter(|id| !on_sheets.contains(id))
        .collect();
    let printer = s
        .doc()
        .and_then(|d| d.machine.as_ref())
        .and_then(|m| m.labels.printer.clone());
    Ok(json!({
        "printer": printer,
        "size": [job.size.0, job.size.1],
        "order": job.order,
        "sheets": sheets,
        "other": other,
        "labels": job.labels.len(),
    }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PrintParams {
    sheet: Option<String>,
    parts: Option<Vec<String>>,
    path: Option<PathBuf>,
    #[serde(default)]
    a4: bool,
    mode: Option<cc_model::ProductionMode>,
}

fn print(s: &mut Session, params: Value) -> Result<Value, EngineError> {
    let p: PrintParams = parse("labels.print", params)?;
    let job = job(s, p.mode)?;
    let labels = match (&p.sheet, &p.parts) {
        (Some(sheet), _) => {
            if !job.sheets.iter().any(|sh| &sh.name == sheet) {
                return Err(tr!("engine.labels.no_sheet", sheet = sheet).into());
            }
            job.sheet_labels(sheet)
        }
        (None, Some(ids)) => {
            let found = job.part_labels(ids);
            if found.len() != ids.len() {
                let missing: Vec<&String> = ids
                    .iter()
                    .filter(|id| !found.iter().any(|l| &&l.part.id == id))
                    .collect();
                return Err(tr!(
                    "engine.labels.parts_without_label",
                    parts = format!("{missing:?}")
                )
                .into());
            }
            found
        }
        (None, None) => job.labels.clone(),
    };
    if labels.is_empty() {
        return Err(tr!("engine.labels.nothing_to_print").into());
    }
    let doc = s.require_doc()?;
    let path = p
        .path
        .unwrap_or_else(|| doc.base.join("out").join("labels").join("tlac.pdf"));
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| {
            let dir = dir.display().to_string();
            tr!(
                "engine.labels.mkdir_failed",
                dir = dir,
                error = e.to_string()
            )
        })?;
    }
    let fonts = cc_out_pdf::Fonts::new(cc_out_pdf::FontChoice::System);
    let pdf = if p.a4 {
        cc_out_pdf::labels_a4_pdf(&labels, &fonts, job.size)
    } else {
        cc_out_pdf::labels_pdf(&labels, &fonts, job.size)
    };
    std::fs::write(&path, pdf).map_err(|e| {
        let file = path.display().to_string();
        tr!(
            "engine.labels.write_failed",
            file = file,
            error = e.to_string()
        )
    })?;
    Ok(json!({
        "path": path,
        "count": labels.len(),
        "parts": labels.iter().map(|l| l.part.id.clone()).collect::<Vec<_>>(),
    }))
}
