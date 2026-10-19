//! Test pre každé pravidlo validácie (špecifikácia MVP, kap. 8).

use std::path::PathBuf;

use cc_model::{
    CabinetModel, CabinetType, ConnectionKind, Drills, Edges, Face, Grain, IssueCode, MachineInfo,
    MachineProfile, MinPart, Model, OneOrMany, Operation, Part, Role, Severity, Transform,
    WorkArea, P2,
};
use glam::DVec3;

// ---------------------------------------------------------------- pomocné

fn machine() -> MachineProfile {
    MachineProfile {
        machine: MachineInfo {
            name: "test".into(),
        },
        work_area: WorkArea {
            max_length: 3050.0,
            max_width: 1300.0,
        },
        min_part: MinPart {
            length: 200.0,
            width: 70.0,
        },
        drills: Drills {
            diameters: vec![5.0, 8.0],
        },
        xcs: None,
        mpr: None,
        production: Default::default(),
        nesting: None,
        labels: Default::default(),
    }
}

fn part(l: f64, w: f64, t: f64, operations: Vec<Operation>) -> Part {
    Part {
        id: "P".into(),
        cabinet: "S".into(),
        role: Role::Bottom,
        material: "M".into(),
        length: l,
        width: w,
        thickness: t,
        grain: Grain::None,
        edges: Edges::default(),
        transform: Transform::from_axes(DVec3::X, DVec3::Y, DVec3::ZERO),
        operations,
    }
}

fn model(parts: Vec<Part>) -> Model {
    Model {
        project: "test".into(),
        cabinets: vec![CabinetModel {
            id: "S".into(),
            cabinet_type: CabinetType::Base,
            w: 600.0,
            h: 720.0,
            d: 560.0,
            parts,
            hardware: Vec::new(),
            options: Default::default(),
            openings: Vec::new(),
        }],
    }
}

fn hole(face: Face, x: f64, y: f64, z: f64, d: f64, depth: f64) -> Operation {
    Operation::Hole {
        face,
        x,
        y,
        z,
        diameter: d,
        depth,
        through: false,
        dowel: false,
    }
}

fn groove(y: f64, width: f64, depth: f64) -> Operation {
    Operation::Groove {
        face: Face::F,
        start: P2 { x: 0.0, y },
        end: P2 { x: 500.0, y },
        width,
        depth,
    }
}

fn codes(parts: Vec<Part>) -> Vec<IssueCode> {
    cc_validate::check(&model(parts), Some(&machine())).codes()
}

/// Dobrá diera na ploche a dobrá diera v hrane.
fn ok_holes() -> Vec<Operation> {
    vec![
        hole(Face::F, 100.0, 100.0, 18.0, 8.0, 12.0),
        hole(Face::X0, 0.0, 100.0, 9.0, 8.0, 25.0),
    ]
}

fn load_example() -> cc_config::LoadedProject {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/s1/project.toml");
    cc_config::load(&path).unwrap()
}

fn pipeline(
    loaded: &cc_config::LoadedProject,
) -> Result<cc_joints::Applied, cc_joints::JointError> {
    let model = cc_generate::build(&loaded.project, &loaded.standard, &loaded.materials).unwrap();
    cc_joints::apply(model, &loaded.standard)
}

// ---------------------------------------------------------------- kontrola

#[test]
fn valid_part_has_no_issues() {
    assert_eq!(codes(vec![part(500.0, 400.0, 18.0, ok_holes())]), vec![]);
}

#[test]
fn example_s1_has_no_issues() {
    let loaded = load_example();
    let applied = pipeline(&loaded).unwrap();
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/s1/machine.toml");
    let machine = cc_config::load_machine(&path).unwrap();
    let report = cc_validate::check(&applied.model, Some(&machine));
    assert!(applied.warnings.is_empty(), "{:?}", applied.warnings);
    assert!(report.issues.is_empty(), "{:#?}", report.issues);
}

// ---------------------------------------------------------------- pravidlá

#[test]
fn part_too_small_invalid_dimension() {
    assert_eq!(
        codes(vec![part(0.0, 400.0, 18.0, vec![])]),
        vec![IssueCode::PartTooSmall]
    );
}

