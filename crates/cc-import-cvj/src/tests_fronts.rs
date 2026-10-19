//! Testy prevodu čiel, zásuviek a atribútov skrinky (syntetické úlohy a voliteľne
//! skutočné súbory).

use std::path::PathBuf;

use cc_model::{
    CabinetSpec, Defaults, FrontAdjust, FrontKind, FrontReveals, HingeSide, JointType,
    ShelfSupport, TipOn, TipOnColor, TipOnKind,
};

use super::*;
use crate::classes::tests::{case_group, face, job_head, wall, IN, W};

fn defaults() -> Defaults {
    Defaults {
        panel_material: "P".into(),
        back_material: "B".into(),
        construction: "C".into(),
    }
}

/// Čelo (`CVDoor`): ASM 11 dvierka/panel, 12 zásuvka; poloha a rozmer voči otvoru.
struct Fr {
    asm: u16,
    code: &'static str,
    pos: [f64; 2],
    size: [f64; 2],
}

fn fr(asm: u16, code: &'static str, pos: [f64; 2], size: [f64; 2]) -> Fr {
    Fr {
        asm,
        code,
        pos,
        size,
    }
}

/// Uzol stromu otvorov (priamo v skupine `Face`) s celočíselnými a dĺžkovými parametrami.
#[allow(clippy::too_many_arguments)]
fn section(
    w: &mut W,
    code: &str,
    asm: u16,
    pos: [f64; 2],
    size: [f64; 2],
    ints: &[(&str, i32)],
    lens: &[(&str, f64)],
    fronts: &[Fr],
) {
    w.tag("CVAssembly");
    w.asm_head(0, code, asm, (4 + ints.len() + lens.len()) as u16);
    w.param_f64("X", 1, pos[0] / IN)
        .param_f64("Y", 1, pos[1] / IN)
        .param_f64("DX", 1, size[0] / IN)
        .param_f64("DY", 1, size[1] / IN);
    for (n, v) in ints {
        w.param_int(n, *v);
    }
    for (n, v) in lens {
        w.param_f64(n, 1, v / IN);
    }
    w.asm_tail(1, 0);
    w.u16(0xEA01).children(fronts.len() as u16);
    for f in fronts {
        w.tag("CVDoor").asm_base(
            0,
            f.code,
            f.asm,
            14,
            17,
            &[
                ("X", 1, f.pos[0] / IN),
                ("Y", 1, f.pos[1] / IN),
                ("Z", 1, 3.0 / IN),
                ("DX", 1, f.size[0] / IN),
                ("DY", 1, f.size[1] / IN),
                ("DZ", 1, 19.0 / IN),
            ],
        );
        w.u16(0xEA01).i32(0).u32(0).i32(0).children(0);
    }
}

/// Skupina `Face` nad soklom 100 s `n` uzlami (zapíše volajúci).
fn face_group(w: &mut W, n: u16) {
    w.tag("CVAssembly")
        .asm_base(0, "Face", 14, 1, 0, &[("Y", 1, 100.0 / IN)]);
    w.u16(0xEA01).children(n);
}

/// Kovanie `HW2` s názvom (spojovací prvok).
fn hardware(w: &mut W, desc: &str) {
    w.tag("CVHardware");
    w.cv_base(0, "HW2", desc, 56);
    w.u16(0xEA01).u16(0);
    w.asm_tail(0, 0);
    w.u16(0xEA01);
}

/// Spodná skrinka 600 × 820 × 500 (sokel 100) na stene s atribútmi a `children` deťmi.
fn cab(w: &mut W, no: i32, x: f64, ints: &[(&str, i32)], children: u16) {
    w.tag("CVCabinet")
        .asm_head(100 + no as u32, "S_TEST", 9, (8 + ints.len()) as u16);
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
    for (n, v) in ints {
        w.param_int(n, *v);
    }
    w.asm_tail(1, 1);
    w.u16(0xEA01).u16(0).u32(0).u32(0).p3();
    w.u16(0xEA01).i32(0).i32(0);
    w.u32(0).u32(0).u16(0).p3().p3().u32(0);
    w.children(children);
}

