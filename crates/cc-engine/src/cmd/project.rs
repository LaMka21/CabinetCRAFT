//! Projekt: otvorenie, nový, uloženie, názov, predvolené materiály.

use std::path::PathBuf;

use cc_i18n::tr;
use serde::Deserialize;
use serde_json::{json, Value};

use super::{always, has_doc, parse, CommandSpec, Effect};
use crate::{Document, EngineError, Session};

pub static COMMANDS: &[CommandSpec] = &[
    CommandSpec {
        id: "project.import_cvj",
        menu: true,
        shortcut: None,
        params: r#"{"file": "úloha.cvj", "out": "cesta/project.toml", "template"?: "priečinok alebo project.toml"} – zapíše projekt (materiály, štandard a stroj zo vzoru; predvolene z otvoreného projektu) a otvorí ho"#,
        effect: Effect::Other,
        enabled: always,
        run: import_cvj,
    },
    CommandSpec {
        id: "project.export_cvj",
        menu: true,
        shortcut: None,
        params: r#"{"out": "nová zákazka.cvj", "template"?: "zákazka CV ako šablóna; predvolene tá, z ktorej sa projekt importoval"}"#,
        effect: Effect::Other,
        enabled: has_cv_template,
        run: export_cvj,
    },
    CommandSpec {
        id: "project.open",
        menu: true,
        shortcut: Some("Ctrl+O"),
        params: r#"{"path": "cesta k project.toml"}"#,
        effect: Effect::Other,
        enabled: always,
        run: open,
    },
    CommandSpec {
        id: "project.new",
        menu: true,
        shortcut: Some("Ctrl+N"),
        params: "{} – materiály, štandard a stroj prevezme z otvoreného projektu",
        effect: Effect::Other,
        enabled: has_doc,
        run: new,
    },
    CommandSpec {
        id: "project.save",
        menu: true,
        shortcut: Some("Ctrl+S"),
        params: "{} – uloží projekt a zmenený štandard",
        effect: Effect::Other,
        enabled: has_path,
        run: save,
    },
    CommandSpec {
        id: "project.save_as",
        menu: true,
        shortcut: Some("Ctrl+Shift+S"),
        params: r#"{"path": "nový project.toml"}"#,
        effect: Effect::Other,
        enabled: has_doc,
        run: save_as,
    },
    CommandSpec {
        id: "project.rename",
        menu: false,
        shortcut: None,
        params: r#"{"name": "názov zákazky"}"#,
        effect: Effect::Edit,
        enabled: has_doc,
        run: rename,
    },
    CommandSpec {
        id: "project.info",
        menu: false,
        shortcut: None,
        params: r#"{"number"?: "REF2026", "customer"?: "…", "designer"?: "…", "company"?: "…", "logo"?: "logo.png (PNG/JPEG, relatívne k projektu)"} – prázdny text alebo null pole zmaže"#,
        effect: Effect::Edit,
        enabled: has_doc,
        run: set_info,
    },
    CommandSpec {
        id: "project.set",
        menu: false,
        shortcut: None,
        params: r#"{"project": {celé zadanie ako project.toml}} – dočasné pre pôdorys a pohľad na stenu, kým nebudú príkazy room.*"#,
        effect: Effect::Edit,
        enabled: has_doc,
        run: set_project,
    },
    CommandSpec {
        id: "defaults.set",
        menu: false,
        shortcut: None,
        params: r#"{"panel_material"?: "id", "back_material"?: "id"}"#,
        effect: Effect::Edit,
        enabled: has_doc,
        run: set_defaults,
    },
];

