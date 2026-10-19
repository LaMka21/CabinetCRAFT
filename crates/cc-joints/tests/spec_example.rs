//! Akceptačný test M2: operácie vzorového príkladu zo špecifikácie MVP, kap. 10.

use std::path::PathBuf;

use approx::assert_abs_diff_eq;
use cc_model::{CabinetType, Face, Issue, Model, Operation, Part, Role};

fn load() -> cc_config::LoadedProject {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/s1/project.toml");
    cc_config::load(&path).unwrap()
}

fn applied(loaded: &cc_config::LoadedProject) -> (Model, Vec<Issue>) {
    let model = cc_generate::build(&loaded.project, &loaded.standard, &loaded.materials).unwrap();
    let applied = cc_joints::apply(model, &loaded.standard).unwrap();
    (applied.model, applied.warnings)
}

fn part(model: &Model, role: Role) -> &Part {
    model.parts().find(|p| p.role == role).unwrap()
}

/// (face, x, y, z, diameter, depth, dowel) všetkých dier dielu.
fn holes(p: &Part) -> Vec<(Face, f64, f64, f64, f64, f64, bool)> {
    p.operations
        .iter()
        .filter_map(|o| match *o {
            Operation::Hole {
                face,
                x,
                y,
                z,
                diameter,
                depth,
                dowel,
                ..
            } => Some((face, x, y, z, diameter, depth, dowel)),
            _ => None,
        })
        .collect()
}

fn grooves(p: &Part) -> Vec<&Operation> {
    p.operations
        .iter()
        .filter(|o| matches!(o, Operation::Groove { .. }))
        .collect()
}

const DOWELS_BOTTOM: [f64; 5] = [32.0, 151.375, 270.75, 390.125, 509.5];

#[test]
fn no_warnings_for_example() {
    let (_, warnings) = applied(&load());
    assert!(warnings.is_empty(), "{warnings:?}");
}

#[test]
fn left_side_operations() {
    let (model, _) = applied(&load());
    let side = part(&model, Role::SideLeft);
    let h = holes(side);
    // 5 + 2 + 2 kolíky, 2 rady × 18 dier, 1 drážka
    assert_eq!(h.len(), 9 + 36);
    assert_eq!(grooves(side).len(), 1);
    assert!(
        h.iter().all(|h| h.0 == Face::F && h.3 == 18.0),
        "všetko na ploche F"
    );

    // Ľavý bok: x dielu = H − z skrinky (zhora nadol), y dielu = hĺbka.
    let dowels: Vec<_> = h.iter().filter(|h| h.6).collect();
    let to_bottom: Vec<f64> = dowels
        .iter()
        .filter(|h| h.1 == 711.0)
        .map(|h| h.2)
        .collect();
    assert_eq!(to_bottom, DOWELS_BOTTOM);
    let to_stretchers: Vec<f64> = dowels.iter().filter(|h| h.1 == 9.0).map(|h| h.2).collect();
    assert_eq!(to_stretchers, [32.0, 68.0, 473.5, 509.5]);
    assert!(dowels.iter().all(|h| h.4 == 8.0 && h.5 == 12.0));

    let bores: Vec<_> = h.iter().filter(|h| !h.6).collect();
    assert!(bores.iter().all(|h| h.4 == 5.0 && h.5 == 12.0));
    let mut ys: Vec<f64> = bores.iter().map(|h| h.2).collect();
    ys.dedup();
    ys.sort_by(f64::total_cmp);
    ys.dedup();
    assert_eq!(ys, [37.0, 504.5]);
    let xs: Vec<f64> = bores.iter().filter(|h| h.2 == 37.0).map(|h| h.1).collect();
    // z skrinky 82 … 626 → x dielu 638 … 94
    assert_eq!(xs.first(), Some(&638.0));
    assert_eq!(xs.last(), Some(&94.0));
    assert_eq!(xs.len(), 18);

    match grooves(side)[0] {
        Operation::Groove {
            face,
            start,
            end,
            width,
            depth,
        } => {
            assert_eq!(*face, Face::F);
            assert_eq!(
                (start.x, start.y, end.x, end.y),
                (0.0, 545.75, 720.0, 545.75)
            );
            assert_eq!((*width, *depth), (8.5, 8.0));
        }
        _ => unreachable!(),
    }
}

#[test]
fn right_side_mirrors_left() {
    let (model, _) = applied(&load());
    let side = part(&model, Role::SideRight);
    let h = holes(side);
    assert_eq!(h.len(), 45);
    // Pravý bok: x dielu = z skrinky (zdola nahor)
    let to_bottom: Vec<f64> = h
        .iter()
        .filter(|h| h.6 && h.1 == 9.0)
        .map(|h| h.2)
        .collect();
    assert_eq!(to_bottom, DOWELS_BOTTOM);
    assert!(h.iter().all(|h| h.0 == Face::F));
}

