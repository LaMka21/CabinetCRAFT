//! Testy prevodu do projektu a voliteľný test na skutočných súboroch.

use std::path::{Path, PathBuf};

use cc_model::{CabinetType, Defaults, ProjectFile};
use glam::DVec3;

use super::*;
use crate::classes::tests::sample_job;

fn defaults() -> Defaults {
    Defaults {
        panel_material: "P".into(),
        back_material: "B".into(),
        construction: "C".into(),
    }
}

/// Každá skrinka pri stene: `cc_room::Room::placement` musí dať tie isté rohy korpusu
/// ako svetová poloha v CV (do 0,5 mm). Vráti počet overených skriniek.
fn check_placements(job: &CvjJob, imported: &Imported) -> usize {
    let project: &ProjectFile = &imported.project;
    let room = cc_room::Room::new(project);
    let cv_room = &job.rooms[imported.room.unwrap()];
    let mut checked = 0;
    for (spec, &src) in project.cabinets.iter().zip(&imported.sources) {
        let Some(t) = room.placement(spec) else {
            continue;
        };
        let cab = &cv_room.cabinets[src];
        let c = cab.carcass;
        for (x, y, z) in [
            (0.0, 0.0, 0.0),
            (spec.w, 0.0, 0.0),
            (0.0, spec.d, 0.0),
            (spec.w, spec.d, spec.h),
            (0.0, spec.d, spec.h),
        ] {
            let ours = t.point_to_cabinet(DVec3::new(x, y, z));
            // náš roh (x vpravo, y dozadu, z hore) = bod korpusu CV (X vpravo, Z dopredu, Y hore)
            let cv_local = DVec3::new(c.min.x + x, c.min.y + z, c.max.z - y);
            let cv = to_plan(cab.world.transform_point3(cv_local));
            assert!(
                (ours - cv).length() < 0.5,
                "skrinka {}: náš roh {ours} ≠ CV {cv}",
                spec.id
            );
        }
        // miestnosť je napravo od steny: predok skrinky smeruje k vnútru steny
        let wall = &room.walls[room.wall_index(spec.wall.as_deref().unwrap()).unwrap()];
        let back = t.point_to_cabinet(DVec3::new(0.0, spec.d, 0.0)).truncate();
        let front = t.point_to_cabinet(DVec3::ZERO).truncate();
        assert!((front - back).dot(wall.inward) > 0.0);
        checked += 1;
    }
    checked
}

#[test]
fn synthetic_job_converts_to_project() {
    let job = parse_contents(&sample_job()).unwrap();
    assert!(job.consumed_all);
    assert_eq!(job.name, "JOB");
    let room = &job.rooms[0];
    assert_eq!(room.walls.len(), 1);
    let w = &room.walls[0];
    assert!((w.length - 2000.0).abs() < 1e-9);
    assert!((w.start - DVec3::new(0.0, 0.0, -1000.0)).length() < 1e-9);
    assert!((w.end - DVec3::new(2000.0, 0.0, -1000.0)).length() < 1e-9);
    let cab = &room.cabinets[0];
    assert_eq!(cab.class, CvClass::Base);
    assert_eq!(cab.number, Some(7));
    assert_eq!(cab.comment.as_deref(), Some("poznámka"));
    assert_eq!(cab.doors, 1);
    assert_eq!(cab.parts.len(), 6);
    assert_eq!(cab.carcass_source, CarcassSource::Parts);
    // korpus bez sokla: 600 × 720 × 500 nad soklom 100
    assert!((cab.carcass.min - DVec3::new(0.0, 100.0, 0.0)).length() < 1e-9);
    assert!((cab.carcass.max - DVec3::new(600.0, 820.0, 500.0)).length() < 1e-9);
    let roles: Vec<PartRole> = cab.parts.iter().map(|p| p.role).collect();
    assert_eq!(roles.iter().filter(|r| **r == PartRole::Carcass).count(), 4);
    assert!(roles.contains(&PartRole::Front));
    assert!(roles.contains(&PartRole::Loose));

    let imported = to_project_detailed(&job, &defaults());
    let p = &imported.project;
    assert_eq!(p.walls.len(), 1);
    assert_eq!(p.walls[0].id, "W1");
    // (X, Z) = (0, −1000) → (x, y) = (0, 1000), smer +x
    assert_eq!(
        (p.walls[0].x, p.walls[0].y, p.walls[0].angle),
        (0.0, 1000.0, 0.0)
    );
    let room_spec = p.room.as_ref().unwrap();
    assert_eq!(room_spec.wall_height, 2400.0);
    assert_eq!(room_spec.wall_thickness, 100.0);
    assert_eq!(room_spec.toe_height, 100.0);
    let c = &p.cabinets[0];
    assert_eq!(c.id, "SP7");
    assert_eq!(c.cabinet_type, CabinetType::Base);
    assert_eq!((c.w, c.h, c.d), (600.0, 720.0, 500.0));
    assert_eq!(c.wall.as_deref(), Some("W1"));
    assert_eq!((c.at, c.outset, c.elevation), (100.0, 20.0, None));
    assert_eq!(check_placements(&job, &imported), 1);
    // TOML sa dá zapísať a prečítať späť
    let text = toml_roundtrip(p);
    assert!(text.contains("[[wall]]"));
}

