//! Programy WoodWOP `.mpr` (špecifikácia MVP, kap. 7.3).
//!
//! Formát je riadený šablónami z `machine.toml` (`[mpr]`). Kód rieši iba
//! súradnice, formát čísel, poradie blokov a rozdelenie na súbory F/B.
//! Výstup je čisté ASCII (diakritika sa prepíše, ostatné znaky na `?`).

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

use std::collections::{BTreeMap, BTreeSet};

use cc_model::{Face, LineEnding, Model, MprField, MprProfile, Operation, Part, P2};
pub use cc_out_util::fmt_num;
use cc_out_util::{file_stem, join_lines, substitute, to_ascii};

/// Jeden súbor `.mpr`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MprFile {
    /// Názov súboru, napr. `S1-SIDE-L.mpr`.
    pub name: String,
    pub part_id: String,
    /// ASCII obsah vrátane koncov riadkov.
    pub content: Vec<u8>,
}

#[derive(Debug, Clone, Default)]
pub struct MprOutput {
    pub files: Vec<MprFile>,
    /// Makrá s `verified = false`, ktoré sa vo výstupe použili (`BohrHoriz`, `Nuten`…).
    pub unverified_macros: BTreeSet<&'static str>,
}

impl MprOutput {
    /// `part_id` → názvy programov oddelené čiarkou (stĺpec `program` v rozpise).
    pub fn program_names(&self) -> BTreeMap<String, String> {
        let mut map: BTreeMap<String, String> = BTreeMap::new();
        for f in &self.files {
            map.entry(f.part_id.clone())
                .and_modify(|s| {
                    s.push(',');
                    s.push_str(&f.name);
                })
                .or_insert_with(|| f.name.clone());
        }
        map
    }
}

/// Vytvorí `.mpr` pre všetky diely s aspoň jednou operáciou.
pub fn render(model: &Model, profile: &MprProfile) -> MprOutput {
    let mut out = MprOutput::default();
    for cabinet in &model.cabinets {
        for part in &cabinet.parts {
            if part.operations.is_empty() {
                continue;
            }
            // Strana B: iba diery a drážky na ploche B. Hrany a plocha F idú do hlavného súboru.
            let (b_ops, f_ops): (Vec<&Operation>, Vec<&Operation>) = part
                .operations
                .iter()
                .partition(|op| op_face(op) == Face::B);
            let base = file_stem(&part.id);
            let ctx = Ctx {
                model,
                part,
                profile,
            };
            if b_ops.is_empty() {
                out.files.push(ctx.file(
                    format!("{base}.mpr"),
                    &f_ops,
                    false,
                    &mut out.unverified_macros,
                ));
            } else {
                if !f_ops.is_empty() {
                    out.files.push(ctx.file(
                        format!("{base}_F.mpr"),
                        &f_ops,
                        false,
                        &mut out.unverified_macros,
                    ));
                }
                out.files.push(ctx.file(
                    format!("{base}_B.mpr"),
                    &b_ops,
                    true,
                    &mut out.unverified_macros,
                ));
            }
        }
    }
    out
}

fn op_face(op: &Operation) -> Face {
    match op {
        Operation::Hole { face, .. } | Operation::Groove { face, .. } => *face,
    }
}

struct Ctx<'a> {
    model: &'a Model,
    part: &'a Part,
    profile: &'a MprProfile,
}

