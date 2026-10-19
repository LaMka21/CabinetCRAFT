//! Zápis projektu do zákazky CABINET VISION (`.cvj`): príkaz `project.export_cvj` a
//! `cabinetcraft export-cvj projekt.toml --out zákazka.cvj [--template pôvodná.cvj]`.
//!
//! Šablóna je zákazka CV, z ktorej sa projekt importoval (`project.cv_template`), alebo iná
//! zákazka CV dielne. Šablóna sa iba číta; výstup je nový súbor (pôvodnú zákazku neprepíše).
//! Prúd `Header` (licencia) sa vynechá, CV si ho pri uložení vytvorí sám (findings 13 §3.4).

use std::fs;
use std::path::{Path, PathBuf};

use cc_i18n::tr;
use cc_model::{ProjectFile, Standard};

/// Výsledok zápisu.
pub struct Outcome {
    /// skrinky zapísané do súboru (ID projektu, číslo CABNO)
    pub cabinets: Vec<(String, i32)>,
    pub notes: Vec<String>,
    pub template: PathBuf,
}

/// Šablóna: zadaná, inak zákazka CV, z ktorej sa projekt importoval.
pub fn template_of(project: &ProjectFile, template: Option<&Path>) -> Result<PathBuf, String> {
    template
        .map(Path::to_path_buf)
        .or_else(|| project.project.cv_template.as_ref().map(PathBuf::from))
        .ok_or_else(|| tr!("engine.export_cvj.no_template"))
}

pub fn run(
    project: &ProjectFile,
    standard: &Standard,
    out: &Path,
    template: Option<&Path>,
) -> Result<Outcome, String> {
    let template = template_of(project, template)?;
    if !template.exists() {
        return Err(tr!(
            "engine.export_cvj.template_missing",
            file = template.display().to_string()
        ));
    }
    if out.extension().and_then(|e| e.to_str()) != Some("cvj") {
        return Err(tr!(
            "engine.export_cvj.bad_extension",
            file = out.display().to_string()
        ));
    }
    let same = |a: &Path, b: &Path| match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    };
    if same(out, &template) {
        return Err(tr!("engine.export_cvj.overwrites_template"));
    }
    let t = cc_out_cvj::Template::open(&template).map_err(|e| e.to_string())?;
    let ex =
        cc_out_cvj::export::export_project(project, standard, &t).map_err(|e| e.to_string())?;

    // zápis do dočasného súboru a premenovanie: pri chybe ostane predchádzajúci výstup celý
    if let Some(dir) = out.parent().filter(|d| !d.as_os_str().is_empty()) {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let tmp = out.with_extension("cvj.tmp");
    let written = fs::File::create(&tmp)
        .map_err(|e| e.to_string())
        .and_then(|f| {
            cc_out_cvj::write_cvj(f, &t, &ex.contents, &["Header"]).map_err(|e| e.to_string())
        })
        .and_then(|_| fs::rename(&tmp, out).map_err(|e| e.to_string()));
    if let Err(e) = written {
        let _ = fs::remove_file(&tmp);
        return Err(tr!(
            "engine.export_cvj.write_failed",
            file = out.display().to_string(),
            error = e
        ));
    }
    Ok(Outcome {
        cabinets: ex.cabinets,
        notes: ex.notes,
        template,
    })
}