fn toml_roundtrip(p: &ProjectFile) -> String {
    let text = toml::to_string(p).unwrap();
    let back: ProjectFile = toml::from_str(&text).unwrap();
    assert_eq!(&back, p);
    text
}

#[test]
fn truncated_contents_never_panics() {
    let data = sample_job();
    for n in 0..data.len() {
        let r = parse_contents(&data[..n]);
        // koreň bez konca je chyba; skrátenie iba v zástupných objektoch nevadí
        if let Ok(job) = r {
            assert_eq!(job.rooms[0].cabinets.len(), 1);
        }
    }
}

#[test]
fn corrupted_contents_never_panics() {
    let data = sample_job();
    let mut seed: u64 = 0x2545_F491_4F6C_DD1D;
    for _ in 0..3000 {
        let mut d = data.clone();
        for _ in 0..4 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let i = (seed as usize) % d.len();
            d[i] = (seed >> 32) as u8;
        }
        if let Ok(job) = parse_contents(&d) {
            let _ = to_project(&job, &defaults());
        }
    }
}

#[test]
fn container_roundtrip_in_memory() {
    use std::io::{Cursor, Write};
    let mut comp = cfb::CompoundFile::create(Cursor::new(Vec::new())).unwrap();
    comp.create_stream("/Contents")
        .unwrap()
        .write_all(&sample_job())
        .unwrap();
    let mut v = Vec::new();
    for x in [2026u32, 1, 1515] {
        v.extend(x.to_le_bytes());
    }
    comp.create_stream("/Version")
        .unwrap()
        .write_all(&v)
        .unwrap();
    comp.flush().unwrap();
    let bytes = comp.into_inner().into_inner();
    let f = CvjFile::read_from(Cursor::new(bytes)).unwrap();
    assert_eq!(f.version(), Some([2026, 1, 1515]));
    let job = f.job().unwrap();
    assert_eq!(job.version, Some([2026, 1, 1515]));
    assert_eq!(job.rooms[0].cabinets.len(), 1);

    // bez prúdu Contents
    let mut comp = cfb::CompoundFile::create(Cursor::new(Vec::new())).unwrap();
    comp.create_stream("/Other")
        .unwrap()
        .write_all(b"x")
        .unwrap();
    comp.flush().unwrap();
    let bytes = comp.into_inner().into_inner();
    assert!(matches!(
        CvjFile::read_from(Cursor::new(bytes)),
        Err(CvjError::MissingStream("Contents"))
    ));
    // vôbec nie CFB
    assert!(CvjFile::read_from(Cursor::new(b"nie je to cfb".to_vec())).is_err());
}

fn real_file() -> Option<PathBuf> {
    let path = std::env::var("CABINETCRAFT_CVJ")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(r"C:\CABINET VISION\CV 2026\Jobs\test_1.cvj"));
    path.exists().then_some(path)
}

