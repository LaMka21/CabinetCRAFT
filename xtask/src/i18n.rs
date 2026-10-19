//! `cargo xtask i18n`: kontrola súborov prekladu `locales/*.toml`.
//!
//! - Každý jazyk voči referenčnému `en.toml`: chýbajúce kľúče, kľúče navyše, iné parametre.
//!   Vstavané jazyky (sk, en) musia byť úplné, inak kontrola zlyhá; ostatné jazyky
//!   (preklady komunity) sa iba vypíšu.
//! - Kľúče v kóde: každé `tr!("…")`, `tr("…")` a `tr_args("…")` v `crates/*/src` musí byť
//!   v `en.toml` aj `sk.toml`.

use std::path::{Path, PathBuf};

use cc_i18n::{diff, Catalog, BUILTIN, REFERENCE_LANGUAGE};

pub fn run(root: &Path) -> Result<(), String> {
    let dir = root.join("locales");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "toml"))
        .collect();
    files.sort();
    let mut catalogs = Vec::new();
    let mut errors = Vec::new();
    for path in &files {
        let code = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_owned();
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        match Catalog::parse(&code, &text) {
            Ok(c) => catalogs.push(c),
            Err(e) => errors.push(format!("{}: {e}", path.display())),
        }
    }
    let Some(reference) = catalogs.iter().find(|c| c.code == REFERENCE_LANGUAGE) else {
        return Err(format!(
            "locales/en.toml chýba alebo sa nedá prečítať: {}",
            errors.join("; ")
        ));
    };
    println!(
        "Referencia {REFERENCE_LANGUAGE}.toml: {} kľúčov",
        reference.len()
    );
    for c in &catalogs {
        let builtin = BUILTIN.iter().any(|(code, _)| *code == c.code);
        let d = diff(reference, c);
        let status = if d.is_empty() { "ok" } else { "rozdiely" };
        println!(
            "\n{}.toml ({}){}: {} kľúčov, chýba {}, navyše {}, parametre {} – {status}",
            c.code,
            c.name,
            if builtin { ", vstavaný" } else { "" },
            c.len(),
            d.missing.len(),
            d.extra.len(),
            d.placeholders.len(),
        );
        for k in &d.missing {
            println!("  chýba    {k}");
        }
        for k in &d.extra {
            println!("  navyše   {k}");
        }
        for k in &d.placeholders {
            println!("  parameter {k}");
        }
        for w in &c.warnings {
            println!("  pozor    {w}");
        }
        if builtin && (!d.is_empty() || !c.warnings.is_empty()) {
            errors.push(format!("{}.toml nie je úplný voči en.toml", c.code));
        }
    }
    if !BUILTIN
        .iter()
        .all(|(code, _)| catalogs.iter().any(|c| c.code == *code))
    {
        errors.push("chýba niektorý vstavaný jazyk (sk.toml, en.toml)".into());
    }

    // kľúče v kóde
    let mut used = Vec::new();
    let crates = root.join("crates");
    collect_keys(&crates, &mut used)?;
    used.sort();
    used.dedup_by(|a, b| a.0 == b.0);
    let mut unknown = 0;
    for (key, place) in &used {
        for c in catalogs
            .iter()
            .filter(|c| BUILTIN.iter().any(|(code, _)| *code == c.code))
        {
            if !c.contains(key) {
                println!("  v kóde, nie v {}.toml: {key} ({place})", c.code);
                unknown += 1;
            }
        }
    }
    println!("\nKľúče v kóde: {} rôznych", used.len());
    if unknown > 0 {
        errors.push(format!("{unknown} kľúčov z kódu chýba v sk/en"));
    }
    if errors.is_empty() {
        println!("OK: preklady sú úplné.");
        Ok(())
    } else {
        for e in &errors {
            println!("CHYBA: {e}");
        }
        Err(format!("{} chýb v prekladoch", errors.len()))
    }
}

/// Kľúče z volaní `tr!("…")`, `tr("…")`, `tr_args("…")` vo všetkých `.rs`.
fn collect_keys(dir: &Path, out: &mut Vec<(String, String)>) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            // cc-i18n má v dokumentácii a testoch vzorové kľúče
            if path
                .file_name()
                .is_some_and(|n| n == "target" || n == "cc-i18n")
            {
                continue;
            }
            collect_keys(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "rs") {
            let text =
                std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            for (line, key) in keys_in_text(&text) {
                out.push((key, format!("{}:{line}", path.display())));
            }
        }
    }
    Ok(())
}

/// Kľúče a čísla riadkov; kľúč môže byť na riadku za `tr!(` (rustfmt pri dlhom volaní).
/// Riadky komentárov `//` sa vynechajú.
fn keys_in_text(text: &str) -> Vec<(usize, String)> {
    let code: String = text
        .lines()
        .map(|l| {
            if l.trim_start().starts_with("//") {
                ""
            } else {
                l
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let mut keys = Vec::new();
    for pat in ["tr!(", "tr(", "tr_args("] {
        let mut from = 0;
        while let Some(pos) = code[from..].find(pat).map(|p| p + from) {
            from = pos + pat.len();
            // `tr(` nesmie byť koncom iného mena (`attr(`, `str(`)
            let before = code[..pos].chars().next_back();
            if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                continue;
            }
            let Some(lit) = code[from..].trim_start().strip_prefix('"') else {
                continue;
            };
            if let Some(end) = lit.find('"') {
                let key = &lit[..end];
                if !key.is_empty()
                    && key
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
                {
                    let line = code[..pos].matches('\n').count() + 1;
                    keys.push((line, key.to_owned()));
                }
            }
        }
    }
    keys
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_keys() {
        let keys = |t: &str| -> Vec<String> { keys_in_text(t).into_iter().map(|k| k.1).collect() };
        assert_eq!(
            keys(r#"ui.button(tr!("ribbon.return")); x(cc_i18n::tr("a.b"), attr("no"))"#),
            ["ribbon.return", "a.b"]
        );
        assert_eq!(
            keys(r#"tr!( "x.y", count = n) tr!(key) tr_args("z", &[])"#),
            ["x.y", "z"]
        );
        let multi = "a\n// tr!(\"komentar\")\nlet s = tr!(\n    \"app.saved\",\n    name = n\n);";
        assert_eq!(keys_in_text(multi), [(3, "app.saved".to_owned())]);
    }
}
