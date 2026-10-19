//! Test šikmej skrinky (CV REF skrinka 1 `S_KUCH_D SIKMA`): rozmery skrinky z CV namiesto
//! obálky dielov a prevod šikmého boku na `slant`.

use cc_model::{CabinetSpec, Defaults, SlantSide};

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

/// Skrinka 642,968 × 862 × 510 (sokel 100) s dielmi ako REF skrinka 1: šikmý `LF`
/// (AX −30,5243°, 60 mm za skrinku), `RU`, `DE`, úzky `TO` a pohľadový chrbát `FB`.
fn slanted_cabinet(w: &mut W, no: i32, x: f64) {
    w.tag("CVCabinet")
        .asm_head(100 + no as u32, "S_SIKMA", 9, 8);
    for (n, v) in [
        ("X", x),
        ("Y", 0.0),
        ("Z", 62.0),
        ("DX", 642.968),
        ("DY", 862.0),
        ("DZ", 510.0),
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
    w.tag("CVAssembly").asm_base(0, "Case", 14, 1, 0, &[]);
    w.u16(0xEA01).children(5);
    w.tag("CVAssembly").asm_head(0, "LF", 16, 9);
    for (n, t, v) in [
        ("X", 1, 16.1015 / IN),
        ("Y", 1, 90.3498 / IN),
        ("Z", 1, -60.0 / IN),
        ("DX", 1, 570.0 / IN),
        ("DY", 1, 895.7951 / IN),
        ("DZ", 1, 19.0 / IN),
        ("AX", 2, -30.5243),
        ("AY", 2, -90.0),
        ("AZ", 2, 0.0),
    ] {
        w.param_f64(n, t, v);
    }
    w.asm_tail(1, 0);
    w.u16(0xEA01).children(0);
    part(w, "RU", [642.968, 100.0, 18.0], [492.0, 762.0, 18.0], -90.0);
    part(w, "DE", [21.791, 100.0, 18.0], [603.177, 18.0, 492.0], 0.0);
    part(w, "TO", [460.466, 844.0, 18.0], [164.502, 18.0, 492.0], 0.0);
    part(w, "FB", [21.791, 100.0, 0.0], [621.177, 762.0, 18.0], 0.0);
}

fn slant_job() -> Vec<u8> {
    let mut w = W::new();
    job_head(&mut w, false, 1);
    wall(&mut w, 1, 0.0, -1000.0, 3000.0, 0.0);
    face(&mut w, 1, false, 1, &[]);
    slanted_cabinet(&mut w, 1, 100.0);
    face(&mut w, 1, true, 0, &[]);
    w.buf
}

#[test]
fn slanted_side_sets_cabinet_size_and_slant() {
    let job = parse_contents(&slant_job()).unwrap();
    assert!(job.consumed_all);
    let c = &job.rooms[0].cabinets[0];
    // korpus = rozmery CV bez sokla, nie obálka dielov (tá by mala 643,23 × 781,3 × 570)
    assert!((c.carcass.size() - glam::DVec3::new(642.968, 762.0, 510.0)).length() < 1e-6);
    assert!((c.carcass.min.y - 100.0).abs() < 1e-6);

    let imported = to_project_detailed(&job, &defaults());
    let s = spec(&imported.project, "SP1");
    assert_eq!((s.w, s.h, s.d), (642.968, 762.0, 510.0));
    let slant = s.slant.as_ref().unwrap();
    assert_eq!(slant.side, SlantSide::Left);
    assert!((slant.bottom + 0.2654).abs() < 0.001, "{slant:?}");
    assert!((slant.top - 449.0224).abs() < 0.001, "{slant:?}");
    assert!((slant.extend_back - 60.0).abs() < 1e-6, "{slant:?}");
    assert!(slant.finished_back);
    assert!(imported.notes.iter().any(|n| n.contains("šikmý bok")));
    // v TOML sa šikmý bok zapíše a načíta späť
    let text = toml::to_string(&imported.project).unwrap();
    let back: cc_model::ProjectFile = toml::from_str(&text).unwrap();
    assert_eq!(spec(&back, "SP1").slant.as_ref(), Some(slant));
}

#[test]
fn straight_cabinet_has_no_slant() {
    let job = parse_contents(&crate::classes::tests::sample_job()).unwrap();
    let imported = to_project_detailed(&job, &defaults());
    assert!(imported.project.cabinets.iter().all(|c| c.slant.is_none()));
}
