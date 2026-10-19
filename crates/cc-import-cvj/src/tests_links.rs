//! Testy prevodu spojovacieho kovania skriniek medzi sebou (atribút ľavého boku,
//! findings 12 §8): syntetická úloha a voliteľne zákazka dielne REF.

use std::path::PathBuf;

use cc_model::{CabinetSpec, Defaults, LinkHole};

use super::*;
use crate::classes::tests::{face, job_head, part, wall, IN, W};

fn defaults() -> Defaults {
    Defaults {
        panel_material: "P".into(),
        back_material: "B".into(),
        construction: "C".into(),
    }
}

fn spec<'a>(p: &'a cc_model::ProjectFile, id: &str) -> &'a CabinetSpec {
    p.cabinets.iter().find(|c| c.id == id).unwrap()
}

/// Spodná skrinka 600 × 820 × 500 (sokel 100) s korpusom, ktorého ľavý bok `LU` nesie
/// celočíselné a dĺžkové parametre (mm).
fn cab_with_left_side(w: &mut W, no: i32, x: f64, ints: &[(&str, i32)], lens: &[(&str, f64)]) {
    w.tag("CVCabinet").asm_head(100 + no as u32, "S_TEST", 9, 8);
    for (n, v) in [
        ("X", x),
        ("Y", 0.0),
        ("Z", 0.0),
        ("DX", 600.0),
        ("DY", 820.0),
        ("DZ", 500.0),
        ("TOEH", 100.0),
    ] {
        w.param_f64(n, 1, v / IN);
    }
    w.param_int("CABNO", no);
    w.asm_tail(1, 1);
    w.u16(0xEA01).u16(0).u32(0).u32(0).p3();
    w.u16(0xEA01).i32(0).i32(0);
    w.u32(0).u32(0).u16(0).p3().p3().u32(0);
    w.children(1);
    // korpus: LU s parametrami spojenia + RU, DE, TS
    w.tag("CVAssembly").asm_base(0, "Case", 14, 1, 0, &[]);
    w.u16(0xEA01).children(4);
    w.tag("CVAssembly")
        .asm_head(0, "LU", 16, (7 + ints.len() + lens.len()) as u16);
    for (n, t, v) in [
        ("X", 1, 18.0 / IN),
        ("Y", 1, 100.0 / IN),
        ("Z", 1, 0.0),
        ("DX", 1, 500.0 / IN),
        ("DY", 1, 720.0 / IN),
        ("DZ", 1, 18.0 / IN),
        ("AY", 2, -90.0),
    ] {
        w.param_f64(n, t, v);
    }
    for (n, v) in ints {
        w.param_int(n, *v);
    }
    for (n, v) in lens {
        w.param_f64(n, 1, v / IN);
    }
    w.asm_tail(1, 0);
    w.u16(0xEA01).children(0);
    part(w, "RU", [600.0, 100.0, 0.0], [500.0, 720.0, 18.0], -90.0);
    part(w, "DE", [18.0, 100.0, 0.0], [564.0, 18.0, 500.0], 0.0);
    part(w, "TS", [18.0, 802.0, 0.0], [564.0, 18.0, 500.0], 0.0);
}

/// 4 skrinky vedľa seba: 1) „Zap/Edit“ s ručnými polohami (jedna diera vypnutá) a posunom,
/// 2) zapnuté (variant `_zap_1`), 3) vypnuté, 4) bez atribútu.
fn links_job() -> Vec<u8> {
    let mut w = W::new();
    job_head(&mut w, false, 1);
    wall(&mut w, 1, 0.0, -1000.0, 6000.0, 0.0);
    face(&mut w, 1, false, 4, &[]);
    cab_with_left_side(
        &mut w,
        1,
        0.0,
        &[
            ("Spojovacie_kovanie_zap_2", 2),
            ("Spojkov_F_on_1", 1),
            ("Spojkov_F_on_2", 1),
            ("Spojkov_B_on_1", 1),
            ("Spojkov_B_on_2", 0),
        ],
        &[
            ("Posun_vrtania", 3.0),
            ("Spojkov_F_X_1", 100.0),
            ("Spojkov_F_Y_1", 100.0),
            ("Spojkov_F_X_2", 100.0),
            ("Spojkov_F_Y_2", 702.0),
            ("Spojkov_B_X_1", 406.0),
            ("Spojkov_B_Y_1", 100.0),
            ("Spojkov_B_X_2", 406.0),
            ("Spojkov_B_Y_2", 702.0),
        ],
    );
    cab_with_left_side(
        &mut w,
        2,
        600.0,
        &[("Spojovacie_kovanie_zap_1", 1), ("Spojkov_F_on_1", 1)],
        &[
            ("Posun_vrtania", 0.0),
            ("Spojkov_F_X_1", 100.0),
            ("Spojkov_F_Y_1", 80.0),
        ],
    );
    cab_with_left_side(
        &mut w,
        3,
        1200.0,
        &[("Spojovacie_kovanie_zap_2", 0)],
        &[("Posun_vrtania", 0.0)],
    );
    cab_with_left_side(&mut w, 4, 1800.0, &[], &[]);
    face(&mut w, 1, true, 0, &[]);
    w.buf
}

