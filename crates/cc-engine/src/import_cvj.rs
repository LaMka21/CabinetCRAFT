//! Import úlohy CABINET VISION (`.cvj`): príkaz `project.import_cvj` a
//! `cabinetcraft import-cvj úloha.cvj --out projekt.toml [--template priečinok]`.
//!
//! Prečíta úlohu CABINET VISION (iba čítanie), prevedie ju na projekt a zapíše
//! `project.toml`. Predvolené materiály a konštrukciu berie zo vzorového projektu;
//! `materials.toml`, štandard a `machine.toml` skopíruje vedľa výstupu, ak tam chýbajú.
//! Skrinky nesú čelá, zásuvky, škáry a atribúty z CV. Čo náš generátor alebo kontrola
//! odmietne, sa najprv zjednoduší (s poznámkou), inak sa skrinka vynechá, aby
//! `cabinetcraft build` na výsledku prešiel. Čelá, ktoré po stavbe nesedia s CV, sú
//! v poznámkach.

use std::fs;
use std::path::{Path, PathBuf};

use cc_config::LoadedProject;
use cc_i18n::tr;
use cc_import_cvj::CvjRoom;
use cc_model::{CabinetSpec, FrontKind, FrontSpec, MachineProfile, ProjectFile};

/// Predvolený vzorový projekt (relatívne k aktuálnemu priečinku).
pub const DEFAULT_TEMPLATE: &str = "examples/dielna";

/// Výsledok importu.
pub struct Outcome {
    pub notes: Vec<String>,
    pub cabinets: usize,
    /// čelá importovaných skriniek podľa druhu
    pub fronts: FrontCounts,
    pub walls: usize,
    pub openings: usize,
    pub copied: Vec<PathBuf>,
}

/// Počty čiel podľa druhu (dvojité dvierka = 1, stĺpce sa rátajú zvlášť).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub struct FrontCounts {
    pub doors: usize,
    pub pairs: usize,
    pub drawers: usize,
    pub false_fronts: usize,
    pub panels: usize,
    pub open: usize,
    /// skrinky iba s čelami (bez korpusu)
    pub front_only: usize,
}

impl FrontCounts {
    pub fn of(project: &ProjectFile) -> FrontCounts {
        let mut c = FrontCounts::default();
        for cab in &project.cabinets {
            if !cab.carcass {
                c.front_only += 1;
            }
            for f in &cab.fronts {
                let leaves: Vec<&FrontSpec> = if f.columns.is_empty() {
                    vec![f]
                } else {
                    f.columns.iter().collect()
                };
                for l in leaves {
                    match l.kind {
                        FrontKind::Door => c.doors += 1,
                        FrontKind::Pair => c.pairs += 1,
                        FrontKind::Drawer => c.drawers += 1,
                        FrontKind::FalseFront => c.false_fronts += 1,
                        FrontKind::Panel => c.panels += 1,
                        FrontKind::Open => c.open += 1,
                    }
                }
            }
        }
        c
    }
}

impl std::fmt::Display for FrontCounts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = tr!(
            "engine.import_cvj.front_counts",
            doors = self.doors,
            pairs = self.pairs,
            drawers = self.drawers,
            false_fronts = self.false_fronts,
            panels = self.panels,
            open = self.open,
            front_only = self.front_only
        );
        f.write_str(&text)
    }
}

/// `project.toml` vzoru: priečinok → `priečinok/project.toml`, inak súbor sám.
pub fn template_project(template: Option<&Path>) -> PathBuf {
    let t = template.unwrap_or_else(|| Path::new(DEFAULT_TEMPLATE));
    if t.is_dir() {
        t.join("project.toml")
    } else {
        t.to_path_buf()
    }
}

pub fn run(cvj: &Path, out: &Path, template: Option<&Path>) -> Result<Outcome, String> {
    run_inner(cvj, out, template).map_err(|e| e.to_string())
}