fn has_cv_template(s: &Session) -> Result<(), String> {
    let doc = s.doc().ok_or_else(|| tr!("engine.errors.no_project"))?;
    crate::export_cvj::template_of(&doc.project, None).map(|_| ())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExportCvjParams {
    out: PathBuf,
    template: Option<PathBuf>,
}

fn export_cvj(s: &mut Session, params: Value) -> Result<Value, EngineError> {
    let p: ExportCvjParams = parse("project.export_cvj", params)?;
    let doc = s.require_doc()?;
    let outcome =
        crate::export_cvj::run(&doc.project, &doc.standard, &p.out, p.template.as_deref())?;
    Ok(json!({
        "out": p.out,
        "template": outcome.template,
        "cabinets": outcome.cabinets.len(),
        "notes": outcome.notes,
    }))
}

fn has_path(s: &Session) -> Result<(), String> {
    match s.doc() {
        None => Err(tr!("engine.errors.no_project")),
        Some(d) if d.path.is_none() => Err(tr!("engine.document.no_file")),
        Some(_) => Ok(()),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PathParams {
    path: PathBuf,
}

fn open(s: &mut Session, params: Value) -> Result<Value, EngineError> {
    let p: PathParams = parse("project.open", params)?;
    let doc = Document::open(&p.path)?;
    let title = doc.title();
    s.set_document(doc);
    Ok(json!({ "title": title }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ImportParams {
    file: PathBuf,
    out: PathBuf,
    template: Option<PathBuf>,
}

fn import_cvj(s: &mut Session, params: Value) -> Result<Value, EngineError> {
    let p: ImportParams = parse("project.import_cvj", params)?;
    // vzor: zadaný, inak otvorený projekt (jeho materiály, štandard a stroj), inak examples/dielna
    let template = p.template.or_else(|| s.doc().and_then(|d| d.path.clone()));
    let outcome = crate::import_cvj::run(&p.file, &p.out, template.as_deref())?;
    let doc = Document::open(&p.out)?;
    let title = doc.title();
    s.set_document(doc);
    Ok(json!({
        "title": title,
        "walls": outcome.walls,
        "cabinets": outcome.cabinets,
        "openings": outcome.openings,
        "fronts": outcome.fronts,
        "notes": outcome.notes,
    }))
}

fn new(s: &mut Session, params: Value) -> Result<Value, EngineError> {
    let _: Empty = parse("project.new", params)?;
    let doc = Document::new_like(s.require_doc()?);
    let title = doc.title();
    s.set_document(doc);
    Ok(json!({ "title": title }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}

fn save(s: &mut Session, params: Value) -> Result<Value, EngineError> {
    let _: Empty = parse("project.save", params)?;
    let doc = s.doc_mut()?;
    doc.save()?;
    let standard = doc.standard_dirty;
    if standard {
        doc.save_standard()?;
    }
    Ok(json!({ "path": doc.path, "standardSaved": standard }))
}

fn save_as(s: &mut Session, params: Value) -> Result<Value, EngineError> {
    let p: PathParams = parse("project.save_as", params)?;
    let doc = s.doc_mut()?;
    let notes = doc.save_as(&p.path)?;
    Ok(json!({ "path": doc.path, "notes": notes }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Rename {
    name: String,
}

fn rename(s: &mut Session, params: Value) -> Result<Value, EngineError> {
    let p: Rename = parse("project.rename", params)?;
    let name = p.name.trim();
    if name.is_empty() {
        return Err(tr!("engine.project.empty_name").into());
    }
    s.doc_mut()?.project.project.name = name.to_owned();
    Ok(Value::Null)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Info {
    #[serde(default)]
    number: Option<Option<String>>,
    #[serde(default)]
    customer: Option<Option<String>>,
    #[serde(default)]
    designer: Option<Option<String>>,
    #[serde(default)]
    company: Option<Option<String>>,
    #[serde(default)]
    logo: Option<Option<String>>,
}

/// Najdlhší text v údajoch zákazky.
const MAX_INFO: usize = 200;
/// Najdlhšia cesta k logu.
const MAX_PATH: usize = 1000;

fn set_info(s: &mut Session, params: Value) -> Result<Value, EngineError> {
    const C: &str = "project.info";
    // null musí ostať odlíšiteľný od chýbajúceho poľa (Option<Option<_>> to so serde nevie)
    let nulls: Vec<String> = match &params {
        Value::Object(map) => map
            .iter()
            .filter(|(_, v)| v.is_null())
            .map(|(k, _)| k.clone())
            .collect(),
        _ => Vec::new(),
    };
    let p: Info = parse(C, params)?;
    // logo musí ísť načítať (chybu ohlási hneď, nie až pri tlači)
    if let Some(Some(path)) = &p.logo {
        let path = path.trim();
        if !path.is_empty() && path.chars().count() <= MAX_PATH {
            let base = s.require_doc()?.base.clone();
            crate::logo::load(&base, path).map_err(|message| EngineError::Params {
                command: C.into(),
                message,
            })?;
        }
    }
    let info = &mut s.doc_mut()?.project.project;
    for (key, value, slot) in [
        ("number", p.number, &mut info.number),
        ("customer", p.customer, &mut info.customer),
        ("designer", p.designer, &mut info.designer),
        ("company", p.company, &mut info.company),
        ("logo", p.logo, &mut info.logo),
    ] {
        let value = match value {
            Some(v) => v,
            None if nulls.iter().any(|k| k == key) => None,
            None => continue,
        };
        let value = value.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty());
        let max = if key == "logo" { MAX_PATH } else { MAX_INFO };
        if value.as_ref().is_some_and(|v| v.chars().count() > max) {
            return Err(EngineError::Params {
                command: C.into(),
                message: tr!("engine.project.too_long", key = key, max = max),
            });
        }
        *slot = value;
    }
    Ok(Value::Null)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SetProject {
    project: cc_model::ProjectFile,
}

fn set_project(s: &mut Session, params: Value) -> Result<Value, EngineError> {
    let p: SetProject = parse("project.set", params)?;
    if p.project.cabinets.len() > crate::MAX_CABINETS {
        return Err(tr!(
            "engine.session.too_many_cabinets",
            max = crate::MAX_CABINETS
        )
        .into());
    }
    s.doc_mut()?.project = p.project;
    s.fix_selection();
    Ok(Value::Null)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Defaults {
    panel_material: Option<String>,
    back_material: Option<String>,
}

fn set_defaults(s: &mut Session, params: Value) -> Result<Value, EngineError> {
    let p: Defaults = parse("defaults.set", params)?;
    let doc = s.doc_mut()?;
    if let Some(m) = p.panel_material {
        doc.project.defaults.panel_material = m;
    }
    if let Some(m) = p.back_material {
        doc.project.defaults.back_material = m;
    }
    Ok(Value::Null)
}
