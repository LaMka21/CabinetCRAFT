//! Golden testy SCM Maestro `.xcs` a správanie zapisovača (kap. 7.4).

use std::collections::BTreeSet;
use std::path::PathBuf;

use cc_model::{
    CabinetModel, CabinetType, Edges, Face, Grain, Model, Operation, Part, Role, Transform,
    XcsProfile, P2,
};
use glam::DVec3;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn profile() -> XcsProfile {
    cc_config::load_machine(&root().join("examples/s1/machine.toml"))
        .unwrap()
        .xcs
        .expect("vzorový profil má sekciu [xcs]")
}

fn example_model() -> Model {
    let loaded = cc_config::load(&root().join("examples/s1/project.toml")).unwrap();
    let model = cc_generate::build(&loaded.project, &loaded.standard, &loaded.materials).unwrap();
    cc_joints::apply(model, &loaded.standard).unwrap().model
}

#[test]
fn example_matches_golden_files() {
    let out = cc_out_xcs::render(&example_model(), &profile());
    let dir = root().join("tests/golden/s1/xcs");
    let mut expected = BTreeSet::new();
    for entry in std::fs::read_dir(&dir).unwrap() {
        expected.insert(entry.unwrap().file_name().to_string_lossy().into_owned());
    }
    let mut actual: BTreeSet<String> = out.files.iter().map(|f| f.name.clone()).collect();
    let (bat_name, bat) = out.run_bat.as_ref().unwrap();
    actual.insert(bat_name.clone());
    assert_eq!(actual, expected, "zoznam súborov");
    for f in &out.files {
        let golden = std::fs::read(dir.join(&f.name)).unwrap();
        assert!(f.content == golden, "{} sa líši od golden súboru", f.name);
    }
    assert!(
        *bat == std::fs::read(dir.join(bat_name)).unwrap(),
        "RUN.bat sa líši"
    );
}

#[test]
fn example_counts() {
    let out = cc_out_xcs::render(&example_model(), &profile());
    let text = |name: &str| {
        String::from_utf8(
            out.files
                .iter()
                .find(|f| f.name == name)
                .unwrap()
                .content
                .clone(),
        )
        .unwrap()
    };
    let side = text("S1-SIDE-L.xcs");
    assert_eq!(side.matches("CreateDrill(").count(), 45);
    assert_eq!(side.matches("SelectWorkplane(").count(), 1, "iba Top");
    // drážka 8.5 mm pílkou 4 mm: 3 prechody
    assert_eq!(side.matches("CreateSlot(").count(), 3);
    let bottom = text("S1-BOTTOM.xcs");
    assert_eq!(bottom.matches("SelectWorkplane(\"Left\")").count(), 1);
    assert_eq!(bottom.matches("SelectWorkplane(\"Right\")").count(), 1);
    assert_eq!(bottom.matches("Vrtani Leva ").count(), 5);
    assert!(out.unverified.is_empty(), "{:?}", out.unverified);
    assert_eq!(out.program_names()["S1-SIDE-L"], "S1-SIDE-L.xcs");
}

fn part(l: f64, w: f64, operations: Vec<Operation>) -> Part {
    Part {
        id: "T1".into(),
        cabinet: "S".into(),
        role: Role::Bottom,
        material: "M".into(),
        length: l,
        width: w,
        thickness: 18.0,
        grain: Grain::None,
        edges: Edges::default(),
        transform: Transform::from_axes(DVec3::X, DVec3::Y, DVec3::ZERO),
        operations,
    }
}

fn model(p: Part) -> Model {
    Model {
        project: "Kuchyňa".into(),
        cabinets: vec![CabinetModel {
            id: "S".into(),
            cabinet_type: CabinetType::Base,
            w: 600.0,
            h: 720.0,
            d: 560.0,
            parts: vec![p],
            hardware: Vec::new(),
            options: Default::default(),
            openings: Vec::new(),
        }],
    }
}

fn hole(face: Face, x: f64, y: f64, z: f64) -> Operation {
    Operation::Hole {
        face,
        x,
        y,
        z,
        diameter: 8.0,
        depth: 26.0,
        through: false,
        dowel: true,
    }
}