fn run_inner(
    cvj: &Path,
    out: &Path,
    template: Option<&Path>,
) -> Result<Outcome, Box<dyn std::error::Error>> {
    let template_path = template_project(template);
    if !template_path.exists() {
        let file = template_path.display().to_string();
        return Err(tr!("engine.import_cvj.template_missing", file = file).into());
    }
    let tpl = cc_config::load(&template_path)?;
    let machine = match cc_pipeline::default_machine(&template_path) {
        Some(p) => Some(cc_config::load_machine(&p)?),
        None => None,
    };

    let job = cc_import_cvj::read_job(cvj)?;
    let imported = cc_import_cvj::to_project_with(&job, &tpl.project.defaults, Some(&tpl.standard));
    let (mut project, mut notes, mut sources) =
        (imported.project, imported.notes, imported.sources);
    // zákazka CV ako šablóna pre zápis .cvj (absolútna cesta)
    project.project.cv_template = Some(
        fs::canonicalize(cvj)
            .unwrap_or_else(|_| cvj.to_path_buf())
            .display()
            .to_string()
            .trim_start_matches(r"\\?\")
            .to_owned(),
    );
    if !job.consumed_all {
        let reason = match job.trailing_error.as_deref() {
            Some(e) => e.to_owned(),
            None => tr!("engine.import_cvj.rest_of_data"),
        };
        notes.push(tr!("engine.import_cvj.incomplete", reason = reason));
    }
    let built = drop_unbuildable(
        &mut project,
        &mut sources,
        &tpl,
        machine.as_ref(),
        &mut notes,
    );
    if let Some(room) = imported.room.and_then(|i| job.rooms.get(i)) {
        compare_fronts(&project, &sources, &built, room, &mut notes);
    }

    let mut text =
        String::from("# Importované z CABINET VISION (.cvj) príkazom `cabinetcraft import-cvj`.\n");
    for n in &notes {
        text.push_str(&format!("# pozn.: {}\n", n.replace('\n', " ")));
    }
    text.push('\n');
    text.push_str(&toml::to_string(&project)?);

    if let Some(dir) = out.parent().filter(|d| !d.as_os_str().is_empty()) {
        fs::create_dir_all(dir)?;
    }
    fs::write(out, text)?;

    // podporné súbory vedľa výstupu
    let src_dir = template_path.parent().unwrap_or_else(|| Path::new("."));
    let dst_dir = out
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut copied = Vec::new();
    let construction = format!("{}.toml", project.defaults.construction);
    for rel in [
        PathBuf::from("materials.toml"),
        Path::new("standards").join(&construction),
        PathBuf::from("machine.toml"),
    ] {
        let (src, dst) = (src_dir.join(&rel), dst_dir.join(&rel));
        if src.exists() && !dst.exists() {
            if let Some(d) = dst.parent() {
                fs::create_dir_all(d)?;
            }
            fs::copy(&src, &dst)?;
            copied.push(dst);
        }
    }

    Ok(Outcome {
        fronts: FrontCounts::of(&project),
        cabinets: project.cabinets.len(),
        walls: project.walls.len(),
        openings: project.openings.len(),
        notes,
        copied,
    })
}

/// Čelá postavenej skrinky: (výška, šírka).
type FrontSizes = Vec<(f64, f64)>;

/// Postaví jednu skrinku; vráti chybu generátora alebo kontroly, inak rozmery čiel.
fn try_build(
    project: &ProjectFile,
    cab: &CabinetSpec,
    tpl: &LoadedProject,
    machine: Option<&MachineProfile>,
) -> Result<FrontSizes, String> {
    let single = ProjectFile {
        cabinets: vec![cab.clone()],
        ..project.clone()
    };
    let built = cc_pipeline::build(&single, &tpl.standard, &tpl.materials, machine)
        .map_err(|e| e.to_string())?;
    if let Some(i) = built.report.errors().next() {
        return Err(i.to_string());
    }
    Ok(built
        .model
        .cabinets
        .iter()
        .flat_map(|c| c.parts.iter())
        .filter(|p| p.role.is_front())
        .map(|p| (p.length, p.width))
        .collect())
}

/// Zjednodušenie skrinky, ktorú generátor odmietol: (popis v zvolenom jazyku, úprava;
/// `true` = niečo sa zmenilo).
type Relax = (fn() -> String, fn(&mut CabinetSpec) -> bool);

const RELAX: [Relax; 6] = [
    // diely blendy (dno 50, čelo 60) sú pri CX100 pod minimom stroja: bez kolíkov sa
    // nevŕtajú a kontrola ich pustí (v nestingu sa obrobia na tabuli)
    (
        || tr!("engine.import_cvj.relax_filler_dowels"),
        |c| {
            c.filler
                .as_mut()
                .is_some_and(|f| std::mem::replace(&mut f.dowels, false))
        },
    ),
    (
        || tr!("engine.import_cvj.relax_box_heights"),
        |c| {
            let mut changed = false;
            for f in &mut c.fronts {
                changed |= f.box_height.take().is_some();
                for col in &mut f.columns {
                    changed |= col.box_height.take().is_some();
                }
            }
            changed
        },
    ),
    (
        || tr!("engine.import_cvj.relax_front_sizes"),
        |c| {
            let mut changed = false;
            for f in &mut c.fronts {
                changed |= f.height.take().is_some();
                for col in &mut f.columns {
                    changed |= col.width.take().is_some();
                }
            }
            changed
        },
    ),
    (
        || tr!("engine.import_cvj.relax_reveals"),
        |c| {
            let mut changed = !c.reveals.is_empty();
            c.reveals = Default::default();
            for f in &mut c.fronts {
                changed |= !f.adjust.is_empty();
                f.adjust = Default::default();
                for col in &mut f.columns {
                    changed |= !col.adjust.is_empty();
                    col.adjust = Default::default();
                }
            }
            changed
        },
    ),
    (
        || tr!("engine.import_cvj.relax_slant"),
        |c| c.slant.take().is_some(),
    ),
    (
        || tr!("engine.import_cvj.relax_fronts"),
        |c| {
            let changed = !c.fronts.is_empty();
            c.fronts.clear();
            c.drawer_length = None;
            changed
        },
    ),
];

/// Vynechá skrinky, ktoré generátor odmietne alebo pri ktorých kontrola nájde chyby.
/// Najprv skúsi skrinku zjednodušiť (výšky korpusov zásuviek, výšky čiel, úpravy čiel
/// a škáry, nakoniec bez čiel) a každé zjednodušenie zapíše do poznámok. Vráti rozmery
/// čiel ponechaných skriniek; `sources` ostane zarovnané so skrinkami.
fn drop_unbuildable(
    project: &mut ProjectFile,
    sources: &mut Vec<usize>,
    tpl: &LoadedProject,
    machine: Option<&MachineProfile>,
    notes: &mut Vec<String>,
) -> Vec<FrontSizes> {
    let cabinets = std::mem::take(&mut project.cabinets);
    let srcs = std::mem::take(sources);
    let mut sizes = Vec::new();
    for (i, mut cab) in cabinets.into_iter().enumerate() {
        let mut result = try_build(project, &cab, tpl, machine);
        let first_error = result.as_ref().err().cloned();
        let mut relaxed: Vec<String> = Vec::new();
        for (what, relax) in RELAX {
            if result.is_ok() {
                break;
            }
            if relax(&mut cab) {
                relaxed.push(what());
                result = try_build(project, &cab, tpl, machine);
            }
        }
        match result {
            Ok(s) => {
                if let (false, Some(e)) = (relaxed.is_empty(), first_error) {
                    let changes = relaxed.join("; ");
                    notes.push(tr!(
                        "engine.import_cvj.relaxed",
                        id = &cab.id,
                        changes = changes,
                        error = e
                    ));
                }
                // zjednodušené čelá sú stále „čelá z CV“: zápis .cvj ponechá delenie zo zákazky
                if cab.cv_fronts.is_some() {
                    cab.cv_fronts = Some(cab.front_fingerprint());
                }
                project.cabinets.push(cab);
                if let Some(s) = srcs.get(i) {
                    sources.push(*s);
                }
                sizes.push(s);
            }
            Err(p) => {
                notes.push(tr!(
                    "engine.import_cvj.skipped",
                    id = &cab.id,
                    w = format!("{:.0}", cab.w),
                    h = format!("{:.0}", cab.h),
                    d = format!("{:.0}", cab.d),
                    error = first_error.unwrap_or(p)
                ));
                // v zákazke CV ostane aj po zápise .cvj
                project.project.cv_skipped.extend(cab.cv_id);
            }
        }
    }
    sizes
}

/// Porovná čelá postavených skriniek s čelami v CV (do 0,5 mm) a rozdiely zapíše.
fn compare_fronts(
    project: &ProjectFile,
    sources: &[usize],
    built: &[FrontSizes],
    room: &CvjRoom,
    notes: &mut Vec<String>,
) {
    let fmt = |v: &[(f64, f64)]| {
        if v.is_empty() {
            return tr!("engine.import_cvj.no_fronts");
        }
        v.iter()
            .map(|(h, w)| format!("{w:.1} × {h:.1}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let sorted = |mut v: Vec<(f64, f64)>| {
        v.sort_by(|a, b| b.0.total_cmp(&a.0).then(b.1.total_cmp(&a.1)));
        v
    };
    for ((cab, src), ours) in project.cabinets.iter().zip(sources).zip(built) {
        let Some(cv) = room.cabinets.get(*src) else {
            continue;
        };
        let theirs = sorted(cv.face_fronts().map(|f| (f.size.y, f.size.x)).collect());
        let ours = sorted(ours.clone());
        let same = theirs.len() == ours.len()
            && theirs
                .iter()
                .zip(&ours)
                .all(|(a, b)| (a.0 - b.0).abs() < 0.5 && (a.1 - b.1).abs() < 0.5);
        if !same {
            notes.push(tr!(
                "engine.import_cvj.fronts_differ",
                id = &cab.id,
                cv = fmt(&theirs),
                ours = fmt(&ours)
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cc_model::{CabinetSpec, CabinetType};

    fn example_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/dielna")
    }

    fn cab(id: &str, w: f64) -> CabinetSpec {
        CabinetSpec {
            id: id.into(),
            cabinet_type: CabinetType::Base,
            w,
            h: 720.0,
            d: 500.0,
            adjustable_shelves: 0,
            fixed_shelves: Vec::new(),
            wall: None,
            at: 0.0,
            elevation: None,
            outset: 0.0,
            fronts: Vec::new(),
            reveals: Default::default(),
            front_mount: None,
            hinge: None,
            drawer: None,
            drawer_length: None,
            top_style: None,
            stretcher_drop: None,
            back: true,
            carcass: true,
            lock_width: false,
            comment: None,
            placement: None,
            catalog: None,
            cv_id: None,
            cv_fronts: None,
            filler: None,
            slant: None,
            niche: false,
            led_rail: None,
            options: Default::default(),
        }
    }

    #[test]
    fn template_dir_or_file() {
        let dir = example_dir();
        assert_eq!(template_project(Some(&dir)), dir.join("project.toml"));
        let file = dir.join("kuchyna.toml");
        assert_eq!(template_project(Some(&file)), file);
    }

    #[test]
    fn unbuildable_cabinets_become_notes() {
        let tpl = cc_config::load(&example_dir().join("project.toml")).unwrap();
        let mut project = tpl.project.clone();
        project.cabinets = vec![cab("SP1", 600.0), cab("SP2", 20.0)];
        let mut notes = Vec::new();
        let mut sources = vec![4, 7];
        let sizes = drop_unbuildable(&mut project, &mut sources, &tpl, None, &mut notes);
        assert_eq!(project.cabinets.len(), 1);
        assert_eq!(project.cabinets[0].id, "SP1");
        assert_eq!(sources, [4]);
        assert_eq!(sizes.len(), 1);
        assert_eq!(notes.len(), 1);
        assert!(notes[0].contains("SP2"), "{}", notes[0]);
    }

    /// Blenda REF skr. 9: čelo 60 a dno 50 sú pod minimom CX100 (200 × 70); bez kolíkov sa
    /// nevŕtajú, skrinka aj čelá ostanú. Bez profilu stroja (bez limitov) kolíky ostanú.
    #[test]
    fn filler_dowels_are_relaxed_for_cx100() {
        let tpl = cc_config::load(&example_dir().join("project.toml")).unwrap();
        let machine = cc_config::load_machine(&example_dir().join("machine.toml")).unwrap();
        let mut a = cab("HO9", 576.5);
        a.cabinet_type = CabinetType::Wall;
        a.h = 682.0;
        a.d = 330.0;
        a.fronts = vec![FrontSpec::new(FrontKind::Door)];
        a.filler = Some(cc_model::FillerSpec {
            width: 50.0,
            front_width: Some(60.0),
            bottom: true,
            ..Default::default()
        });
        for (m, dowels) in [(Some(&machine), false), (None, true)] {
            let mut project = tpl.project.clone();
            project.cabinets = vec![a.clone()];
            let mut notes = Vec::new();
            let mut sources = vec![0];
            drop_unbuildable(&mut project, &mut sources, &tpl, m, &mut notes);
            assert_eq!(project.cabinets.len(), 1, "{notes:?}");
            let c = &project.cabinets[0];
            assert_eq!(c.filler.as_ref().map(|f| f.dowels), Some(dowels));
            assert_eq!(c.fronts.len(), 1);
            assert_eq!(notes.len(), usize::from(!dowels), "{notes:?}");
            if !dowels {
                assert!(notes[0].contains("kolíky blendy"), "{}", notes[0]);
            }
        }
    }

    /// Čelá z CV, ktoré náš generátor nevie postaviť, sa zjednodušia, skrinka ostane.
    #[test]
    fn unbuildable_fronts_are_relaxed() {
        let tpl = cc_config::load(&example_dir().join("project.toml")).unwrap();
        let mut project = tpl.project.clone();
        // korpus zásuvky 999 nie je v knižnici štandardu
        let mut a = cab("SP1", 600.0);
        let mut drawer = FrontSpec::new(FrontKind::Drawer);
        drawer.box_height = Some(999.0);
        a.fronts = vec![drawer];
        // čelá spolu vyššie ako skrinka
        let mut b = cab("SP2", 600.0);
        let mut door = FrontSpec::new(FrontKind::Door);
        door.height = Some(500.0);
        let mut door2 = door.clone();
        door2.height = Some(400.0);
        b.fronts = vec![door, door2];
        project.cabinets = vec![a, b];
        let mut notes = Vec::new();
        let mut sources = vec![0, 1];
        let sizes = drop_unbuildable(&mut project, &mut sources, &tpl, None, &mut notes);
        assert_eq!(project.cabinets.len(), 2, "{notes:?}");
        assert_eq!(sources, [0, 1]);
        assert_eq!(project.cabinets[0].fronts[0].box_height, None);
        assert!(project.cabinets[1]
            .fronts
            .iter()
            .all(|f| f.height.is_none()));
        assert_eq!(sizes[1].len(), 2);
        assert_eq!(notes.len(), 2, "{notes:?}");
        assert!(notes[0].contains("korpusov zásuviek"), "{}", notes[0]);
        assert!(notes[1].contains("delia skrinku rovnako"), "{}", notes[1]);
    }

    #[test]
    fn front_counts() {
        let mut p = cc_config::load(&example_dir().join("kuchyna.toml"))
            .unwrap()
            .project;
        let c = FrontCounts::of(&p);
        assert_eq!((c.doors, c.pairs, c.drawers), (7, 1, 4));
        p.cabinets[0].carcass = false;
        assert_eq!(FrontCounts::of(&p).front_only, 1);
        assert!(c.to_string().contains("zásuvky 4"));
    }

    #[test]
    fn missing_template_is_error() {
        let r = run(
            Path::new("neexistuje.cvj"),
            Path::new("out.toml"),
            Some(Path::new("neexistujuci/vzor.toml")),
        );
        assert!(r.is_err());
    }
}
