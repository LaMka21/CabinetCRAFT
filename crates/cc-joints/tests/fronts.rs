//! Čelá (dvierka, zásuvky) a pánty: rozmery čiel, miska v dvierkach, podložka na boku.

use std::path::PathBuf;

use approx::assert_abs_diff_eq;
use cc_model::{Face, FrontKind, FrontMount, FrontSpec, IssueCode, Model, Operation, Part, Role};
use glam::DVec3;

fn load() -> cc_config::LoadedProject {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/dielna/kuchyna.toml");
    cc_config::load(&path).unwrap()
}

fn build(loaded: &cc_config::LoadedProject) -> (Model, Vec<cc_model::Issue>) {
    let model = cc_generate::build(&loaded.project, &loaded.standard, &loaded.materials).unwrap();
    let applied = cc_joints::apply(model, &loaded.standard).unwrap();
    (applied.model, applied.warnings)
}

fn part<'a>(m: &'a Model, id: &str) -> &'a Part {
    m.parts().find(|p| p.id == id).unwrap()
}

/// (strana, x, y, priemer, hĺbka) dier do plochy.
fn face_holes(p: &Part, diameter: f64) -> Vec<(Face, f64, f64, f64)> {
    p.operations
        .iter()
        .filter_map(|o| match *o {
            Operation::Hole {
                face,
                x,
                y,
                diameter: d,
                depth,
                ..
            } if d == diameter && matches!(face, Face::F | Face::B) => Some((face, x, y, depth)),
            _ => None,
        })
        .collect()
}

/// Kváder dielu v súradniciach skrinky.
fn bbox(p: &Part) -> (DVec3, DVec3) {
    let a = p.transform.point_to_cabinet(DVec3::ZERO);
    let b = p
        .transform
        .point_to_cabinet(DVec3::new(p.length, p.width, p.thickness));
    (a.min(b), a.max(b))
}

#[test]
fn overlay_fronts_have_expected_sizes() {
    let loaded = load();
    let (m, warnings) = build(&loaded);
    // čelá nie sú „dotyk bez pravidla spoja“
    assert!(
        warnings
            .iter()
            .all(|w| w.code != IssueCode::JointRuleMissing),
        "{warnings:?}"
    );

    // SP2 900: dvojité dvierka (900 − 2 · 1 − 2) / 2 = 448, výška 762 − 32 = 730 (ako v CV)
    for id in ["SP2-DOOR-1", "SP2-DOOR-2"] {
        let d = part(&m, id);
        assert_eq!(d.role, Role::Door);
        assert_abs_diff_eq!(d.length, 730.0);
        assert_abs_diff_eq!(d.width, 448.0);
        assert_eq!(d.material, "MDF_19_W1000_biela");
        assert!(d.edges.x0.is_some() && d.edges.x1.is_some());
        assert!(d.edges.y0.is_some() && d.edges.y1.is_some());
        let (lo, hi) = bbox(d);
        // 3 mm pred lícom korpusu, spodok lícuje so spodkom korpusu
        assert_abs_diff_eq!(lo.y, -22.0);
        assert_abs_diff_eq!(hi.y, -3.0);
        assert_abs_diff_eq!(lo.z, 0.0);
    }
    // SP3: tri rovnaké zásuvky (730 − 2 · 32) / 3 = 222, šírka 600 − 2 = 598
    for k in 1..=3 {
        let d = part(&m, &format!("SP3-DRW-{k}"));
        assert_abs_diff_eq!(d.length, 222.0);
        assert_abs_diff_eq!(d.width, 598.0);
    }
    // SP1: zásuvka 200 + dvierka 730 − 200 − 32 = 498; vrch 32 pod vrchom korpusu (UK16)
    assert_abs_diff_eq!(part(&m, "SP1-DRW-1").length, 200.0);
    assert_abs_diff_eq!(part(&m, "SP1-DOOR-1").length, 498.0);
    let (lo, hi) = bbox(part(&m, "SP1-DRW-1"));
    assert_abs_diff_eq!(hi.z, 730.0);
    assert_abs_diff_eq!(lo.z, 530.0);
    // horná skrinka: 663 + 19 = 682, presah 19 pod dno, vrch lícuje
    let d = part(&m, "HO1-DOOR-1");
    assert_abs_diff_eq!(d.length, 682.0);
    let (lo, hi) = bbox(d);
    assert_abs_diff_eq!(lo.z, -19.0);
    assert_abs_diff_eq!(hi.z, 663.0);
}