/// Úloha so skrinkami: 3 zásuvky, slepý roh, dvojité dvierka, umývačka, nepodporované
/// delenie a zacyklený strom.
fn fronts_job() -> Vec<u8> {
    let mut w = W::new();
    job_head(&mut w, false, 1);
    wall(&mut w, 1, 0.0, -1000.0, 6000.0, 0.0);
    face(&mut w, 1, false, 6, &[]);

    // 1) tri zásuvky ako S_KUCH_VARNA_D (korpus 720): úchytka OLT −30, škáry 2,
    //    ľavá 1, pravá 2; spodné dve EQ
    cab(
        &mut w,
        1,
        0.0,
        &[
            ("APPLY_HW_CAB", 0),
            ("UCS_3DHWcabinet", 2),
            ("Zapnut_cocky", 1),
            ("tip_on_zap_cab_2", 1),
            ("tip_on_bar_cab_2", 2),
            ("tip_on_typ_cab_2", 1),
            ("MAKEDADO", 1),
            ("Zmazat_nohu", 1),
            ("Zmazat_nohu_1", 1),
            ("Zmazat_nohu_2", 0),
        ],
        3,
    );
    case_group(&mut w);
    hardware(&mut w, "Vrut 3.5x50mm");
    face_group(&mut w, 7);
    let drawer = |h: f64| [("BOXH", h), ("BOXD", 500.0), ("OLT", -30.0)];
    section(
        &mut w,
        "FO",
        15,
        [0.0, 0.0],
        [600.0, 720.0],
        &[("ID", 1), ("PARID", 0), ("TYPE", 1), ("STYPE", 0)],
        &[],
        &[],
    );
    section(
        &mut w,
        "DWR_OPEN",
        30,
        [0.0, 557.0],
        [600.0, 163.0],
        &[("ID", 2), ("PARID", 1), ("TYPE", 1), ("STYPE", 2)],
        &drawer(103.0),
        &[fr(12, "DWR", [1.0, 1.0], [597.0, 130.0])],
    );
    section(
        &mut w,
        "MR",
        16,
        [0.0, 557.0],
        [0.0, 600.0],
        &[("ID", 3), ("PARID", 1), ("TYPE", 2), ("STYPE", 0)],
        &[],
        &[],
    );
    section(
        &mut w,
        "FO",
        15,
        [0.0, 0.0],
        [600.0, 557.0],
        &[("ID", 4), ("PARID", 1), ("TYPE", 1), ("STYPE", 0)],
        &[],
        &[],
    );
    section(
        &mut w,
        "DWR_OPEN",
        30,
        [0.0, 278.0],
        [600.0, 279.0],
        &[
            ("ID", 5),
            ("PARID", 4),
            ("TYPE", 1),
            ("STYPE", 2),
            ("EQ", 2),
        ],
        &drawer(204.0),
        &[fr(12, "DWR", [1.0, 1.0], [597.0, 247.0])],
    );
    section(
        &mut w,
        "MR",
        16,
        [0.0, 278.0],
        [0.0, 600.0],
        &[("ID", 6), ("PARID", 4), ("TYPE", 2), ("STYPE", 0)],
        &[],
        &[],
    );
    section(
        &mut w,
        "DWR_OPEN",
        30,
        [0.0, 0.0],
        [600.0, 278.0],
        &[
            ("ID", 7),
            ("PARID", 4),
            ("TYPE", 1),
            ("STYPE", 2),
            ("EQ", 2),
        ],
        &drawer(204.0),
        &[fr(12, "DWR", [1.0, 0.0], [597.0, 247.0])],
    );

    // 2) slepý roh: panel 0–300, čiara na 300, dvierka vľavo pri paneli (úchytka)
    cab(
        &mut w,
        2,
        700.0,
        &[("UCS_3DHWcabinet", 2), ("Policove_kovanie", 8)],
        2,
    );
    case_group(&mut w);
    face_group(&mut w, 4);
    section(
        &mut w,
        "FO",
        15,
        [0.0, 0.0],
        [600.0, 720.0],
        &[("ID", 1), ("PARID", 0), ("TYPE", 1), ("STYPE", 0)],
        &[],
        &[],
    );
    section(
        &mut w,
        "DOR_OPEN",
        29,
        [0.0, 0.0],
        [300.0, 720.0],
        &[("ID", 2), ("PARID", 1), ("TYPE", 1), ("STYPE", 8)],
        &[],
        &[fr(11, "PANEL", [0.0, 0.0], [299.0, 718.0])],
    );
    section(
        &mut w,
        "MS",
        16,
        [300.0, 0.0],
        [0.0, 720.0],
        &[("ID", 3), ("PARID", 1), ("TYPE", 3), ("STYPE", 0)],
        &[],
        &[],
    );
    section(
        &mut w,
        "DOR_OPEN",
        29,
        [300.0, 0.0],
        [300.0, 720.0],
        &[
            ("ID", 4),
            ("PARID", 1),
            ("TYPE", 1),
            ("STYPE", 1),
            ("HNG", 1),
            ("HNGQ", 3),
        ],
        &[("OLT", -30.0), ("HNGT", 300.0)],
        &[fr(11, "DOR", [1.0, 0.0], [298.0, 688.0])],
    );

    // 3) dvojité dvierka: dva uzly s rovnakým ID
    cab(&mut w, 3, 1400.0, &[], 2);
    case_group(&mut w);
    face_group(&mut w, 2);
    for (x, hng) in [(0.0, 1), (300.0, 2)] {
        section(
            &mut w,
            "DOR_OPEN",
            29,
            [x, 0.0],
            [300.0, 720.0],
            &[
                ("ID", 1),
                ("PARID", 0),
                ("TYPE", 1),
                ("STYPE", 4),
                ("HNG", hng),
            ],
            &[],
            &[fr(
                11,
                "DOR",
                [if x > 0.0 { 1.0 } else { 0.0 }, 0.0],
                [299.0, 718.0],
            )],
        );
    }

    // 4) umývačka: bez korpusu, dvierka bez závesov (HNG 4), nožičky zmazané
    let mut legs: Vec<(&str, i32)> = vec![("Zmazat_nohu", 1), ("Policove_kovanie", 0)];
    let names = [
        "Zmazat_nohu_1",
        "Zmazat_nohu_2",
        "Zmazat_nohu_3",
        "Zmazat_nohu_4",
        "Zmazat_nohu_5",
        "Zmazat_nohu_6",
    ];
    legs.extend(names.iter().map(|n| (*n, 1)));
    cab(&mut w, 4, 2100.0, &legs, 1);
    face_group(&mut w, 1);
    section(
        &mut w,
        "DOR_OPEN",
        29,
        [0.0, 0.0],
        [600.0, 720.0],
        &[
            ("ID", 1),
            ("PARID", 0),
            ("TYPE", 1),
            ("STYPE", 1),
            ("HNG", 4),
        ],
        &[("OLT", -30.0)],
        &[fr(11, "DOR", [2.0, 0.0], [596.0, 688.0])],
    );

    // 5) zvislé delenie, pravý stĺpec ďalej delený vodorovne (náš model nevie)
    cab(&mut w, 5, 2800.0, &[], 2);
    case_group(&mut w);
    face_group(&mut w, 7);
    let open = |id: i32, par: i32| [("ID", id), ("PARID", par), ("TYPE", 1), ("STYPE", 1)];
    section(
        &mut w,
        "FO",
        15,
        [0.0, 0.0],
        [600.0, 720.0],
        &[("ID", 1), ("PARID", 0), ("TYPE", 1), ("STYPE", 0)],
        &[],
        &[],
    );
    section(
        &mut w,
        "DOR_OPEN",
        29,
        [0.0, 0.0],
        [300.0, 720.0],
        &open(2, 1),
        &[],
        &[fr(11, "DOR", [0.0, 0.0], [299.0, 718.0])],
    );
    section(
        &mut w,
        "MS",
        16,
        [300.0, 0.0],
        [0.0, 720.0],
        &[("ID", 3), ("PARID", 1), ("TYPE", 3)],
        &[],
        &[],
    );
    section(
        &mut w,
        "FO",
        15,
        [300.0, 0.0],
        [300.0, 720.0],
        &[("ID", 4), ("PARID", 1), ("TYPE", 1), ("STYPE", 0)],
        &[],
        &[],
    );
    section(
        &mut w,
        "DOR_OPEN",
        29,
        [300.0, 360.0],
        [300.0, 360.0],
        &open(5, 4),
        &[],
        &[fr(11, "DOR", [1.0, 0.0], [299.0, 358.0])],
    );
    section(
        &mut w,
        "MR",
        16,
        [300.0, 360.0],
        [0.0, 300.0],
        &[("ID", 6), ("PARID", 4), ("TYPE", 2)],
        &[],
        &[],
    );
    section(
        &mut w,
        "DOR_OPEN",
        29,
        [300.0, 0.0],
        [300.0, 360.0],
        &open(7, 4),
        &[],
        &[fr(11, "DOR", [1.0, 0.0], [299.0, 358.0])],
    );

    // 6) zacyklený strom (1 → 2 → 1): bez koreňa, nesmie spadnúť
    cab(&mut w, 6, 3500.0, &[], 2);
    case_group(&mut w);
    face_group(&mut w, 2);
    section(
        &mut w,
        "FO",
        15,
        [0.0, 0.0],
        [600.0, 720.0],
        &[("ID", 1), ("PARID", 2), ("TYPE", 1)],
        &[],
        &[],
    );
    section(
        &mut w,
        "FO",
        15,
        [0.0, 0.0],
        [600.0, 720.0],
        &[("ID", 2), ("PARID", 1), ("TYPE", 1)],
        &[],
        &[],
    );

    face(&mut w, 1, true, 0, &[]);
    w.buf
}

