//! `export.machine` s režimom výroby (cx100 / nesting) na vzorovej kuchyni.

use std::path::PathBuf;

use cc_engine::Session;
use serde_json::json;

#[test]
fn export_nesting_and_cx100_modes() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/dielna/kuchyna.toml");
    let mut s = Session::open(&path).unwrap();
    let base = std::env::temp_dir().join(format!("cc-engine-export-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);

    let nest = s
        .run(
            "export.machine",
            json!({"dir": base.join("nest"), "mode": "nesting"}),
        )
        .unwrap();
    assert!(nest["sheets"].as_u64().unwrap() >= 1, "{nest}");
    assert!(base.join("nest/nesting/CncRun.txt").is_file());

    let all = s
        .run(
            "export.machine",
            json!({"dir": base.join("cx"), "mode": "cx100"}),
        )
        .unwrap();
    assert_eq!(all["sheets"], 0);
    // v režime nesting ide na CX100 menej programov (iba zvyšná práca)
    assert!(nest["xcs"].as_u64().unwrap() <= all["xcs"].as_u64().unwrap());
    assert!(!base.join("cx/nesting").exists());

    assert!(s
        .run(
            "export.machine",
            json!({"dir": base.join("x"), "mode": "laser"})
        )
        .is_err());
    let _ = std::fs::remove_dir_all(&base);
}