#[test]
fn hinge_cups_and_plates() {
    let loaded = load();
    let (m, _) = build(&loaded);
    // SP4: dvierka 730 × 598, pánty vpravo, 2 pánty 100 mm od hrán (ako dvierka 0093 v CV)
    let door = part(&m, "SP4-DOOR-1");
    let cups = face_holes(door, 35.0);
    assert_eq!(cups.len(), 2, "{cups:?}");
    for (face, _, y, depth) in &cups {
        assert_eq!(*face, Face::F, "miska z vnútornej plochy dvierok");
        assert_abs_diff_eq!(*y, 598.0 - 22.5);
        assert_abs_diff_eq!(*depth, 13.5);
    }
    let mut xs: Vec<f64> = cups.iter().map(|c| c.1).collect();
    xs.sort_by(f64::total_cmp);
    assert_eq!(xs, vec![100.0, 630.0]);
    // kolíky INSERTA Ø8 × 12: ±22,5 pozdĺž hrany, 9,5 od misky dovnútra (32 od hrany)
    let mut dowels: Vec<(f64, f64)> = face_holes(door, 8.0)
        .into_iter()
        .map(|h| (h.1, h.2))
        .collect();
    dowels.sort_by(|a, b| a.0.total_cmp(&b.0));
    assert_eq!(
        dowels,
        vec![
            (77.5, 566.0),
            (122.5, 566.0),
            (607.5, 566.0),
            (652.5, 566.0)
        ]
    );

    // priama podložka na pravom boku: Ø5 × 12 v 20 a 52 od prednej hrany, vo výške misky
    let side = part(&m, "SP4-SIDE-R");
    let mut plate: Vec<(f64, f64)> = face_holes(side, 5.0)
        .into_iter()
        .filter(|h| h.0 == Face::F && h.3 == 12.0)
        .map(|h| (h.1, h.2))
        .collect();
    plate.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(
        plate,
        vec![(100.0, 20.0), (100.0, 52.0), (630.0, 20.0), (630.0, 52.0)]
    );
    // ľavý bok SP4 nemá podložky
    let left = part(&m, "SP4-SIDE-L");
    assert!(face_holes(left, 5.0).iter().all(|h| h.3 != 12.0));

    // horná skrinka: podložky 100 − 19 = 81 nad spodkom boku (dvierka presahujú 19 pod dno)
    let mut zs: Vec<f64> = face_holes(part(&m, "HO1-SIDE-L"), 5.0)
        .into_iter()
        .filter(|h| h.3 == 12.0 && h.2 == 20.0)
        .map(|h| h.1)
        .collect();
    zs.sort_by(f64::total_cmp);
    assert_eq!(zs.len(), 2);
    // ľavý bok: x ide zhora nadol; bok je o vrch 19 nižší ako skrinka 663 (vrch na bokoch)
    let side_h = part(&m, "HO1-SIDE-L").length;
    assert_eq!(side_h, 644.0);
    let mut from_bottom: Vec<f64> = zs.iter().map(|x| side_h - x).collect();
    from_bottom.sort_by(f64::total_cmp);
    // dvierka 682 od −19: 100 od spodku a 100 od vrchu dvierok (CV pri korpuse 682: 81 / 582)
    assert_eq!(from_bottom, vec![81.0, 563.0]);

    // vysoké dvierka VY1 (2 × 1033): po 3 pánty
    let vy = m.cabinets.iter().find(|c| c.id == "VY1").unwrap();
    assert_eq!(vy.hardware.len(), 6);
    assert_eq!(face_holes(part(&m, "VY1-DOOR-1"), 35.0).len(), 3);
}

