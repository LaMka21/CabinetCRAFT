//! Sokel po zostavách skriniek, bočný sokel a krycí bok (findings 12 §6) na vzorovej kuchyni.

use std::path::PathBuf;

use approx::assert_abs_diff_eq;
use cc_generate::build;
use cc_model::{Face, Model, Operation, Part, PlinthSpan, Role};
use glam::DVec3;

fn kitchen() -> cc_config::LoadedProject {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/dielna/kuchyna.toml");
    cc_config::load(&path).expect("vzorová kuchyňa sa musí načítať")
}

fn parts<'a>(m: &'a Model, cab: &str, role: Role) -> Vec<&'a Part> {
    m.cabinets
        .iter()
        .find(|c| c.id == cab)
        .map(|c| c.parts.iter().filter(|p| p.role == role).collect())
        .unwrap_or_default()
}

fn by_id<'a>(m: &'a Model, id: &str) -> &'a Part {
    m.cabinets
        .iter()
        .flat_map(|c| &c.parts)
        .find(|p| p.id == id)
        .unwrap_or_else(|| panic!("chýba diel {id}"))
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

fn holes(p: &Part) -> Vec<(Face, f64, f64, f64, f64)> {
    p.operations
        .iter()
        .filter_map(|o| match *o {
            Operation::Hole {
                face,
                x,
                y,
                diameter,
                depth,
                ..
            } => Some((face, x, y, diameter, depth)),
            Operation::Groove { .. } => None,
        })
        .collect()
}

#[test]
fn one_plinth_per_run_with_side_plinth_at_exposed_end() {
    let k = kitchen();
    let m = build(&k.project, &k.standard, &k.materials).unwrap();
    // stena W1: VY1, SP1, SP2, SP3 (0 … 2750) = jeden sokel na VY1
    let run = by_id(&m, "VY1-SOKEL");
    assert_eq!(run.material, "MDF_19_W1000_biela");
    assert_abs_diff_eq!(run.length, 2750.0, epsilon = 1e-9);
    assert_abs_diff_eq!(run.width, 97.0, epsilon = 1e-9);
    // zadná plocha 68 za čelom, predná 49; od podlahy (z = −100) po 3 pod korpus
    assert_bbox(run, [0.0, 49.0, -100.0], [2750.0, 68.0, -3.0]);
    for id in ["SP1", "SP2", "SP3"] {
        assert!(
            parts(&m, id, Role::Plinth)
                .iter()
                .all(|p| p.id.ends_with("-SOKEL-R")),
            "{id} nemá vlastný predný sokel"
        );
    }
    // VY1 stojí v rohu pri stene W4: bez bočného sokla; SP3 končí voľne: bočný sokel vpravo
    assert_eq!(parts(&m, "VY1", Role::Plinth).len(), 1);
    let side = by_id(&m, "SP3-SOKEL-R");
    assert_abs_diff_eq!(side.length, 510.0 - 68.0, epsilon = 1e-9);
    assert_abs_diff_eq!(side.width, 97.0, epsilon = 1e-9);
    assert_bbox(side, [581.0, 68.0, -100.0], [600.0, 510.0, -3.0]);
    let sh = holes(side);
    assert_eq!(sh.len(), 2);
    assert!(sh
        .iter()
        .all(|h| h.0 == Face::X0 && h.3 == 8.0 && h.4 == 26.0));
    // protikus v zadnej ploche predného sokla: stred hrúbky bočného sokla, 18 / 79 od podlahy
    let mut rh = holes(run);
    rh.sort_by(|a, b| a.2.total_cmp(&b.2));
    assert_eq!(rh.len(), 2);
    for (h, y) in rh.iter().zip([18.0, 79.0]) {
        assert_eq!(h.0, Face::F);
        assert_abs_diff_eq!(h.1, 2750.0 - 9.5, epsilon = 1e-6);
        assert_abs_diff_eq!(h.2, y, epsilon = 1e-6);
        assert_eq!((h.3, h.4), (8.0, 13.0));
    }
    // stena W2: SP4 samostatne s voľným rohom → bočné sokle na oboch stranách
    assert_abs_diff_eq!(by_id(&m, "SP4-SOKEL").length, 600.0, epsilon = 1e-9);
    by_id(&m, "SP4-SOKEL-L");
    by_id(&m, "SP4-SOKEL-R");
    assert_eq!(holes(by_id(&m, "SP4-SOKEL")).len(), 4);
}