fn lines_of(out: &cc_out_xcs::XcsOutput, i: usize) -> Vec<String> {
    String::from_utf8(out.files[i].content.clone())
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

#[test]
fn side_planes_measured_from_outside() {
    // diel 663 × 447 ako vzor 966f0049: diery na prednej a zadnej hrane na rovnakom x
    let p = part(
        663.0,
        447.0,
        vec![
            hole(Face::Y0, 64.0, 0.0, 9.0),
            hole(Face::Y1, 64.0, 447.0, 9.0),
            hole(Face::X0, 0.0, 32.0, 9.0),
            hole(Face::X1, 663.0, 32.0, 9.0),
        ],
    );
    let out = cc_out_xcs::render(&model(p), &profile());
    let l = lines_of(&out, 0);
    let after = |plane: &str| {
        let i = l
            .iter()
            .position(|s| s == &format!("SelectWorkplane(\"{plane}\");"))
            .unwrap();
        l[i + 1].clone()
    };
    assert!(after("Front").contains("\",64,9,26,8,"), "{l:?}");
    // Back = L − x = 599 (ako vo vzore 966f0049)
    assert!(after("Back").contains("\",599,9,26,8,"), "{l:?}");
    // Left = W − y = 415, Right = y = 32
    assert!(after("Left").contains("\",415,9,26,8,"), "{l:?}");
    assert!(after("Right").contains("\",32,9,26,8,"), "{l:?}");
}

#[test]
fn slot_passes_reproduce_cv_pattern() {
    // vzor 966b0051: prechody 83, 79.6, 76.2, 73 → šírka 10 + rez 4 = 14, stred 78
    let prof = profile();
    let p = part(485.0, 351.0, vec![]);
    let passes = cc_out_xcs::slot_passes(
        P2 { x: 0.0, y: 78.0 },
        P2 { x: 485.0, y: 78.0 },
        14.0,
        &p,
        &prof,
    );
    let ys: Vec<String> = passes
        .iter()
        .map(|(a, _)| cc_out_util::fmt_num(a.y, 4))
        .collect();
    assert_eq!(ys, ["83", "79.6", "76.2", "73"]);
    // drážka cez celý diel: −60 … L + 60 (ako vo vzore)
    assert!(passes.iter().all(|(a, b)| a.x == -60.0 && b.x == 545.0));
    // úzka drážka (≤ rez): jeden prechod v strede
    let one = cc_out_xcs::slot_passes(
        P2 { x: 100.0, y: 311.5 },
        P2 { x: 300.0, y: 311.5 },
        3.5,
        &p,
        &prof,
    );
    assert_eq!(one.len(), 1);
    assert_eq!((one[0].0.x, one[0].0.y, one[0].1.x), (100.0, 311.5, 300.0));
}

#[test]
fn back_face_goes_to_flipped_program() {
    let p = part(
        500.0,
        400.0,
        vec![
            hole(Face::F, 100.0, 50.0, 18.0),
            hole(Face::B, 100.0, 60.0, 0.0),
        ],
    );
    let out = cc_out_xcs::render(&model(p), &profile());
    let names: Vec<_> = out.files.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["T1_F.xcs", "T1_B.xcs"]);
    let b = lines_of(&out, 1).join("\n");
    assert!(b.contains("\",400,60,26,8,"), "x' = L − x: {b}");
    assert!(b.contains("//--PROJEKT: Kuchyna"), "ASCII: {b}");
    let (_, bat) = out.run_bat.as_ref().unwrap();
    let bat = String::from_utf8(bat.clone()).unwrap();
    assert!(bat.contains(r#"-i "C:\Vyroba\CX100\T1_B.xcs""#), "{bat}");
    assert!(bat.contains(r#"-o "C:\Vyroba\CX100\T1_B.pgmx""#), "{bat}");
}

#[test]
fn template_with_unknown_variable_is_rejected() {
    let mut prof = profile();
    prof.drill.line.push_str("{zz}");
    prof.slot.max_step = 5.0;
    let problems = prof.template_problems();
    assert_eq!(problems.len(), 2, "{problems:?}");
    assert!(
        matches!(&problems[0], cc_model::TemplateProblem::UnknownVariable { var, .. } if var == "zz"),
        "{problems:?}"
    );
    assert!(
        matches!(problems[1], cc_model::TemplateProblem::StepOverKerf { .. }),
        "{problems:?}"
    );
}
