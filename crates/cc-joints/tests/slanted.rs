//! Šikmá skrinka (CV REF skrinky 1 a 6, `tests/fixtures/cv-ref`): rozmery dielov, spoje
//! s pohľadovým chrbtom, drážka chrbta v šikmom boku, zrkadlenie a odmietnutia.

use std::path::PathBuf;

use approx::assert_abs_diff_eq;
use cc_model::{CabinetSpec, Face, Model, Operation, Part, Role};
use glam::DVec3;

fn load() -> cc_config::LoadedProject {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/dielna/kuchyna.toml");
    cc_config::load(&path).unwrap()
}

/// CV REF skrinka 1 `S_KUCH_D SIKMA` (hodnoty z importu `.cvj`).
const BASE: &str = r#"
id = "SP1"
type = "base"
w = 642.968
h = 762
d = 510
fronts = [{ kind = "door", hinge_side = "right" }]
reveals = { left = 2.0, right = 0.1, top = 33.0, bottom = 0.0 }
options = { joint_type = "confirmat" }
slant = { bottom = -0.2654, top = 449.0224, extend_back = 60.0, finished_back = true }
"#;

/// CV REF skrinka 6 `S_KUCH_NASTENNA SIKMA`: trojuholník bez vrchu.
const WALL: &str = r#"
id = "HO6"
type = "wall"
w = 402.75
h = 682
d = 330
fronts = [{ kind = "door", hinge_side = "right" }]
slant = { top = 402.7507 }
"#;

fn spec(text: &str) -> CabinetSpec {
    toml::from_str(text).unwrap()
}

fn build_with(specs: Vec<CabinetSpec>) -> Result<Model, String> {
    let mut loaded = load();
    loaded.project.cabinets = specs;
    loaded.project.walls.clear();
    loaded.project.openings.clear();
    let model = cc_generate::build(&loaded.project, &loaded.standard, &loaded.materials)
        .map_err(|e| e.to_string())?;
    cc_joints::apply(model, &loaded.standard)
        .map(|a| a.model)
        .map_err(|e| e.to_string())
}

fn build(text: &str) -> Model {
    build_with(vec![spec(text)]).unwrap()
}

fn part<'a>(m: &'a Model, id: &str) -> &'a Part {
    m.parts()
        .find(|p| p.id == id)
        .unwrap_or_else(|| panic!("chýba {id}"))
}

fn corners(p: &Part) -> Vec<DVec3> {
    let mut out = Vec::new();
    for x in [0.0, p.length] {
        for y in [0.0, p.width] {
            for z in [0.0, p.thickness] {
                out.push(p.transform.point_to_cabinet(DVec3::new(x, y, z)));
            }
        }
    }
    out
}

fn bbox(p: &Part) -> (DVec3, DVec3) {
    let c = corners(p);
    let lo = c.iter().fold(DVec3::splat(f64::INFINITY), |a, v| a.min(*v));
    let hi = c
        .iter()
        .fold(DVec3::splat(f64::NEG_INFINITY), |a, v| a.max(*v));
    (lo, hi)
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
            _ => None,
        })
        .collect()
}

fn assert_near(got: &[f64], want: &[f64]) {
    assert_eq!(got.len(), want.len(), "{got:?}");
    for (g, w) in got.iter().zip(want) {
        assert_abs_diff_eq!(*g, *w, epsilon = 0.01);
    }
}

fn size(p: &Part) -> (f64, f64, f64) {
    (p.length, p.width, p.thickness)
}

fn assert_size(p: &Part, l: f64, w: f64, t: f64) {
    assert_abs_diff_eq!(p.length, l, epsilon = 0.01);
    assert_abs_diff_eq!(p.width, w, epsilon = 0.01);
    assert_abs_diff_eq!(p.thickness, t, epsilon = 0.01);
}