#[test]
fn part_too_small_for_machine() {
    let p = part(
        150.0,
        400.0,
        18.0,
        vec![hole(Face::F, 75.0, 100.0, 18.0, 8.0, 12.0)],
    );
    assert_eq!(codes(vec![p]), vec![IssueCode::PartTooSmall]);
    // neobrábaný diel sa voči stroju nekontroluje
    assert_eq!(codes(vec![part(150.0, 400.0, 18.0, vec![])]), vec![]);
}

/// Nesting: malé čelo iba s dierami do jednej plochy sa celé obrobí na tabuli,
/// limity CX100 preň neplatia; s dierou v hrane ide na CX100 a kontroluje sa.
#[test]
fn nesting_only_part_skips_cx100_limits() {
    let mut m = machine();
    m.production.mode = cc_model::ProductionMode::Nesting;
    let check = |ops: Vec<Operation>| {
        cc_validate::check(&model(vec![part(130.0, 598.0, 19.0, ops)]), Some(&m)).codes()
    };
    let face = vec![
        hole(Face::F, 35.5, 37.5, 19.0, 8.0, 12.0),
        hole(Face::F, 67.5, 37.5, 19.0, 8.0, 12.0),
    ];
    assert_eq!(check(face.clone()), vec![]);
    let mut edge = face;
    edge.push(hole(Face::X0, 0.0, 100.0, 9.5, 8.0, 25.0));
    assert_eq!(check(edge), vec![IssueCode::PartTooSmall]);
}

#[test]
fn part_too_large() {
    let p = part(3100.0, 400.0, 18.0, ok_holes());
    assert_eq!(codes(vec![p]), vec![IssueCode::PartTooLarge]);
}

#[test]
fn hole_outside_face() {
    // Ø8: od hrany musí byť ≥ 4 + 2 = 6 mm
    let p = part(
        500.0,
        400.0,
        18.0,
        vec![hole(Face::F, 5.0, 100.0, 18.0, 8.0, 12.0)],
    );
    assert_eq!(codes(vec![p]), vec![IssueCode::HoleOutside]);
    let p = part(
        500.0,
        400.0,
        18.0,
        vec![hole(Face::F, 6.0, 100.0, 18.0, 8.0, 12.0)],
    );
    assert_eq!(codes(vec![p]), vec![]);
}

#[test]
fn hole_outside_edge_thickness() {
    // diera v hrane mimo stredu hrúbky: z + d/2 > T
    let p = part(
        500.0,
        400.0,
        18.0,
        vec![hole(Face::X0, 0.0, 100.0, 15.0, 8.0, 25.0)],
    );
    assert_eq!(codes(vec![p]), vec![IssueCode::HoleOutside]);
}

#[test]
fn face_hole_too_deep() {
    // T − 3 = 15
    let p = part(
        500.0,
        400.0,
        18.0,
        vec![hole(Face::F, 100.0, 100.0, 18.0, 8.0, 16.0)],
    );
    assert_eq!(codes(vec![p]), vec![IssueCode::FaceHoleTooDeep]);
    let through = Operation::Hole {
        face: Face::F,
        x: 100.0,
        y: 100.0,
        z: 18.0,
        diameter: 8.0,
        depth: 18.0,
        through: true,
        dowel: false,
    };
    assert_eq!(codes(vec![part(500.0, 400.0, 18.0, vec![through])]), vec![]);
}

#[test]
fn edge_hole_too_deep() {
    // hrana Y0: rozmer v smere vŕtania je šírka 80 → max 75
    let p = part(
        500.0,
        80.0,
        18.0,
        vec![hole(Face::Y0, 100.0, 0.0, 9.0, 8.0, 76.0)],
    );
    assert_eq!(codes(vec![p]), vec![IssueCode::EdgeHoleTooDeep]);
}

#[test]
fn edge_hole_too_wide() {
    // T − 6 = 12 → Ø14 je príliš veľa (z = 9 ± 7 je ešte v hrúbke 18)
    let p = part(
        500.0,
        400.0,
        18.0,
        vec![hole(Face::X1, 500.0, 100.0, 9.0, 14.0, 20.0)],
    );
    let c = codes(vec![p]);
    assert!(c.contains(&IssueCode::EdgeHoleTooWide), "{c:?}");
}