fn spec<'a>(p: &'a cc_model::ProjectFile, id: &str) -> &'a CabinetSpec {
    p.cabinets.iter().find(|c| c.id == id).unwrap()
}

#[test]
fn fronts_drawers_and_attributes_from_tree() {
    let job = parse_contents(&fronts_job()).unwrap();
    assert!(job.consumed_all);
    let room = &job.rooms[0];
    assert_eq!(room.cabinets.len(), 6);
    let c1 = &room.cabinets[0];
    assert_eq!(c1.sections_in(SectionGroup::Face).count(), 7);
    let d2 = &c1.sections[1];
    assert_eq!((d2.id, d2.parent, d2.kind), (2, 1, SectionKind::Opening));
    assert_eq!(d2.face_style(), FaceStyle::Drawer);
    assert!((d2.params.box_height.unwrap() - 103.0).abs() < 1e-9);
    assert!((d2.params.overlay_top.unwrap() + 30.0).abs() < 1e-9);
    assert_eq!(d2.fronts.len(), 1);
    assert!(d2.fronts[0].drawer);
    // čelo v súradniciach skrinky: sokel 100 + otvor 557 + 1
    assert!((d2.fronts[0].bbox.min.y - 658.0).abs() < 1e-9);
    assert_eq!(c1.attributes.joint_type, Some(2));
    assert_eq!(c1.attributes.tip_on, Some(1));
    assert_eq!(c1.connectors, ["Vrut 3.5x50mm"]);

    let imported = to_project_detailed(&job, &defaults());
    let p = &imported.project;
    let notes = imported.notes.join("\n");

    // 1) zásuvky zhora nadol: 160 (+ úprava −30 = čelo 130), dve rovnaké
    let sp1 = spec(p, "SP1");
    let kinds: Vec<FrontKind> = sp1.fronts.iter().map(|f| f.kind).collect();
    assert_eq!(kinds, [FrontKind::Drawer; 3]);
    let heights: Vec<Option<f64>> = sp1.fronts.iter().map(|f| f.height).collect();
    assert_eq!(heights, [Some(160.0), None, None]);
    let boxes: Vec<Option<f64>> = sp1.fronts.iter().map(|f| f.box_height).collect();
    assert_eq!(boxes, [Some(103.0), Some(204.0), Some(204.0)]);
    for f in &sp1.fronts {
        assert_eq!(
            f.adjust,
            FrontAdjust {
                top: -30.0,
                ..Default::default()
            }
        );
    }
    assert_eq!(sp1.drawer_length, Some(500.0));
    assert_eq!(
        sp1.reveals,
        FrontReveals {
            top: Some(2.0),
            bottom: Some(0.0),
            left: Some(1.0),
            right: Some(2.0),
            horizontal: Some(2.0),
            vertical: None,
        }
    );
    let o = &sp1.options;
    assert_eq!(o.joints, Some(false));
    assert_eq!(o.joint_type, Some(JointType::Screw));
    assert_eq!(o.bumpers, Some(true));
    assert_eq!(
        (o.tip_on, o.tip_on_color, o.tip_on_kind),
        (
            Some(TipOn::Drilled),
            Some(TipOnColor::Black),
            Some(TipOnKind::Magnet)
        )
    );
    // nožička č. 1 zmazaná, č. 2 nie: nožičky ostávajú, poznámka
    assert_eq!(o.legs, Some(true));
    // korpus case_group nemá chrbát ani vrch ako diely UB/TO
    assert!(!sp1.back);

    // 2) slepý roh: panel 300 a dvierka vľavo pri paneli s úpravou −30
    let sp2 = spec(p, "SP2");
    assert_eq!(sp2.fronts.len(), 1);
    let cols = &sp2.fronts[0].columns;
    assert_eq!(cols.len(), 2);
    assert_eq!(
        (cols[0].kind, cols[0].width),
        (FrontKind::Panel, Some(300.0))
    );
    assert_eq!(
        (cols[1].kind, cols[1].width, cols[1].hinge_side),
        (FrontKind::Door, None, Some(HingeSide::Left))
    );
    assert_eq!(cols[1].adjust.top, -30.0);
    assert_eq!(sp2.reveals.vertical, Some(2.0));
    assert_eq!(
        (sp2.reveals.left, sp2.reveals.right),
        (Some(0.0), Some(1.0))
    );
    assert_eq!(sp2.options.joint_type, Some(JointType::Confirmat));
    assert_eq!(sp2.options.shelf_support, None);

    // 3) dvojité dvierka
    let sp3 = spec(p, "SP3");
    assert_eq!(sp3.fronts.len(), 1);
    assert_eq!(sp3.fronts[0].kind, FrontKind::Pair);
    assert_eq!(sp3.reveals.vertical, Some(2.0));

    // 4) umývačka: iba čelo, rozmery z obálky bez sokla
    let sp4 = spec(p, "SP4");
    assert!(!sp4.carcass);
    assert_eq!((sp4.w, sp4.h, sp4.d), (600.0, 720.0, 500.0));
    assert_eq!(sp4.fronts.len(), 1);
    assert_eq!(sp4.fronts[0].kind, FrontKind::FalseFront);
    assert_eq!(sp4.fronts[0].adjust.top, -30.0);
    assert_eq!(sp4.options.legs, Some(false));
    assert_eq!(sp4.options.shelf_support, Some(ShelfSupport::None));
    assert_eq!(sp4.reveals.left, Some(2.0));

    // 5) a 6) nepodporované delenie a zacyklený strom: bez čiel, s poznámkou
    assert!(spec(p, "SP5").fronts.is_empty());
    assert!(spec(p, "SP6").fronts.is_empty());

    for needle in [
        "skrinka č. 1 `S_TEST`: chrbát „VSAZENÁ“",
        "zmazané iba nožičky č. 1",
        "policové kovanie Tab 18",
        "ručná poloha závesov (HNGT 300)",
        "zadaný počet závesov 3",
        "skrinka č. 4 `S_TEST`: iba čelo bez korpusu",
        "skrinka č. 5 `S_TEST`: čelá sa neimportovali, stĺpec čiel je ďalej delený",
        "skrinka č. 6 `S_TEST`: čelá sa neimportovali, strom otvorov čela nemá koreň",
    ] {
        assert!(
            notes.contains(needle),
            "chýba poznámka `{needle}`:\n{notes}"
        );
    }
    // umývačka nemá poznámku o dvierkach bez závesov (to je jej podstata)
    assert!(!notes.contains("skrinka č. 4 `S_TEST`: dvierka bez závesov"));
    // TOML sa dá zapísať a prečítať späť
    let text = toml::to_string(p).unwrap();
    let back: cc_model::ProjectFile = toml::from_str(&text).unwrap();
    assert_eq!(&back, p);
}