#[test]
fn base_slanted_matches_cv_ref_cabinet_1() {
    let m = build(BASE);
    let cab = &m.cabinets[0];
    // [P] 0025 DE 603,18 × 492, 0026 RU 762 × 492, 0027 TO 164,5 × 492, 0028 LF 895,8 × 570 × 19,
    // 0029 FB 621,18 × 762 × 18, 0032 dvierka 640,88 × 729
    assert_size(part(&m, "SP1-BOTTOM"), 603.177, 492.0, 18.0);
    assert_size(part(&m, "SP1-SIDE-R"), 762.0, 492.0, 18.0);
    assert_size(part(&m, "SP1-TOP"), 164.502, 492.0, 18.0);
    let lf = part(&m, "SP1-SIDE-L");
    assert_size(lf, 895.795, 570.0, 19.0);
    assert_eq!(lf.material, "MDF_19_W1000_biela");
    assert_size(part(&m, "SP1-FBACK"), 621.177, 762.0, 18.0);
    let door = part(&m, "SP1-DOOR-1");
    assert_abs_diff_eq!(door.length, 729.0, epsilon = 0.01);
    assert_abs_diff_eq!(door.width, 640.868, epsilon = 0.01);
    assert!(cab.parts.iter().all(|p| p.role != Role::Back));

    // poloha ako v CV (rám skrinky CV posunutý o sokel 100, Z = 510 − y)
    let (lo, hi) = bbox(part(&m, "SP1-BOTTOM"));
    assert_abs_diff_eq!(lo.x, 21.791, epsilon = 0.01);
    assert_abs_diff_eq!(hi.x, 624.968, epsilon = 0.01);
    let (lo, _) = bbox(part(&m, "SP1-TOP"));
    assert_abs_diff_eq!(lo.x, 460.466, epsilon = 0.01);
    assert_abs_diff_eq!(lo.z, 744.0, epsilon = 0.01);
    // šikmý bok: obdĺžnik pred zrezaním, vnútorný roh pri vrchu na 762, vonkajší pri spodku na 0
    let (lo, hi) = bbox(lf);
    assert_abs_diff_eq!(lo.x, -0.265, epsilon = 0.01);
    assert_abs_diff_eq!(lo.z, -9.65, epsilon = 0.01);
    assert_abs_diff_eq!(hi.z, 771.65, epsilon = 0.01);
    assert_abs_diff_eq!(hi.x, 471.079, epsilon = 0.01);
    assert_abs_diff_eq!(hi.y, 570.0, epsilon = 0.01);
    assert!(lf.transform.is_right_handed_orthonormal());
    // F šikmého boku je k interiéru (normála doprava nadol)
    assert!(lf.transform.z_axis.x > 0.8 && lf.transform.z_axis.z < -0.5);
    // šikmý bok nemá v CV ani jednu dieru
    assert!(lf.operations.is_empty(), "{:?}", lf.operations);

    // [P] 0029 FB: 21 dier – k dnu kolíky 32/96/269,6/333,6/507,2/571,2 + konfirmáty 64/301,6/539,2,
    // k boku 32/96/349/413/666/730 + 64/381/698, k vrchu 470,7/571,2 + 520,9 (od ľavého konca chrbta)
    let fb = part(&m, "SP1-FBACK");
    let fb_holes = holes(fb);
    assert_eq!(fb_holes.len(), 21, "{fb_holes:?}");
    assert!(fb_holes.iter().all(|h| h.0 == Face::F));
    let at_bottom: Vec<f64> = {
        let mut v: Vec<f64> = fb_holes
            .iter()
            .filter(|h| (h.2 - 9.0).abs() < 0.01)
            .map(|h| (h.1 * 1000.0).round() / 1000.0)
            .collect();
        v.sort_by(f64::total_cmp);
        v
    };
    assert_near(
        &at_bottom,
        &[
            32.0, 64.0, 96.0, 269.589, 301.589, 333.589, 507.177, 539.177, 571.177,
        ],
    );
    let to_top: Vec<f64> = {
        let mut v: Vec<f64> = fb_holes
            .iter()
            .filter(|h| (h.2 - 753.0).abs() < 0.01)
            .map(|h| (h.1 * 1000.0).round() / 1000.0)
            .collect();
        v.sort_by(f64::total_cmp);
        v
    };
    assert_near(&to_top, &[470.675, 520.926, 571.177]);
    // dno: kolíky do zadnej hrany (k chrbtu) aj do pravej hrany (k boku), k šikmému boku nič
    let de = holes(part(&m, "SP1-BOTTOM"));
    assert_eq!(de.len(), 12, "{de:?}");
    assert!(de.iter().all(|h| h.0 != Face::X0));
    // vrch: 6 kolíkov k boku a 2 k chrbtu
    assert_eq!(holes(part(&m, "SP1-TOP")).len(), 8);
}