/// Zásuvka Merivobox (findings 09 §3): korpus, výsuvy v bokoch, upevnenie čela.
#[test]
fn merivobox_drawer() {
    let loaded = load();
    let (m, warnings) = build(&loaded);
    assert!(
        warnings
            .iter()
            .all(|w| w.code != IssueCode::JointRuleMissing),
        "{warnings:?}"
    );
    let sp1 = m.cabinets.iter().find(|c| c.id == "SP1").unwrap();
    let Some(cc_model::Hardware::Drawer {
        box_bottom,
        box_height,
        length,
        box_left,
        box_right,
        ..
    }) = sp1
        .hardware
        .iter()
        .find(|h| matches!(h, cc_model::Hardware::Drawer { .. }))
    else {
        panic!("SP1 nemá zásuvku");
    };
    // čelo 530–730: korpus 2 nad spodkom čela, najvyšší korpus pod vrch čela − 20 je K 141, NL 500
    assert_abs_diff_eq!(*box_bottom, 532.0);
    assert_abs_diff_eq!(*box_height, 141.0);
    assert_abs_diff_eq!(*length, 500.0);
    assert_abs_diff_eq!(*box_left, 38.5);
    assert_abs_diff_eq!(*box_right, 650.0 - 38.5);

    // výsuv: Ø5 × 12 v 37/69/261/293 od prednej hrany, 54 nad spodkom korpusu (z = 586)
    for side in ["SP1-SIDE-L", "SP1-SIDE-R"] {
        let p = part(&m, side);
        let mut runner: Vec<(f64, f64)> = face_holes(p, 5.0)
            .into_iter()
            .filter(|h| h.3 == 12.0)
            .map(|h| {
                let z = p.transform.point_to_cabinet(DVec3::new(h.1, h.2, 0.0)).z;
                (z, h.2)
            })
            .filter(|(z, _)| *z > 500.0)
            .collect();
        runner.sort_by(|a, b| a.1.total_cmp(&b.1));
        assert_eq!(
            runner,
            vec![(586.0, 37.0), (586.0, 69.0), (586.0, 261.0), (586.0, 293.0)],
            "{side}"
        );
    }
    // čelo: Ø10 × 12 35,5 a 67,5 od spodku čela, 37,5 od bočných hrán (ako čelo 0007 v CV)
    let front = part(&m, "SP1-DRW-1");
    let mut holes: Vec<(f64, f64)> = face_holes(front, 10.0)
        .into_iter()
        .map(|h| (h.1, h.2))
        .collect();
    holes.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(
        holes,
        vec![(35.5, 37.5), (35.5, 610.5), (67.5, 37.5), (67.5, 610.5)]
    );
    // dno a zadok 16 mm: 474 × 563 (NL − 26, svetlá šírka 614 − 51), zadok výška 141 − 20
    let bottom = part(&m, "SP1-DRWB-1");
    assert_eq!(bottom.material, "DTDL_16_W960_biela");
    assert_abs_diff_eq!(bottom.length, 563.0);
    assert_abs_diff_eq!(bottom.width, 474.0);
    let back = part(&m, "SP1-DRWK-1");
    assert_abs_diff_eq!(back.width, 121.0);

    // vysoký korpus E: navyše príchytka relingu 193,5 nad spodkom korpusu
    let mut loaded = load();
    let c = loaded
        .project
        .cabinets
        .iter_mut()
        .find(|c| c.id == "SP1")
        .unwrap();
    c.fronts = vec![
        FrontSpec {
            height: Some(300.0),
            ..FrontSpec::new(FrontKind::Drawer)
        },
        FrontSpec::new(FrontKind::Drawer),
    ];
    let (m, _) = build(&loaded);
    let top = part(&m, "SP1-DRW-1");
    let zs: std::collections::BTreeSet<i64> = face_holes(top, 10.0)
        .iter()
        .map(|h| h.1.round() as i64)
        .collect();
    // korpus 2 nad spodkom čela: 35,5 / 67,5 / 195,5
    assert_eq!(zs.into_iter().collect::<Vec<_>>(), vec![36, 68, 196]);
}

