//! Akceptačný test M1: vzorový príklad zo špecifikácie MVP, kap. 10.

use std::path::PathBuf;

use approx::assert_abs_diff_eq;
use cc_generate::{build, front_edge};
use cc_model::{CabinetType, Edge, Part, Role};
use glam::DVec3;

fn example() -> cc_config::LoadedProject {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/s1/project.toml");
    cc_config::load(&path).expect("vzorový projekt sa musí načítať")
}

fn part(parts: &[Part], role: Role) -> &Part {
    parts
        .iter()
        .find(|p| p.role == role)
        .unwrap_or_else(|| panic!("chýba diel {role:?}"))
}

/// Rozsah dielu v súradniciach skrinky (min, max).
fn bbox(p: &Part) -> (DVec3, DVec3) {
    let a = p.transform.point_to_cabinet(DVec3::ZERO);
    let b = p
        .transform
        .point_to_cabinet(DVec3::new(p.length, p.width, p.thickness));
    (a.min(b), a.max(b))
}

fn assert_bbox(p: &Part, min: [f64; 3], max: [f64; 3]) {
    let (lo, hi) = bbox(p);
    for i in 0..3 {
        assert_abs_diff_eq!(lo[i], min[i], epsilon = 0.01);
        assert_abs_diff_eq!(hi[i], max[i], epsilon = 0.01);
    }
}

#[test]
fn spec_chapter_10_parts() {
    let loaded = example();
    let model = build(&loaded.project, &loaded.standard, &loaded.materials).unwrap();
    let parts = &model.cabinets[0].parts;

    let expected = [
        ("S1-SIDE-L", Role::SideLeft, 720.0, 560.0, 18.0),
        ("S1-SIDE-R", Role::SideRight, 720.0, 560.0, 18.0),
        ("S1-BOTTOM", Role::Bottom, 564.0, 560.0, 18.0),
        ("S1-STR-F", Role::StretcherFront, 564.0, 100.0, 18.0),
        ("S1-STR-R", Role::StretcherRear, 564.0, 100.0, 18.0),
        ("S1-BACK", Role::Back, 578.0, 709.0, 8.0),
        ("S1-ASHELF-1", Role::AdjustableShelf, 562.0, 521.5, 18.0),
    ];
    assert_eq!(parts.len(), expected.len());
    for (p, (id, role, l, w, t)) in parts.iter().zip(expected) {
        assert_eq!(p.id, id);
        assert_eq!(p.role, role);
        assert_abs_diff_eq!(p.length, l, epsilon = 0.01);
        assert_abs_diff_eq!(p.width, w, epsilon = 0.01);
        assert_abs_diff_eq!(p.thickness, t, epsilon = 0.01);
        assert!(p.transform.is_right_handed_orthonormal(), "{id}");
    }
}

#[test]
fn spec_chapter_10_positions() {
    let loaded = example();
    let model = build(&loaded.project, &loaded.standard, &loaded.materials).unwrap();
    let parts = &model.cabinets[0].parts;

    assert_bbox(
        part(parts, Role::SideLeft),
        [0.0, 0.0, 0.0],
        [18.0, 560.0, 720.0],
    );
    assert_bbox(
        part(parts, Role::SideRight),
        [582.0, 0.0, 0.0],
        [600.0, 560.0, 720.0],
    );
    assert_bbox(
        part(parts, Role::Bottom),
        [18.0, 0.0, 0.0],
        [582.0, 560.0, 18.0],
    );
    assert_bbox(
        part(parts, Role::StretcherFront),
        [18.0, 0.0, 702.0],
        [582.0, 100.0, 720.0],
    );
    assert_bbox(
        part(parts, Role::StretcherRear),
        [18.0, 441.5, 702.0],
        [582.0, 541.5, 720.0],
    );
    // zadná stena: do drážok bokov 8 mm mínus vôľa 1, na dne drážky v dne (z = 10), stredom v drážke
    assert_bbox(
        part(parts, Role::Back),
        [11.0, 541.75, 10.0],
        [589.0, 549.75, 719.0],
    );
}

