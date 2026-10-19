//! Príkazy miestnosti (kap. 13) cez `Session::run` bez okna.

use std::path::PathBuf;

use cc_engine::Session;
use serde_json::{json, Value};

fn open(name: &str) -> Session {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/dielna")
        .join(name);
    Session::open(&path).unwrap()
}

fn run(s: &mut Session, id: &str, params: Value) -> Value {
    s.run(id, params).unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn issues(s: &Session) -> Vec<String> {
    s.model()
        .map(|b| b.issues().map(|i| i.message.clone()).collect())
        .unwrap_or_default()
}

#[test]
fn rect_room_walls_openings_and_fill() {
    let mut s = open("project.toml");
    let r = run(&mut s, "room.add_rect", json!({"w": 3000, "d": 2400}));
    assert_eq!(r["walls"], json!(["W1", "W2", "W3", "W4"]));
    run(
        &mut s,
        "room.set",
        json!({"set": {"toe_height": 120, "upper_top": 2250}}),
    );
    let o = run(
        &mut s,
        "opening.add",
        json!({"wall": "W1", "kind": "window", "at": 1800, "width": 800}),
    );
    assert_eq!(o["index"], 0);
    // skrinky vzoru pri stene W1
    run(
        &mut s,
        "cabinet.set",
        json!({"id": "SP1", "set": {"wall": "W1", "at": 0}}),
    );
    run(
        &mut s,
        "cabinet.set",
        json!({"id": "HO1", "set": {"wall": "W1", "at": 0}}),
    );
    // spodný rad: 600–3000 bez rohu (W2 je prázdna) → vyplnenie
    let f = run(&mut s, "cabinet.fill", json!({"wall": "W1", "row": "base"}));
    let added: Vec<String> = serde_json::from_value(f["added"].clone()).unwrap();
    assert!(!added.is_empty());
    let doc = s.doc().unwrap();
    let total: f64 = doc
        .project
        .cabinets
        .iter()
        .filter(|c| {
            c.wall.as_deref() == Some("W1") && c.cabinet_type == cc_model::CabinetType::Base
        })
        .map(|c| c.w)
        .sum();
    assert!((2950.0..=3000.0 + 1e-9).contains(&total), "{total}");
    // horný rad sa vyhne oknu (1800–2600)
    run(
        &mut s,
        "cabinet.fill",
        json!({"wall": "W1", "row": "upper"}),
    );
    assert!(
        !issues(&s)
            .iter()
            .any(|m| m.contains("zasahuje") || m.contains("prekrývajú")),
        "{:?}",
        issues(&s)
    );
    // späť vráti vyplnenie
    let before = s.doc().unwrap().project.cabinets.len();
    run(&mut s, "edit.undo", json!({}));
    assert!(s.doc().unwrap().project.cabinets.len() < before);
}

#[test]
fn wall_rename_and_remove_keep_references() {
    let mut s = open("kuchyna.toml");
    run(
        &mut s,
        "wall.set",
        json!({"id": "W1", "set": {"id": "ZADNA", "thickness": 150}}),
    );
    let doc = s.doc().unwrap();
    assert!(doc
        .project
        .cabinets
        .iter()
        .any(|c| c.wall.as_deref() == Some("ZADNA")));
    assert!(doc.project.openings.iter().any(|o| o.wall == "ZADNA"));
    let r = run(&mut s, "wall.remove", json!({"id": "ZADNA"}));
    assert!(r["detached"].as_array().unwrap().len() >= 5);
    let doc = s.doc().unwrap();
    assert!(doc
        .project
        .cabinets
        .iter()
        .all(|c| c.wall.as_deref() != Some("ZADNA")));
    assert!(doc.project.openings.iter().all(|o| o.wall != "ZADNA"));
}

#[test]
fn hostile_parameters_are_rejected_without_change() {
    let mut s = open("kuchyna.toml");
    let before = s.doc().unwrap().project.clone();
    for (id, params) in [
        ("wall.add", json!({"x": 0, "y": 0, "length": -5})),
        ("wall.add", json!({"x": 1e300, "y": 0, "length": 100})),
        ("wall.set", json!({"id": "W9", "set": {"length": 10}})),
        ("wall.set", json!({"id": "W1", "set": {"id": "W2"}})),
        (
            "opening.add",
            json!({"wall": "W1", "kind": "door", "width": 0}),
        ),
        ("opening.add", json!({"wall": "NIE", "kind": "door"})),
        ("opening.set", json!({"index": 99, "set": {"at": 1}})),
        ("room.set", json!({"set": {"wall_height": -1}})),
        ("room.add_rect", json!({"w": 0, "d": 100})),
        (
            "cabinet.fill",
            json!({"wall": "W1", "row": "base", "increment": 0}),
        ),
        ("cabinet.fill", json!({"wall": "W1", "row": "side"})),
    ] {
        assert!(
            s.run(id, params.clone()).is_err(),
            "{id} {params} mal zlyhať"
        );
    }
    assert_eq!(s.doc().unwrap().project, before);
}
