//! Slepý roh ako v CV REF skr. 10 `S_KUC_ROH_P` (findings 11 §5.2): bok pri paneli vystupuje
//! 3 dopredu, panel je spojený s prednou hranou boku a cez medzeru s prednou hranou dna.

use std::path::PathBuf;

use approx::assert_abs_diff_eq;
use cc_model::{Face, FrontKind, FrontSpec, JointType, Model, Operation, Part, Role, TopStyle};
use glam::DVec3;

fn load() -> cc_config::LoadedProject {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/dielna/kuchyna.toml");
    cc_config::load(&path).unwrap()
}

/// SP4 prerobená na roh 1000 × 762 × 392 ako v REF (`panel_left` = panel vľavo, `_P`).
fn corner(loaded: &mut cc_config::LoadedProject, panel_left: bool) {
    let c = loaded
        .project
        .cabinets
        .iter_mut()
        .find(|c| c.id == "SP4")
        .unwrap();
    c.w = 1000.0;
    c.d = 392.0;
    c.top_style = Some(TopStyle::Panel);
    c.options.joint_type = Some(JointType::Confirmat);
    c.reveals = cc_model::FrontReveals {
        top: Some(2.0),
        left: Some(0.0),
        right: Some(0.0),
        vertical: Some(2.0),
        ..Default::default()
    };
    let mut door = FrontSpec::new(FrontKind::Door);
    door.adjust.top = -30.0;
    let mut panel = FrontSpec::new(FrontKind::Panel);
    panel.width = Some(500.0);
    let columns = if panel_left {
        vec![panel, door]
    } else {
        door.width = Some(500.0);
        vec![door, FrontSpec::new(FrontKind::Panel)]
    };
    c.fronts = vec![FrontSpec {
        columns,
        ..FrontSpec::new(FrontKind::Door)
    }];
}

fn build(loaded: &cc_config::LoadedProject) -> Model {
    let model = cc_generate::build(&loaded.project, &loaded.standard, &loaded.materials).unwrap();
    cc_joints::apply(model, &loaded.standard).unwrap().model
}

fn part<'a>(m: &'a Model, id: &str) -> &'a Part {
    m.parts().find(|p| p.id == id).unwrap()
}

/// (strana, x, y, priemer, hĺbka) všetkých dier dielu, zoradené.
fn holes(p: &Part) -> Vec<(Face, f64, f64, f64, f64)> {
    let mut v: Vec<_> = p
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
            } => Some((face, x, y, diameter, depth)),
            _ => None,
        })
        .collect();
    v.sort_by(|a, b| {
        (a.0 as u8, a.1, a.2)
            .partial_cmp(&(b.0 as u8, b.1, b.2))
            .unwrap()
    });
    v
}

fn front_y(p: &Part) -> f64 {
    let a = p.transform.point_to_cabinet(DVec3::ZERO);
    let b = p
        .transform
        .point_to_cabinet(DVec3::new(p.length, p.width, p.thickness));
    a.y.min(b.y)
}

