//! Celý beh CabinetCRAFT: zadanie → diely → spoje a operácie → validácia → výstupy.
//!
//! Spoločné pre príkazový riadok (`cc-cli`) a aplikáciu (`cc-app`).

#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::todo,
        clippy::unimplemented,
        clippy::unreachable
    )
)]

pub mod labels;
mod link;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use cc_i18n::tr;
use cc_model::{
    Issue, MachineProfile, MaterialLibrary, Model, NestingProfile, ProductionMode, ProjectFile,
    Standard, XcsProfile,
};
use cc_validate::Report;

pub use cc_out_csv::CsvOptions;
pub use cc_out_pdf::FontChoice;

#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error(transparent)]
    Generate(#[from] cc_generate::GenerateError),
    #[error(transparent)]
    Joints(#[from] cc_joints::JointError),
}

/// Model s operáciami a všetky zistenia.
#[derive(Debug, Clone)]
pub struct Built {
    pub model: Model,
    /// Varovania z tvorby spojov (napr. `JointRuleMissing`).
    pub warnings: Vec<Issue>,
    /// Validácia (kap. 8).
    pub report: Report,
}

impl Built {
    /// Varovania zo spojov a zistenia validácie v poradí výpisu.
    pub fn issues(&self) -> impl Iterator<Item = &Issue> {
        self.warnings.iter().chain(&self.report.issues)
    }

    pub fn has_errors(&self) -> bool {
        self.report.has_errors()
    }
}

/// Vygeneruje diely a operácie a skontroluje ich. Bez profilu stroja sa vynechajú
/// kontroly rozmerov stroja a vrtákov.
pub fn build(
    project: &ProjectFile,
    standard: &Standard,
    materials: &MaterialLibrary,
    machine: Option<&MachineProfile>,
) -> Result<Built, BuildError> {
    let model = cc_generate::build(project, standard, materials)?;
    let mut applied = cc_joints::apply(model, standard)?;
    link::apply(&mut applied.model, project, standard);
    let report = cc_validate::check(&applied.model, machine);
    let mut warnings = applied.warnings;
    // umiestnenie v miestnosti (kap. 13.4): iba varovania, výrobu neblokuje
    warnings.extend(cc_room::check(project));
    // police v skrinke iba so zásuvkami sa nevložia (generátor), povedať prečo
    for c in &project.cabinets {
        if c.adjustable_shelves > 0 && c.carcass && !c.has_shelf_opening() {
            warnings.push(Issue::warning(
                cc_model::IssueCode::ShelvesNoOpening,
                None,
                tr!(
                    "pipeline.shelves_no_opening",
                    cabinet = &c.id,
                    count = c.adjustable_shelves
                ),
            ));
        }
    }
    Ok(Built {
        model: applied.model,
        warnings,
        report,
    })
}

/// `machine.toml` vedľa projektu, ak existuje.
pub fn default_machine(project: &Path) -> Option<PathBuf> {
    let candidate = project
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("machine.toml");
    candidate.is_file().then_some(candidate)
}

/// Čo sa zapísalo.
#[derive(Debug, Clone, Default)]
pub struct Written {
    pub parts: usize,
    pub operations: usize,
    /// Programy CX100 (`xcs/*.xcs`).
    pub xcs: usize,
    pub mpr: usize,
    /// Programy tabúľ pre nesting (`nesting/*.xcs`); 0 v režime `cx100`.
    pub sheets: usize,
    /// Súhrn nestingu po materiáloch (počet tabúľ, využitie), pre výpis.
    pub nesting: Vec<String>,
    /// Štítky dielov (`labels/stitky.pdf` podľa `[labels]`, predvolene 80 × 80 mm, a `labels/stitky-A4.pdf`).
    pub labels: usize,
    /// Upozornenia pre používateľa (neoverené šablóny, chýbajúci výstup pre stroj).
    pub notices: Vec<String>,
}

/// Profily pre režim nesting, ak je zapnutý a úplný.
pub(crate) fn nesting_profiles(
    machine: Option<&MachineProfile>,
) -> Option<(&XcsProfile, &NestingProfile)> {
    let m = machine?;
    if m.production.mode != ProductionMode::Nesting {
        return None;
    }
    Some((m.xcs.as_ref()?, m.nesting.as_ref()?))
}

/// Zapíše `cutlist.csv`, `model.json`, `xcs/*.xcs` + `RUN.bat` a `mpr/*.mpr` do `out`.
/// V režime `production.mode = "nesting"` navyše `nesting/*.xcs` (programy tabúľ)
/// a `nesting/CncRun.txt`; v `xcs/` sú vtedy iba programy CX100 pre zvyšnú prácu.
///
/// Validáciu nekontroluje: o zápise pri chybách rozhoduje volajúci.
/// Štítky vloží systémové písmo s diakritikou ([`write_outputs_with`]).
pub fn write_outputs(
    model: &Model,
    machine: Option<&MachineProfile>,
    out: &Path,
    csv: CsvOptions,
) -> std::io::Result<Written> {
    write_outputs_with(model, machine, out, csv, FontChoice::System)
}

/// Ako [`write_outputs`], s voľbou písma štítkov (testy so zlatými súbormi:
/// [`FontChoice::Base14`], lebo systémové písmo sa medzi počítačmi líši).
pub fn write_outputs_with(
    model: &Model,
    machine: Option<&MachineProfile>,
    out: &Path,
    csv: CsvOptions,
    fonts: FontChoice,
) -> std::io::Result<Written> {
    fs::create_dir_all(out)?;
    let mut written = Written {
        parts: model.parts().count(),
        operations: model.parts().map(|p| p.operations.len()).sum(),
        ..Written::default()
    };
    let mut programs = BTreeMap::<String, String>::new();

    let mut has_xcs = false;
    // štítky: tabule nestingu (číslo dielu = poradie prerezania, poradie tlače z profilu)
    let label_settings = machine.map(|m| m.labels.clone()).unwrap_or_default();
    let mut label_sheets: Vec<labels::LabelSheet> = Vec::new();
    if let Some((xcs_profile, nest_profile)) = nesting_profiles(machine) {
        // SCM nesting + CX100 (findings 08): tabule a zvyšná práca
        let output = cc_out_xcs::nest::render_nesting(model, xcs_profile, nest_profile)
            .map_err(std::io::Error::other)?;
        write_xcs(out, &output.cx100, &mut written, false)?;
        let dir = out.join("nesting");
        fs::create_dir_all(&dir)?;
        remove_stale(&dir, &["xcs"])?;
        for sheet in &output.sheets {
            fs::write(dir.join(&sheet.name), &sheet.content)?;
        }
        if let Some((name, content)) = &output.cnc_run {
            fs::write(dir.join(name), content)?;
        }
        if !output.unverified.is_empty() {
            written.notices.push(tr!(
                "pipeline.nesting_unverified",
                templates = output
                    .unverified
                    .iter()
                    .copied()
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if !output.plan.unplaced.is_empty() {
            written.notices.push(tr!(
                "pipeline.nesting_unplaced",
                parts = output.plan.unplaced.join(", ")
            ));
        }
        for group in &output.plan.groups {
            let names: Vec<&str> = output
                .sheets
                .iter()
                .filter(|s| s.material == group.material && s.thickness == group.thickness)
                .map(|s| s.name.as_str())
                .collect();
            written.nesting.push(tr!(
                "pipeline.nesting_summary",
                material = &group.material,
                thickness = group.thickness,
                sheets = group.sheets.len(),
                names = names.join(", "),
                utilisation = format!("{:.0}", group.utilisation() * 100.0)
            ));
        }
        written.sheets = output.sheets.len();
        label_sheets = labels::label_sheets(
            &output.sheets,
            label_settings.order,
            label_settings.offcut_number,
        );
        add_programs(&mut programs, output.program_names());
        has_xcs = true;
    } else {
        if machine.is_some_and(|m| m.production.mode == ProductionMode::Nesting) {
            written
                .notices
                .push(tr!("pipeline.nesting_profile_incomplete"));
        }
        // SCM Maestro .xcs (kap. 7.4), iba ak má profil stroja sekciu [xcs].
        let xcs = machine
            .and_then(|m| m.xcs.as_ref())
            .map(|profile| cc_out_xcs::render(model, profile));
        if let Some(output) = &xcs {
            write_xcs(out, output, &mut written, true)?;
            add_programs(&mut programs, output.program_names());
            has_xcs = true;
        }
    }

    // WoodWOP .mpr (kap. 7.3, alternatíva), iba ak má profil stroja sekciu [mpr].
    let mpr = machine
        .and_then(|m| m.mpr.as_ref())
        .map(|profile| cc_out_mpr::render(model, profile));
    if let Some(output) = &mpr {
        let dir = out.join("mpr");
        fs::create_dir_all(&dir)?;
        remove_stale(&dir, &["mpr"])?;
        for file in &output.files {
            fs::write(dir.join(&file.name), &file.content)?;
        }
        if !output.unverified_macros.is_empty() {
            written.notices.push(tr!(
                "pipeline.mpr_unverified",
                macros = output
                    .unverified_macros
                    .iter()
                    .copied()
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        written.mpr = output.files.len();
        add_programs(&mut programs, output.program_names());
    }
    if !has_xcs && mpr.is_none() {
        written.notices.push(tr!("pipeline.no_machine_output"));
    }

    let job = labels::build_label_job(model, &programs, label_sheets, &label_settings);
    let dir = out.join("labels");
    fs::create_dir_all(&dir)?;
    let fonts = cc_out_pdf::Fonts::new(fonts);
    fs::write(
        dir.join("stitky.pdf"),
        cc_out_pdf::labels_pdf(&job.labels, &fonts, job.size),
    )?;
    fs::write(
        dir.join("stitky-A4.pdf"),
        cc_out_pdf::labels_a4_pdf(&job.labels, &fonts, job.size),
    )?;
    written.labels = job.labels.len();

    fs::write(
        out.join("cutlist.csv"),
        cc_out_csv::render_with_programs(model, csv, &programs),
    )?;
    let mut json = serde_json::to_string_pretty(model).map_err(std::io::Error::other)?;
    json.push('\n');
    fs::write(out.join("model.json"), json)?;
    Ok(written)
}

/// `part_id` → názvy programov `.xcs` aj `.mpr` (stĺpec `program` v rozpise).
pub fn program_names(model: &Model, machine: Option<&MachineProfile>) -> BTreeMap<String, String> {
    let mut programs = BTreeMap::new();
    if let Some((xcs, nest)) = nesting_profiles(machine) {
        if let Ok(output) = cc_out_xcs::nest::render_nesting(model, xcs, nest) {
            add_programs(&mut programs, output.program_names());
        }
    } else if let Some(profile) = machine.and_then(|m| m.xcs.as_ref()) {
        add_programs(
            &mut programs,
            cc_out_xcs::render(model, profile).program_names(),
        );
    }
    if let Some(profile) = machine.and_then(|m| m.mpr.as_ref()) {
        add_programs(
            &mut programs,
            cc_out_mpr::render(model, profile).program_names(),
        );
    }
    programs
}

/// Zapíše `xcs/*.xcs` a `RUN.bat` (programy CX100) a upozornenie na neoverené šablóny.
/// `notice = false`: upozornenie pridá volajúci (režim nesting ho spája s nestingom).
fn write_xcs(
    out: &Path,
    output: &cc_out_xcs::XcsOutput,
    written: &mut Written,
    notice: bool,
) -> std::io::Result<()> {
    let dir = out.join("xcs");
    fs::create_dir_all(&dir)?;
    remove_stale(&dir, &["xcs", "bat"])?;
    for file in &output.files {
        fs::write(dir.join(&file.name), &file.content)?;
    }
    if let Some((name, content)) = &output.run_bat {
        fs::write(dir.join(name), content)?;
    }
    if notice && !output.unverified.is_empty() {
        written.notices.push(tr!(
            "pipeline.xcs_unverified",
            templates = output
                .unverified
                .iter()
                .copied()
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    written.xcs = output.files.len();
    Ok(())
}

fn add_programs(all: &mut BTreeMap<String, String>, more: BTreeMap<String, String>) {
    for (part, names) in more {
        all.entry(part)
            .and_modify(|s| {
                s.push(',');
                s.push_str(&names);
            })
            .or_insert(names);
    }
}

/// Zmaže výstupy s danými príponami z predchádzajúceho behu, aby v priečinku
/// neostali programy dielov, ktoré už neexistujú. Iné súbory nechá tak.
fn remove_stale(dir: &Path, extensions: &[&str]) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        let matches = path
            .extension()
            .is_some_and(|e| extensions.iter().any(|ext| e.eq_ignore_ascii_case(ext)));
        if matches {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod i18n_keys {
    /// Kľúče z volaní makra prekladu v zdroji (aj keď rustfmt dá kľúč na ďalší riadok,
    /// ktorý `cargo xtask i18n` nevidí).
    fn keys(src: &str) -> Vec<&str> {
        src.match_indices(concat!("tr", "!("))
            // nie `include_str!(`
            .filter(|(i, _)| {
                !src[..*i]
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_alphanumeric() || c == '_')
            })
            .filter_map(|(i, m)| {
                src[i + m.len()..]
                    .trim_start()
                    .strip_prefix('"')?
                    .split('"')
                    .next()
            })
            .filter(|k| {
                k.contains('.')
                    && k.chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
            })
            .collect()
    }

    #[test]
    fn every_key_is_in_builtin_languages() {
        let sources = [include_str!("lib.rs")];
        let keys: Vec<&str> = sources.iter().flat_map(|s| keys(s)).collect();
        assert!(keys.len() >= 8, "{keys:?}");
        for (code, text) in cc_i18n::BUILTIN {
            let catalog = cc_i18n::Catalog::parse(code, text).unwrap();
            let missing: Vec<&&str> = keys.iter().filter(|k| !catalog.contains(k)).collect();
            assert!(missing.is_empty(), "{code}.toml: chýbajú {missing:?}");
        }
    }
}
