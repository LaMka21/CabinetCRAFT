//! Blenda (findings 11 §5.3, 12 §2): diely blendy pri hornej skrinke a samostatnej blendy
//! a kolíky lišty do čela a dna blendy, porovnané s CV zákazky REF (skr. 9 a 5).

use std::path::PathBuf;

use approx::assert_abs_diff_eq;
use cc_model::{CabinetSpec, Face, Hardware, JointType, Model, Operation, Part, Role};
use glam::DVec3;
use serde_json::json;

fn load() -> cc_config::LoadedProject {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/dielna/kuchyna.toml");
    cc_config::load(&path).unwrap()
}

fn spec(v: serde_json::Value) -> CabinetSpec {
    serde_json::from_value(v).unwrap()
}

/// Horná skrinka REF skr. 9: 576,5 × 682 × 330, dvierka s presahom 19 pod dno, blenda 50
/// vpravo s čelom 60 a dnom blendy.
fn ho9(filler: serde_json::Value) -> CabinetSpec {
    spec(json!({
        "id": "HO9", "type": "wall", "w": 576.5, "h": 682.0, "d": 330.0,
        "adjustable_shelves": 2,
        "fronts": [{ "kind": "door", "hinge_side": "right", "adjust": { "bottom": 19.0 } }],
        "reveals": { "bottom": 0.0, "right": 2.0 },
        "filler": filler
    }))
}

/// Samostatná blenda REF skr. 5: 18 × 762 × 100 bez korpusu, čelo 78.
fn sp5(filler: serde_json::Value) -> CabinetSpec {
    spec(json!({
        "id": "SP5", "type": "base", "w": 18.0, "h": 762.0, "d": 100.0,
        "carcass": false, "filler": filler
    }))
}

fn build_with(cabinets: Vec<CabinetSpec>) -> Result<Model, String> {
    let loaded = load();
    let mut project = loaded.project.clone();
    project.cabinets = cabinets;
    let model = cc_generate::build(&project, &loaded.standard, &loaded.materials)
        .map_err(|e| e.to_string())?;
    let applied = cc_joints::apply(model, &loaded.standard).map_err(|e| e.to_string())?;
    Ok(applied.model)
}

fn part<'a>(m: &'a Model, id: &str) -> &'a Part {
    m.parts()
        .find(|p| p.id == id)
        .unwrap_or_else(|| panic!("diel {id}"))
}

fn bbox(p: &Part) -> (DVec3, DVec3) {
    let a = p.transform.point_to_cabinet(DVec3::ZERO);
    let b = p
        .transform
        .point_to_cabinet(DVec3::new(p.length, p.width, p.thickness));
    (a.min(b), a.max(b))
}

/// Diery Ø8 dielu: (strana, x, y, hĺbka), zoradené.
fn holes(p: &Part) -> Vec<(Face, f64, f64, f64)> {
    let mut v: Vec<(Face, f64, f64, f64)> = p
        .operations
        .iter()
        .filter_map(|o| match *o {
            Operation::Hole {
                face,
                x,
                y,
                diameter,
                depth,
                ..
            } => (diameter == 8.0).then_some((face, x, y, depth)),
            _ => None,
        })
        .collect();
    v.sort_by(|a, b| {
        format!("{:?}", a.0)
            .cmp(&format!("{:?}", b.0))
            .then(a.1.total_cmp(&b.1))
            .then(a.2.total_cmp(&b.2))
    });
    v
}

