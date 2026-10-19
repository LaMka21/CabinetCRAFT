//! Programy SCM Maestro `.xcs` pre CX100 (špecifikácia MVP, kap. 7.4).
//!
//! Formát vychádza z programov, ktoré pre CX100 vyrábal CABINET VISION
//! (zákazka REF2026). Celý text riadkov je v šablónach `machine.toml` (`[xcs]`);
//! kód rieši roviny, súradnice, prechody pílky, číslovanie a súbory F/B.

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

pub mod nest;

use std::collections::{BTreeMap, BTreeSet};

use cc_model::{Face, LineEnding, Model, Operation, Part, XcsProfile, P2};
use cc_out_util::{file_stem, fmt_num, join_lines, substitute, to_ascii};

/// Jeden program `.xcs`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XcsFile {
    /// Názov súboru, napr. `S1-SIDE-L.xcs`.
    pub name: String,
    pub part_id: String,
    /// ASCII obsah vrátane koncov riadkov.
    pub content: Vec<u8>,
}

#[derive(Debug, Clone, Default)]
pub struct XcsOutput {
    pub files: Vec<XcsFile>,
    /// Dávka pre XConverter: (názov, obsah).
    pub run_bat: Option<(String, Vec<u8>)>,
    /// Šablóny s `verified = false`, ktoré sa vo výstupe použili.
    pub unverified: BTreeSet<&'static str>,
}

impl XcsOutput {
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

/// Vytvorí `.xcs` pre všetky diely s aspoň jednou operáciou (a `RUN.bat`, ak je v profile).
pub fn render(model: &Model, profile: &XcsProfile) -> XcsOutput {
    let mut out = XcsOutput::default();
    for part in model.parts() {
        if part.operations.is_empty() {
            continue;
        }
        // Plocha B ide do samostatného programu s otočeným dielom; hrany a F do hlavného.
        let (b_ops, f_ops): (Vec<&Operation>, Vec<&Operation>) = part
            .operations
            .iter()
            .partition(|op| op_face(op) == Face::B);
        let base = file_stem(&part.id);
        let writer = Writer {
            model,
            part,
            profile,
        };
        if b_ops.is_empty() {
            out.files
                .push(writer.file(format!("{base}.xcs"), &f_ops, false, &mut out.unverified));
        } else {
            if !f_ops.is_empty() {
                out.files.push(writer.file(
                    format!("{base}_F.xcs"),
                    &f_ops,
                    false,
                    &mut out.unverified,
                ));
            }
            out.files
                .push(writer.file(format!("{base}_B.xcs"), &b_ops, true, &mut out.unverified));
        }
    }
    if let Some(bat) = &profile.run_bat {
        let dir = bat.target_dir.trim_end_matches(['\\', '/']);
        let mut lines: Vec<String> = bat.header.clone();
        for f in &out.files {
            let stem = f.name.trim_end_matches(".xcs");
            let vars = [
                ("input", format!("{dir}\\{}", f.name)),
                ("output", format!("{dir}\\{stem}.pgmx")),
                ("stem", stem.to_owned()),
            ];
            lines.push(substitute(&bat.line, &vars));
        }
        // dávka pre Windows: vždy CRLF
        out.run_bat = Some((bat.file_name.clone(), to_ascii(&join_lines(&lines, true))));
    }
    out
}

fn op_face(op: &Operation) -> Face {
    match op {
        Operation::Hole { face, .. } | Operation::Groove { face, .. } => *face,
    }
}

struct Writer<'a> {
    model: &'a Model,
    part: &'a Part,
    profile: &'a XcsProfile,
}

