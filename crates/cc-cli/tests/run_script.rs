//! `cabinetcraft run --script` a `cabinetcraft commands`.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn exe() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cabinetcraft"))
}

fn temp_project(name: &str) -> PathBuf {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/dielna");
    let dir = std::env::temp_dir().join(format!("cc-cli-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    copy_dir(&src, &dir);
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

fn lines(out: &[u8]) -> Vec<Value> {
    String::from_utf8_lossy(out)
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

#[test]
fn script_runs_commands_and_saves() {
    let dir = temp_project("save");
    let script = dir.join("skript.jsonl");
    std::fs::write(
        &script,
        "# pridá skrinku a zmení šírku\n\
         {\"command\": \"cabinet.add\", \"params\": {\"type\": \"base\", \"w\": 450}}\n\
         \n\
         {\"command\": \"cabinet.set\", \"params\": {\"id\": \"SP1\", \"set\": {\"w\": 900}}}\n\
         {\"command\": \"model.inspect\"}\n",
    )
    .unwrap();
    let out = exe()
        .arg("run")
        .arg(dir.join("project.toml"))
        .arg("--script")
        .arg(&script)
        .arg("--save")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let l = lines(&out.stdout);
    assert_eq!(l.len(), 4);
    assert_eq!(l[0]["result"]["id"], "SP2");
    assert_eq!(l[0]["line"], 2);
    assert_eq!(l[2]["result"]["cabinets"], 3);
    assert_eq!(l[3]["command"], "project.save");
    let saved = std::fs::read_to_string(dir.join("project.toml")).unwrap();
    assert!(saved.contains("w = 900"), "{saved}");
    assert!(saved.contains("id = \"SP2\""), "{saved}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn script_stops_at_first_error() {
    let dir = temp_project("error");
    let script = dir.join("skript.jsonl");
    std::fs::write(
        &script,
        "{\"command\": \"cabinet.remove\", \"params\": {\"id\": \"XX\"}}\n\
         {\"command\": \"cabinet.add\", \"params\": {\"type\": \"wall\"}}\n",
    )
    .unwrap();
    let out = exe()
        .arg("run")
        .arg(dir.join("project.toml"))
        .arg("--script")
        .arg(&script)
        .output()
        .unwrap();
    assert!(!out.status.success());
    let l = lines(&out.stdout);
    assert_eq!(l.len(), 1);
    assert_eq!(l[0]["ok"], false);
    assert_eq!(l[0]["kind"], "failed");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn serve_answers_over_tcp() {
    use std::io::{BufRead, BufReader, Write};

    let dir = temp_project("serve");
    let mut child = exe()
        .arg("serve")
        .arg(dir.join("project.toml"))
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut out = BufReader::new(child.stdout.take().unwrap());
    let mut first = String::new();
    out.read_line(&mut first).unwrap();
    let port = serde_json::from_str::<Value>(&first).unwrap()["port"]
        .as_u64()
        .unwrap();
    let stream = std::net::TcpStream::connect(("127.0.0.1", port as u16)).unwrap();
    let mut writer = stream.try_clone().unwrap();
    let mut reader = BufReader::new(stream);
    let mut call = |line: &str| -> Value {
        writeln!(writer, "{line}").unwrap();
        let mut answer = String::new();
        reader.read_line(&mut answer).unwrap();
        serde_json::from_str(&answer).unwrap()
    };
    let r = call(
        r#"{"id":1,"method":"engine.execute","params":{"command":"cabinet.add","params":{"type":"wall"}}}"#,
    );
    assert_eq!(r["result"]["id"], "HO2", "{r}");
    let r = call(r#"{"id":2,"method":"ui.screenshot","params":{}}"#);
    assert_eq!(r["kind"], "unavailable");
    let r = call(r#"{"id":3,"method":"control.methods"}"#);
    assert!(r["result"]
        .as_array()
        .unwrap()
        .iter()
        .any(|m| m["method"] == "app.quit"));
    let r = call(r#"{"id":4,"method":"app.quit"}"#);
    assert_eq!(r["ok"], true);
    assert!(child.wait().unwrap().success());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn commands_lists_registry() {
    let out = exe().arg("commands").output().unwrap();
    assert!(out.status.success());
    let list: Value = serde_json::from_slice(&out.stdout).unwrap();
    let ids: Vec<&str> = list
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&"cabinet.add"));
    assert!(ids.contains(&"edit.undo"));
}