#[test]
fn wall_cabinet_filler_like_cv_ref() {
    let m = build_with(vec![ho9(
        json!({ "side": "right", "width": 50.0, "front_width": 60.0, "bottom": true }),
    )])
    .unwrap();
    // korpus 576,5 (dno 540,5, police 539,5, chrbát 574,5), vrch cez blendu 626,5
    assert_abs_diff_eq!(part(&m, "HO9-BOTTOM").length, 540.5);
    assert_abs_diff_eq!(part(&m, "HO9-ASHELF-1").length, 539.5);
    assert_abs_diff_eq!(part(&m, "HO9-BACK").length, 574.5);
    let top = part(&m, "HO9-TOP");
    assert_abs_diff_eq!(top.length, 626.5);
    let (lo, hi) = bbox(top);
    assert_abs_diff_eq!(lo.x, 0.0);
    assert_abs_diff_eq!(hi.x, 626.5);
    // vrch: diery do bokov ostali nad bokmi (9 a 567,5 od ľavého konca)
    let mut top_x: Vec<f64> = holes(top).iter().map(|h| 626.5 - h.1).collect();
    top_x.sort_by(f64::total_cmp);
    top_x.dedup();
    assert_eq!(top_x, vec![9.0, 567.5]);

    // lišta 661 × 309,5 × 18 za pravým bokom, 2 pred korpusom
    let strip = part(&m, "HO9-BL-LISTA");
    assert_eq!(strip.role, Role::FillerStrip);
    assert_eq!(strip.material, "DTDL_18_W960_biela");
    assert_abs_diff_eq!(strip.length, 661.0);
    assert_abs_diff_eq!(strip.width, 309.5);
    let (lo, hi) = bbox(strip);
    assert_abs_diff_eq!(lo.x, 576.5);
    assert_abs_diff_eq!(hi.x, 594.5);
    assert_abs_diff_eq!(lo.y, -2.0);
    assert_abs_diff_eq!(lo.z, 0.0);
    // [P] 966f0041: 3 × Ø8 × 26 do spodnej hrany, 4 × Ø8 × 26 do prednej hrany
    let expected = vec![
        (Face::X0, 0.0, 50.0, 26.0),
        (Face::X0, 0.0, 154.75, 26.0),
        (Face::X0, 0.0, 259.5, 26.0),
        (Face::Y0, 50.0, 0.0, 26.0),
        (Face::Y0, 237.0, 0.0, 26.0),
        (Face::Y0, 424.0, 0.0, 26.0),
        (Face::Y0, 611.0, 0.0, 26.0),
    ];
    assert_eq!(holes(strip), expected);

    // čelo blendy 701 × 60 × 19 v rovine dvierok (2 pred korpusom), 4 × Ø8 × 13 zo zadnej plochy
    let front = part(&m, "HO9-BL-CELO");
    assert_eq!(front.role, Role::FillerFront);
    assert_eq!(front.material, "MDF_19_W1000_biela");
    assert_abs_diff_eq!(front.length, 701.0);
    assert_abs_diff_eq!(front.width, 60.0);
    let (lo, hi) = bbox(front);
    assert_abs_diff_eq!(lo.x, 576.5);
    assert_abs_diff_eq!(hi.x, 636.5);
    assert_abs_diff_eq!(lo.y, -21.0);
    assert_abs_diff_eq!(lo.z, -19.0);
    assert_abs_diff_eq!(hi.z, 682.0);
    let fh = holes(front);
    assert_eq!(fh.len(), 4);
    assert!(fh.iter().all(|h| h.0 == Face::F && h.3 == 13.0));
    let xs: Vec<f64> = fh.iter().map(|h| h.1).collect();
    assert_eq!(xs, vec![69.0, 256.0, 443.0, 630.0]);
    assert!(fh.iter().all(|h| h.2 == 9.0));

    // dno blendy 50 × 333 × 19 pod korpusom, 3 × Ø8 × 13 zhora (9 od boku)
    let bottom = part(&m, "HO9-BL-DNO");
    assert_eq!(bottom.role, Role::FillerBottom);
    assert_abs_diff_eq!(bottom.length, 50.0);
    assert_abs_diff_eq!(bottom.width, 333.0);
    let (lo, hi) = bbox(bottom);
    assert_abs_diff_eq!(lo.y, -3.0);
    assert_abs_diff_eq!(hi.y, 330.0);
    assert_abs_diff_eq!(lo.z, -19.0);
    let bh = holes(bottom);
    assert_eq!(
        bh,
        vec![
            (Face::F, 9.0, 51.0, 13.0),
            (Face::F, 9.0, 155.75, 13.0),
            (Face::F, 9.0, 260.5, 13.0),
        ]
    );
}

#[test]
fn left_filler_is_mirrored() {
    let m = build_with(vec![ho9(
        json!({ "side": "left", "width": 50.0, "front_width": 60.0, "bottom": true }),
    )])
    .unwrap();
    let (lo, hi) = bbox(part(&m, "HO9-TOP"));
    assert_abs_diff_eq!(lo.x, -50.0);
    assert_abs_diff_eq!(hi.x, 576.5);
    let (lo, hi) = bbox(part(&m, "HO9-BL-LISTA"));
    assert_abs_diff_eq!(lo.x, -18.0);
    assert_abs_diff_eq!(hi.x, 0.0);
    let (lo, _) = bbox(part(&m, "HO9-BL-CELO"));
    assert_abs_diff_eq!(lo.x, -60.0);
    assert_eq!(holes(part(&m, "HO9-BL-LISTA")).len(), 7);
    assert_eq!(holes(part(&m, "HO9-BL-CELO")).len(), 4);
    assert_eq!(holes(part(&m, "HO9-BL-DNO")).len(), 3);
}

