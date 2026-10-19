//! Štandard dielne (`examples/dielna`) voči reálnym programom CV pre CX100.
//!
//! Porovnáva polohy kolíkov a drážky zadnej steny s programmi zo zákazky REF2026
//! (`tests/fixtures/scm-cx100`), plus golden súbory a bezchybnú validáciu.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

struct Built {
    model: cc_model::Model,
    machine: cc_model::MachineProfile,
    warnings: Vec<cc_model::Issue>,
    out: cc_out_xcs::XcsOutput,
}

fn build() -> Built {
    let dir = root().join("examples/dielna");
    let loaded = cc_config::load(&dir.join("project.toml")).unwrap();
    let machine = cc_config::load_machine(&dir.join("machine.toml")).unwrap();
    let model = cc_generate::build(&loaded.project, &loaded.standard, &loaded.materials).unwrap();
    let applied = cc_joints::apply(model, &loaded.standard).unwrap();
    let out = cc_out_xcs::render(&applied.model, machine.xcs.as_ref().unwrap());
    Built {
        model: applied.model,
        machine,
        warnings: applied.warnings,
        out,
    }
}

/// Vŕtania a drážky programu: rovina → [(x, y, hĺbka, Ø)], drážky (y, hĺbka, xs, xe), L.
struct Program {
    drills: BTreeMap<String, Vec<(f64, f64, f64, f64)>>,
    slots: Vec<(f64, f64, f64, f64)>,
    length: f64,
}

fn args(line: &str) -> Vec<String> {
    let inner = &line[line.find('(').unwrap() + 1..line.rfind(')').unwrap()];
    inner.split(',').map(|s| s.trim().to_owned()).collect()
}

fn parse(text: &str) -> Program {
    let mut p = Program {
        drills: BTreeMap::new(),
        slots: Vec::new(),
        length: 0.0,
    };
    let mut plane = String::new();
    let mut segment: Option<(f64, f64, f64)> = None;
    for line in text.lines().map(str::trim) {
        let num = |s: &str| s.parse::<f64>().unwrap();
        if line.starts_with("CreateFinishedWorkpieceBox(") {
            p.length = num(&args(line)[1]);
        } else if line.starts_with("SelectWorkplane(") {
            plane = args(line)[0].trim_matches('"').to_owned();
        } else if line.starts_with("CreateDrill(") {
            // meno môže obsahovať čiarky? nie – vo vzoroch ani u nás
            let a = args(line);
            p.drills.entry(plane.clone()).or_default().push((
                num(&a[1]),
                num(&a[2]),
                num(&a[3]),
                num(&a[4]),
            ));
        } else if line.starts_with("CreateSegment(") {
            let a = args(line);
            segment = Some((num(&a[2]), num(&a[1]), num(&a[3])));
        } else if line.starts_with("CreateSlot(") {
            let (y, xs, xe) = segment.take().expect("CreateSlot bez CreateSegment");
            p.slots.push((y, num(&args(line)[1]), xs, xe));
        }
    }
    p
}

fn fixture(name: &str) -> Program {
    let path = root().join("tests/fixtures/scm-cx100").join(name);
    parse(&String::from_utf8_lossy(&std::fs::read(path).unwrap()))
}

fn ours(b: &Built, name: &str) -> Program {
    let f = b.out.files.iter().find(|f| f.name == name).unwrap();
    parse(std::str::from_utf8(&f.content).unwrap())
}

fn xs(p: &Program, plane: &str) -> BTreeSet<String> {
    p.drills[plane].iter().map(|d| format!("{}", d.0)).collect()
}

/// Kolíky do hrán: rovnaké x, výška = T/2, hĺbka 26, Ø8 na Left aj Right.
fn assert_same_edge_dowels(our: &Program, cv: &Program) {
    for plane in ["Left", "Right"] {
        assert_eq!(xs(our, plane), xs(cv, plane), "rovina {plane}");
        for (a, b) in our.drills[plane].iter().zip(&cv.drills[plane]) {
            assert_eq!(
                (a.1, a.2, a.3),
                (b.1, b.2, b.3),
                "rovina {plane}: y/hĺbka/Ø"
            );
        }
    }
}