/// So štandardom sa hodnoty zhodné so štandardom nezapíšu.
#[test]
fn values_equal_to_standard_are_omitted() {
    let job = parse_contents(&fronts_job()).unwrap();
    let text = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/dielna/standards/dielna-cx100.toml"),
    )
    .unwrap();
    let mut standard: cc_model::Standard = toml::from_str(&text).unwrap();
    standard.cabinet_options.joint_type = JointType::Confirmat;
    standard.fronts.base.top = 2.0;
    standard.fronts.base.bottom = 0.0;
    standard.fronts.base.gap = 2.0;
    standard.fronts.reveal_side = 1.0;
    standard.fronts.pair_gap = 2.0;
    let p = to_project_with(&job, &defaults(), Some(&standard)).project;
    let sp1 = spec(&p, "SP1");
    assert_eq!(
        sp1.reveals,
        FrontReveals {
            right: Some(2.0),
            ..Default::default()
        }
    );
    // vrut sa líši od konfirmátu štandardu
    assert_eq!(sp1.options.joint_type, Some(JointType::Screw));
    assert_eq!(spec(&p, "SP2").options.joint_type, None);
    assert_eq!(spec(&p, "SP3").reveals.vertical, None);
}

#[test]
fn corrupted_fronts_job_never_panics() {
    let data = fronts_job();
    for n in (0..data.len()).step_by(7) {
        if let Ok(job) = parse_contents(&data[..n]) {
            let _ = to_project_detailed(&job, &defaults());
        }
    }
    let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
    for _ in 0..2000 {
        let mut d = data.clone();
        for _ in 0..4 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let i = (seed as usize) % d.len();
            d[i] = (seed >> 32) as u8;
        }
        if let Ok(job) = parse_contents(&d) {
            let _ = to_project_detailed(&job, &defaults());
        }
    }
}

