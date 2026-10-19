//! `project.import_cvj` na skutočnej úlohe CV, ak je na tomto PC (inak sa preskočí).

use std::path::PathBuf;

use cc_engine::Session;
use serde_json::json;

#[test]
fn import_opens_project_with_walls() {
    let cvj = std::env::var_os("CABINETCRAFT_CVJ")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("C:/CABINET VISION/CV 2026/Jobs/test_1.cvj"));
    if !cvj.is_file() {
        return;
    }
    let dir = std::env::temp_dir().join(format!("cc-engine-import-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let out = dir.join("project.toml");
    let template = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/dielna");
    let mut s = Session::new();
    let r = s
        .run(
            "project.import_cvj",
            json!({"file": cvj, "out": out, "template": template}),
        )
        .unwrap();
    assert_eq!(r["walls"], 4);
    let doc = s.doc().unwrap();
    assert_eq!(doc.project.walls.len(), 4);
    assert!(doc.project.cabinets.iter().all(|c| c.wall.is_some()));
    assert!(s.model().is_some_and(|b| !b.has_errors()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn missing_file_is_error_and_keeps_session() {
    let mut s = Session::new();
    let r = s.run(
        "project.import_cvj",
        json!({"file": "neexistuje.cvj", "out": "x/project.toml"}),
    );
    assert!(r.is_err());
    assert!(s.doc().is_none());
}