#[test]
fn inset_fronts_sit_inside_the_carcass() {
    let mut loaded = load();
    let c = loaded
        .project
        .cabinets
        .iter_mut()
        .find(|c| c.id == "SP4")
        .unwrap();
    c.front_mount = Some(FrontMount::Inset);
    let (m, _) = build(&loaded);
    let d = part(&m, "SP4-DOOR-1");
    // 600 − 2 · 18 − 2 · 2 = 560, 762 − 2 · 18 − 2 · 2 = 722
    assert_abs_diff_eq!(d.width, 560.0);
    assert_abs_diff_eq!(d.length, 722.0);
    let (lo, hi) = bbox(d);
    assert_abs_diff_eq!(lo.y, 0.0);
    assert_abs_diff_eq!(hi.y, 19.0);
    // podložka o hrúbku dvierok ďalej od prednej hrany boku
    let side = part(&m, "SP4-SIDE-R");
    assert!(face_holes(side, 5.0)
        .iter()
        .any(|h| (h.2 - 39.0).abs() < 1e-9));
}

#[test]
fn bad_fronts_are_rejected() {
    let mut loaded = load();
    let c = loaded
        .project
        .cabinets
        .iter_mut()
        .find(|c| c.id == "SP4")
        .unwrap();
    c.fronts = vec![
        FrontSpec {
            height: Some(500.0),
            ..FrontSpec::new(FrontKind::Drawer)
        },
        FrontSpec {
            height: Some(500.0),
            ..FrontSpec::new(FrontKind::Drawer)
        },
    ];
    let err = cc_generate::build(&loaded.project, &loaded.standard, &loaded.materials)
        .unwrap_err()
        .to_string();
    assert!(err.contains("SP4") && err.contains("vyššie"), "{err}");

    let c = loaded
        .project
        .cabinets
        .iter_mut()
        .find(|c| c.id == "SP4")
        .unwrap();
    c.fronts = vec![FrontSpec::new(FrontKind::Door)];
    c.hinge = Some("neexistuje".into());
    let err = cc_generate::build(&loaded.project, &loaded.standard, &loaded.materials)
        .unwrap_err()
        .to_string();
    assert!(err.contains("neexistuje"), "{err}");
}

#[test]
fn hinge_count_and_positions() {
    // pravidlo CV (findings 10 §5): Blum 110° odsadenie 100, rozostup 609,6
    let n = |h| cc_model::FrontStandard::hinge_count(h, 100.0, 609.6);
    assert_eq!(n(730.0), 2);
    assert_eq!(n(809.6), 2);
    assert_eq!(n(809.7), 3);
    assert_eq!(n(1419.2), 3);
    assert_eq!(n(1419.3), 4);
    assert_eq!(n(2028.8), 4);
    assert_eq!(n(2100.0), 5);
    // slepý záves 80: 2 do 769,6
    assert_eq!(cc_model::FrontStandard::hinge_count(769.6, 80.0, 609.6), 2);
    assert_eq!(cc_model::FrontStandard::hinge_count(769.7, 80.0, 609.6), 3);
    // nezmyselné hodnoty nespadnú
    assert_eq!(
        cc_model::FrontStandard::hinge_count(f64::NAN, 100.0, 609.6),
        2
    );
    assert_eq!(cc_model::FrontStandard::hinge_count(900.0, 100.0, 0.0), 2);
    assert_eq!(
        cc_generate::hinge_positions(1000.0, 100.0, 3),
        vec![100.0, 500.0, 900.0]
    );
    // krátke dvierka: krajné pánty nie za stredom
    assert_eq!(
        cc_generate::hinge_positions(150.0, 100.0, 2),
        vec![75.0, 75.0]
    );
}

