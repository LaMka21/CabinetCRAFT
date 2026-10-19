//! Otvorená nika a LED rampa ako v CV REF (skrinky 7 a 8, dataset `tests/fixtures/cv-ref`):
//! diely a diery v našom ráme dielu (README datasetu, stĺpce `our_*`).

use std::path::PathBuf;

use cc_model::{CabinetSpec, Face, JointType, LedRail, Model, Operation, Part, Role};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Vzor dielne s nikou HO7 (485 × 701 × 351, 2 police) a hornou HO8 s LED rampou 1153.
fn built() -> cc_pipeline::Built {
    let project = root().join("examples/dielna/project.toml");
    let mut loaded = cc_config::load(&project).unwrap();
    let machine =
        cc_config::load_machine(&cc_pipeline::default_machine(&project).unwrap()).unwrap();
    let upper: CabinetSpec = loaded
        .project
        .cabinets
        .iter()
        .find(|c| c.id == "HO1")
        .cloned()
        .unwrap();
    let mut niche = upper.clone();
    niche.id = "HO7".into();
    (niche.w, niche.h, niche.d) = (485.0, 701.0, 351.0);
    niche.adjustable_shelves = 2;
    niche.fronts.clear();
    niche.niche = true;
    niche.options.joint_type = Some(JointType::Screw);
    let mut led = upper;
    led.id = "HO8".into();
    (led.w, led.h, led.d) = (576.5, 682.0, 330.0);
    led.led_rail = Some(LedRail {
        length: Some(1153.0),
        ..LedRail::default()
    });
    loaded.project.cabinets = vec![niche, led];
    cc_pipeline::build(
        &loaded.project,
        &loaded.standard,
        &loaded.materials,
        Some(&machine),
    )
    .unwrap()
}

fn part<'a>(model: &'a Model, id: &str) -> &'a Part {
    model.parts().find(|p| p.id == id).unwrap()
}

/// Diery dielu (plocha, x, y, priemer, hĺbka) zaokrúhlené na 0,01, zoradené.
fn holes(p: &Part) -> Vec<(Face, i64, i64, i64, i64)> {
    let r = |v: f64| (v * 100.0).round() as i64;
    let mut out: Vec<_> = p
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
            } => Some((face, r(x), r(y), r(diameter), r(depth))),
            Operation::Groove { .. } => None,
        })
        .collect();
    out.sort_by_key(|h| (format!("{:?}", h.0), h.1, h.2, h.3, h.4));
    out
}

fn expect(list: &[(Face, f64, f64, f64, f64)]) -> Vec<(Face, i64, i64, i64, i64)> {
    let r = |v: f64| (v * 100.0).round() as i64;
    let mut out: Vec<_> = list
        .iter()
        .map(|&(f, x, y, d, depth)| (f, r(x), r(y), r(d), r(depth)))
        .collect();
    out.sort_by_key(|h| (format!("{:?}", h.0), h.1, h.2, h.3, h.4));
    out
}

fn dims(p: &Part) -> (f64, f64, f64) {
    (p.length, p.width, p.thickness)
}

#[test]
fn niche_parts_like_cv_ref() {
    let b = built();
    assert!(!b.has_errors(), "{:?}", b.issues().collect::<Vec<_>>());
    assert_eq!(b.warnings.len(), 0, "{:?}", b.warnings);
    let m = &b.model;
    let cab = m.cabinets.iter().find(|c| c.id == "HO7").unwrap();
    let roles: Vec<Role> = cab.parts.iter().map(|p| p.role).collect();
    assert_eq!(
        roles,
        [
            Role::SideLeft,
            Role::SideRight,
            Role::Top,
            Role::Bottom,
            Role::FinishedBack,
            Role::AdjustableShelf,
            Role::AdjustableShelf
        ]
    );
    // 0047/0048 663 × 351, 0050/0051 485 × 351, 0049 447 × 663, police 446 × 310 – všetko MDF 19
    assert_eq!(dims(part(m, "HO7-SIDE-L")), (663.0, 351.0, 19.0));
    assert_eq!(dims(part(m, "HO7-TOP")), (485.0, 351.0, 19.0));
    assert_eq!(dims(part(m, "HO7-BOTTOM")), (485.0, 351.0, 19.0));
    assert_eq!(dims(part(m, "HO7-FBACK")), (447.0, 663.0, 19.0));
    assert_eq!(dims(part(m, "HO7-ASHELF-1")), (446.0, 310.0, 19.0));
    assert!(cab.parts.iter().all(|p| p.material == "MDF_19_W1000_biela"));
}

