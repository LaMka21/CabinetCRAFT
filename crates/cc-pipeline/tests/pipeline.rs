//! Celý beh nad `examples/dielna`: rovnaké programy ako golden súbory.

use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn dielna_build_and_write() {
    let project = root().join("examples/dielna/project.toml");
    let loaded = cc_config::load(&project).unwrap();
    let machine_path = cc_pipeline::default_machine(&project).unwrap();
    let machine = cc_config::load_machine(&machine_path).unwrap();
    let built = cc_pipeline::build(
        &loaded.project,
        &loaded.standard,
        &loaded.materials,
        Some(&machine),
    )
    .unwrap();
    assert!(!built.has_errors());
    assert_eq!(built.issues().count(), 0);

    let out = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("pipeline-dielna");
    let written = cc_pipeline::write_outputs_with(
        &built.model,
        Some(&machine),
        &out,
        Default::default(),
        cc_pipeline::FontChoice::Base14,
    )
    .unwrap();
    assert_eq!(written.xcs, 12);
    assert_eq!(written.mpr, 0);
    assert!(written.notices.is_empty(), "{:?}", written.notices);
    for name in ["SP1-BOTTOM.xcs", "HO1-SIDE-L.xcs", "RUN.bat"] {
        let ours = std::fs::read(out.join("xcs").join(name)).unwrap();
        let golden = std::fs::read(root().join("tests/golden/dielna/xcs").join(name)).unwrap();
        assert!(ours == golden, "{name}");
    }
    let csv = std::fs::read_to_string(out.join("cutlist.csv")).unwrap();
    assert!(csv.contains("SP1-BOTTOM.xcs"));

    let programs = cc_pipeline::program_names(&built.model, Some(&machine));
    assert_eq!(programs["SP1-BOTTOM"], "SP1-BOTTOM.xcs");

    // štítky: jeden na diel, bajtovo zhodné s golden súbormi
    // (pregenerovanie: CABINETCRAFT_BLESS=1 cargo test -p cc-pipeline --test pipeline)
    assert_eq!(written.labels, 15, "14 dielov + sokel spodnej skrinky");
    let golden_dir = root().join("tests/golden/dielna/labels");
    for name in ["stitky.pdf", "stitky-A4.pdf"] {
        let ours = std::fs::read(out.join("labels").join(name)).unwrap();
        if std::env::var_os("CABINETCRAFT_BLESS").is_some() {
            std::fs::create_dir_all(&golden_dir).unwrap();
            std::fs::write(golden_dir.join(name), &ours).unwrap();
        }
        let golden = std::fs::read(golden_dir.join(name)).unwrap();
        assert!(ours == golden, "{name} sa líši od golden súboru");
    }
}

/// Režim nesting: tabule v `nesting/`, v `xcs/` iba zvyšná práca pre CX100.
#[test]
fn dielna_nesting_mode_writes_sheets_and_cx100_subset() {
    let project = root().join("examples/dielna/project.toml");
    let loaded = cc_config::load(&project).unwrap();
    let machine =
        cc_config::load_machine(&root().join("examples/dielna/machine-nesting.toml")).unwrap();
    let built = cc_pipeline::build(
        &loaded.project,
        &loaded.standard,
        &loaded.materials,
        Some(&machine),
    )
    .unwrap();
    assert!(!built.has_errors());

    let out = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("pipeline-dielna-nesting");
    let _ = std::fs::remove_dir_all(&out);
    let written = cc_pipeline::write_outputs_with(
        &built.model,
        Some(&machine),
        &out,
        Default::default(),
        cc_pipeline::FontChoice::Base14,
    )
    .unwrap();
    // DTDL 18 + MDF 19 (vrch hornej skrinky)
    assert_eq!(written.sheets, 2);
    // 9 dielov s obrábaním; boky spodnej (naložený chrbát, iba diery do plochy) robí celé nesting
    assert_eq!(written.xcs, 7);
    assert_eq!(written.nesting.len(), 2);
    assert!(written
        .notices
        .iter()
        .any(|n| n.contains("nesting.CreateRoughFinish")));
    let golden = root().join("tests/golden/dielna-nesting/project");
    for name in [
        "nesting/0101N.xcs",
        "nesting/CncRun.txt",
        "xcs/HO1-SIDE-L.xcs",
        "xcs/RUN.bat",
    ] {
        let ours = std::fs::read(out.join(name)).unwrap();
        assert!(ours == std::fs::read(golden.join(name)).unwrap(), "{name}");
    }
    // bok hornej skrinky: tabuľa (diery) + CX100 (drážka); bok spodnej iba tabuľa
    let csv = std::fs::read_to_string(out.join("cutlist.csv")).unwrap();
    assert!(csv.contains("0101N.xcs,HO1-SIDE-L.xcs"));
    let programs = cc_pipeline::program_names(&built.model, Some(&machine));
    assert_eq!(programs["HO1-SIDE-L"], "0101N.xcs,HO1-SIDE-L.xcs");
    assert_eq!(programs["SP1-SIDE-L"], "0101N.xcs");
}