/// Voliteľný test na skutočnom súbore (`CABINETCRAFT_CVJ` alebo ukážka CV 2026).
/// Bez súboru sa ticho preskočí. Hodnoty sú zo vzorovej úlohy `test_1` dodanej s CV.
#[test]
fn optional_real_file() {
    let Some(path) = real_file() else {
        return;
    };
    let job = read_job(Path::new(&path)).unwrap();
    assert!(job.consumed_all, "{:?}", job.trailing_error);
    let imported = to_project_detailed(&job, &defaults());
    let checked = check_placements(&job, &imported);
    assert!(checked > 0);
    if std::env::var("CABINETCRAFT_CVJ").is_ok() {
        // iný súbor: iba úplnosť a polohy
        return;
    }
    assert_eq!(job.version.map(|v| v[0]), Some(2026));
    assert_eq!(job.rooms.len(), 1);
    let room = &job.rooms[0];
    assert_eq!(room.walls.len(), 4);
    assert_eq!(room.cabinets.len(), 12);
    let parts: usize = room.cabinets.iter().map(|c| c.parts.len()).sum();
    assert_eq!(parts, 115);
    // uzavretý obdĺžnik 2988 × 2324 mm (hodnoty overené referenčným výpisom cvj_dump.py)
    let w1 = &room.walls[0];
    assert_eq!(w1.number, 1);
    assert_eq!((w1.prev, w1.next), (Some(4), Some(2)));
    assert!((w1.start - DVec3::new(-1496.75, 0.0, -1162.0)).length() < 1e-6);
    assert!((w1.end - DVec3::new(1491.25, 0.0, -1162.0)).length() < 1e-6);
    assert!((w1.height - 2427.0).abs() < 1e-6);
    assert!((w1.thickness - 150.0).abs() < 1e-6);
    for (i, w) in room.walls.iter().enumerate() {
        let next = &room.walls[(i + 1) % 4];
        assert!((w.end - next.start).length() < 1e-6);
        let expected = if i % 2 == 0 { 2988.0 } else { 2324.0 };
        assert!((w.length - expected).abs() < 1e-6);
    }
    let c1 = &room.cabinets[0];
    assert_eq!(c1.number, Some(1));
    assert_eq!(c1.class, CvClass::Base);
    assert_eq!(c1.wall, Some(1));
    assert_eq!(c1.face, Some(Face::Front));
    assert!((c1.world.translation - DVec3::new(-1431.75, 0.0, -1102.0)).length() < 1e-6);
    assert!((c1.size - DVec3::new(643.0, 862.0, 510.0)).length() < 1e-6);
    assert_eq!(c1.doors, 1);
    // korpus: 643 × 762 × 510 nad soklom 100
    assert!((c1.carcass.size() - DVec3::new(643.0, 762.0, 510.0)).length() < 1e-6);
    assert!((c1.carcass.min.y - 100.0).abs() < 1e-6);
    let upper = room
        .cabinets
        .iter()
        .find(|c| c.class == CvClass::Upper)
        .unwrap();
    assert!((upper.position.y - 1537.0).abs() < 1e-6);
    assert_eq!(upper.shelves, 2);

    let p = &imported.project;
    assert_eq!(p.walls.len(), 4);
    let room_spec = p.room.as_ref().unwrap();
    assert_eq!(room_spec.wall_height, 2427.0);
    assert_eq!(room_spec.wall_thickness, 150.0);
    assert_eq!(room_spec.toe_height, 100.0);
    let sp1 = p.cabinets.iter().find(|c| c.id == "SP1").unwrap();
    assert_eq!((sp1.w, sp1.h, sp1.d), (643.0, 762.0, 510.0));
    assert_eq!(sp1.wall.as_deref(), Some("W1"));
    assert_eq!((sp1.at, sp1.outset, sp1.elevation), (65.0, 60.0, None));
}

/// Miestnosť so všetkými prípadmi umiestnenia: šikmá stena, horná skrinka, natočená
/// skrinka, spotrebič, okno, výplň, pracovná doska, zadná plocha a ostrov.
fn scenario_job() -> Vec<u8> {
    use crate::classes::tests::{cabinet, case_group, face, job_head, part, wall, W};
    let mut w = W::new();
    job_head(&mut w, false, 3);
    // stena 1: (0, −1000) → (3000, −1000)
    wall(&mut w, 1, 0.0, -1000.0, 3000.0, 0.0);
    face(&mut w, 1, false, 7, &[]);
    // a) spodná skrinka s korpusom a dvierkami
    cabinet(
        &mut w,
        1,
        1,
        1,
        [100.0, 0.0, 20.0],
        [600.0, 820.0, 500.0],
        0.0,
        2,
    );
    case_group(&mut w);
    w.tag("CVDoor").asm_base(0, "DOR", 11, 1, 17, &[]);
    w.u16(0xEA01).i32(0).u32(0).i32(0).children(1);
    part(
        &mut w,
        "S_DSLAB",
        [0.0, 100.0, 500.0],
        [596.0, 716.0, 18.0],
        0.0,
    );
    // b) horná skrinka (korpus 100…820 v jej rámci → spodok 1600 nad podlahou)
    cabinet(
        &mut w,
        2,
        2,
        1,
        [100.0, 1500.0, 0.0],
        [600.0, 820.0, 500.0],
        0.0,
        1,
    );
    case_group(&mut w);
    // c) natočená skrinka
    cabinet(
        &mut w,
        3,
        1,
        1,
        [1500.0, 0.0, 0.0],
        [600.0, 820.0, 500.0],
        30.0,
        1,
    );
    case_group(&mut w);
    // d) spotrebič
    cabinet(
        &mut w,
        4,
        5,
        9,
        [2200.0, 0.0, 0.0],
        [600.0, 2000.0, 600.0],
        0.0,
        0,
    );
    // e) okno
    cabinet(
        &mut w,
        0,
        7,
        19,
        [1000.0, 1000.0, 0.0],
        [800.0, 1200.0, 100.0],
        0.0,
        0,
    );
    // f) výplň s jediným dielom
    cabinet(
        &mut w,
        6,
        3,
        6,
        [2900.0, 0.0, 0.0],
        [50.0, 2000.0, 500.0],
        0.0,
        1,
    );
    part(&mut w, "LF", [18.0, 0.0, 0.0], [500.0, 2000.0, 18.0], -90.0);
    // g) pracovná doska
    w.tag("CVCounter").asm_base(
        0,
        "Counter",
        7,
        8,
        0,
        &[
            ("DX", 1, 1000.0 / IN),
            ("DY", 1, 38.0 / IN),
            ("DZ", 1, 600.0 / IN),
        ],
    );
    w.u16(0xEA01).u16(0).u32(0).u16(0).children(0);
    // zadná plocha so skrinkou
    face(
        &mut w,
        1,
        true,
        1,
        &[
            ("X", 1, 3100.0 / IN),
            ("Z", 1, -1100.0 / IN),
            ("AY", 2, 180.0),
        ],
    );
    cabinet(
        &mut w,
        7,
        1,
        1,
        [100.0, 0.0, 0.0],
        [600.0, 820.0, 500.0],
        0.0,
        1,
    );
    case_group(&mut w);
    // stena 2: (3000, −1000) → (3000, 1000), AY = 270
    wall(&mut w, 2, 3000.0, -1000.0, 2000.0, 270.0);
    face(&mut w, 2, false, 1, &[]);
    cabinet(
        &mut w,
        8,
        1,
        1,
        [500.0, 0.0, 0.0],
        [600.0, 820.0, 500.0],
        0.0,
        1,
    );
    case_group(&mut w);
    face(&mut w, 2, true, 0, &[]);
    // ostrov priamo v miestnosti
    cabinet(
        &mut w,
        9,
        1,
        1,
        [0.0, 0.0, 0.0],
        [600.0, 820.0, 500.0],
        0.0,
        1,
    );
    case_group(&mut w);
    w.buf
}

