//! Príkazy cez `Session::run` na vzorovom projekte dielne (bez okna).

use std::path::{Path, PathBuf};

use cc_engine::{commands, EngineError, Session};
use serde_json::{json, Value};

fn example_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/dielna")
}

/// Kópia vzorového projektu v dočasnom priečinku (testy, ktoré zapisujú).
fn temp_copy(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cc-engine-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    copy_dir(&example_dir(), &dir);
    dir
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let target = to.join(e.file_name());
        if e.path().is_dir() {
            copy_dir(&e.path(), &target);
        } else {
            std::fs::copy(e.path(), target).unwrap();
        }
    }
}

fn open() -> Session {
    Session::open(&example_dir().join("project.toml")).unwrap()
}

fn run(s: &mut Session, id: &str, params: Value) -> Value {
    s.run(id, params).unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn cabinet_ids(s: &mut Session) -> Vec<String> {
    let p = run(s, "project.inspect", json!({}));
    p["cabinets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap().to_owned())
        .collect()
}

fn width(s: &Session, id: &str) -> f64 {
    let doc = s.doc().unwrap();
    doc.project.cabinets.iter().find(|c| c.id == id).unwrap().w
}

#[test]
fn every_command_describes_itself() {
    let s = open();
    for c in commands() {
        let d = c.describe(&s);
        assert_eq!(d["id"], c.id);
        assert!(d["enabled"].is_boolean());
    }
}

#[test]
fn opened_example_builds() {
    let mut s = open();
    let m = run(&mut s, "model.inspect", json!({"parts": true}));
    assert_eq!(m["ok"], true, "{m}");
    assert_eq!(m["cabinets"], 2);
    assert!(m["parts"].as_u64().unwrap() > 10);
    assert_eq!(m["errors"], 0, "{m}");
    let part = m["partList"][0]["id"].as_str().unwrap().to_owned();
    let one = run(&mut s, "model.inspect", json!({"part": part}));
    assert!(one["part"]["operations"].is_array());
}

#[test]
fn add_undo_redo() {
    let mut s = open();
    let v0 = s.version();
    let r = run(&mut s, "cabinet.add", json!({"type": "base", "w": 450}));
    assert_eq!(r["id"], "SP2");
    assert_eq!(cabinet_ids(&mut s), ["SP1", "HO1", "SP2"]);
    assert!(s.version() > v0);
    assert!(s.doc().unwrap().dirty);
    assert_eq!(s.selection().cabinet.as_deref(), Some("SP2"));
    assert_eq!(s.undo_label().as_deref(), Some("Pridať skrinku"));

    run(&mut s, "edit.undo", json!({}));
    assert_eq!(cabinet_ids(&mut s), ["SP1", "HO1"]);
    assert!(
        !s.doc().unwrap().dirty,
        "späť vráti aj príznak neuložených zmien"
    );
    assert_eq!(
        s.selection().cabinet,
        None,
        "výber neexistujúcej skrinky sa zruší"
    );

    run(&mut s, "edit.redo", json!({}));
    assert_eq!(cabinet_ids(&mut s), ["SP1", "HO1", "SP2"]);
    assert!(matches!(
        s.run("edit.redo", json!({})),
        Err(EngineError::Disabled { .. })
    ));
}

#[test]
fn set_changes_model_and_rejects_bad_values() {
    let mut s = open();
    let before = run(&mut s, "model.inspect", json!({"parts": true}));
    run(
        &mut s,
        "cabinet.set",
        json!({"id": "SP1", "set": {"w": 800}}),
    );
    assert_eq!(width(&s, "SP1"), 800.0);
    let after = run(&mut s, "model.inspect", json!({"parts": true}));
    assert_ne!(before["partList"], after["partList"]);

    let undo_len = run(&mut s, "project.inspect", json!({}))["history"]["undo"].clone();
    for bad in [
        json!({"w": -5}),
        json!({"w": 0}),
        json!({"w": 1e9}),
        json!({"adjustable_shelves": 1000}),
        json!({"adjustable_shelves": -1}),
        json!({"nonsense": 1}),
        json!({"type": "chair"}),
        json!({"id": "HO1"}),
        json!({"id": ""}),
    ] {
        let r = s.run("cabinet.set", json!({"id": "SP1", "set": bad.clone()}));
        assert!(r.is_err(), "{bad} sa mal odmietnuť");
        assert_eq!(
            width(&s, "SP1"),
            800.0,
            "po odmietnutí {bad} sa projekt nezmenil"
        );
    }
    let p = run(&mut s, "project.inspect", json!({}));
    assert_eq!(
        p["history"]["undo"], undo_len,
        "odmietnutá zmena nevytvorí krok späť"
    );
    assert_eq!(p["cabinets"].as_array().unwrap().len(), 2);
}

#[test]
fn null_removes_optional_field() {
    let mut s = open();
    run(
        &mut s,
        "cabinet.set",
        json!({"id": "SP1", "set": {"at": 120, "elevation": 150}}),
    );
    run(
        &mut s,
        "cabinet.set",
        json!({"id": "SP1", "set": {"at": null, "elevation": null}}),
    );
    let c = &s.doc().unwrap().project.cabinets[0];
    assert_eq!(c.at, 0.0);
    assert_eq!(c.elevation, None);
}

#[test]
fn project_set_replaces_and_validates() {
    let mut s = open();
    let mut project = s.doc().unwrap().project.clone();
    project.cabinets[1].w = 450.0;
    run(&mut s, "project.set", json!({"project": project}));
    assert_eq!(width(&s, "HO1"), 450.0);
    let mut bad = s.doc().unwrap().project.clone();
    bad.cabinets[0].d = -1.0;
    assert!(s.run("project.set", json!({"project": bad})).is_err());
    assert_eq!(s.doc().unwrap().project.cabinets[0].d, 510.0);
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(width(&s, "HO1"), 600.0);
}

/// Regresia: `null` zruší výber (predtým sa `null` čítal ako chýbajúce pole).
#[test]
fn selection_null_clears() {
    let mut s = open();
    run(&mut s, "selection.set", json!({"part": "SP1-SIDE-L"}));
    assert_eq!(s.selection().cabinet.as_deref(), Some("SP1"));
    run(&mut s, "selection.set", json!({"part": null}));
    assert_eq!(s.selection().part, None);
    assert_eq!(s.selection().cabinet.as_deref(), Some("SP1"));
    run(&mut s, "selection.set", json!({"cabinet": null}));
    assert_eq!(s.selection().cabinet, None);
    // chýbajúce pole výber nemení
    run(&mut s, "selection.set", json!({"cabinet": "HO1"}));
    run(&mut s, "selection.set", json!({}));
    assert_eq!(s.selection().cabinet.as_deref(), Some("HO1"));
}

#[test]
fn rename_keeps_selection() {
    let mut s = open();
    run(&mut s, "selection.set", json!({"cabinet": "SP1"}));
    run(
        &mut s,
        "cabinet.set",
        json!({"id": "SP1", "set": {"id": "DREZ"}}),
    );
    assert_eq!(s.selection().cabinet.as_deref(), Some("DREZ"));
    assert_eq!(cabinet_ids(&mut s), ["DREZ", "HO1"]);
}

#[test]
fn coalesce_merges_steps() {
    let mut s = open();
    for w in [610, 620, 630] {
        run(
            &mut s,
            "cabinet.set",
            json!({"id": "SP1", "set": {"w": w}, "coalesce": true}),
        );
    }
    s.seal_history();
    run(
        &mut s,
        "cabinet.set",
        json!({"id": "SP1", "set": {"w": 640}, "coalesce": true}),
    );
    assert_eq!(width(&s, "SP1"), 640.0);
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(
        width(&s, "SP1"),
        630.0,
        "po zapečatení je ďalší krok samostatný"
    );
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(
        width(&s, "SP1"),
        600.0,
        "tri zlúčené kroky = jeden krok späť"
    );
}

#[test]
fn remove_duplicate_move() {
    let mut s = open();
    let r = run(&mut s, "cabinet.duplicate", json!({"id": "HO1"}));
    assert_eq!(r["id"], "HO2");
    run(&mut s, "cabinet.move", json!({"id": "HO2", "to": 0}));
    assert_eq!(cabinet_ids(&mut s), ["HO2", "SP1", "HO1"]);
    run(&mut s, "cabinet.move", json!({"id": "HO2", "by": 99}));
    assert_eq!(cabinet_ids(&mut s), ["SP1", "HO1", "HO2"]);
    run(&mut s, "cabinet.remove", json!({"id": "HO2"}));
    run(&mut s, "cabinet.remove", json!({"id": "HO1"}));
    assert!(s.run("cabinet.remove", json!({"id": "SP1"})).is_err());
    assert!(s.run("cabinet.remove", json!({"id": "XX"})).is_err());
    assert!(s
        .run("cabinet.move", json!({"id": "SP1", "by": 1, "to": 0}))
        .is_err());
}

#[test]
fn standard_set_and_undo() {
    let mut s = open();
    let r = run(
        &mut s,
        "standard.set",
        json!({"path": "dowel.face_depth", "value": 12}),
    );
    assert_eq!(r["old"], 13.0);
    let doc = s.doc().unwrap();
    assert_eq!(doc.standard.dowel.face_depth, 12.0);
    assert!(doc.standard_dirty);
    assert!(!doc.dirty);

    assert!(s
        .run("standard.set", json!({"path": "dowel.nope", "value": 1}))
        .is_err());
    assert!(s
        .run(
            "standard.set",
            json!({"path": "dowel.face_depth", "value": "x"})
        )
        .is_err());
    assert!(
        s.run(
            "standard.set",
            json!({"path": "dowel.face_depth", "value": -3})
        )
        .is_err(),
        "neplatný štandard sa odmietne"
    );
    run(&mut s, "edit.undo", json!({}));
    let doc = s.doc().unwrap();
    assert_eq!(doc.standard.dowel.face_depth, 13.0);
    assert!(!doc.standard_dirty);
}

#[test]
fn standard_reload_is_not_dirty() {
    let mut s = open();
    run(
        &mut s,
        "standard.set",
        json!({"path": "dowel.face_depth", "value": 12}),
    );
    run(&mut s, "standard.reload", json!({}));
    let doc = s.doc().unwrap();
    assert_eq!(doc.standard.dowel.face_depth, 13.0);
    assert!(!doc.standard_dirty);
    assert!(s.run("standard.use", json!({"name": "../x"})).is_err());
}

#[test]
fn unknown_command_and_params() {
    let mut s = open();
    assert!(matches!(
        s.run("cabinet.fly", json!({})),
        Err(EngineError::UnknownCommand(_))
    ));
    assert!(matches!(
        s.run("cabinet.add", json!({"type": "base", "colour": "red"})),
        Err(EngineError::Params { .. })
    ));
    assert!(matches!(
        s.run("project.inspect", json!([1, 2])),
        Err(EngineError::Params { .. })
    ));
    let mut empty = Session::new();
    assert!(matches!(
        empty.run("cabinet.add", json!({"type": "base"})),
        Err(EngineError::Disabled { .. })
    ));
}

#[test]
fn save_as_reopen_and_export() {
    let dir = temp_copy("save");
    let mut s = Session::open(&dir.join("project.toml")).unwrap();
    run(
        &mut s,
        "cabinet.set",
        json!({"id": "SP1", "set": {"w": 700}}),
    );
    run(&mut s, "project.rename", json!({"name": "Test"}));
    let target = dir.join("kopia").join("project.toml");
    let r = run(
        &mut s,
        "project.save_as",
        json!({"path": target.to_string_lossy()}),
    );
    assert!(r["notes"].as_array().unwrap().len() >= 2, "{r}");
    assert!(!s.doc().unwrap().dirty);

    let mut again = Session::open(&target).unwrap();
    assert_eq!(width(&again, "SP1"), 700.0);
    let text = std::fs::read_to_string(&target).unwrap();
    assert!(
        text.contains("w = 700"),
        "celé číslo ostane celým číslom: {text}"
    );

    let out = dir.join("out");
    let e = run(
        &mut again,
        "export.machine",
        json!({"dir": out.to_string_lossy()}),
    );
    assert!(e["xcs"].as_u64().unwrap() > 0, "{e}");
    assert!(out.join("cutlist.csv").is_file());
    assert!(out.join("xcs").join("RUN.bat").is_file());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn new_project_keeps_library() {
    let mut s = open();
    run(&mut s, "project.new", json!({}));
    let doc = s.doc().unwrap();
    assert!(doc.path.is_none());
    assert_eq!(cabinet_ids(&mut s), ["SP1"]);
    assert!(matches!(
        s.run("project.save", json!({})),
        Err(EngineError::Disabled { .. })
    ));
    assert!(s.undo_label().is_none());
}

#[test]
fn project_info_set_clear_undo() {
    let mut s = open();
    run(
        &mut s,
        "project.info",
        json!({"number": " REF2026 ", "company": "CASTEL", "customer": "Ján Vzorový"}),
    );
    let info = &s.doc().unwrap().project.project;
    assert_eq!(info.number.as_deref(), Some("REF2026"));
    assert_eq!(info.company.as_deref(), Some("CASTEL"));
    run(
        &mut s,
        "project.info",
        json!({"customer": null, "number": ""}),
    );
    let info = &s.doc().unwrap().project.project;
    assert_eq!(info.customer, None);
    assert_eq!(info.number, None);
    assert_eq!(
        info.company.as_deref(),
        Some("CASTEL"),
        "nezmenené pole ostane"
    );
    assert!(s
        .run("project.info", json!({"designer": "x".repeat(500)}))
        .is_err());
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(
        s.doc().unwrap().project.project.number.as_deref(),
        Some("REF2026")
    );
}

#[test]
fn export_drawings_writes_pdf() {
    let dir = temp_copy("drawings");
    let mut s = Session::open(&dir.join("kuchyna.toml")).unwrap();
    run(
        &mut s,
        "project.info",
        json!({"number": "REF2026", "company": "CASTEL"}),
    );
    let pdf = dir.join("vykresy.pdf");
    let r = run(
        &mut s,
        "export.drawings",
        json!({"path": pdf.to_string_lossy(), "sheets": ["job", "cabinet:SP2", "part:SP2-SIDE-L"]}),
    );
    assert_eq!(r["pages"], 3, "{r}");
    let bytes = std::fs::read(&pdf).unwrap();
    assert!(bytes.starts_with(b"%PDF-1.4"));
    let text = String::from_utf8_lossy(&bytes);
    assert_eq!(text.matches("/Type /Page ").count(), 3);
    // zákazka 3 pohľady, skrinka 4, diel 1 náhľad
    assert_eq!(text.matches("/Subtype /Image").count(), 8);
    // firma tučným písmom (vloženým systémovým, alebo Helvetica)
    let company = cc_out_pdf::pdf::encoded_text(
        &cc_out_pdf::Fonts::new(cc_out_pdf::FontChoice::System),
        "CASTEL",
        true,
    );
    assert!(bytes
        .windows(company.len())
        .any(|w| w == company.as_slice()));

    let all = run(
        &mut s,
        "export.drawings",
        json!({"path": pdf.to_string_lossy(), "parts": "machined"}),
    );
    let cabinets = s.doc().unwrap().project.cabinets.len() as u64;
    assert!(all["pages"].as_u64().unwrap() > 1 + cabinets);
    assert!(s
        .run("export.drawings", json!({"sheets": ["cabinet:NIE"]}))
        .is_err());
    assert!(s
        .run("export.drawings", json!({"sheets": ["nonsense"]}))
        .is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn export_drawings_walls() {
    let dir = temp_copy("drawings-walls");
    let mut s = Session::open(&dir.join("kuchyna.toml")).unwrap();
    let pdf = dir.join("steny.pdf");
    let path = pdf.to_string_lossy();
    // steny so skrinkami alebo otvormi (prázdna stena sa vynechá)
    let doc = s.doc().unwrap();
    let used = doc
        .project
        .walls
        .iter()
        .filter(|w| {
            doc.project
                .cabinets
                .iter()
                .any(|c| c.wall.as_deref() == Some(&w.id))
                || doc.project.openings.iter().any(|o| o.wall == w.id)
        })
        .count() as u64;
    assert!(used >= 1);
    let r = run(
        &mut s,
        "export.drawings",
        json!({"path": path, "sheets": ["walls"]}),
    );
    assert_eq!(r["pages"], used, "{r}");
    let bytes = std::fs::read(&pdf).unwrap();
    let text = String::from_utf8_lossy(&bytes);
    // pohľad na stenu je vektorový: žiadne obrázky
    assert_eq!(text.matches("/Subtype /Image").count(), 0);
    let r = run(
        &mut s,
        "export.drawings",
        json!({"path": path, "sheets": ["wall:W1"]}),
    );
    assert_eq!(r["pages"], 1);
    assert!(s
        .run("export.drawings", json!({"sheets": ["wall:NIE"]}))
        .is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn logo_in_title_block() {
    let dir = temp_copy("drawings-logo");
    let mut s = Session::open(&dir.join("kuchyna.toml")).unwrap();
    // logo s priehľadnosťou
    let mut img = image::RgbaImage::new(40, 20);
    for (x, _, p) in img.enumerate_pixels_mut() {
        *p = image::Rgba([200, 30, 30, if x < 20 { 255 } else { 0 }]);
    }
    img.save(dir.join("logo.png")).unwrap();
    std::fs::write(dir.join("zle.png"), b"toto nie je obrazok").unwrap();

    assert!(s.run("project.info", json!({"logo": "chyba.png"})).is_err());
    assert!(s.run("project.info", json!({"logo": "zle.png"})).is_err());
    assert_eq!(s.doc().unwrap().project.project.logo, None);
    run(&mut s, "project.info", json!({"logo": "logo.png"}));
    assert_eq!(
        s.doc().unwrap().project.project.logo.as_deref(),
        Some("logo.png")
    );

    let pdf = dir.join("vykresy.pdf");
    let path = pdf.to_string_lossy();
    let r = run(
        &mut s,
        "export.drawings",
        json!({"path": path, "sheets": ["job", "cabinet:SP2"]}),
    );
    assert_eq!(r["warnings"], json!([]));
    let bytes = std::fs::read(&pdf).unwrap();
    let text = String::from_utf8_lossy(&bytes);
    // 3 + 4 pohľady a logo raz pre obe strany
    assert_eq!(text.matches("/Subtype /Image").count(), 8);
    assert_eq!(text.matches("/Width 40 /Height 20").count(), 1);

    // stratené logo výkresy nezastaví
    std::fs::remove_file(dir.join("logo.png")).unwrap();
    let r = run(
        &mut s,
        "export.drawings",
        json!({"path": path, "sheets": ["job"]}),
    );
    assert_eq!(r["warnings"].as_array().unwrap().len(), 1);
    run(&mut s, "project.info", json!({"logo": null}));
    assert_eq!(s.doc().unwrap().project.project.logo, None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cabinet_fronts_set_and_undo() {
    let mut s = open();
    let id = cabinet_ids(&mut s)[0].clone();
    run(
        &mut s,
        "cabinet.set",
        json!({"id": id, "set": {"fronts": [{"kind": "drawer", "height": 180}, {"kind": "door", "hinge_side": "right"}]}}),
    );
    let model = s.model().unwrap();
    let parts: Vec<&str> = model
        .model
        .parts()
        .filter(|p| p.role.is_front())
        .map(|p| p.id.as_str())
        .collect();
    assert_eq!(parts.len(), 2, "{parts:?}");
    // zlé vstupy
    for bad in [
        json!({"fronts": [{"kind": "door", "height": -5}]}),
        json!({"fronts": [{"kind": "okno"}]}),
        json!({"fronts": vec![json!({"kind": "drawer"}); 13]}),
        json!({"hinge": ""}),
    ] {
        assert!(
            s.run("cabinet.set", json!({"id": id, "set": bad})).is_err(),
            "{bad}"
        );
    }
    run(&mut s, "edit.undo", json!({}));
    let model = s.model().unwrap();
    assert!(model.model.parts().all(|p| !p.role.is_front()));
}

/// Vloženie ako v CV bez myši (findings 06 §4.3): do stredu steny vybranej skrinky
/// a pri prekrytí na najbližšie voľné miesto radu; myšou (`snap`) k susedovi.
#[test]
fn add_places_next_to_selected_cabinet() {
    let mut s = Session::open(&example_dir().join("kuchyna.toml")).unwrap();
    let place = |s: &Session, id: &str| {
        let c = s
            .doc()
            .unwrap()
            .project
            .cabinets
            .iter()
            .find(|c| c.id == id)
            .unwrap()
            .clone();
        (c.wall, c.at)
    };
    run(&mut s, "selection.set", json!({"cabinet": "SP4"}));
    // stena W2 2800: stred 1400 − 300 = 1100 prekrýva SP4 (510 … 1110) → dotyk od 1110
    let id = run(&mut s, "cabinet.add", json!({"type": "base"}))["id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(place(&s, &id), (Some("W2".into()), 1110.0));
    // ďalšia: stred je obsadený, najbližšie voľné od 1710 (vľavo od SP4 je iba 510)
    let id2 = run(&mut s, "cabinet.add", json!({"type": "base"}))["id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(place(&s, &id2), (Some("W2".into()), 1710.0));
    // horná: stred 1100 je v hornom rade voľný (HO3 zaberá 330 … 930)
    let up = run(&mut s, "cabinet.add", json!({"type": "wall"}))["id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(place(&s, &up), (Some("W2".into()), 1100.0));
    // myšou na 1000 (150 široká): prekrýva novú hornú od 1100 → najbližší dotyk 950
    let drop = run(
        &mut s,
        "cabinet.add",
        json!({"type": "wall", "w": 150.0, "wall": "W2", "at": 1000.0, "snap": true}),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(place(&s, &drop), (Some("W2".into()), 950.0));
    // výslovná poloha sa nemení
    let free = run(&mut s, "cabinet.add", json!({"type": "base", "at": 5.0}))["id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(place(&s, &free), (None, 5.0));
}

/// Editor sekcií (ako Section Face v CV): delenie otvoru, typ, zlúčenie, zámok výšky.
#[test]
fn front_sections_split_set_remove() {
    let mut s = Session::open(&example_dir().join("kuchyna.toml")).unwrap();
    let fronts = |s: &Session| {
        s.doc()
            .unwrap()
            .project
            .cabinets
            .iter()
            .find(|c| c.id == "SP4")
            .unwrap()
            .fronts
            .clone()
    };
    use cc_model::FrontKind;
    // SP4: jedny dvierka → zásuvka nad dvierkami: vodorovné delenie a typ horného otvoru
    run(
        &mut s,
        "front.split",
        json!({"cabinet": "SP4", "row": 0, "direction": "horizontal"}),
    );
    let f = fronts(&s);
    assert_eq!(f.len(), 2);
    // horná časť dostala výšku polovice otvoru 730 bez škáry 32: (730 − 32) / 2 = 349
    assert_eq!(f[0].height, Some(349.0));
    assert_eq!(f[1].height, None);
    run(
        &mut s,
        "front.set",
        json!({"cabinet": "SP4", "row": 0, "kind": "drawer"}),
    );
    assert_eq!(fronts(&s)[0].kind, FrontKind::Drawer);
    // spodný otvor zvislo na dve dvierka: ľavé pánty vľavo, pravé vpravo
    run(
        &mut s,
        "front.split",
        json!({"cabinet": "SP4", "row": 1, "direction": "vertical"}),
    );
    let cols = fronts(&s)[1].columns.clone();
    assert_eq!(cols.len(), 2);
    assert_eq!(cols[1].hinge_side, Some(cc_model::HingeSide::Right));
    // model sa postaví (bez chyby generátora)
    assert!(s.model().is_some());
    // zmazanie delenia stĺpcov → späť jedny dvierka
    run(
        &mut s,
        "front.remove",
        json!({"cabinet": "SP4", "row": 1, "column": 1}),
    );
    assert!(fronts(&s)[1].columns.is_empty());
    // uvoľnenie výšky zásuvky a zlúčenie riadkov
    run(
        &mut s,
        "front.set",
        json!({"cabinet": "SP4", "row": 0, "lock": false}),
    );
    assert_eq!(fronts(&s)[0].height, None);
    run(&mut s, "front.remove", json!({"cabinet": "SP4", "row": 0}));
    assert_eq!(fronts(&s).len(), 1);
    // zlé vstupy sú chyba, nie pád
    assert!(s
        .run(
            "front.split",
            json!({"cabinet": "SP4", "row": 9, "direction": "vertical"})
        )
        .is_err());
    assert!(s
        .run(
            "front.split",
            json!({"cabinet": "SP4", "row": 0, "direction": "vertical", "count": 99})
        )
        .is_err());
}

/// Pravý klik do rohu ako v CV (Extend Left/Right End): skrinka sa predĺži do rohu
/// a predĺžená časť dostane slepý panel (slepý roh).
#[test]
fn corner_extend() {
    let get = |s: &Session, id: &str| {
        s.doc()
            .unwrap()
            .project
            .cabinets
            .iter()
            .find(|c| c.id == id)
            .unwrap()
            .clone()
    };
    // roh W1/W2: SP4 (dvierka, W2 od 510) sa predĺži na začiatok W2 → 0 … 1110
    let mut s = Session::open(&example_dir().join("kuchyna.toml")).unwrap();
    let r = run(&mut s, "room.corner", json!({"wall": "W1", "extend": "W2"}));
    assert_eq!(r["id"], "SP4");
    let sp4 = get(&s, "SP4");
    assert_eq!((sp4.at, sp4.w), (0.0, 1110.0));
    // slepý panel 510 vľavo (v rohu), pôvodné dvierka vpravo
    let cols: Vec<_> = sp4.fronts[0]
        .columns
        .iter()
        .map(|c| (c.kind, c.width))
        .collect();
    assert_eq!(
        cols,
        [
            (cc_model::FrontKind::Panel, Some(510.0)),
            (cc_model::FrontKind::Door, None)
        ]
    );
    // SP3 na W1 končí 2750 ≤ 3600 − 510 → ostane
    let sp3 = get(&s, "SP3");
    assert_eq!((sp3.at, sp3.w), (2150.0, 600.0));
    let built = s.model().unwrap();
    assert!(!built.has_errors(), "{:?}", built.report.issues);
    // SP3 má zásuvky: do rohu sa predĺžiť nedá (chyba, nie pád)
    let mut s = Session::open(&example_dir().join("kuchyna.toml")).unwrap();
    let err = s
        .run("room.corner", json!({"wall": "W1", "extend": "W1"}))
        .unwrap_err()
        .to_string();
    assert!(err.contains("zásuvkami"), "{err}");
    assert!(s
        .run("room.corner", json!({"wall": "W1", "extend": "W9"}))
        .is_err());
}

/// Prisunutie do rohu (Snap to Corner pre skrinky bez natočenia).
#[test]
fn snap_to_corner() {
    let mut s = Session::open(&example_dir().join("kuchyna.toml")).unwrap();
    // W1 3600, v rohu W1/W2 je SP4 hĺbky 510 → SP3 končí na 3090
    let r = run(
        &mut s,
        "cabinet.snap_corner",
        json!({"id": "SP3", "side": "right"}),
    );
    assert_eq!(r["at"], 2490.0);
    // horná HO2: v hornom rade W2 je HO3 hĺbky 330 → 3600 − 330 − 600
    let r = run(
        &mut s,
        "cabinet.snap_corner",
        json!({"id": "HO2", "side": "right"}),
    );
    assert_eq!(r["at"], 2670.0);
    assert!(s
        .run("cabinet.snap_corner", json!({"id": "SP3", "side": "hore"}))
        .is_err());
}