#[test]
fn hole_collision() {
    // Ø8 + Ø8: min. vzdialenosť osí 8 + 2 = 10
    let ops = vec![
        hole(Face::F, 100.0, 100.0, 18.0, 8.0, 12.0),
        hole(Face::F, 109.0, 100.0, 18.0, 8.0, 12.0),
    ];
    assert_eq!(
        codes(vec![part(500.0, 400.0, 18.0, ops)]),
        vec![IssueCode::HoleCollision]
    );
    // rovnaké miesto, ale iná strana → nekolíduje
    let ops = vec![
        hole(Face::F, 100.0, 100.0, 18.0, 8.0, 12.0),
        hole(Face::B, 100.0, 100.0, 0.0, 8.0, 12.0),
    ];
    assert_eq!(codes(vec![part(500.0, 400.0, 18.0, ops)]), vec![]);
}

#[test]
fn hole_in_groove() {
    // drážka y = 300 ± 4.25; diera Ø8 v y = 305 → vzdialenosť 5 < 4.25 + 4
    let ops = vec![
        groove(300.0, 8.5, 8.0),
        hole(Face::F, 100.0, 305.0, 18.0, 8.0, 12.0),
    ];
    assert_eq!(
        codes(vec![part(500.0, 400.0, 18.0, ops)]),
        vec![IssueCode::HoleInGroove]
    );
    let ops = vec![
        groove(300.0, 8.5, 8.0),
        hole(Face::F, 100.0, 309.0, 18.0, 8.0, 12.0),
    ];
    assert_eq!(codes(vec![part(500.0, 400.0, 18.0, ops)]), vec![]);
}

#[test]
fn groove_too_deep() {
    // T − 6 = 12
    let p = part(500.0, 400.0, 18.0, vec![groove(300.0, 8.5, 13.0)]);
    assert_eq!(codes(vec![p]), vec![IssueCode::GrooveTooDeep]);
}

#[test]
fn tool_missing() {
    let p = part(
        500.0,
        400.0,
        18.0,
        vec![hole(Face::F, 100.0, 100.0, 18.0, 6.0, 12.0)],
    );
    assert_eq!(codes(vec![p]), vec![IssueCode::ToolMissing]);
    // bez profilu stroja sa nekontroluje
    let p = part(
        500.0,
        400.0,
        18.0,
        vec![hole(Face::F, 100.0, 100.0, 18.0, 6.0, 12.0)],
    );
    assert!(cc_validate::check(&model(vec![p]), None).issues.is_empty());
}

#[test]
fn joint_too_short() {
    // spojka šírky 60 < 2 · 32 + 8 = 72
    let mut loaded = load_example();
    loaded.standard.carcass.stretcher_width = 60.0;
    let err = pipeline(&loaded).unwrap_err();
    assert_eq!(err.code(), Some(IssueCode::JointTooShort));
    assert!(err.to_string().starts_with("[JointTooShort]"), "{err}");
}

#[test]
fn shelf_no_support_is_warning() {
    // bez pravidla side × adjustable_shelf nevznikne rad dier
    let mut loaded = load_example();
    loaded
        .standard
        .joint_rules
        .retain(|r| r.connection != ConnectionKind::LineBore);
    let applied = pipeline(&loaded).unwrap();
    let report = cc_validate::check(&applied.model, Some(&machine()));
    assert_eq!(
        report.codes(),
        vec![IssueCode::ShelfNoSupport, IssueCode::ShelfNoSupport]
    );
    assert!(report
        .issues
        .iter()
        .all(|i| i.severity == Severity::Warning));
    assert!(!report.has_errors());
}

#[test]
fn joint_rule_missing_is_warning() {
    // predná spojka sa dotýka bokov, ale pravidlo pre ňu chýba
    let mut loaded = load_example();
    loaded.standard.joint_rules[0].b = OneOrMany::Many(vec![
        "bottom".into(),
        "top".into(),
        "stretcher_rear".into(),
        "fixed_shelf".into(),
    ]);
    let applied = pipeline(&loaded).unwrap();
    let codes: Vec<_> = applied.warnings.iter().map(|w| w.code).collect();
    assert_eq!(codes, vec![IssueCode::JointRuleMissing; 2]);
    assert!(applied
        .warnings
        .iter()
        .all(|w| w.severity == Severity::Warning));
}

// UnsupportedToolAxis: generátor MVP tvorí iba osi rovnobežné so skrinkou, preto sa
// pravidlo testuje priamo v cc-joints (`tests::diagonal_tool_axis_is_unsupported`).