#[test]
fn left_side_link_attribute_is_imported() {
    let job = parse_contents(&links_job()).unwrap();
    assert!(job.consumed_all);
    let cabs = &job.rooms[0].cabinets;
    assert_eq!(cabs.len(), 4);
    let l1 = cabs[0].attributes.link.as_ref().unwrap();
    assert_eq!(l1.state, 2);
    assert!((l1.shift.unwrap() - 3.0).abs() < 1e-9);
    // B 2 je vypnutá
    assert_eq!(l1.holes.len(), 3);
    assert_eq!(cabs[1].attributes.link.as_ref().unwrap().state, 1);
    assert_eq!(cabs[2].attributes.link.as_ref().unwrap().state, 0);
    assert!(cabs[3].attributes.link.is_none());

    let imported = to_project_detailed(&job, &defaults());
    let p = &imported.project;
    let o1 = &spec(p, "SP1").options;
    assert_eq!(o1.link_left, Some(true));
    assert_eq!(o1.link_shift, Some(3.0));
    let holes: Vec<(f64, f64)> = o1
        .link_holes
        .iter()
        .map(|h| {
            (
                (h.from_front * 1e6).round() / 1e6,
                (h.from_bottom * 1e6).round() / 1e6,
            )
        })
        .collect();
    assert_eq!(holes, [(100.0, 100.0), (100.0, 702.0), (406.0, 100.0)]);
    // režim 1: polohy podľa štandardu, posun 0 sa nezapíše
    let o2 = &spec(p, "SP2").options;
    assert_eq!(o2.link_left, Some(true));
    assert!(o2.link_holes.is_empty());
    assert_eq!(o2.link_shift, None);
    let o3 = &spec(p, "SP3").options;
    assert_eq!((o3.link_left, o3.link_shift), (Some(false), None));
    assert_eq!(spec(p, "SP4").options.link_left, None);
    let notes = imported.notes.join("\n");
    // diery vŕta cc-pipeline, import už neupozorňuje, že chýbajú
    assert!(!notes.contains("nevznikajú"), "{notes}");
    // TOML sa dá zapísať a prečítať späť
    let text = toml::to_string(p).unwrap();
    let back: cc_model::ProjectFile = toml::from_str(&text).unwrap();
    assert_eq!(&back, p);

    // so štandardom (predvolene vypnuté) sa „vypnuté“ nezapíše
    let text = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/dielna/standards/dielna-cx100.toml"),
    )
    .unwrap();
    let standard: cc_model::Standard = toml::from_str(&text).unwrap();
    assert!(!standard.cabinet_options.link_left);
    let p = to_project_with(&job, &defaults(), Some(&standard)).project;
    assert_eq!(spec(&p, "SP3").options.link_left, None);
    assert_eq!(spec(&p, "SP1").options.link_left, Some(true));
}

#[test]
fn hostile_link_holes_are_filtered() {
    let mut o = cc_model::CabinetOptions::default();
    let mut notes = Vec::new();
    let link = CvjLink {
        state: 2,
        shift: Some(f64::NAN),
        holes: (0..40)
            .map(|i| (f64::from(i), 80.0))
            .chain([(f64::INFINITY, 1.0), (-5.0, 1.0)])
            .collect(),
    };
    crate::features::link_options(Some(&link), &mut o, "x", &mut notes);
    assert_eq!(o.link_left, Some(true));
    assert_eq!(o.link_shift, None);
    assert_eq!(o.link_holes.len(), 12);
    assert_eq!(
        o.link_holes[0],
        LinkHole {
            from_front: 0.0,
            from_bottom: 80.0
        }
    );
    // neznámy stav: nič, iba poznámka
    let mut o = cc_model::CabinetOptions::default();
    let link = CvjLink {
        state: 7,
        ..Default::default()
    };
    crate::features::link_options(Some(&link), &mut o, "x", &mut notes);
    assert!(o.is_empty());
    assert!(notes.iter().any(|n| n.contains("neznámy stav")));
}

/// Voliteľný test na zákazke dielne REF: cesta v `CABINETCRAFT_CVJ_REF` (mená zákazníkov
/// do repa nepatria). Bez premennej alebo bez súboru sa preskočí.
#[test]
fn optional_ref_links() {
    let Some(path) = std::env::var_os("CABINETCRAFT_CVJ_REF").map(PathBuf::from) else {
        return;
    };
    if !path.exists() {
        return;
    }
    let job = read_job(&path).unwrap();
    let p = to_project_detailed(&job, &defaults()).project;
    // findings 12 §8.2: zapnuté pri 4 (ručne 100 / 702), 9, 11, 13
    for id in ["SP4", "HO9", "SP11", "SP13"] {
        assert_eq!(spec(&p, id).options.link_left, Some(true), "{id}");
    }
    for id in ["SP1", "SP3", "HO6", "HO7", "HO8", "SP10"] {
        assert_eq!(spec(&p, id).options.link_left, Some(false), "{id}");
    }
    let holes: Vec<(i64, i64)> = spec(&p, "SP4")
        .options
        .link_holes
        .iter()
        .map(|h| (h.from_front.round() as i64, h.from_bottom.round() as i64))
        .collect();
    assert_eq!(holes, [(100, 100), (100, 702), (406, 100), (406, 702)]);
    assert!(spec(&p, "SP11").options.link_holes.is_empty());
    assert!(p.cabinets.iter().all(|c| c.options.link_shift.is_none()));
}