/// Spojovacie kovanie (findings 12 §8): SP2 „spojiť s ľavou susedou“ → 4 × Ø5 × 16 v ploche F
/// ľavého boku SP2 (100 / D − 100 od čela × 80 / H − 80 od spodku) a na tom istom mieste
/// v pravom boku SP1. Skrinka bez atribútu diery nedostane.
#[test]
fn cabinet_link_drills_both_sides() {
    let project = root().join("examples/dielna/kuchyna.toml");
    let mut loaded = cc_config::load(&project).unwrap();
    let sp2 = loaded
        .project
        .cabinets
        .iter_mut()
        .find(|c| c.id == "SP2")
        .unwrap();
    sp2.options.link_left = Some(true);
    let built =
        cc_pipeline::build(&loaded.project, &loaded.standard, &loaded.materials, None).unwrap();
    let holes = |id: &str| {
        let p = built.model.parts().find(|p| p.id == id).unwrap();
        let mut v: Vec<(f64, f64)> = p
            .operations
            .iter()
            .filter_map(|o| match *o {
                cc_model::Operation::Hole {
                    face: cc_model::Face::F,
                    x,
                    y,
                    diameter,
                    depth,
                    ..
                } if diameter == 5.0 && depth == 16.0 => Some((x, y)),
                _ => None,
            })
            .collect();
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        (p.length, p.width, v)
    };
    let (h, d, left) = holes("SP2-SIDE-L");
    assert_eq!(left.len(), 4, "{left:?}");
    // ľavý bok: x ide zhora nadol, y od čela
    let mut from_bottom: Vec<(f64, f64)> = left.iter().map(|(x, y)| (*y, h - x)).collect();
    from_bottom.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(
        from_bottom,
        [
            (100.0, 80.0),
            (100.0, h - 80.0),
            (d - 100.0, 80.0),
            (d - 100.0, h - 80.0)
        ]
    );
    // pravý bok SP1: x ide zdola nahor, rovnaké miesto v priestore
    let (_, _, right) = holes("SP1-SIDE-R");
    let mut r: Vec<(f64, f64)> = right.iter().map(|(x, y)| (*y, *x)).collect();
    r.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(r, from_bottom);
    // bez atribútu nič
    assert!(holes("SP3-SIDE-L").2.is_empty());
}

/// Štítky po tabuliach (findings 14): na každej tabuli čísla 1…n podľa prerezania, poradie tlače
/// podľa `[labels] order`, každý diel má práve jeden štítok.
#[test]
fn label_job_numbers_parts_per_sheet() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/dielna");
    let loaded = cc_config::load(&root.join("kuchyna.toml")).unwrap();
    let mut machine = cc_config::load_machine(&root.join("machine-nesting.toml")).unwrap();
    let built = cc_pipeline::build(
        &loaded.project,
        &loaded.standard,
        &loaded.materials,
        Some(&machine),
    )
    .unwrap();
    let job = cc_pipeline::labels::label_job(&built.model, Some(&machine));
    assert_eq!(job.size, (80.0, 80.0));
    assert!(!job.sheets.is_empty());
    assert_eq!(job.labels.len(), built.model.parts().count());
    for s in &job.sheets {
        let mut numbers: Vec<usize> = s.parts.iter().map(|p| p.number).collect();
        // predvolené poradie tlače = poradie prerezania; zvyšok tabule má číslo 1 (ako CV)
        assert!(numbers.windows(2).all(|w| w[0] < w[1]), "{numbers:?}");
        numbers.sort();
        let first = numbers[0];
        assert!(first == 1 || first == 2, "{numbers:?}");
        assert_eq!(numbers, (first..first + s.parts.len()).collect::<Vec<_>>());
        for l in job.sheet_labels(&s.name) {
            assert_eq!(l.nesting.as_ref().unwrap().sheet, s.name);
        }
    }
    // riadky zhora, zľava: Y klesá, pri rovnakom Y X rastie
    machine.labels.order = cc_model::LabelOrder::RowsTopLeft;
    machine.labels.width = 100.0;
    let job = cc_pipeline::labels::label_job(&built.model, Some(&machine));
    assert_eq!(job.size, (100.0, 80.0));
    for s in &job.sheets {
        for w in s.parts.windows(2) {
            assert!(w[0].y > w[1].y || (w[0].y == w[1].y && w[0].x < w[1].x));
        }
    }
}
