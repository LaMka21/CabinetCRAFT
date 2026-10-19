//! `cargo xtask stats`: riadky Rustu a počet testov (`#[test]`) na crate.

use std::path::Path;

pub fn run(root: &Path) -> Result<(), String> {
    let mut rows = Vec::new();
    for group in ["crates", "xtask"] {
        let dir = root.join(group);
        if group == "xtask" {
            rows.push(("xtask".to_owned(), count(&dir)?));
            continue;
        }
        let entries = std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let mut names: Vec<_> = entries
            .filter_map(Result::ok)
            .filter(|e| e.path().join("Cargo.toml").is_file())
            .collect();
        names.sort_by_key(|e| e.file_name());
        for e in names {
            rows.push((
                e.file_name().to_string_lossy().into_owned(),
                count(&e.path())?,
            ));
        }
    }
    println!("{:<16} {:>8} {:>7}", "crate", "riadky", "testy");
    let (mut lines, mut tests) = (0, 0);
    for (name, (l, t)) in &rows {
        println!("{name:<16} {l:>8} {t:>7}");
        lines += l;
        tests += t;
    }
    println!("{:<16} {lines:>8} {tests:>7}", "spolu");
    Ok(())
}

fn count(dir: &Path) -> Result<(usize, usize), String> {
    let (mut lines, mut tests) = (0, 0);
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let entries = std::fs::read_dir(&d).map_err(|e| format!("{}: {e}", d.display()))?;
        for e in entries.filter_map(Result::ok) {
            let p = e.path();
            if p.is_dir() {
                if p.file_name().is_some_and(|n| n != "target") {
                    stack.push(p);
                }
            } else if p.extension().is_some_and(|x| x == "rs") {
                let text =
                    std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
                lines += text.lines().count();
                tests += text.matches("#[test]").count();
            }
        }
    }
    Ok((lines, tests))
}