#[test]
fn face_f_points_to_interior() {
    let loaded = example();
    let model = build(&loaded.project, &loaded.standard, &loaded.materials).unwrap();
    let parts = &model.cabinets[0].parts;
    let f_normal = |r: Role| part(parts, r).transform.z_axis;
    assert_eq!(f_normal(Role::SideLeft), DVec3::X);
    assert_eq!(f_normal(Role::SideRight), DVec3::NEG_X);
    assert_eq!(f_normal(Role::Bottom), DVec3::Z);
    assert_eq!(f_normal(Role::StretcherFront), DVec3::NEG_Z);
    assert_eq!(f_normal(Role::StretcherRear), DVec3::NEG_Z);
}

#[test]
fn front_edge_is_y0_and_banded() {
    let loaded = example();
    let model = build(&loaded.project, &loaded.standard, &loaded.materials).unwrap();
    for p in &model.cabinets[0].parts {
        if p.role == Role::Back {
            assert_eq!(p.edges, Default::default(), "zadná stena sa neolepuje");
            continue;
        }
        assert_eq!(front_edge(p), Some(Edge::Y0), "{}", p.id);
        assert_eq!(p.edges.get(Edge::Y0), Some("ABS_2_biela"), "{}", p.id);
        assert_eq!(p.edges.get(Edge::Y1), None, "{}", p.id);
    }
}

#[test]
fn wall_cabinet_has_full_top_and_back_in_two_grooves() {
    let mut loaded = example();
    loaded.project.cabinets[0].cabinet_type = CabinetType::Wall;
    let model = build(&loaded.project, &loaded.standard, &loaded.materials).unwrap();
    let parts = &model.cabinets[0].parts;
    assert!(parts.iter().all(|p| p.role != Role::StretcherFront));
    let top = part(parts, Role::Top);
    assert_abs_diff_eq!(top.length, 564.0);
    assert_abs_diff_eq!(top.width, 560.0);
    assert_bbox(top, [18.0, 0.0, 702.0], [582.0, 560.0, 720.0]);
    // Hb = H − 2·(T − gd) − c = 720 − 20 − 1
    assert_abs_diff_eq!(part(parts, Role::Back).width, 699.0, epsilon = 0.01);
}

#[test]
fn tall_cabinet_fixed_shelf() {
    let mut loaded = example();
    let c = &mut loaded.project.cabinets[0];
    c.cabinet_type = CabinetType::Tall;
    c.h = 2100.0;
    c.fixed_shelves = vec![1100.0];
    let model = build(&loaded.project, &loaded.standard, &loaded.materials).unwrap();
    let parts = &model.cabinets[0].parts;
    let shelf = part(parts, Role::FixedShelf);
    assert_eq!(shelf.id, "S1-FSHELF-1");
    assert_abs_diff_eq!(shelf.length, 564.0);
    assert_abs_diff_eq!(shelf.width, 541.5);
    assert_bbox(shelf, [18.0, 0.0, 1082.0], [582.0, 541.5, 1100.0]);
}

#[test]
fn fixed_shelf_rejected_outside_tall() {
    let mut loaded = example();
    loaded.project.cabinets[0].fixed_shelves = vec![400.0];
    let err = build(&loaded.project, &loaded.standard, &loaded.materials).unwrap_err();
    assert!(err.to_string().contains("iba pri type `tall`"), "{err}");
}

#[test]
fn too_narrow_cabinet_is_rejected() {
    let mut loaded = example();
    loaded.project.cabinets[0].w = 30.0;
    let err = build(&loaded.project, &loaded.standard, &loaded.materials).unwrap_err();
    assert!(err.to_string().contains("vnútorná šírka"), "{err}");
}
