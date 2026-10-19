//! Uloženie hodnôt do existujúceho TOML tak, aby ostali komentáre a poradie kľúčov.

use serde::Serialize;
use toml_edit::{DocumentMut, Item, Table, Value};

/// Hodnotu `value` zapíše do textu `existing` (ak je); zmenené kľúče prepíše a zachová
/// ich komentáre, chýbajúce doplní a nadbytočné odstráni.
pub fn merge<T: Serialize>(existing: Option<&str>, value: &T) -> Result<String, String> {
    let fresh = toml::to_string(value).map_err(|e| e.to_string())?;
    let Some(existing) = existing else {
        return Ok(fresh);
    };
    let Ok(mut doc) = existing.parse::<DocumentMut>() else {
        return Ok(fresh);
    };
    let new_doc: DocumentMut = fresh
        .parse()
        .map_err(|e: toml_edit::TomlError| e.to_string())?;
    merge_table(doc.as_table_mut(), new_doc.as_table());
    Ok(doc.to_string())
}

fn merge_table(dst: &mut Table, src: &Table) {
    let stale: Vec<String> = dst
        .iter()
        .map(|(k, _)| k.to_owned())
        .filter(|k| !src.contains_key(k))
        .collect();
    for key in stale {
        dst.remove(&key);
    }
    for (key, item) in src.iter() {
        match (dst.get_mut(key), item) {
            (Some(Item::Table(d)), Item::Table(s)) => merge_table(d, s),
            (Some(Item::ArrayOfTables(d)), Item::ArrayOfTables(s)) if d.len() == s.len() => {
                for (dt, st) in d.iter_mut().zip(s.iter()) {
                    merge_table(dt, st);
                }
            }
            (Some(Item::Value(d)), Item::Value(s)) => merge_value(d, s),
            // v súbore pole vložených tabuliek (`x = [{ … }, …]`), serializované `[[x]]`
            (Some(Item::Value(d)), Item::ArrayOfTables(s)) if d.is_array() => {
                merge_value(d, &Value::Array(s.clone().into_array()));
            }
            // v súbore vložená tabuľka (`x = { … }`), serializovaná ako `[a.x]`
            (Some(Item::Value(d)), Item::Table(s)) if d.is_inline_table() => {
                merge_value(d, &Value::InlineTable(s.clone().into_inline_table()));
            }
            (Some(slot), _) => *slot = item.clone(),
            (None, _) => {
                dst.insert(key, item.clone());
            }
        }
    }
}

/// Prepíše hodnotu iba pri zmene, s pôvodným komentárom.
fn merge_value(d: &mut Value, s: &Value) {
    if same_value(d, s) {
        return;
    }
    let decor = d.decor().clone();
    // celé číslo ostane celým číslom (`w = 650`, nie `650.0`)
    *d = match (&*d, s) {
        (Value::Integer(_), Value::Float(f))
            if f.value().fract() == 0.0 && f.value().abs() < 9.0e15 =>
        {
            Value::from(*f.value() as i64)
        }
        _ => s.clone(),
    };
    *d.decor_mut() = decor;
}

/// Rovnaká hodnota bez ohľadu na komentáre a zápis; `600` a `600.0` sú rovnaké.
fn same_value(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Integer(i), Value::Float(f)) | (Value::Float(f), Value::Integer(i)) => {
            *i.value() as f64 == *f.value()
        }
        (Value::Float(x), Value::Float(y)) => x.value() == y.value(),
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y.iter()).all(|(p, q)| same_value(p, q))
        }
        (Value::InlineTable(x), Value::InlineTable(y)) => {
            x.len() == y.len()
                && x.iter()
                    .all(|(k, v)| y.get(k).is_some_and(|w| same_value(v, w)))
        }
        _ => {
            let mut a = a.clone();
            let mut b = b.clone();
            a.decor_mut().clear();
            b.decor_mut().clear();
            a.to_string() == b.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Serialize)]
    struct Doc {
        a: A,
    }
    #[derive(Serialize)]
    struct A {
        w: f64,
        name: String,
    }

    #[test]
    fn keeps_comments_and_integers() {
        let text = "# hlavička\n[a]\nw = 600        # šírka\nname = \"x\"   # meno\n";
        let doc = |w| Doc {
            a: A {
                w,
                name: "x".into(),
            },
        };
        assert_eq!(merge(Some(text), &doc(600.0)).unwrap(), text);
        let changed = merge(Some(text), &doc(650.0)).unwrap();
        assert!(changed.contains("w = 650        # šírka"), "{changed}");
        assert!(changed.starts_with("# hlavička"));
    }

    /// Vložená tabuľka (`groove = { … }`, `end_groups = { … }` v `[[joint_rule]]`) ostane
    /// vložená; zmena prepíše iba ju.
    #[test]
    fn inline_tables_stay_inline() {
        let root =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/dielna");
        let text = std::fs::read_to_string(root.join("standards/dielna-cx100.toml")).unwrap();
        let mut value: cc_model::Standard = toml::from_str(&text).unwrap();
        if let Some(g) = value.led_rail.as_mut().and_then(|l| l.groove.as_mut()) {
            g.width = 16.0;
        }
        let merged = merge(Some(&text), &value).unwrap();
        let back: cc_model::Standard = toml::from_str(&merged).unwrap();
        assert_eq!(back, value);
        assert!(!merged.contains("[led_rail.groove]"), "{merged}");
        assert!(!merged.contains("end_groups]"), "{merged}");
        assert!(merged.contains("width = 16.0"), "{merged}");
    }

    /// Pole vložených tabuliek v `[[hinge]]`: zmena hodnoty prepíše iba to pole,
    /// komentáre okolo ostanú.
    #[test]
    fn inline_table_arrays_in_array_of_tables() {
        let root =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/dielna");
        let text = std::fs::read_to_string(root.join("standards/dielna-cx100.toml")).unwrap();
        let mut value: cc_model::Standard = toml::from_str(&text).unwrap();
        value.hinges[0].plate_holes[0].depth = 12.0;
        let merged = merge(Some(&text), &value).unwrap();
        let back: cc_model::Standard = toml::from_str(&merged).unwrap();
        assert_eq!(back, value);
        assert!(merged.contains("# [P] Ø35 × 13,5"), "{merged}");
        assert!(!merged.contains("[[hinge.plate_holes"), "{merged}");
    }

    /// Uloženie bez zmien nesmie zmeniť súbory vzorového projektu (komentáre, čísla).
    #[test]
    fn unchanged_files_stay_identical() {
        let root =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/dielna");
        let project = std::fs::read_to_string(root.join("project.toml")).unwrap();
        let value: cc_model::ProjectFile = toml::from_str(&project).unwrap();
        assert_eq!(merge(Some(&project), &value).unwrap(), project);
        let standard = std::fs::read_to_string(root.join("standards/dielna-cx100.toml")).unwrap();
        let value: cc_model::Standard = toml::from_str(&standard).unwrap();
        assert_eq!(merge(Some(&standard), &value).unwrap(), standard);
    }
}