/// Krátky výsuv (NL ≤ 350): zadné diery 197/229 (findings 10 §3.4); `drawer_length` obmedzí NL.
#[test]
fn short_runner_variant() {
    let mut loaded = load();
    let c = loaded
        .project
        .cabinets
        .iter_mut()
        .find(|c| c.id == "SP1")
        .unwrap();
    c.drawer_length = Some(320.0);
    let (m, _) = build(&loaded);
    let sp1 = m.cabinets.iter().find(|c| c.id == "SP1").unwrap();
    assert!(sp1.hardware.iter().any(|h| matches!(
        h,
        cc_model::Hardware::Drawer { length, .. } if *length == 300.0
    )));
    let side = part(&m, "SP1-SIDE-R");
    let mut ys: Vec<f64> = face_holes(side, 5.0)
        .into_iter()
        .filter(|h| {
            h.3 == 12.0 && side.transform.point_to_cabinet(DVec3::new(h.1, h.2, 0.0)).z > 500.0
        })
        .map(|h| h.2)
        .collect();
    ys.sort_by(f64::total_cmp);
    assert_eq!(ys, vec![37.0, 69.0, 197.0, 229.0]);
}

fn with_options(f: impl Fn(&mut cc_model::CabinetOptions)) -> cc_config::LoadedProject {
    let mut loaded = load();
    for c in &mut loaded.project.cabinets {
        f(&mut c.options);
    }
    loaded
}

/// Atribút „A-SPOJE TYP = K+Konfirmát“ (findings 11 §1.3): konfirmát Ø5 × 13 v boku v strede
/// každej dvojice kolíkov; súpis 20 kolíkov + 10 konfirmátov ako v CV pri spodnej skrinke.
#[test]
fn confirmat_between_dowel_pairs() {
    let loaded = with_options(|o| o.joint_type = Some(cc_model::JointType::Confirmat));
    let (m, _) = build(&loaded);
    let side = part(&m, "SP2-SIDE-L");
    let mut k: Vec<f64> = side
        .operations
        .iter()
        .filter_map(|op| match *op {
            Operation::Hole {
                y,
                diameter,
                depth,
                dowel: false,
                ..
            } if diameter == 5.0 && depth == 13.0 => Some(y),
            _ => None,
        })
        .collect();
    k.sort_by(f64::total_cmp);
    k.dedup();
    // naložený chrbát: dno 506 ako VARNA_D v CV – kolíky 32/96, 221/285, 410/474 →
    // konfirmáty 64/253/442; predný vlys 18/110 → 64, zadný vlys 396/488 → 442
    assert_eq!(k, vec![64.0, 253.0, 442.0], "{:?}", side.operations);

    let bill = cc_joints::hardware_bill(&m, &loaded.standard);
    let sp2 = m.cabinets.iter().find(|c| c.id == "SP2").unwrap();
    let lines = cc_joints::bill::cabinet_bill(sp2, &loaded.standard);
    let qty = |name: &str| {
        lines
            .iter()
            .find(|l| l.1.starts_with(name))
            .map_or(0, |l| l.2)
    };
    assert_eq!(qty("Kolík"), 20);
    assert_eq!(qty("Konfirmát"), 10);
    assert_eq!(qty("Nožička"), 4);
    assert!(bill.iter().any(|l| l.name.starts_with("Blum 110")));
}