#[test]
fn bottom_edge_holes_and_groove() {
    let (model, _) = applied(&load());
    let bottom = part(&model, Role::Bottom);
    let h = holes(bottom);
    assert_eq!(h.len(), 10);
    let x0: Vec<_> = h.iter().filter(|h| h.0 == Face::X0).collect();
    let x1: Vec<_> = h.iter().filter(|h| h.0 == Face::X1).collect();
    assert_eq!(x0.len(), 5);
    assert_eq!(x1.len(), 5);
    for (hole, y) in x0.iter().zip(DOWELS_BOTTOM) {
        assert_eq!((hole.1, hole.2, hole.3), (0.0, y, 9.0));
        assert_eq!((hole.4, hole.5), (8.0, 25.0));
    }
    assert!(x1.iter().all(|h| h.1 == 564.0 && h.3 == 9.0));
    match grooves(bottom)[0] {
        Operation::Groove {
            face, start, end, ..
        } => {
            assert_eq!(*face, Face::F);
            assert_eq!(
                (start.x, start.y, end.x, end.y),
                (0.0, 545.75, 564.0, 545.75)
            );
        }
        _ => unreachable!(),
    }
}

#[test]
fn stretchers_edge_holes() {
    let (model, _) = applied(&load());
    for role in [Role::StretcherFront, Role::StretcherRear] {
        let p = part(&model, role);
        let h = holes(p);
        assert_eq!(h.len(), 4, "{role:?}");
        assert!(grooves(p).is_empty());
        let mut ys: Vec<f64> = h.iter().map(|h| h.2).collect();
        ys.sort_by(f64::total_cmp);
        ys.dedup();
        assert_eq!(ys, [32.0, 68.0], "{role:?}: y v súradniciach spojky");
        assert!(h
            .iter()
            .all(|h| (h.0 == Face::X0 || h.0 == Face::X1) && h.3 == 9.0));
    }
}

#[test]
fn back_and_shelf_have_no_operations() {
    let (model, _) = applied(&load());
    assert!(part(&model, Role::Back).operations.is_empty());
    assert!(part(&model, Role::AdjustableShelf).operations.is_empty());
}

#[test]
fn wall_cabinet_top_gets_dowels_and_groove() {
    let mut loaded = load();
    loaded.project.cabinets[0].cabinet_type = CabinetType::Wall;
    let (model, warnings) = applied(&loaded);
    assert!(warnings.is_empty(), "{warnings:?}");
    let top = part(&model, Role::Top);
    let h = holes(top);
    // celá hĺbka 560 orezaná na 541.5 → 5 kolíkov na každej strane
    assert_eq!(h.len(), 10);
    assert_eq!(grooves(top).len(), 1);
    match grooves(top)[0] {
        // vrch: y dielu = y skrinky; drážka na ploche F (spodná, k zadnej stene)
        Operation::Groove { face, start, .. } => {
            assert_eq!(*face, Face::F);
            assert_abs_diff_eq!(start.y, 545.75);
        }
        _ => unreachable!(),
    }
    // rad dier končí 64 mm pod spodnou plochou vrchu (702)
    let side = part(&model, Role::SideRight);
    let top_bore = holes(side)
        .iter()
        .filter(|h| !h.6)
        .map(|h| h.1)
        .fold(f64::NEG_INFINITY, f64::max);
    assert_abs_diff_eq!(top_bore, 626.0);
}

#[test]
fn tall_cabinet_line_bore_skips_fixed_shelf() {
    let mut loaded = load();
    let c = &mut loaded.project.cabinets[0];
    c.cabinet_type = CabinetType::Tall;
    c.h = 2100.0;
    c.fixed_shelves = vec![1100.0];
    let (model, warnings) = applied(&loaded);
    assert!(warnings.is_empty(), "{warnings:?}");
    let side = part(&model, Role::SideRight);
    // pravý bok: x dielu = z skrinky
    let zs: Vec<f64> = holes(side)
        .iter()
        .filter(|h| !h.6 && h.2 == 37.0)
        .map(|h| h.1)
        .collect();
    // pod policou: 82 … ≤ 1082 − 64 = 1018 → 82 + 32k ≤ 1018 → 30 dier (82 … 1010)
    // nad policou: 1164 … ≤ 2082 − 64 = 2018 → 27 dier (1164 … 1996)
    assert!(zs.iter().all(|z| *z <= 1018.0 || *z >= 1164.0), "{zs:?}");
    assert_eq!(zs.iter().filter(|z| **z <= 1018.0).count(), 30);
    assert_eq!(zs.iter().filter(|z| **z >= 1164.0).count(), 27);
    // pevná polica má kolíky v hranách
    let shelf = part(&model, Role::FixedShelf);
    assert_eq!(holes(shelf).len(), 10);
}

#[test]
fn face_from_axis_table() {
    use cc_joints::face_from_tool_axis;
    use glam::DVec3;
    assert_eq!(face_from_tool_axis(DVec3::NEG_Z), Some(Face::F));
    assert_eq!(face_from_tool_axis(DVec3::Z), Some(Face::B));
    assert_eq!(face_from_tool_axis(DVec3::X), Some(Face::X0));
    assert_eq!(face_from_tool_axis(DVec3::NEG_X), Some(Face::X1));
    assert_eq!(face_from_tool_axis(DVec3::Y), Some(Face::Y0));
    assert_eq!(face_from_tool_axis(DVec3::NEG_Y), Some(Face::Y1));
    assert_eq!(
        face_from_tool_axis(DVec3::new(1.0, 1.0, 0.0).normalize()),
        None
    );
}

#[test]
fn model_json_matches_golden() {
    let (model, _) = applied(&load());
    let actual = serde_json::to_string_pretty(&model).unwrap() + "\n";
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden/s1/model.json");
    let expected = std::fs::read_to_string(path).unwrap().replace("\r\n", "\n");
    assert!(
        actual == expected,
        "model.json sa líši od golden súboru tests/golden/s1/model.json"
    );
}