/// Voliteľný test na ukážke CV 2026 `test_1.cvj` (bez súboru sa preskočí).
#[test]
fn optional_test_1_fronts() {
    let path = PathBuf::from(r"C:\CABINET VISION\CV 2026\Jobs\test_1.cvj");
    if !path.exists() || std::env::var("CABINETCRAFT_CVJ").is_ok() {
        return;
    }
    let job = read_job(&path).unwrap();
    let p = to_project_detailed(&job, &defaults()).project;
    // 3 zásuvky (čelá 150 / 302,5 / 303,5, korpusy N / E / E, NL 500)
    let sp4 = spec(&p, "SP4");
    assert_eq!(sp4.fronts.len(), 3);
    assert!(sp4.fronts.iter().all(|f| f.kind == FrontKind::Drawer));
    let boxes: Vec<Option<f64>> = sp4.fronts.iter().map(|f| f.box_height).collect();
    assert_eq!(boxes, [Some(80.5), Some(204.0), Some(204.0)]);
    assert_eq!(sp4.drawer_length, Some(500.0));
    // drez: 1 zásuvka, NL 450, bez chrbta
    let sp3 = spec(&p, "SP3");
    assert_eq!(sp3.drawer_length, Some(450.0));
    assert!(!sp3.back);
    // umývačka: iba čelo
    let sp2 = spec(&p, "SP2");
    assert!(!sp2.carcass);
    assert_eq!(sp2.fronts[0].kind, FrontKind::FalseFront);
    // dvierka s pántmi vľavo, horné dvojité dvierka
    assert_eq!(spec(&p, "SP1").fronts[0].hinge_side, Some(HingeSide::Left));
    assert_eq!(spec(&p, "HO6").fronts[0].kind, FrontKind::Pair);
    // slepý roh: panel 500 + dvierka pri paneli
    let cols = &spec(&p, "SP10").fronts[0].columns;
    assert_eq!(
        cols.iter().map(|c| c.kind).collect::<Vec<_>>(),
        [FrontKind::Panel, FrontKind::Door]
    );
    assert_eq!(cols[0].width, Some(500.0));
}

