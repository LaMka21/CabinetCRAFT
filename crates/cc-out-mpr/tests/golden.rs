//! Golden testy WoodWOP `.mpr` pre vzorový projekt (špecifikácia MVP, kap. 7.3, 10).

use std::collections::BTreeSet;
use std::path::PathBuf;

use cc_model::{
    CabinetModel, Edges, Face, Grain, Model, MprProfile, Operation, Part, Role, Transform, P2,
};
use glam::DVec3;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn example() -> (Model, MprProfile) {
    let loaded = cc_config::load(&root().join("examples/s1/project.toml")).unwrap();
    let machine =
        cc_config::load_machine(&root().join("examples/machines/woodwop-vzor.toml")).unwrap();
    let model = cc_generate::build(&loaded.project, &loaded.standard, &loaded.materials).unwrap();
    let model = cc_joints::apply(model, &loaded.standard).unwrap().model;
    (model, machine.mpr.expect("vzorový profil má sekciu [mpr]"))
}

#[test]
fn example_matches_golden_files() {
    let (model, profile) = example();
    let out = cc_out_mpr::render(&model, &profile);

    let dir = root().join("tests/golden/s1/mpr");
    let mut expected: BTreeSet<String> = BTreeSet::new();
    for entry in std::fs::read_dir(&dir).unwrap() {
        expected.insert(entry.unwrap().file_name().to_string_lossy().into_owned());
    }
    let actual: BTreeSet<String> = out.files.iter().map(|f| f.name.clone()).collect();
    assert_eq!(actual, expected, "zoznam súborov .mpr");

    for f in &out.files {
        let golden = std::fs::read(dir.join(&f.name)).unwrap();
        assert!(
            f.content == golden,
            "{} sa líši od golden súboru (bajtové porovnanie, CRLF)",
            f.name
        );
        assert!(f.content.is_ascii(), "{} nie je ASCII", f.name);
    }
}

#[test]
fn example_counts_and_unverified() {
    let (model, profile) = example();
    let out = cc_out_mpr::render(&model, &profile);
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
    let side = text("S1-SIDE-L.mpr");
    assert_eq!(side.matches("<102 \\BohrVert\\").count(), 45);
    assert_eq!(side.matches("<109 \\Nuten\\").count(), 1);
    let bottom = text("S1-BOTTOM.mpr");
    assert_eq!(bottom.matches("<103 \\BohrHoriz\\").count(), 10);
    assert_eq!(bottom.matches("BM=\"XP\"").count(), 5);
    assert_eq!(bottom.matches("BM=\"XM\"").count(), 5);
    assert!(bottom.ends_with("!\r\n"));
    assert_eq!(
        out.unverified_macros.iter().copied().collect::<Vec<_>>(),
        vec!["BohrHoriz", "Nuten"]
    );
    let programs = out.program_names();
    assert_eq!(programs.len(), 5);
    assert_eq!(programs["S1-SIDE-L"], "S1-SIDE-L.mpr");
}

fn test_part(operations: Vec<Operation>) -> Part {
    Part {
        id: "T1".into(),
        cabinet: "S".into(),
        role: Role::Bottom,
        material: "M".into(),
        length: 500.0,
        width: 400.0,
        thickness: 18.0,
        grain: Grain::None,
        edges: Edges::default(),
        transform: Transform::from_axes(DVec3::X, DVec3::Y, DVec3::ZERO),
        operations,
    }
}

fn single(part: Part) -> Model {
    Model {
        project: "Kuchyňa".into(),
        cabinets: vec![CabinetModel {
            id: "S".into(),
            cabinet_type: cc_model::CabinetType::Base,
            w: 600.0,
            h: 720.0,
            d: 560.0,
            parts: vec![part],
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
        depth: 12.0,
        through: false,
        dowel: false,
    }
}

#[test]
fn back_face_goes_to_mirrored_file() {
    let (_, profile) = example();
    let part = test_part(vec![
        hole(Face::F, 100.0, 50.0, 18.0),
        hole(Face::B, 100.0, 60.0, 0.0),
    ]);
    let out = cc_out_mpr::render(&single(part), &profile);
    let names: Vec<_> = out.files.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["T1_F.mpr", "T1_B.mpr"]);
    let b = String::from_utf8(out.files[1].content.clone()).unwrap();
    // B: x' = L − x = 400
    assert!(b.contains("XA=\"400\"\r\nYA=\"60\""), "{b}");
    assert_eq!(out.program_names()["T1"], "T1_F.mpr,T1_B.mpr");
    // diakritika v komentári → ASCII
    assert!(b.contains("CabinetCRAFT: Kuchyna"), "{b}");
}

#[test]
fn through_groove_extension() {
    let (_, mut profile) = example();
    profile.nuten.through_extension = 35.0;
    let groove = Operation::Groove {
        face: Face::F,
        start: P2 { x: 0.0, y: 100.0 },
        end: P2 { x: 500.0, y: 100.0 },
        width: 8.5,
        depth: 8.0,
    };
    let out = cc_out_mpr::render(&single(test_part(vec![groove])), &profile);
    let text = String::from_utf8(out.files[0].content.clone()).unwrap();
    assert!(text.contains("XA=\"-35\""), "{text}");
    assert!(text.contains("XE=\"535\""), "{text}");
}

#[test]
fn line_ending_lf() {
    let (model, mut profile) = example();
    profile.line_ending = cc_model::LineEnding::Lf;
    let out = cc_out_mpr::render(&model, &profile);
    assert!(out.files.iter().all(|f| !f.content.contains(&b'\r')));
}

#[test]
fn template_with_unknown_variable_is_rejected() {
    let (_, mut profile) = example();
    profile.bohr_vert.fields.push(["XX".into(), "{zz}".into()]);
    let problems = profile.template_problems();
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(
        matches!(&problems[0], cc_model::TemplateProblem::UnknownVariable { var, .. } if var == "zz"),
        "{problems:?}"
    );
}