#[test]
fn blind_side_and_panel_joints_like_cv_ref() {
    let mut loaded = load();
    corner(&mut loaded, true);
    let m = build(&loaded);
    let side_l = part(&m, "SP4-SIDE-L");
    let side_r = part(&m, "SP4-SIDE-R");
    // LU 391 od Z 4 po Z 395 (3 pred čelom korpusu), RU 388
    assert_abs_diff_eq!(side_l.width, 391.0);
    assert_abs_diff_eq!(front_y(side_l), -3.0);
    assert_abs_diff_eq!(side_r.width, 388.0);
    assert_abs_diff_eq!(front_y(side_r), 0.0);

    // kolíky dna a vrchu v boku pri paneli o 3 ďalej od hrany boku (35/99/165/229/295/359)
    let f8: Vec<f64> = holes(side_l)
        .iter()
        .filter(|h| h.0 == Face::F && h.3 == 8.0 && (h.1 - 752.5).abs() < 1e-6)
        .map(|h| h.2)
        .collect();
    assert_eq!(f8, vec![35.0, 99.0, 165.0, 229.0, 295.0, 359.0]);

    // predná hrana boku: 6 kolíkov Ø8 × 26 v strede hrúbky
    let edge: Vec<(f64, f64)> = holes(side_l)
        .iter()
        .filter(|h| h.0 == Face::Y0)
        .map(|h| (h.1, h.4))
        .collect();
    let xs: Vec<f64> = edge.iter().map(|e| e.0).collect();
    assert_eq!(xs, vec![32.0, 96.0, 349.0, 413.0, 666.0, 730.0]);
    assert!(edge.iter().all(|e| e.1 == 26.0));

    // panel 499 × 760 od X 0: k boku Ø8 × 13 a K Ø5 × 13 9 od kraja, k dnu Ø8 × 13 9,5 nad spodkom
    let panel = m
        .parts()
        .find(|p| p.cabinet == "SP4" && p.role == Role::FalseFront)
        .unwrap();
    assert_abs_diff_eq!(panel.width, 499.0);
    let ph = holes(panel);
    let at = |d: f64, y: f64| -> Vec<f64> {
        ph.iter()
            .filter(|h| h.0 == Face::F && h.3 == d && (h.2 - y).abs() < 1e-6)
            .map(|h| h.1)
            .collect()
    };
    assert_eq!(at(8.0, 9.0), vec![32.0, 96.0, 349.0, 413.0, 666.0, 730.0]);
    assert_eq!(at(5.0, 9.0), vec![64.0, 381.0, 698.0]);
    let low: Vec<f64> = ph
        .iter()
        .filter(|h| h.3 == 8.0 && (h.1 - 9.5).abs() < 1e-6)
        .map(|h| h.2)
        .collect();
    assert_eq!(low, vec![50.0, 114.0, 368.0, 432.0]);
    assert!(ph.iter().filter(|h| h.3 == 8.0).all(|h| h.4 == 13.0));

    // predná hrana dna: Ø8 × 26 na 32/96/350/414 od ľavého boku
    let bottom = part(&m, "SP4-BOTTOM");
    let b: Vec<(f64, f64)> = holes(bottom)
        .iter()
        .filter(|h| h.0 == Face::Y0)
        .map(|h| (h.1, h.4))
        .collect();
    assert_eq!(
        b,
        vec![(32.0, 26.0), (96.0, 26.0), (350.0, 26.0), (414.0, 26.0)]
    );
    // vrch sa s panelom nespája
    assert!(holes(part(&m, "SP4-TOP")).iter().all(|h| h.0 != Face::Y0));
}

#[test]
fn mirrored_corner_extends_right_side() {
    let mut loaded = load();
    corner(&mut loaded, false);
    let m = build(&loaded);
    assert_abs_diff_eq!(part(&m, "SP4-SIDE-L").width, 388.0);
    let side_r = part(&m, "SP4-SIDE-R");
    assert_abs_diff_eq!(side_r.width, 391.0);
    assert_abs_diff_eq!(front_y(side_r), -3.0);
    assert_eq!(holes(side_r).iter().filter(|h| h.0 == Face::Y0).count(), 6);
    // dno 964: kolíky od pravého boku
    let mut b: Vec<f64> = holes(part(&m, "SP4-BOTTOM"))
        .iter()
        .filter(|h| h.0 == Face::Y0)
        .map(|h| h.1)
        .collect();
    b.sort_by(f64::total_cmp);
    assert_eq!(b, vec![550.0, 614.0, 868.0, 932.0]);
}

#[test]
fn without_standard_values_nothing_changes() {
    let mut loaded = load();
    corner(&mut loaded, true);
    loaded.standard.blind_corner = Default::default();
    let m = build(&loaded);
    assert_abs_diff_eq!(part(&m, "SP4-SIDE-L").width, 388.0);
    let panel = m
        .parts()
        .find(|p| p.cabinet == "SP4" && p.role == Role::FalseFront)
        .unwrap();
    // iba krížová podložka slepého pántu
    assert!(holes(panel).iter().all(|h| h.3 == 5.0 && h.4 == 12.0));
    assert!(holes(part(&m, "SP4-BOTTOM"))
        .iter()
        .all(|h| h.0 != Face::Y0));
}

#[test]
fn hostile_blind_corner_values_do_not_break() {
    for ext in [f64::NAN, f64::INFINITY, -5.0, 1e9] {
        let mut loaded = load();
        corner(&mut loaded, true);
        loaded.standard.blind_corner.side_extend = ext;
        loaded.standard.blind_corner.panel_bottom_dowels = vec![f64::NAN, -1.0, 0.0, 1e12, 32.0];
        let m = build(&loaded);
        let w = part(&m, "SP4-SIDE-L").width;
        // nikdy viac ako po panel (medzera čiel 3)
        assert!(w == 388.0 || w == 391.0, "{ext}: {w}");
        // z neplatných polôh dna ostane iba 32 (ak sa panel spojil)
        let b: Vec<f64> = holes(part(&m, "SP4-BOTTOM"))
            .iter()
            .filter(|h| h.0 == Face::Y0)
            .map(|h| h.1)
            .collect();
        assert!(b.is_empty() || b == vec![32.0], "{ext}: {b:?}");
    }
}