use crate::classes::tests::IN;

#[test]
fn scenario_placements_and_notes() {
    let job = parse_contents(&scenario_job()).unwrap();
    let room = &job.rooms[0];
    assert_eq!(room.walls.len(), 2);
    assert_eq!(room.cabinets.len(), 9);
    assert_eq!(room.counters.len(), 1);
    let back = room.cabinets.iter().find(|c| c.number == Some(7)).unwrap();
    assert_eq!(back.face, Some(Face::Back));

    let imported = to_project_detailed(&job, &defaults());
    let p = &imported.project;
    let ids: Vec<&str> = p.cabinets.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["SP1", "HO2", "SP3", "SP7", "SP8", "SP9"]);
    // stena 2: (X, Z) = (3000, −1000) → (x, y) = (3000, 1000), smer −y
    let w2 = &p.walls[1];
    assert_eq!(
        (w2.x, w2.y, w2.angle, w2.length),
        (3000.0, 1000.0, -90.0, 2000.0)
    );
    let room_spec = p.room.as_ref().unwrap();
    assert_eq!(room_spec.toe_height, 100.0);
    assert_eq!(room_spec.upper_top, Some(2320.0));
    let ho2 = &p.cabinets[1];
    assert_eq!(ho2.cabinet_type, CabinetType::Wall);
    assert_eq!(
        (ho2.at, ho2.outset, ho2.elevation, ho2.h),
        (100.0, 0.0, None, 720.0)
    );
    for id in ["SP3", "SP7", "SP9"] {
        let c = p.cabinets.iter().find(|c| c.id == id).unwrap();
        assert!(c.wall.is_none(), "{id}");
    }
    let sp8 = p.cabinets.iter().find(|c| c.id == "SP8").unwrap();
    assert_eq!((sp8.wall.as_deref(), sp8.at), (Some("W2"), 500.0));
    assert_eq!(p.openings.len(), 1);
    let o = &p.openings[0];
    assert_eq!(
        (o.wall.as_str(), o.at, o.width, o.height, o.elevation),
        ("W1", 1000.0, 800.0, 1200.0, 1000.0)
    );
    let notes = imported.notes.join("\n");
    for needle in [
        "natočená",
        "zadnej ploche steny č. 1",
        "nestojí pri stene",
        "Appliance",
        "skrinka č. 6 `S_TEST`: neimportuje sa, nemá diely korpusu",
        "pracovná doska",
    ] {
        assert!(
            notes.contains(needle),
            "chýba poznámka `{needle}`:\n{notes}"
        );
    }
    // SP1, HO2 a SP8 stoja pri stenách a sedia s polohou v CV
    assert_eq!(check_placements(&job, &imported), 3);
    // projekt prejde kontrolou miestnosti
    assert!(cc_room::check(p).is_empty(), "{:?}", cc_room::check(p));
}