#[test]
fn wall_bottom_matches_cv_966f0040() {
    let b = build();
    let our = ours(&b, "HO1-BOTTOM.xcs");
    let cv = fixture("966f0040.xcs");
    assert_same_edge_dowels(&our, &cv);
    // drážka zadnej steny: jeden prechod, y = 311.5, hĺbka 9, −60 … L+60
    assert_eq!(our.slots.len(), 1);
    assert_eq!(cv.slots.len(), 1);
    let (o, c) = (our.slots[0], cv.slots[0]);
    assert_eq!((o.0, o.1, o.2), (c.0, c.1, c.2));
    assert_eq!(o.3, our.length + 60.0);
    assert_eq!(c.3, cv.length + 60.0);
}

/// Spodná s naloženým chrbtom (dno 564 × 506 ako S_KUCH_VARNA_D v CV, program 966b0006):
/// kolíky do hrán 32/96, 221/285, 410/474 (findings 11 §1.3; program nie je vo fixtures).
#[test]
fn base_bottom_with_applied_back_matches_cv_966b0006() {
    let b = build();
    let our = ours(&b, "SP1-BOTTOM.xcs");
    assert_eq!(our.length, 564.0);
    let expected: BTreeSet<String> = ["32", "96", "221", "285", "410", "474"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    for plane in ["Left", "Right"] {
        assert_eq!(xs(&our, plane), expected, "rovina {plane}");
    }
}

/// Drezová skrinka bez chrbta: dno 564 × 510 cez celú hĺbku ako program CV 966f0020.
#[test]
fn sink_bottom_without_back_matches_cv_966f0020() {
    let dir = root().join("examples/dielna");
    let mut loaded = cc_config::load(&dir.join("project.toml")).unwrap();
    let machine = cc_config::load_machine(&dir.join("machine.toml")).unwrap();
    let sp1 = loaded
        .project
        .cabinets
        .iter_mut()
        .find(|c| c.id == "SP1")
        .unwrap();
    sp1.back = false;
    sp1.adjustable_shelves = 0;
    let model = cc_generate::build(&loaded.project, &loaded.standard, &loaded.materials).unwrap();
    let applied = cc_joints::apply(model, &loaded.standard).unwrap();
    let out = cc_out_xcs::render(&applied.model, machine.xcs.as_ref().unwrap());
    let f = out
        .files
        .iter()
        .find(|f| f.name == "SP1-BOTTOM.xcs")
        .unwrap();
    let our = parse(std::str::from_utf8(&f.content).unwrap());
    assert_same_edge_dowels(&our, &fixture("966f0020.xcs"));
}

#[test]
fn stretcher_matches_cv_966f0004() {
    let b = build();
    assert_same_edge_dowels(&ours(&b, "SP1-STR-F.xcs"), &fixture("966f0004.xcs"));
}

#[test]
fn validation_is_clean_and_all_templates_verified() {
    let b = build();
    assert!(b.warnings.is_empty(), "{:?}", b.warnings);
    let report = cc_validate::check(&b.model, Some(&b.machine));
    assert!(report.issues.is_empty(), "{:#?}", report.issues);
    assert!(b.out.unverified.is_empty(), "{:?}", b.out.unverified);
}

#[test]
fn shelf_holes_in_groups_of_three() {
    let b = build();
    let side = ours(&b, "HO1-SIDE-L.xcs");
    let holes: Vec<_> = side.drills["Top"].iter().filter(|d| d.3 == 5.0).collect();
    // 2 police × 3 diery × 2 rady, Ø5 hĺbka 10
    assert_eq!(holes.len(), 12);
    assert!(holes.iter().all(|d| d.2 == 10.0));
    let mut heights: Vec<f64> = holes.iter().map(|d| d.0).collect();
    heights.sort_by(f64::total_cmp);
    heights.dedup();
    assert_eq!(heights.len(), 6);
    // dve skupiny po 3 s roztečou 32
    assert_eq!(heights[1] - heights[0], 32.0);
    assert_eq!(heights[2] - heights[1], 32.0);
    assert_eq!(heights[4] - heights[3], 32.0);
    assert_eq!(heights[5] - heights[4], 32.0);
}

/// Spodná ako S_KUCH_D v CV (findings 09 §5.3, 10 §2.3, 11 §1.7): polica na 372 (rovnaké
/// otvory), podperky Sekura 7 – diery 334/366/398, rady 53 od čela a 50 od chrbta (456).
#[test]
fn base_shelf_holes_match_cv() {
    let b = build();
    let side = ours(&b, "SP1-SIDE-R.xcs");
    let holes: Vec<_> = side.drills["Top"].iter().filter(|d| d.3 == 5.0).collect();
    // pravý bok: x ide od spodku skrinky
    let heights: BTreeSet<String> = holes.iter().map(|d| format!("{}", d.0)).collect();
    let rows: BTreeSet<String> = holes.iter().map(|d| format!("{}", d.1)).collect();
    let set = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<BTreeSet<_>>();
    assert_eq!(heights, set(&["334", "366", "398"]));
    assert_eq!(rows, set(&["53", "456"]));
}

#[test]
fn dielna_matches_golden_files() {
    let b = build();
    let dir = root().join("tests/golden/dielna/xcs");
    let mut expected = BTreeSet::new();
    for entry in std::fs::read_dir(&dir).unwrap() {
        expected.insert(entry.unwrap().file_name().to_string_lossy().into_owned());
    }
    let (bat_name, bat) = b.out.run_bat.as_ref().unwrap();
    let mut actual: BTreeSet<String> = b.out.files.iter().map(|f| f.name.clone()).collect();
    actual.insert(bat_name.clone());
    assert_eq!(actual, expected);
    for f in &b.out.files {
        assert!(
            f.content == std::fs::read(dir.join(&f.name)).unwrap(),
            "{} sa líši od golden súboru",
            f.name
        );
    }
    assert!(*bat == std::fs::read(dir.join(bat_name)).unwrap());
}

/// Vrch hornej skrinky 19 mm (MDF) s drážkou zadnej steny ako program CV 966b0039.
#[test]
fn wall_top_is_19_like_cv_966b0039() {
    let b = build();
    let our = ours(&b, "HO1-TOP.xcs");
    let cv = fixture("966b0039.xcs");
    let top = b.model.parts().find(|p| p.id == "HO1-TOP").unwrap();
    assert_eq!(top.thickness, 19.0);
    assert_eq!(top.material, "MDF_19_W1000_biela");
    assert_eq!(our.slots.len(), 1);
    assert_eq!(cv.slots.len(), 1);
    // drážka: rovnaká os a hĺbka
    assert_eq!(
        (our.slots[0].0, our.slots[0].1),
        (cv.slots[0].0, cv.slots[0].1)
    );
}

/// Horná 576,5 × 682 × 330 s 2 policami ako skrinka 8 v REF (findings 09 §5.3): police 221 a 442,
/// diery 183/215/247 a 404/436/468.
#[test]
fn wall_shelves_and_holes_match_cv_cabinet_8() {
    let dir = root().join("examples/dielna");
    let mut loaded = cc_config::load(&dir.join("project.toml")).unwrap();
    let ho1 = loaded
        .project
        .cabinets
        .iter_mut()
        .find(|c| c.id == "HO1")
        .unwrap();
    ho1.w = 576.5;
    ho1.h = 682.0;
    let model = cc_generate::build(&loaded.project, &loaded.standard, &loaded.materials).unwrap();
    let shelves: BTreeSet<String> = model
        .parts()
        .filter(|p| p.role == cc_model::Role::AdjustableShelf && p.cabinet == "HO1")
        .map(|p| {
            let z = p.transform.point_to_cabinet(glam::DVec3::ZERO).z.min(
                p.transform
                    .point_to_cabinet(glam::DVec3::new(0.0, 0.0, p.thickness))
                    .z,
            );
            format!("{z}")
        })
        .collect();
    assert_eq!(
        shelves,
        ["221", "442"].iter().map(|s| s.to_string()).collect()
    );
    let applied = cc_joints::apply(model, &loaded.standard).unwrap();
    let side = applied
        .model
        .parts()
        .find(|p| p.id == "HO1-SIDE-R")
        .unwrap();
    let heights: BTreeSet<String> = side
        .operations
        .iter()
        .filter_map(|op| match *op {
            cc_model::Operation::Hole {
                x, diameter, depth, ..
            } if diameter == 5.0 && depth == 10.0 => Some(format!("{x}")),
            _ => None,
        })
        .collect();
    let want: BTreeSet<String> = ["183", "215", "247", "404", "436", "468"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    assert_eq!(heights, want);
}
