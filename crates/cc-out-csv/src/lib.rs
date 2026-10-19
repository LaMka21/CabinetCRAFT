//! Rozpis dielov `cutlist.csv` (špecifikácia MVP, kap. 7.1).
//!
//! UTF-8, oddeľovač `;`, rozmery s 1 desatinným miestom.

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

use std::collections::BTreeMap;
use std::fmt::Write as _;

use cc_model::{Edge, Model, Part};

pub const HEADER: [&str; 14] = [
    "part_id",
    "cabinet",
    "role",
    "material",
    "length",
    "width",
    "thickness",
    "qty",
    "grain",
    "edge_X0",
    "edge_X1",
    "edge_Y0",
    "edge_Y1",
    "program",
];

#[derive(Debug, Clone, Copy, Default)]
pub struct CsvOptions {
    /// Zlúčiť rovnaké diely do jedného riadku s `qty > 1`.
    pub merge: bool,
    /// Desatinná čiarka namiesto bodky.
    pub decimal_comma: bool,
}

/// Jeden riadok rozpisu. Pri zlúčení nesie ID všetkých zlúčených dielov.
#[derive(Debug, Clone, PartialEq)]
struct Row<'a> {
    ids: Vec<&'a str>,
    part: &'a Part,
}

impl Row<'_> {
    /// Kľúč zhody pre `--merge`: všetko okrem ID a skrinky.
    fn same_as(&self, other: &Part) -> bool {
        let p = self.part;
        p.role.as_str() == other.role.as_str()
            && p.material == other.material
            && fmt1(p.length) == fmt1(other.length)
            && fmt1(p.width) == fmt1(other.width)
            && fmt1(p.thickness) == fmt1(other.thickness)
            && p.grain == other.grain
            && p.edges == other.edges
    }
}

/// Vytvorí obsah `cutlist.csv` bez stĺpca `program`.
pub fn render(model: &Model, opts: CsvOptions) -> String {
    render_with_programs(model, opts, &BTreeMap::new())
}

/// Vytvorí obsah `cutlist.csv`; `programs` je `part_id` → názvy `.mpr` (kap. 7.1, 7.3).
pub fn render_with_programs(
    model: &Model,
    opts: CsvOptions,
    programs: &BTreeMap<String, String>,
) -> String {
    let mut rows: Vec<Row> = Vec::new();
    for part in model.parts() {
        if opts.merge {
            if let Some(row) = rows.iter_mut().find(|r| r.same_as(part)) {
                row.ids.push(&part.id);
                continue;
            }
        }
        rows.push(Row {
            ids: vec![&part.id],
            part,
        });
    }

    let mut out = String::new();
    out.push_str(&HEADER.join(";"));
    out.push('\n');
    for row in &rows {
        let p = row.part;
        let num = |v: f64| {
            let s = fmt1(v);
            if opts.decimal_comma {
                s.replace('.', ",")
            } else {
                s
            }
        };
        let cabinet = if row.ids.len() > 1 && !all_same_cabinet(model, &row.ids) {
            String::new()
        } else {
            p.cabinet.clone()
        };
        let fields = [
            row.ids.join(","),
            cabinet,
            p.role.as_str().to_owned(),
            p.material.clone(),
            num(p.length),
            num(p.width),
            num(p.thickness),
            row.ids.len().to_string(),
            p.grain.as_str().to_owned(),
            p.edges.get(Edge::X0).unwrap_or_default().to_owned(),
            p.edges.get(Edge::X1).unwrap_or_default().to_owned(),
            p.edges.get(Edge::Y0).unwrap_or_default().to_owned(),
            p.edges.get(Edge::Y1).unwrap_or_default().to_owned(),
            row.ids
                .iter()
                .filter_map(|id| programs.get(*id).map(String::as_str))
                .collect::<Vec<_>>()
                .join(","),
        ];
        let line = fields
            .iter()
            .map(|f| escape(f))
            .collect::<Vec<_>>()
            .join(";");
        let _ = writeln!(out, "{line}");
    }
    out
}

fn all_same_cabinet(model: &Model, ids: &[&str]) -> bool {
    let mut cabinets = model
        .parts()
        .filter(|p| ids.contains(&p.id.as_str()))
        .map(|p| p.cabinet.as_str());
    let first = cabinets.next();
    cabinets.all(|c| Some(c) == first)
}

/// Dĺžka s 1 desatinným miestom, bez „-0.0“.
fn fmt1(v: f64) -> String {
    let s = format!("{v:.1}");
    if s == "-0.0" {
        "0.0".to_owned()
    } else {
        s
    }
}

/// Pole s `;`, `"` alebo koncom riadku dá do úvodzoviek.
fn escape(field: &str) -> String {
    if field.contains([';', '"', '\n', '\r']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_format() {
        assert_eq!(fmt1(521.5), "521.5");
        assert_eq!(fmt1(720.0), "720.0");
        assert_eq!(fmt1(-0.0001), "0.0");
    }

    #[test]
    fn escaping() {
        assert_eq!(escape("a;b"), "\"a;b\"");
        assert_eq!(escape("12\" doska"), "\"12\"\" doska\"");
        assert_eq!(escape("bez"), "bez");
    }
}
