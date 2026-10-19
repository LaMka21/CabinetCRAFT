//! Testy rozpoznania blendy (findings 11 §5.3, 12 §2): syntetická úloha s blendou za pravým
//! bokom hornej skrinky a voliteľne zákazka dielne REF (skr. 5 a 9).

use std::path::PathBuf;

use cc_model::{CabinetSpec, Defaults, FillerSide};

use super::*;
use crate::classes::tests::{cabinet, face, job_head, part, wall, W};

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

/// Horná skrinka 600 široká (boky 0…600), vrch 650 cez blendu; za pravým bokom voľné diely:
/// lišta 18, čelo blendy 60 × 19 pred lištou a dno blendy 50 × 19 pod bokmi.
/// Druhá skrinka je bez blendy (iba korpus).
fn blenda_job(with_strip: bool) -> Vec<u8> {
    let mut w = W::new();
    job_head(&mut w, false, 1);
    wall(&mut w, 1, 0.0, -1000.0, 3000.0, 0.0);
    face(&mut w, 1, false, 2, &[]);
    let loose = if with_strip { 3 } else { 2 };
    cabinet(
        &mut w,
        9,
        2,
        1,
        [100.0, 1500.0, 0.0],
        [650.0, 820.0, 500.0],
        0.0,
        1 + loose,
    );
    w.tag("CVAssembly").asm_base(0, "Case", 14, 1, 0, &[]);
    w.u16(0xEA01).children(4);
    part(
        &mut w,
        "LU",
        [18.0, 100.0, 0.0],
        [500.0, 701.0, 18.0],
        -90.0,
    );
    part(
        &mut w,
        "RU",
        [600.0, 100.0, 0.0],
        [500.0, 701.0, 18.0],
        -90.0,
    );
    part(&mut w, "BO", [18.0, 100.0, 0.0], [564.0, 18.0, 500.0], 0.0);
    part(&mut w, "TO", [0.0, 801.0, 0.0], [650.0, 19.0, 500.0], 0.0);
    // voľné diely blendy (mimo skupiny Case)
    if with_strip {
        part(
            &mut w,
            "RU",
            [618.0, 100.0, 2.0],
            [500.0, 699.0, 18.0],
            -90.0,
        );
    }
    part(&mut w, "BO", [600.0, 81.0, 502.0], [60.0, 720.0, 19.0], 0.0);
    part(&mut w, "BO", [600.0, 81.0, 0.0], [50.0, 19.0, 503.0], 0.0);
    // skrinka bez blendy
    cabinet(
        &mut w,
        8,
        2,
        1,
        [1000.0, 1500.0, 0.0],
        [600.0, 820.0, 500.0],
        0.0,
        1,
    );
    crate::classes::tests::case_group(&mut w);
    // stena má 2 plochy
    face(&mut w, 1, true, 0, &[]);
    w.buf
}

#[test]
fn attached_filler_narrows_carcass() {
    let job = parse_contents(&blenda_job(true)).unwrap();
    let imported = to_project_detailed(&job, &defaults());
    let p = &imported.project;
    let ho9 = spec(p, "HO9");
    // korpus po vonkajšie plochy bokov, nie po koniec vrchu
    assert_eq!(ho9.w, 600.0);
    assert_eq!(ho9.at, 100.0);
    let f = ho9.filler.as_ref().expect("blenda");
    assert_eq!(f.side, FillerSide::Right);
    assert_eq!(f.width, 50.0);
    assert_eq!(f.front_width, Some(60.0));
    assert!(f.bottom);
    assert!(f.dowels);
    assert!(ho9.carcass);
    assert!(imported.notes.iter().any(|n| n.contains("blenda vpravo")));
    // skrinka bez blendy
    let ho8 = spec(p, "HO8");
    assert_eq!(ho8.filler, None);
    assert_eq!(ho8.w, 600.0);
}

#[test]
fn no_strip_no_filler() {
    // bez lišty to nie je blenda: korpus ostane po koniec vrchu
    let job = parse_contents(&blenda_job(false)).unwrap();
    let p = to_project_detailed(&job, &defaults()).project;
    let ho9 = spec(&p, "HO9");
    assert_eq!(ho9.filler, None);
    assert_eq!(ho9.w, 650.0);
}

/// Voliteľný test na zákazke dielne REF: cesta v `CABINETCRAFT_CVJ_REF` (mená zákazníkov
/// do repa nepatria). Bez premennej alebo bez súboru sa preskočí.
#[test]
fn optional_ref_fillers() {
    let Some(path) = std::env::var_os("CABINETCRAFT_CVJ_REF").map(PathBuf::from) else {
        return;
    };
    if !path.exists() {
        return;
    }
    let job = read_job(&path).unwrap();
    let p = to_project_detailed(&job, &defaults()).project;
    // skr. 9: korpus 576,5, blenda 50 vpravo, čelo 60, dno blendy (findings 12 §2)
    let ho9 = spec(&p, "HO9");
    assert_eq!(ho9.w, 576.5);
    let f = ho9.filler.as_ref().unwrap();
    assert_eq!(
        (f.side, f.width, f.front_width, f.bottom),
        (FillerSide::Right, 50.0, Some(60.0), true)
    );
    assert_eq!(ho9.reveals.right, Some(2.0));
    // skr. 5: samostatná blenda 18 × 762 × 100, čelo 78 (OLR 60), bez fronts
    let sp5 = spec(&p, "SP5");
    assert!(!sp5.carcass);
    assert_eq!((sp5.w, sp5.h, sp5.d), (18.0, 762.0, 100.0));
    assert!(sp5.fronts.is_empty());
    let f = sp5.filler.as_ref().unwrap();
    assert_eq!((f.side, f.front_width), (FillerSide::Right, Some(78.0)));
    // ostatné horné skrinky blendu nemajú
    for id in ["HO6", "HO7", "HO8"] {
        assert_eq!(spec(&p, id).filler, None, "{id}");
    }
}
