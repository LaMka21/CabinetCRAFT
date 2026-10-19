//! Validácia geometrie a obrábania (špecifikácia MVP, kap. 8).
//!
//! Pravidlá `JointTooShort` a `UnsupportedToolAxis` hlási už `cc-joints`
//! ako chybu pri tvorbe operácií; tu sa kontroluje hotový model.

#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::todo,
        clippy::unimplemented,
        clippy::unreachable
    )
)]

use cc_i18n::tr;
use cc_model::{
    CabinetModel, Face, Hardware, Issue, IssueCode, MachineProfile, Model, Operation, Part,
    ProductionMode, Role, EPS, P2,
};
use glam::DVec3;

/// Výsledok validácie.
#[derive(Debug, Clone, Default)]
pub struct Report {
    pub issues: Vec<Issue>,
}

impl Report {
    pub fn has_errors(&self) -> bool {
        self.issues
            .iter()
            .any(|i| i.severity == cc_model::Severity::Error)
    }

    pub fn errors(&self) -> impl Iterator<Item = &Issue> {
        self.issues
            .iter()
            .filter(|i| i.severity == cc_model::Severity::Error)
    }

    pub fn warnings(&self) -> impl Iterator<Item = &Issue> {
        self.issues
            .iter()
            .filter(|i| i.severity == cc_model::Severity::Warning)
    }

    pub fn codes(&self) -> Vec<IssueCode> {
        self.issues.iter().map(|i| i.code).collect()
    }
}

/// Skontroluje model. Bez profilu stroja sa vynechajú pravidlá závislé od stroja
/// (`PartTooSmall` voči min. dielu, `PartTooLarge`, `ToolMissing`).
pub fn check(model: &Model, machine: Option<&MachineProfile>) -> Report {
    let mut issues = Vec::new();
    for cabinet in &model.cabinets {
        for part in &cabinet.parts {
            check_part(part, machine, &mut issues);
        }
        check_shelf_support(cabinet, &mut issues);
    }
    Report { issues }
}

/// Diera rozložená na súradnice v rovine strany (a, b) pre porovnávanie.
struct HoleRef {
    face: Face,
    /// poloha v rovine strany: plocha (x, y), hrana X (y, z), hrana Y (x, z)
    a: f64,
    b: f64,
    diameter: f64,
    index: usize,
}

fn is_face(face: Face) -> bool {
    matches!(face, Face::F | Face::B)
}