#[test]
fn wall_slanted_matches_cv_ref_cabinet_6() {
    let m = build(WALL);
    // [P] 0055 RU 682 × 309,5, 0056 LF 792,04 × 330 × 19, 0057 UB 382,77 × 648,17 (bez vrchu)
    assert!(m.parts().all(|p| p.role != Role::Top));
    assert_size(part(&m, "HO6-SIDE-R"), 682.0, 309.5, 18.0);
    let lf = part(&m, "HO6-SIDE-L");
    assert_size(lf, 792.043, 330.0, 19.0);
    let back = part(&m, "HO6-BACK");
    assert_abs_diff_eq!(back.length, 382.774, epsilon = 0.01);
    assert_abs_diff_eq!(back.width, 648.174, epsilon = 0.01);
    let (lo, hi) = bbox(back);
    assert_abs_diff_eq!(lo.x, 18.976, epsilon = 0.01);
    assert_abs_diff_eq!(lo.z, 10.5, epsilon = 0.01);
    assert_abs_diff_eq!(hi.x, 401.75, epsilon = 0.01);
    // dno od šikmého boku po rovný bok cez celú hĺbku (CV 363,55: dno v CV zasahuje 1,16 do boku)
    assert_size(part(&m, "HO6-BOTTOM"), 362.684, 330.0, 18.0);
    // drážka chrbta v šikmom boku: plocha F, os 18,5 od zadnej hrany, šírka 4, hĺbka 9
    let grooves: Vec<&Operation> = lf
        .operations
        .iter()
        .filter(|o| matches!(o, Operation::Groove { .. }))
        .collect();
    assert_eq!(grooves.len(), 1, "{:?}", lf.operations);
    if let Operation::Groove {
        face,
        start,
        end,
        width,
        depth,
    } = grooves[0]
    {
        assert_eq!(*face, Face::F);
        assert_abs_diff_eq!(start.y, 330.0 - 18.5, epsilon = 0.01);
        assert_abs_diff_eq!(*width, 4.0, epsilon = 0.01);
        assert_abs_diff_eq!(*depth, 9.0, epsilon = 0.01);
        // pozdĺž šikmej hrany chrbta (v CV rez 18,5 … 771,3 z 792)
        assert!(start.x > 15.0 && start.x < 25.0, "{start:?}");
        assert!(end.x > 765.0 && end.x < 775.0, "{end:?}");
    }
    // chrbát v drážke dna (dno ju dostane podľa pravidla spoja)
    assert!(part(&m, "HO6-BOTTOM")
        .operations
        .iter()
        .any(|o| matches!(o, Operation::Groove { .. })));
}

#[test]
fn right_slant_is_mirror_of_left() {
    let left = build(BASE);
    let mut right_spec = spec(BASE);
    if let Some(s) = right_spec.slant.as_mut() {
        s.side = cc_model::SlantSide::Right;
    }
    right_spec.fronts[0].hinge_side = Some(cc_model::HingeSide::Left);
    let right = build_with(vec![right_spec]).unwrap();
    let w = 642.968;
    for (l_id, r_id) in [
        ("SP1-SIDE-L", "SP1-SIDE-R"),
        ("SP1-SIDE-R", "SP1-SIDE-L"),
        ("SP1-BOTTOM", "SP1-BOTTOM"),
        ("SP1-TOP", "SP1-TOP"),
        ("SP1-FBACK", "SP1-FBACK"),
    ] {
        let (a, b) = (part(&left, l_id), part(&right, r_id));
        assert_eq!(size(a), size(b), "{l_id}");
        assert!(b.transform.is_right_handed_orthonormal(), "{r_id}");
        let mirrored: Vec<DVec3> = corners(a)
            .into_iter()
            .map(|p| DVec3::new(w - p.x, p.y, p.z))
            .collect();
        for c in corners(b) {
            assert!(
                mirrored.iter().any(|m| (*m - c).length() < 1e-6),
                "{r_id}: roh {c:?} nie je zrkadlom"
            );
        }
        // F ostáva k interiéru: rovnaký počet dier v plochách
        assert_eq!(holes(a).len(), holes(b).len(), "{l_id}");
    }
}

#[test]
fn slanted_rejects_shelves_and_hinges_on_slanted_side() {
    let mut s = spec(BASE);
    s.adjustable_shelves = 1;
    let e = build_with(vec![s]).unwrap_err();
    assert!(e.contains("police"), "{e}");
    let mut s = spec(BASE);
    s.fronts[0].hinge_side = Some(cc_model::HingeSide::Left);
    let e = build_with(vec![s]).unwrap_err();
    assert!(e.contains("pánty"), "{e}");
    // nezmyselný sklon (bok sa rozširuje nahor) a nekonečné hodnoty: chyba, nie pád
    for slant in [
        "slant = { bottom = 100.0, top = 0.0 }",
        "slant = { top = 5000.0 }",
        "slant = { top = 449.0, extend_back = -5.0 }",
    ] {
        let text = BASE.replace(
            "slant = { bottom = -0.2654, top = 449.0224, extend_back = 60.0, finished_back = true }",
            slant,
        );
        assert!(build_with(vec![spec(&text)]).is_err(), "{slant}");
    }
    let mut s = spec(BASE);
    if let Some(sl) = s.slant.as_mut() {
        sl.top = f64::NAN;
    }
    assert!(build_with(vec![s]).is_err());
}