#[test]
fn standalone_filler_like_cv_ref() {
    let m = build_with(vec![sp5(json!({ "front_width": 78.0 }))]).unwrap();
    let cab = &m.cabinets[0];
    assert_eq!(cab.parts.len(), 2);
    // [P] lišta 100 × 762 × 18, 3 × Ø8 × 26 do prednej hrany na 32 / 381 / 730
    let strip = part(&m, "SP5-BL-LISTA");
    assert_abs_diff_eq!(strip.length, 762.0);
    assert_abs_diff_eq!(strip.width, 100.0);
    let (lo, hi) = bbox(strip);
    assert_abs_diff_eq!(lo.x, 0.0);
    assert_abs_diff_eq!(hi.x, 18.0);
    let sh = holes(strip);
    assert_eq!(
        sh,
        vec![
            (Face::Y0, 32.0, 0.0, 26.0),
            (Face::Y0, 381.0, 0.0, 26.0),
            (Face::Y0, 730.0, 0.0, 26.0),
        ]
    );
    // [P] čelo 78 × 762 × 19 priamo na lište, 3 × Ø8 × 13 zo zadnej plochy, 9 od ľavej hrany
    let front = part(&m, "SP5-FF");
    assert_eq!(front.role, Role::FalseFront);
    assert_abs_diff_eq!(front.length, 762.0);
    assert_abs_diff_eq!(front.width, 78.0);
    let (lo, hi) = bbox(front);
    assert_abs_diff_eq!(lo.x, 0.0);
    assert_abs_diff_eq!(hi.x, 78.0);
    assert_abs_diff_eq!(hi.y, 0.0);
    let fh = holes(front);
    assert_eq!(
        fh,
        vec![
            (Face::F, 32.0, 9.0, 13.0),
            (Face::F, 381.0, 9.0, 13.0),
            (Face::F, 730.0, 9.0, 13.0),
        ]
    );
    // predvolené čelo: šírka lišty + presah 60 (BLENDA 2177, OLR 60)
    let m = build_with(vec![sp5(json!({}))]).unwrap();
    assert_abs_diff_eq!(part(&m, "SP5-FF").width, 78.0);
}

#[test]
fn shop_catalog_blenda_builds_like_cv() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/dielna");
    let catalogs = cc_config::load_catalogs(&dir).unwrap();
    let (_, item) = cc_model::find_catalog_item(&catalogs, "BLENDA", None).unwrap();
    let m = build_with(vec![item.to_spec("SP5")]).unwrap();
    let ids: Vec<&str> = m.parts().map(|p| p.id.as_str()).collect();
    assert_eq!(ids.len(), 2, "{ids:?}");
    assert_abs_diff_eq!(part(&m, "SP5-FF").width, 78.0);
    assert_eq!(holes(part(&m, "SP5-FF")).len(), 3);
    assert_eq!(holes(part(&m, "SP5-BL-LISTA")).len(), 3);
}

#[test]
fn filler_without_dowels_has_no_holes() {
    let m = build_with(vec![
        ho9(json!({ "width": 50.0, "front_width": 60.0, "bottom": true, "dowels": false })),
        sp5(json!({ "dowels": false })),
    ])
    .unwrap();
    for cab in &m.cabinets {
        assert!(!cab
            .hardware
            .iter()
            .any(|h| matches!(h, Hardware::Filler { .. })));
        for p in cab.parts.iter().filter(|p| p.role.is_filler()) {
            assert!(p.operations.is_empty(), "{}", p.id);
        }
    }
    assert!(part(&m, "SP5-FF").operations.is_empty());
}

#[test]
fn hostile_filler_numbers_are_errors() {
    for f in [
        json!({ "width": 5.0 }),
        json!({ "width": 1e9 }),
        json!({ "width": 50.0, "front_width": -1.0 }),
        json!({ "width": 50.0, "front_width": 1e12 }),
    ] {
        let r = build_with(vec![ho9(f.clone())]);
        assert!(r.is_err(), "{f}: {:?}", r.map(|_| ()));
    }
    // NaN zo vstupu (JSON ho nevie, priamo do zadania)
    let mut c = ho9(json!({ "width": 50.0 }));
    if let Some(f) = c.filler.as_mut() {
        f.width = f64::NAN;
    }
    assert!(build_with(vec![c]).is_err());
    // samostatná blenda užšia ako lišta
    let mut c = sp5(json!({}));
    c.w = 10.0;
    assert!(build_with(vec![c]).is_err());
}

#[test]
fn filler_dowels_are_not_confirmat_pairs() {
    let loaded = load();
    let mut with = ho9(json!({ "width": 50.0, "front_width": 60.0, "bottom": true }));
    with.options.joint_type = Some(JointType::Confirmat);
    let mut without = with.clone();
    without.filler = None;
    let bill = |c: CabinetSpec| {
        let m = build_with(vec![c]).unwrap();
        cc_joints::bill::cabinet_bill(&m.cabinets[0], &loaded.standard)
    };
    let (a, b) = (bill(with), bill(without));
    let qty =
        |v: &[(u8, String, u32)], group: u8| v.iter().find(|l| l.0 == group).map_or(0, |l| l.2);
    // 7 kolíkov lišty navyše, konfirmátov rovnako
    assert_eq!(qty(&a, 2), qty(&b, 2) + 7);
    assert_eq!(qty(&a, 3), qty(&b, 3));
}