fn check_part(part: &Part, machine: Option<&MachineProfile>, issues: &mut Vec<Issue>) {
    let id = Some(part.id.as_str());
    let (l, w, t) = (part.length, part.width, part.thickness);

    // PartTooSmall / PartTooLarge
    if l <= EPS || w <= EPS || t <= EPS {
        issues.push(Issue::error(
            IssueCode::PartTooSmall,
            id,
            tr!("validate.invalid_size", l = l, w = w, t = t),
        ));
        return;
    }
    let machined = !part.operations.is_empty();
    // nesting: diel, ktorý má iba diery do jednej plochy, sa celý obrobí na tabuli
    // (findings 08 §3) a na CX100 nejde – rozmerové limity CX100 preň neplatia
    let only_on_sheet = |face: Face| {
        part.operations
            .iter()
            .all(|op| matches!(op, Operation::Hole { face: f, .. } if *f == face))
    };
    let nesting_only = machine.is_some_and(|m| m.production.mode == ProductionMode::Nesting)
        && (only_on_sheet(Face::F) || only_on_sheet(Face::B));
    if let (Some(m), true) = (machine, machined && !nesting_only) {
        if l < m.min_part.length - EPS || w < m.min_part.width - EPS {
            issues.push(Issue::error(
                IssueCode::PartTooSmall,
                id,
                tr!(
                    "validate.part_too_small",
                    l = l,
                    w = w,
                    min_l = m.min_part.length,
                    min_w = m.min_part.width
                ),
            ));
        }
        if l > m.work_area.max_length + EPS || w > m.work_area.max_width + EPS {
            issues.push(Issue::error(
                IssueCode::PartTooLarge,
                id,
                tr!(
                    "validate.part_too_large",
                    l = l,
                    w = w,
                    max_l = m.work_area.max_length,
                    max_w = m.work_area.max_width
                ),
            ));
        }
    }

    let mut holes = Vec::new();
    let mut grooves = Vec::new();
    for (index, op) in part.operations.iter().enumerate() {
        match *op {
            Operation::Hole {
                face,
                x,
                y,
                z,
                diameter,
                depth,
                through,
                ..
            } => {
                check_hole(part, face, x, y, z, diameter, depth, through, issues);
                let (a, b) = match face {
                    Face::F | Face::B => (x, y),
                    Face::X0 | Face::X1 => (y, z),
                    Face::Y0 | Face::Y1 => (x, z),
                };
                holes.push(HoleRef {
                    face,
                    a,
                    b,
                    diameter,
                    index,
                });
            }
            Operation::Groove {
                face,
                start,
                end,
                width,
                depth,
            } => {
                if depth > t - 6.0 + EPS {
                    issues.push(Issue::error(
                        IssueCode::GrooveTooDeep,
                        id,
                        tr!(
                            "validate.groove_too_deep",
                            depth = depth,
                            t = t,
                            max = t - 6.0
                        ),
                    ));
                }
                grooves.push((face, start, end, width));
            }
        }
    }

    // ToolMissing: jedno hlásenie na diel a chýbajúci priemer
    if let Some(mach) = machine {
        let mut missing: Vec<(f64, usize)> = Vec::new();
        for h in holes.iter().filter(|h| !mach.has_drill(h.diameter)) {
            match missing
                .iter_mut()
                .find(|(d, _)| (d - h.diameter).abs() < 1e-3)
            {
                Some(entry) => entry.1 += 1,
                None => missing.push((h.diameter, 1)),
            }
        }
        for (d, count) in missing {
            issues.push(Issue::error(
                IssueCode::ToolMissing,
                id,
                tr!(
                    "validate.tool_missing",
                    d = d,
                    machine = &mach.machine.name,
                    holes = count
                ),
            ));
        }
    }

    // HoleCollision: dvojice dier na rovnakej strane
    for (i, h1) in holes.iter().enumerate() {
        for h2 in &holes[i + 1..] {
            if h1.face != h2.face {
                continue;
            }
            let dist = ((h1.a - h2.a).powi(2) + (h1.b - h2.b).powi(2)).sqrt();
            let min = (h1.diameter + h2.diameter) / 2.0 + 2.0;
            if dist < min - EPS {
                issues.push(Issue::error(
                    IssueCode::HoleCollision,
                    id,
                    tr!(
                        "validate.hole_collision",
                        a = h1.index + 1,
                        b = h2.index + 1,
                        face = format!("{:?}", h1.face),
                        dist = format!("{dist:.2}"),
                        min = format!("{min:.2}")
                    ),
                ));
            }
        }
    }

    // HoleInGroove: diera na ploche zasahuje do drážky na tej istej ploche
    for h in holes.iter().filter(|h| is_face(h.face)) {
        for (face, start, end, width) in &grooves {
            if *face != h.face {
                continue;
            }
            let dist = point_segment_distance(P2 { x: h.a, y: h.b }, *start, *end);
            let min = width / 2.0 + h.diameter / 2.0;
            if dist < min - EPS {
                issues.push(Issue::error(
                    IssueCode::HoleInGroove,
                    id,
                    tr!(
                        "validate.hole_in_groove",
                        hole = h.index + 1,
                        d = h.diameter,
                        dist = format!("{dist:.2}"),
                        min = format!("{min:.2}")
                    ),
                ));
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn check_hole(
    part: &Part,
    face: Face,
    x: f64,
    y: f64,
    z: f64,
    diameter: f64,
    depth: f64,
    through: bool,
    issues: &mut Vec<Issue>,
) {
    let id = Some(part.id.as_str());
    let (l, w, t) = (part.length, part.width, part.thickness);
    let m = diameter / 2.0 + 2.0;
    let inside = |v: f64, lo: f64, hi: f64| v >= lo - EPS && v <= hi + EPS;

    // HoleOutside
    let outside = match face {
        Face::F | Face::B => !(inside(x, m, l - m) && inside(y, m, w - m)),
        Face::X0 | Face::X1 => {
            !(inside(y, m, w - m) && inside(z, diameter / 2.0, t - diameter / 2.0))
        }
        Face::Y0 | Face::Y1 => {
            !(inside(x, m, l - m) && inside(z, diameter / 2.0, t - diameter / 2.0))
        }
    };
    if outside {
        issues.push(Issue::error(
            IssueCode::HoleOutside,
            id,
            tr!(
                "validate.hole_outside",
                d = diameter,
                face = format!("{face:?}"),
                x = x,
                y = y,
                z = z,
                margin = m
            ),
        ));
    }

    if is_face(face) {
        // FaceHoleTooDeep
        if !through && depth > t - 3.0 + EPS {
            issues.push(Issue::error(
                IssueCode::FaceHoleTooDeep,
                id,
                tr!(
                    "validate.face_hole_too_deep",
                    depth = depth,
                    t = t,
                    max = t - 3.0
                ),
            ));
        }
    } else {
        // EdgeHoleTooDeep / EdgeHoleTooWide
        let extent = match face {
            Face::X0 | Face::X1 => l,
            _ => w,
        };
        if depth > extent - 5.0 + EPS {
            issues.push(Issue::error(
                IssueCode::EdgeHoleTooDeep,
                id,
                tr!(
                    "validate.edge_hole_too_deep",
                    depth = depth,
                    face = format!("{face:?}"),
                    extent = extent
                ),
            ));
        }
        if diameter > t - 6.0 + EPS {
            issues.push(Issue::error(
                IssueCode::EdgeHoleTooWide,
                id,
                tr!("validate.edge_hole_too_wide", d = diameter, t = t),
            ));
        }
    }
}

/// ShelfNoSupport: skrinka má polohovateľné police, ale bok nemá v každom rade
/// aspoň 2 diery (alebo nemá rad vôbec).
fn check_shelf_support(cabinet: &CabinetModel, issues: &mut Vec<Issue>) {
    let parts = &cabinet.parts;
    if !parts.iter().any(|p| p.role == Role::AdjustableShelf) {
        return;
    }
    // diery kovania (podložky pántov, výsuvy) nie sú rad pre police
    let hardware_z = |z: f64| {
        cabinet.hardware.iter().any(|hw| match hw {
            Hardware::Hinge { z: hz, .. } => (z - hz).abs() <= 20.0,
            Hardware::Drawer {
                box_bottom,
                box_height,
                ..
            } => z >= box_bottom - 1.0 && z <= box_bottom + box_height + 1.0,
            Hardware::Filler { .. } => false,
        })
    };
    for side in parts
        .iter()
        .filter(|p| matches!(p.role, Role::SideLeft | Role::SideRight))
    {
        // Rady = diery radu (nie kolíky) zoskupené podľa polohy v hĺbke (y dielu).
        let mut rows: Vec<(f64, usize)> = Vec::new();
        for op in &side.operations {
            if let Operation::Hole {
                face,
                x,
                y,
                dowel: false,
                ..
            } = *op
            {
                if !is_face(face) {
                    continue;
                }
                let z = side.transform.point_to_cabinet(DVec3::new(x, y, 0.0)).z;
                if hardware_z(z) {
                    continue;
                }
                match rows.iter_mut().find(|(ry, _)| (ry - y).abs() < 1e-6) {
                    Some(row) => row.1 += 1,
                    None => rows.push((y, 1)),
                }
            }
        }
        let weak = rows.iter().filter(|(_, n)| *n < 2).count();
        if rows.is_empty() || weak > 0 {
            issues.push(Issue::warning(
                IssueCode::ShelfNoSupport,
                Some(&side.id),
                if rows.is_empty() {
                    tr!("validate.shelf_no_rows")
                } else {
                    tr!("validate.shelf_weak_rows", count = weak)
                },
            ));
        }
    }
}

fn point_segment_distance(p: P2, a: P2, b: P2) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    let t = if len2 <= EPS {
        0.0
    } else {
        (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0)
    };
    let (cx, cy) = (a.x + t * dx, a.y + t * dy);
    ((p.x - cx).powi(2) + (p.y - cy).powi(2)).sqrt()
}

#[cfg(test)]
mod i18n_keys {
    /// Kľúče z volaní makra prekladu v zdroji (aj keď rustfmt dá kľúč na ďalší riadok,
    /// ktorý `cargo xtask i18n` nevidí).
    fn keys(src: &str) -> Vec<&str> {
        src.match_indices(concat!("tr", "!("))
            // nie `include_str!(`
            .filter(|(i, _)| {
                !src[..*i]
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_alphanumeric() || c == '_')
            })
            .filter_map(|(i, m)| {
                src[i + m.len()..]
                    .trim_start()
                    .strip_prefix('"')?
                    .split('"')
                    .next()
            })
            .filter(|k| {
                k.contains('.')
                    && k.chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
            })
            .collect()
    }

    #[test]
    fn every_key_is_in_builtin_languages() {
        let sources = [include_str!("lib.rs")];
        let keys: Vec<&str> = sources.iter().flat_map(|s| keys(s)).collect();
        assert!(keys.len() >= 13, "{keys:?}");
        for (code, text) in cc_i18n::BUILTIN {
            let catalog = cc_i18n::Catalog::parse(code, text).unwrap();
            let missing: Vec<&&str> = keys.iter().filter(|k| !catalog.contains(k)).collect();
            assert!(missing.is_empty(), "{code}.toml: chýbajú {missing:?}");
        }
    }
}