impl Writer<'_> {
    /// `mirror`: program pre plochu B – diel otočený okolo osi y, `x' = L − x`.
    fn file(
        &self,
        name: String,
        ops: &[&Operation],
        mirror: bool,
        unverified: &mut BTreeSet<&'static str>,
    ) -> XcsFile {
        let p = self.part;
        let prof = self.profile;
        let n = |v: f64| fmt_num(v, prof.decimals);
        let header_vars = [
            ("project", self.model.project.clone()),
            ("part_id", p.id.clone()),
            ("cabinet", p.cabinet.clone()),
            ("l", n(p.length)),
            ("w", n(p.width)),
            ("t", n(p.thickness)),
        ];
        let mut lines: Vec<String> = prof
            .header
            .iter()
            .map(|l| substitute(l, &header_vars))
            .collect();

        let mut current_plane: Option<String> = None;
        let mut select = |lines: &mut Vec<String>, plane: &str| {
            if current_plane.as_deref() != Some(plane) {
                lines.push(substitute(
                    &prof.workplanes.select,
                    &[("plane", plane.to_owned())],
                ));
                current_plane = Some(plane.to_owned());
            }
        };
        let mx = |x: f64| if mirror { p.length - x } else { x };
        let mut drill_no = 1u32;
        let mut slot_no = prof.slot.first_number;

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
                    let (plane, side, lx, ly) = self.drill_position(face, mx(x), y, z);
                    select(&mut lines, plane);
                    let name = substitute(
                        &prof.drill.name,
                        &[
                            ("side", side.to_owned()),
                            ("n", drill_no.to_string()),
                            ("d", n(diameter)),
                            ("depth", n(depth)),
                        ],
                    );
                    lines.push(substitute(
                        &prof.drill.line,
                        &[
                            ("name", name),
                            ("x", n(lx)),
                            ("y", n(ly)),
                            ("d", n(diameter)),
                            ("depth", n(depth)),
                        ],
                    ));
                    drill_no += 1;
                    if !prof.drill.verified {
                        unverified.insert("CreateDrill");
                    }
                }
                Operation::Groove {
                    start,
                    end,
                    width,
                    depth,
                    ..
                } => {
                    select(&mut lines, &prof.workplanes.top);
                    let start = P2 {
                        x: mx(start.x),
                        y: start.y,
                    };
                    let end = P2 {
                        x: mx(end.x),
                        y: end.y,
                    };
                    for (a, b) in slot_passes(start, end, width, p, prof) {
                        let name = substitute(&prof.slot.name, &[("n", slot_no.to_string())]);
                        let vars = [
                            ("name", name),
                            ("xs", n(a.x)),
                            ("ys", n(a.y)),
                            ("xe", n(b.x)),
                            ("ye", n(b.y)),
                            ("depth", n(depth)),
                        ];
                        for template in &prof.slot.lines {
                            lines.push(substitute(template, &vars));
                        }
                        slot_no += 1;
                    }
                    if !prof.slot.verified {
                        unverified.insert("CreateSlot");
                    }
                }
            }
        }

        lines.extend(prof.footer.iter().map(|l| substitute(l, &header_vars)));
        let text = join_lines(&lines, prof.line_ending == LineEnding::Crlf);
        XcsFile {
            name,
            part_id: p.id.clone(),
            content: to_ascii(&text),
        }
    }

    /// Rovina, text strany a lokálne súradnice diery (kap. 7.4).
    /// Pri bočných rovinách je lokálne y výška v hrúbke dielu (`z`).
    fn drill_position(&self, face: Face, x: f64, y: f64, z: f64) -> (&str, &str, f64, f64) {
        let wp = &self.profile.workplanes;
        let names = &self.profile.drill.side_names;
        let (l, w) = (self.part.length, self.part.width);
        let rev = |flag: bool, v: f64, full: f64| if flag { full - v } else { v };
        match face {
            // F aj B (v otočenom programe) sa vŕtajú zhora
            Face::F | Face::B => (&wp.top, &names.top, x, y),
            Face::X0 => (&wp.x0, &names.x0, rev(wp.reverse_x0, y, w), z),
            Face::X1 => (&wp.x1, &names.x1, rev(wp.reverse_x1, y, w), z),
            Face::Y0 => (&wp.y0, &names.y0, rev(wp.reverse_y0, x, l), z),
            Face::Y1 => (&wp.y1, &names.y1, rev(wp.reverse_y1, x, l), z),
        }
    }
}

/// Prechody pílky pre drážku so stredovou čiarou `start → end` a šírkou `width`.
///
/// Drážka širšia ako rez sa robí od jednej hrany k druhej s krokom `max_step`;
/// posledný prechod lícuje s druhou hranou (ako v programoch CV: 83 → 79.6 → 76.2 → 73).
/// Drážka cez celý diel sa na oboch koncoch predĺži o `through_extension`.
pub fn slot_passes(
    start: P2,
    end: P2,
    width: f64,
    part: &Part,
    prof: &XcsProfile,
) -> Vec<(P2, P2)> {
    let tol = 1e-6;
    let slot = &prof.slot;
    let along_x = (start.y - end.y).abs() < tol;
    let (mut a, mut b, center, full) = if along_x {
        (start.x.min(end.x), start.x.max(end.x), start.y, part.length)
    } else {
        (start.y.min(end.y), start.y.max(end.y), start.x, part.width)
    };
    if a <= tol && b >= full - tol {
        a -= slot.through_extension;
        b += slot.through_extension;
    }

    let k = slot.saw_kerf;
    let mut offsets = Vec::new();
    if width <= k + tol {
        offsets.push(center);
    } else {
        let hi = center + width / 2.0 - k / 2.0;
        let lo = center - width / 2.0 + k / 2.0;
        let mut pos = hi;
        while pos - lo > tol {
            offsets.push(pos);
            pos -= slot.max_step;
        }
        offsets.push(lo);
    }

    offsets
        .into_iter()
        .map(|c| {
            if along_x {
                (P2 { x: a, y: c }, P2 { x: b, y: c })
            } else {
                (P2 { x: c, y: a }, P2 { x: c, y: b })
            }
        })
        .collect()
}