/// Tlmiace čočky (findings 11 §1.8): dvierka pri hrane oproti pántom 9 od spodku a 39 od vrchu
/// (spodné s úchytkou UK16), čelo zásuvky > 150 štyri diery.
#[test]
fn bumpers_on_fronts() {
    let loaded = with_options(|o| o.bumpers = Some(true));
    let (m, _) = build(&loaded);
    let door = part(&m, "SP4-DOOR-1");
    let mut b: Vec<(f64, f64)> = face_holes(door, 8.0)
        .into_iter()
        .filter(|h| h.1 < 20.0 || h.1 > 680.0)
        .filter(|h| h.2 < 20.0)
        .map(|h| (h.1, h.2))
        .collect();
    b.sort_by(|a, c| a.partial_cmp(c).unwrap());
    // pánty vpravo → čočky pri ľavej hrane (y = 9); výška 730: 9 a 691
    assert_eq!(b, vec![(9.0, 9.0), (691.0, 9.0)]);
    let drawer = part(&m, "SP1-DRW-1");
    assert_eq!(face_holes(drawer, 8.0).len(), 4);
    let wall = part(&m, "HO1-DOOR-1");
    // horná skrinka: 28 od spodku (presah 19), 9 od vrchu (682 − 9 = 673)
    let xs: std::collections::BTreeSet<i64> = face_holes(wall, 8.0)
        .iter()
        .filter(|h| h.2 > 500.0)
        .map(|h| h.1 as i64)
        .collect();
    assert_eq!(xs.into_iter().collect::<Vec<_>>(), vec![28, 673]);
}

/// „Spoje skrinky = nie“ a „Policové kovanie = nič“: bez kolíkov a bez radu dier.
#[test]
fn joints_and_shelf_support_off() {
    let loaded = with_options(|o| {
        o.joints = Some(false);
        o.shelf_support = Some(cc_model::ShelfSupport::None);
    });
    let (m, _) = build(&loaded);
    let side = part(&m, "VY1-SIDE-L");
    assert!(side
        .operations
        .iter()
        .all(|op| !matches!(op, Operation::Hole { dowel: true, .. })));
    assert!(face_holes(side, 5.0).iter().all(|h| h.3 != 10.0));
    let bill = cc_joints::hardware_bill(&m, &loaded.standard);
    assert!(bill.iter().all(|l| !l.name.starts_with("Kolík")));
    assert!(bill.iter().all(|l| !l.name.contains("podperka")));
}

/// Škáry pri skrinke ako Reveals v CV (zákazka REF, skrinka 6 S_KUCH_3_SUP: hore 2, dole 0,
/// boky 0, medzi 2): čelá 600 × 150 / 303 / 303 a korpusy M / E / E ako v zostave CV.
#[test]
fn cabinet_reveals_like_cv_cabinet_6() {
    let mut loaded = load();
    let c = loaded
        .project
        .cabinets
        .iter_mut()
        .find(|c| c.id == "SP3")
        .unwrap();
    c.reveals = cc_model::FrontReveals {
        top: Some(2.0),
        bottom: Some(0.0),
        left: Some(0.0),
        right: Some(0.0),
        horizontal: Some(2.0),
        vertical: Some(2.0),
    };
    c.fronts = vec![
        FrontSpec {
            height: Some(150.0),
            ..FrontSpec::new(FrontKind::Drawer)
        },
        FrontSpec::new(FrontKind::Drawer),
        FrontSpec::new(FrontKind::Drawer),
    ];
    let (m, _) = build(&loaded);
    let sizes: Vec<(f64, f64)> = (1..=3)
        .map(|k| {
            let p = part(&m, &format!("SP3-DRW-{k}"));
            (p.width, p.length)
        })
        .collect();
    assert_eq!(sizes, vec![(600.0, 150.0), (600.0, 303.0), (600.0, 303.0)]);
    let sp3 = m.cabinets.iter().find(|c| c.id == "SP3").unwrap();
    let boxes: Vec<f64> = sp3
        .hardware
        .iter()
        .filter_map(|h| match h {
            cc_model::Hardware::Drawer { box_height, .. } => Some(*box_height),
            _ => None,
        })
        .collect();
    assert_eq!(boxes, vec![103.0, 204.0, 204.0]);
}