#[test]
fn niche_side_holes_like_cv_ref() {
    let b = built();
    let side = part(&b.model, "HO7-SIDE-L");
    let mut cv = vec![
        // spoj s chrbtom: 2 skupiny kolík–vrut–kolík (966b0047)
        (Face::F, 599.0, 322.5, 8.0, 13.0),
        (Face::F, 567.0, 322.5, 5.0, 13.0),
        (Face::F, 535.0, 322.5, 8.0, 13.0),
        (Face::F, 214.0, 322.5, 8.0, 13.0),
        (Face::F, 182.0, 322.5, 5.0, 13.0),
        (Face::F, 150.0, 322.5, 8.0, 13.0),
        // záves Ø10 × 13, 14 od zadnej hrany
        (Face::F, 56.0, 337.0, 10.0, 13.0),
        (Face::F, 88.0, 337.0, 10.0, 13.0),
        (Face::F, 120.0, 337.0, 10.0, 13.0),
    ];
    // rad dier Sekura 7 pri policiach, 53 od čela a 50 pred chrbtom
    for x in [492.667, 460.667, 428.667, 265.333, 233.333, 201.333] {
        for y in [53.0, 263.0] {
            cv.push((Face::F, x, y, 5.0, 10.0));
        }
    }
    // kolíky do hrán z vrchu (X0) a dna (X1)
    for (face, x) in [(Face::X0, 0.0), (Face::X1, 663.0)] {
        for y in [32.0, 96.0, 255.0, 319.0] {
            cv.push((face, x, y, 8.0, 26.0));
        }
    }
    assert_eq!(holes(side), expect(&cv));
}

#[test]
fn niche_back_top_bottom_holes_like_cv_ref() {
    let b = built();
    let m = &b.model;
    let mut back = Vec::new();
    for (face, y) in [(Face::Y0, 0.0), (Face::Y1, 663.0)] {
        for x in [32.0, 96.0, 191.5, 255.5, 351.0, 415.0] {
            back.push((face, x, y, 8.0, 26.0));
        }
    }
    for (face, x) in [(Face::X0, 0.0), (Face::X1, 447.0)] {
        for y in [64.0, 128.0, 449.0, 513.0] {
            back.push((face, x, y, 8.0, 26.0));
        }
    }
    assert_eq!(holes(part(m, "HO7-FBACK")), expect(&back));

    // vrch a dno: kolíky Ø8 × 13 k bokom a k chrbtu, bez vrutov (pohľadové plochy)
    let mut top = Vec::new();
    for x in [9.5, 475.5] {
        for y in [32.0, 96.0, 255.0, 319.0] {
            top.push((Face::F, x, y, 8.0, 13.0));
        }
    }
    for x in [51.0, 115.0, 210.5, 274.5, 370.0, 434.0] {
        top.push((Face::F, x, 322.5, 8.0, 13.0));
    }
    assert_eq!(holes(part(m, "HO7-TOP")), expect(&top));
    let bottom = part(m, "HO7-BOTTOM");
    assert_eq!(holes(bottom), expect(&top));
    // drážka pre LED zospodu 71 … 85 od čela (966b0051)
    let grooves: Vec<&Operation> = bottom
        .operations
        .iter()
        .filter(|o| matches!(o, Operation::Groove { .. }))
        .collect();
    assert_eq!(grooves.len(), 1);
    if let Operation::Groove {
        face,
        start,
        end,
        width,
        depth,
    } = grooves[0]
    {
        assert_eq!(*face, Face::B);
        assert_eq!((start.y, end.y, *width, *depth), (78.0, 78.0, 14.0, 13.0));
        assert_eq!((start.x, end.x), (0.0, 485.0));
    }
    // polica: Ø5 × 10 zospodu 9,5 od koncov, 50 od čela a od zadu
    let shelf = part(m, "HO7-ASHELF-1");
    let mut pins = Vec::new();
    for x in [9.5, 436.5] {
        for y in [50.0, 260.0] {
            pins.push((Face::B, x, y, 5.0, 10.0));
        }
    }
    assert_eq!(holes(shelf), expect(&pins));
}

#[test]
fn led_rail_under_upper_cabinet() {
    let b = built();
    let rail = part(&b.model, "HO8-LED");
    // 0065 „NES_LED RAMPA“ 1153 × 310 × 19, pod dnom pri stene (y 20 … 330, z −19 … 0)
    assert_eq!(dims(rail), (1153.0, 310.0, 19.0));
    assert_eq!(rail.role, Role::LedRail);
    let o = rail.transform.point_to_cabinet(glam::DVec3::ZERO);
    assert_eq!((o.x, o.y, o.z), (0.0, 20.0, -19.0));
    assert!(holes(rail).is_empty());
    // drážka 14 × 13 zospodu, Z 266 … 280 od steny = 30 … 44 od prednej hrany rampy
    assert!(matches!(
        rail.operations.as_slice(),
        [Operation::Groove { face: Face::B, start, width, depth, .. }]
            if start.y == 37.0 && *width == 14.0 && *depth == 13.0
    ));
}