impl Ctx<'_> {
    /// `mirror`: súbor pre plochu B – diel otočený okolo osi y, `x' = L − x`.
    fn file(
        &self,
        name: String,
        ops: &[&Operation],
        mirror: bool,
        unverified: &mut BTreeSet<&'static str>,
    ) -> MprFile {
        let p = self.part;
        let prof = self.profile;
        let n = |v: f64| fmt_num(v, prof.decimals);
        let dims = [("l", n(p.length)), ("w", n(p.width)), ("t", n(p.thickness))];
        let mut lines: Vec<String> = Vec::new();

        lines.push("[H".into());
        push_fields(&mut lines, &prof.header, &dims);
        lines.push(String::new());

        lines.push("[001".into());
        if prof.inch_converter_var {
            lines.push("i=\"25.4\"".into());
            lines.push("KM=\"converter\"".into());
        }
        for ((var, value), label) in dims.iter().zip(["length", "width", "thickness"]) {
            lines.push(format!("{var}=\"{value}\""));
            lines.push(format!("KM=\"{label}\""));
        }
        lines.push(String::new());

        lines.push("<100 \\WerkStck\\".into());
        push_fields(&mut lines, &prof.workpiece, &dims);
        lines.push(String::new());

        let mx = |x: f64| if mirror { p.length - x } else { x };
        for op in ops {
            match **op {
                Operation::Hole {
                    face,
                    x,
                    y,
                    z,
                    diameter,
                    depth,
                    ..
                } => {
                    if matches!(face, Face::F | Face::B) {
                        lines.push("<102 \\BohrVert\\".into());
                        let vars = [
                            ("x", n(mx(x))),
                            ("y", n(y)),
                            ("d", n(diameter)),
                            ("depth", n(depth)),
                        ];
                        push_fields(&mut lines, &prof.bohr_vert.fields, &vars);
                        if !prof.bohr_vert.verified {
                            unverified.insert("BohrVert");
                        }
                    } else {
                        lines.push("<103 \\BohrHoriz\\".into());
                        let dir = match face {
                            Face::X0 => &prof.bohr_horiz.dir.X0,
                            Face::X1 => &prof.bohr_horiz.dir.X1,
                            Face::Y0 => &prof.bohr_horiz.dir.Y0,
                            _ => &prof.bohr_horiz.dir.Y1,
                        };
                        let vars = [
                            ("x", n(x)),
                            ("y", n(y)),
                            ("z", n(z)),
                            ("d", n(diameter)),
                            ("depth", n(depth)),
                            ("dir", dir.clone()),
                        ];
                        push_fields(&mut lines, &prof.bohr_horiz.fields, &vars);
                        if !prof.bohr_horiz.verified {
                            unverified.insert("BohrHoriz");
                        }
                    }
                }
                Operation::Groove {
                    start,
                    end,
                    width,
                    depth,
                    ..
                } => {
                    let (s, e) = extend_through(
                        P2 {
                            x: mx(start.x),
                            y: start.y,
                        },
                        P2 {
                            x: mx(end.x),
                            y: end.y,
                        },
                        p,
                        prof.nuten.through_extension,
                    );
                    lines.push("<109 \\Nuten\\".into());
                    let vars = [
                        ("xs", n(s.x)),
                        ("ys", n(s.y)),
                        ("xe", n(e.x)),
                        ("ye", n(e.y)),
                        ("width", n(width)),
                        ("depth", n(depth)),
                    ];
                    push_fields(&mut lines, &prof.nuten.fields, &vars);
                    if !prof.nuten.verified {
                        unverified.insert("Nuten");
                    }
                }
            }
            lines.push(String::new());
        }

        lines.push("<101 \\Kommentar\\".into());
        let comment_vars = [
            ("part_id", p.id.clone()),
            ("project", self.model.project.clone()),
            ("cabinet", p.cabinet.clone()),
            ("l", n(p.length)),
            ("w", n(p.width)),
            ("t", n(p.thickness)),
        ];
        push_fields(&mut lines, &prof.comment, &comment_vars);
        lines.push(String::new());
        lines.push("!".into());

        let text = join_lines(&lines, prof.line_ending == LineEnding::Crlf);
        MprFile {
            name,
            part_id: p.id.clone(),
            content: to_ascii(&text),
        }
    }
}

/// Drážka cez celý diel sa na oboch koncoch predĺži o `ext` (nábeh/výbeh nástroja).
fn extend_through(s: P2, e: P2, part: &Part, ext: f64) -> (P2, P2) {
    if ext <= 0.0 {
        return (s, e);
    }
    let tol = 1e-6;
    let horizontal = (s.y - e.y).abs() < tol;
    let vertical = (s.x - e.x).abs() < tol;
    let (lo, hi) = if horizontal { (s.x, e.x) } else { (s.y, e.y) };
    let full = if horizontal { part.length } else { part.width };
    let through = lo.min(hi) <= tol && lo.max(hi) >= full - tol;
    if !(through && (horizontal || vertical)) {
        return (s, e);
    }
    let sign = if hi >= lo { 1.0 } else { -1.0 };
    if horizontal {
        (
            P2 {
                x: s.x - sign * ext,
                y: s.y,
            },
            P2 {
                x: e.x + sign * ext,
                y: e.y,
            },
        )
    } else {
        (
            P2 {
                x: s.x,
                y: s.y - sign * ext,
            },
            P2 {
                x: e.x,
                y: e.y + sign * ext,
            },
        )
    }
}

fn push_fields(lines: &mut Vec<String>, fields: &[MprField], vars: &[(&str, String)]) {
    for [name, template] in fields {
        lines.push(format!("{name}=\"{}\"", substitute(template, vars)));
    }
}