/// Voliteľný test na zákazke dielne REF: cesta v `CABINETCRAFT_CVJ_REF` (mená zákazníkov
/// do repa nepatria). Bez premennej sa preskočí.
#[test]
fn optional_ref_fronts() {
    let Some(path) = std::env::var_os("CABINETCRAFT_CVJ_REF").map(PathBuf::from) else {
        return;
    };
    let job = read_job(&path).unwrap();
    let p = to_project_detailed(&job, &defaults()).project;
    // S_KUCH_VARNA_D: čelá 130 / 268 / 268 = miesta 160 / 298 / 298 s úchytkou −30
    let sp4 = spec(&p, "SP4");
    let heights: Vec<Option<f64>> = sp4.fronts.iter().map(|f| f.height).collect();
    assert_eq!(heights, [Some(160.0), None, None]);
    assert!(sp4.fronts.iter().all(|f| f.adjust.top == -30.0));
    let boxes: Vec<Option<f64>> = sp4.fronts.iter().map(|f| f.box_height).collect();
    assert_eq!(boxes, [Some(103.0), Some(204.0), Some(204.0)]);
    assert_eq!(sp4.reveals.right, Some(2.0));
    // umývačka iba s čelom, drez NL 450, horná s presahom 19 dole, roh panel + dvierka
    assert!(!spec(&p, "SP2").carcass);
    assert_eq!(spec(&p, "SP3").drawer_length, Some(450.0));
    let ho9 = spec(&p, "HO9");
    assert_eq!(ho9.fronts[0].adjust.bottom, 19.0);
    assert_eq!(ho9.fronts[0].hinge_side, Some(HingeSide::Right));
    let cols = &spec(&p, "SP10").fronts[0].columns;
    assert_eq!(
        cols.iter().map(|c| c.kind).collect::<Vec<_>>(),
        [FrontKind::Panel, FrontKind::Door]
    );
    assert_eq!(
        spec(&p, "SP13").fronts[0].hinge_side,
        Some(HingeSide::Right)
    );
}
