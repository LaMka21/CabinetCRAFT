//! Testy prevodu sokla cez zostavu, bočného sokla a krycieho boku (findings 12 §6):
//! syntetická úloha a voliteľne zákazka dielne REF.

use std::path::PathBuf;

use cc_model::{CabinetSpec, Defaults, PlinthSpan};

use super::*;
use crate::classes::tests::{case_group, face, job_head, wall, IN, W};

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

/// Spodná skrinka 600 × 820 × 500 (sokel 100) s korpusom a `loose` samostatnými dielmi.
fn cab(w: &mut W, no: i32, x: f64, loose: u16) {
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
    w.children(1 + loose);
    case_group(w);
}

/// Samostatný diel skrinky (mimo skupín) s kódom a popisom, bez natočenia.
fn loose(w: &mut W, code: &str, desc: &str, pos: [f64; 3], size: [f64; 3]) {
    w.tag("CVAssembly");
    w.cv_base(0, code, desc, 16);
    w.u16(0xEA01).u16(6);
    for (n, v) in [
        ("X", pos[0]),
        ("Y", pos[1]),
        ("Z", pos[2]),
        ("DX", size[0]),
        ("DY", size[1]),
        ("DZ", size[2]),
    ] {
        w.param_f64(n, 1, v / IN);
    }
    w.asm_tail(1, 0);
    w.u16(0xEA01).children(0);
}

/// 1) sokel cez obe skrinky od X −60 a bočný sokel vľavo, 2) bez sokla, krycí bok vpravo,
/// 3) sokel ako v katalógu (pod skrinkou).
fn plinth_job() -> Vec<u8> {
    let mut w = W::new();
    job_head(&mut w, false, 1);
    wall(&mut w, 1, 0.0, -1000.0, 6000.0, 0.0);
    face(&mut w, 1, false, 3, &[]);
    cab(&mut w, 1, 0.0, 2);
    loose(
        &mut w,
        "BO",
        "SOKEL",
        [-60.0, 0.0, 432.0],
        [1260.0, 96.5, 19.0],
    );
    loose(
        &mut w,
        "RU",
        "SOKEL BOCNY",
        [0.0, 0.0, -2.0],
        [19.0, 97.0, 434.0],
    );
    cab(&mut w, 2, 600.0, 1);
    loose(
        &mut w,
        "BO",
        "KRYCI BOK",
        [600.0, 0.0, -80.0],
        [38.0, 899.0, 608.0],
    );
    cab(&mut w, 3, 1600.0, 1);
    loose(
        &mut w,
        "BO",
        "SOKEL",
        [0.0, 0.0, 432.0],
        [600.0, 97.0, 19.0],
    );
    face(&mut w, 1, true, 0, &[]);
    w.buf
}

#[test]
fn plinth_span_side_and_cover_are_imported() {
    let job = parse_contents(&plinth_job()).unwrap();
    assert!(job.consumed_all);
    let p = to_project_detailed(&job, &defaults()).project;
    let o1 = &spec(&p, "SP1").options;
    assert_eq!(
        o1.plinth_span,
        Some(PlinthSpan {
            from: -60.0,
            length: 1260.0,
            height: Some(96.5),
        })
    );
    assert_eq!(o1.side_plinth_left, Some(true));
    assert_eq!(o1.side_plinth_right, Some(false));
    assert_eq!(o1.side_plinth_height, Some(97.0));
    assert_eq!(o1.cover_side_left, None);
    // bez sokla: atribúty sokla nemá, krycí bok vpravo
    let o2 = &spec(&p, "SP2").options;
    assert_eq!(o2.plinth, Some(false));
    assert_eq!(o2.plinth_span, None);
    assert_eq!(o2.side_plinth_left, None);
    assert_eq!(o2.cover_side_right, Some(true));
    assert_eq!(o2.cover_side_left, None);
    // sokel ako v katalógu: zostavu urobí generátor, bočné sokle podľa CV (žiadne)
    let o3 = &spec(&p, "SP3").options;
    assert_eq!(o3.plinth_span, None);
    assert_eq!(o3.side_plinth_left, Some(false));
    assert_eq!(o3.side_plinth_right, Some(false));
    assert_eq!(o3.side_plinth_height, None);
}

/// Voliteľný test na zákazke dielne REF (cesta v `CABINETCRAFT_CVJ_REF`).
#[test]
fn optional_ref_plinths() {
    let Some(path) = std::env::var_os("CABINETCRAFT_CVJ_REF").map(PathBuf::from) else {
        return;
    };
    if !path.exists() {
        return;
    }
    let job = read_job(&path).unwrap();
    let p = to_project_detailed(&job, &defaults()).project;
    // findings 12 §6: 0030 na skr. 1 = 2680 od X −60, 0090 na skr. 13 = 1944 od X −1372
    let s1 = spec(&p, "SP1").options.plinth_span.unwrap();
    assert!((s1.length - 2680.0).abs() < 0.01);
    assert!((s1.from + 59.735).abs() < 0.01, "{}", s1.from);
    assert_eq!(s1.height, Some(96.5));
    assert_eq!(spec(&p, "SP1").options.side_plinth_left, Some(true));
    assert_eq!(spec(&p, "SP1").options.side_plinth_height, Some(97.0));
    let s13 = spec(&p, "SP13").options.plinth_span.unwrap();
    assert!((s13.length - 1944.0).abs() < 0.01);
    assert!((s13.from + 1372.0).abs() < 0.01);
    assert_eq!(s13.height, None);
    assert_eq!(spec(&p, "SP13").options.cover_side_right, Some(true));
    assert_eq!(spec(&p, "SP13").options.side_plinth_right, Some(false));
}