#[test]
fn runs_off_gives_plinth_per_cabinet() {
    let mut k = kitchen();
    k.standard.plinth.runs = false;
    k.standard.plinth.side = false;
    let m = build(&k.project, &k.standard, &k.materials).unwrap();
    for (id, w) in [
        ("VY1", 600.0),
        ("SP1", 650.0),
        ("SP2", 900.0),
        ("SP3", 600.0),
    ] {
        let p = parts(&m, id, Role::Plinth);
        assert_eq!(p.len(), 1, "{id}");
        assert_abs_diff_eq!(p[0].length, w, epsilon = 1e-9);
        assert!(p[0].operations.is_empty());
    }
}

#[test]
fn long_run_is_split_at_cabinet_joint() {
    let mut k = kitchen();
    k.standard.plinth.max_length = 1300.0;
    let m = build(&k.project, &k.standard, &k.materials).unwrap();
    // 0 … 1250 (VY1 + SP1), 1250 … 2150 (SP2), 2150 … 2750 (SP3)
    assert_abs_diff_eq!(by_id(&m, "VY1-SOKEL").length, 1250.0, epsilon = 1e-9);
    assert_abs_diff_eq!(by_id(&m, "SP2-SOKEL").length, 900.0, epsilon = 1e-9);
    assert_abs_diff_eq!(by_id(&m, "SP3-SOKEL").length, 600.0, epsilon = 1e-9);
    // styk kusov nie je odkrytý koniec
    assert!(parts(&m, "SP1", Role::Plinth).is_empty());
    assert_eq!(parts(&m, "SP2", Role::Plinth).len(), 1);
}

#[test]
fn manual_span_and_cover_side() {
    let mut k = kitchen();
    let sp4 = k
        .project
        .cabinets
        .iter_mut()
        .find(|c| c.id == "SP4")
        .unwrap();
    sp4.options.plinth_span = Some(PlinthSpan {
        from: -60.0,
        length: 700.0,
        height: Some(96.5),
    });
    sp4.options.side_plinth_left = Some(true);
    sp4.options.side_plinth_right = Some(false);
    sp4.options.side_plinth_height = Some(97.0);
    sp4.options.cover_side_right = Some(true);
    let m = build(&k.project, &k.standard, &k.materials).unwrap();
    let front = by_id(&m, "SP4-SOKEL");
    assert_bbox(front, [-60.0, 49.0, -100.0], [640.0, 68.0, -3.5]);
    let side = by_id(&m, "SP4-SOKEL-L");
    assert_bbox(side, [0.0, 68.0, -100.0], [19.0, 510.0, -3.0]);
    assert!(parts(&m, "SP4", Role::Plinth).len() == 2);
    // diery predného sokla: 60 + 9,5 od ľavého konca, výšky 18 / 79 od podlahy (zhora 78,5 / 17,5)
    let mut fh = holes(front);
    fh.sort_by(|a, b| a.2.total_cmp(&b.2));
    assert_eq!(fh.len(), 2);
    assert_abs_diff_eq!(fh[0].1, 69.5, epsilon = 1e-6);
    assert_abs_diff_eq!(fh[0].2, 17.5, epsilon = 1e-6);
    assert_abs_diff_eq!(fh[1].2, 78.5, epsilon = 1e-6);
    // krycí bok 38: od podlahy po vrch korpusu + 37, 28 pred čelom, 80 za zadnou hranou
    let cover = by_id(&m, "SP4-COVER-R");
    assert_eq!(cover.role, Role::CoverSide);
    assert_eq!(cover.material, "PD_38_H1385_dub");
    assert_abs_diff_eq!(cover.length, 899.0, epsilon = 1e-9);
    assert_abs_diff_eq!(cover.width, 618.0, epsilon = 1e-9);
    assert_bbox(cover, [600.0, -28.0, -100.0], [638.0, 590.0, 799.0]);
}

#[test]
fn hostile_plinth_values_do_not_panic() {
    let mut k = kitchen();
    for c in &mut k.project.cabinets {
        c.options.plinth_span = Some(PlinthSpan {
            from: f64::NAN,
            length: -5.0,
            height: Some(f64::INFINITY),
        });
        c.options.side_plinth_left = Some(true);
        c.options.side_plinth_height = Some(f64::NAN);
        c.options.cover_side_left = Some(true);
    }
    k.standard.plinth.side_dowel.from_edge = f64::NAN;
    k.standard.cover_side.top = f64::NEG_INFINITY;
    let m = build(&k.project, &k.standard, &k.materials).unwrap();
    for p in m.cabinets.iter().flat_map(|c| &c.parts) {
        assert!(p.length.is_finite() && p.width.is_finite(), "{}", p.id);
    }
}
