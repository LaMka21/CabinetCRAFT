//! Kontrola vrstiev crate-ov (plan/architecture.md §2).
//!
//! Bežná alebo build závislosť smie ísť iba na crate z nižšej vrstvy. Testovacie
//! (dev) závislosti sa nekontrolujú, okrem zákazu UI knižníc pod L5.

use serde_json::Value;

/// Najvyššia vrstva knižníc; nad ňou sú iba aplikácie.
const UI_LAYER: u8 = 5;

/// Tabuľka vrstiev. Nový crate treba doplniť sem aj do `plan/architecture.md`.
const LAYERS: &[(&str, u8)] = &[
    ("cc-i18n", 0),
    ("cc-model", 0),
    ("cc-out-util", 0),
    ("cc-config", 1),
    ("cc-room", 1),
    ("cc-nest", 1),
    ("cc-generate", 1),
    ("cc-joints", 1),
    ("cc-validate", 1),
    ("cc-import-cvj", 1),
    ("cc-out-csv", 2),
    ("cc-out-cvj", 2),
    ("cc-out-xcs", 2),
    ("cc-out-mpr", 2),
    ("cc-out-pdf", 2),
    ("cc-scene", 2),
    ("cc-pipeline", 3),
    ("cc-engine", 4),
    ("cc-control", 5),
    ("cc-mcp", 5),
];

/// Aplikácie (môžu závisieť od čohokoľvek).
const APPS: &[&str] = &["cc-cli", "cc-app"];

/// Nástroje mimo vrstiev.
const TOOLS: &[&str] = &["xtask"];

/// Externé knižnice, ktoré smú používať iba crate-y vrstvy L5 a aplikácie.
const UI_ONLY: &[&str] = &[
    "egui",
    "eframe",
    "egui-wgpu",
    "egui_extras",
    "wgpu",
    "winit",
    "rfd",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    Layer(u8),
    App,
    Tool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepKind {
    Normal,
    Dev,
    Build,
}

#[derive(Debug, Clone)]
pub struct Dep {
    pub name: String,
    pub kind: DepKind,
    /// Závislosť je crate z tohto workspace.
    pub workspace: bool,
}

#[derive(Debug, Clone)]
pub struct Crate {
    pub name: String,
    pub deps: Vec<Dep>,
}

pub fn classify(name: &str) -> Option<Class> {
    if let Some((_, l)) = LAYERS.iter().find(|(n, _)| *n == name) {
        return Some(Class::Layer(*l));
    }
    if APPS.contains(&name) {
        return Some(Class::App);
    }
    if TOOLS.contains(&name) {
        return Some(Class::Tool);
    }
    None
}

pub fn describe(class: Option<Class>) -> String {
    match class {
        Some(Class::Layer(l)) => format!("L{l}"),
        Some(Class::App) => "app".into(),
        Some(Class::Tool) => "nástroj".into(),
        None => "?".into(),
    }
}

pub fn from_metadata(meta: &Value) -> Result<Vec<Crate>, String> {
    let packages = meta
        .get("packages")
        .and_then(Value::as_array)
        .ok_or("cargo metadata: chýba `packages`")?;
    let names: Vec<&str> = packages
        .iter()
        .filter_map(|p| p.get("name").and_then(Value::as_str))
        .collect();
    let mut crates = Vec::new();
    for p in packages {
        let name = p
            .get("name")
            .and_then(Value::as_str)
            .ok_or("cargo metadata: balík bez mena")?;
        let mut deps = Vec::new();
        for d in p
            .get("dependencies")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default()
        {
            let Some(dep_name) = d.get("name").and_then(Value::as_str) else {
                continue;
            };
            let kind = match d.get("kind").and_then(Value::as_str) {
                Some("dev") => DepKind::Dev,
                Some("build") => DepKind::Build,
                _ => DepKind::Normal,
            };
            deps.push(Dep {
                name: dep_name.to_owned(),
                kind,
                workspace: names.contains(&dep_name),
            });
        }
        crates.push(Crate {
            name: name.to_owned(),
            deps,
        });
    }
    crates.sort_by_key(|c| {
        let order = match classify(&c.name) {
            Some(Class::Layer(l)) => u16::from(l),
            Some(Class::App) => 100,
            Some(Class::Tool) => 200,
            None => 300,
        };
        (order, c.name.clone())
    });
    Ok(crates)
}

pub fn check(crates: &[Crate]) -> Vec<String> {
    let mut out = Vec::new();
    for c in crates {
        let class = classify(&c.name);
        let own = match class {
            None => {
                out.push(format!(
                    "{}: crate nie je v tabuľke vrstiev (xtask/src/layers.rs, plan/architecture.md §2)",
                    c.name
                ));
                continue;
            }
            Some(Class::Tool) | Some(Class::App) => continue,
            Some(Class::Layer(l)) => l,
        };
        for d in &c.deps {
            if own < UI_LAYER && UI_ONLY.contains(&d.name.as_str()) {
                out.push(format!(
                    "{} (L{own}) závisí od UI knižnice `{}`; tú smie iba L{UI_LAYER} a aplikácie",
                    c.name, d.name
                ));
            }
            if !d.workspace || d.kind == DepKind::Dev {
                continue;
            }
            match classify(&d.name) {
                Some(Class::Layer(l)) if l < own => {}
                Some(Class::Layer(l)) => out.push(format!(
                    "{} (L{own}) závisí od {} (L{l}); smie iba od nižšej vrstvy",
                    c.name, d.name
                )),
                other => out.push(format!(
                    "{} (L{own}) závisí od {} ({})",
                    c.name,
                    d.name,
                    describe(other)
                )),
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn krate(name: &str, deps: &[(&str, DepKind, bool)]) -> Crate {
        Crate {
            name: name.into(),
            deps: deps
                .iter()
                .map(|(n, k, w)| Dep {
                    name: (*n).into(),
                    kind: *k,
                    workspace: *w,
                })
                .collect(),
        }
    }

    #[test]
    fn lower_layer_is_allowed() {
        let c = [krate("cc-pipeline", &[("cc-model", DepKind::Normal, true)])];
        assert!(check(&c).is_empty());
    }

    #[test]
    fn same_or_higher_layer_is_rejected() {
        let c = [
            krate("cc-generate", &[("cc-config", DepKind::Normal, true)]),
            krate("cc-model", &[("cc-engine", DepKind::Normal, true)]),
        ];
        assert_eq!(check(&c).len(), 2);
    }

    #[test]
    fn dev_dependencies_may_go_up() {
        let c = [krate("cc-joints", &[("cc-generate", DepKind::Dev, true)])];
        assert!(check(&c).is_empty());
    }

    #[test]
    fn ui_library_below_l5_is_rejected_even_in_dev() {
        let c = [
            krate("cc-engine", &[("egui", DepKind::Dev, false)]),
            krate("cc-app", &[("eframe", DepKind::Normal, false)]),
        ];
        assert_eq!(check(&c).len(), 1);
    }

    #[test]
    fn unknown_crate_is_reported() {
        assert_eq!(check(&[krate("cc-novy", &[])]).len(), 1);
    }
}