/// „Kolíky + vrut“: diera Ø5 × 13 iba do boku, v súpise vrut.
#[test]
fn screw_joint_in_bill() {
    let loaded = with_options(|o| o.joint_type = Some(cc_model::JointType::Screw));
    let (m, _) = build(&loaded);
    let bill = cc_joints::hardware_bill(&m, &loaded.standard);
    assert!(bill.iter().any(|l| l.name.starts_with("Vrut")), "{bill:?}");
    assert!(bill.iter().all(|l| !l.name.starts_with("Konfirmát")));
}

/// Úprava rozmeru čela pri otvore (Size Adjustments v CV): vpravo +60 ako pri blende,
/// hore −30 ako úchytka UK16; zlá úprava je chyba, nie pád.
#[test]
fn front_size_adjustments() {
    let mut loaded = load();
    let (m, _) = build(&loaded);
    let (lo0, hi0) = bbox(part(&m, "SP4-DOOR-1"));
    let c = loaded
        .project
        .cabinets
        .iter_mut()
        .find(|c| c.id == "SP4")
        .unwrap();
    c.fronts = vec![FrontSpec {
        adjust: cc_model::FrontAdjust {
            right: 60.0,
            top: -30.0,
            ..Default::default()
        },
        ..FrontSpec::new(FrontKind::Door)
    }];
    let (m, _) = build(&loaded);
    let door = part(&m, "SP4-DOOR-1");
    assert_abs_diff_eq!(door.width, 598.0 + 60.0);
    assert_abs_diff_eq!(door.length, 730.0 - 30.0);
    let (lo, hi) = bbox(door);
    assert_abs_diff_eq!(lo.x, lo0.x);
    assert_abs_diff_eq!(hi.x, hi0.x + 60.0);
    assert_abs_diff_eq!(lo.z, lo0.z);
    assert_abs_diff_eq!(hi.z, hi0.z - 30.0);

    let c = loaded
        .project
        .cabinets
        .iter_mut()
        .find(|c| c.id == "SP4")
        .unwrap();
    c.fronts[0].adjust.left = -1000.0;
    let err = cc_generate::build(&loaded.project, &loaded.standard, &loaded.materials)
        .unwrap_err()
        .to_string();
    assert!(err.contains("úprava čela"), "{err}");
}

/// Slepý roh ako z importu CV (REF, skrinka 10): škára hore pri skrinke 2, dvierka
/// s úpravou hore −30 (úchytka), panel bez úchytky → panel 760, dvierka 730.
#[test]
fn blind_panel_with_cabinet_reveals() {
    let mut loaded = load();
    let c = loaded
        .project
        .cabinets
        .iter_mut()
        .find(|c| c.id == "SP4")
        .unwrap();
    c.w = 1000.0;
    c.adjustable_shelves = 0;
    c.reveals = cc_model::FrontReveals {
        top: Some(2.0),
        left: Some(0.0),
        vertical: Some(2.0),
        ..Default::default()
    };
    let mut door = FrontSpec::new(FrontKind::Door);
    door.adjust.top = -30.0;
    let mut panel = FrontSpec::new(FrontKind::Panel);
    panel.width = Some(500.0);
    c.fronts = vec![FrontSpec {
        columns: vec![panel, door],
        ..FrontSpec::new(FrontKind::Door)
    }];
    let (m, _) = build(&loaded);
    let fronts: Vec<&Part> = m
        .parts()
        .filter(|p| p.cabinet == "SP4" && p.role.is_front())
        .collect();
    assert_eq!(fronts.len(), 2);
    let panel = fronts.iter().find(|p| p.role == Role::FalseFront).unwrap();
    let door = fronts.iter().find(|p| p.role == Role::Door).unwrap();
    // ako v CV: panel 499 × 760, dvierka 498 × 730
    assert_abs_diff_eq!(panel.length, 760.0);
    assert_abs_diff_eq!(panel.width, 499.0);
    assert_abs_diff_eq!(door.length, 730.0);
    assert_abs_diff_eq!(door.width, 498.0);
}
