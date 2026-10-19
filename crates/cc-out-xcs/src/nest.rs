//! Programy tabúľ pre SCM nesting a programy CX100 pre zvyšnú prácu (findings 08).
//!
//! Program tabule (`<sheet_name>.xcs`):
//! 1. hlavička (`SetMachiningParameters("AD",…)`, tabuľa ako obrobok),
//! 2. vŕtanie plochy hore, zoskupené podľa nástroja v poradí `drill.order`,
//!    v skupine najkratšia cesta (najbližší sused od posledného bodu),
//! 3. onion skin malých dielov (hĺbka T − skin),
//! 4. obrys každého dielu samostatne (hĺbka T + through_extra): najprv malé, potom veľké,
//!    v rámci skupiny od najmenšieho; v smere hodín s korekciou vľavo (súsledne),
//!    štart v strede ľavej hrany, nábeh po hrane z výšky `lead_in_depth`, prekrytie,
//! 5. odrezanie zvyšku tabule nad dielmi (ak ostane pás aspoň `remnant.min_width`),
//! 6. päta (parkovanie).
//!
//! Programy CX100 vyrába [`crate::render`] nad modelom, v ktorom majú diely iba
//! operácie pre CX100 (hrany, drážky pílou, plocha dole; materiály mimo nestingu celé).
//! Mená programov CX100 sú podľa ID dielu ako v režime `cx100` (`<id>.xcs`, pri práci na oboch
//! plochách `<id>_F.xcs` + `<id>_B.xcs`), nie podľa schémy CV `<beh><b|f><číslo>`.

use std::collections::{BTreeMap, BTreeSet};

use cc_model::{Face, LineEnding, Model, NestingProfile, Operation, Part, XcsProfile};
use cc_nest::{is_small, NestError, NestPlan, PartPlacement, SheetGroup, SheetLayout};
use cc_out_util::{file_stem, fmt_num, join_lines, substitute, to_ascii};

use crate::XcsOutput;

/// Program jednej tabule.
#[derive(Debug, Clone, PartialEq)]
pub struct NestSheetFile {
    /// Názov súboru, napr. `0101N.xcs`.
    pub name: String,
    pub material: String,
    pub thickness: f64,
    pub sheet_length: f64,
    pub sheet_width: f64,
    /// ID dielov na tabuli.
    pub parts: Vec<String>,
    /// ID dielov v poradí prerezania obrysov (číslo dielu na štítku, findings 14 §6.2).
    pub cut_order: Vec<String>,
    /// Diely na tabuli (poloha, natočenie, plocha hore) pre pohľad na tabuľu.
    pub placements: Vec<PartPlacement>,
    /// Plocha dielov / plocha tabule (0–1).
    pub utilisation: f64,
    /// Odrezal sa zvyšok tabule.
    pub remnant_cut: bool,
    /// ASCII obsah vrátane koncov riadkov.
    pub content: Vec<u8>,
}

/// Výstup režimu `nesting`.
#[derive(Debug, Clone, Default)]
pub struct NestOutput {
    pub sheets: Vec<NestSheetFile>,
    /// Programy CX100 pre zvyšnú prácu a `RUN.bat` (iba pre CX100).
    pub cx100: XcsOutput,
    /// Súpis behu `CncRun.txt` (UTF-16 LE s BOM), ak je v profile.
    pub cnc_run: Option<(String, Vec<u8>)>,
    pub plan: NestPlan,
    /// Šablóny s `verified = false`, ktoré sa vo výstupe použili (nesting aj CX100).
    pub unverified: BTreeSet<&'static str>,
}

impl NestOutput {
    /// `part_id` → program tabule a programy CX100 oddelené čiarkou (stĺpec `program`).
    pub fn program_names(&self) -> BTreeMap<String, String> {
        let mut map: BTreeMap<String, String> = BTreeMap::new();
        for s in &self.sheets {
            for id in &s.parts {
                map.insert(id.clone(), s.name.clone());
            }
        }
        for (id, names) in self.cx100.program_names() {
            map.entry(id)
                .and_modify(|s| {
                    s.push(',');
                    s.push_str(&names);
                })
                .or_insert(names);
        }
        map
    }
}

/// Rozloží diely na tabule, rozdelí obrábanie a vytvorí programy tabúľ aj CX100.
pub fn render_nesting(
    model: &Model,
    xcs: &XcsProfile,
    prof: &NestingProfile,
) -> Result<NestOutput, NestError> {
    let plan = cc_nest::plan(model, prof)?;
    let parts: BTreeMap<&str, &Part> = model.parts().map(|p| (p.id.as_str(), p)).collect();
    let mut unverified = BTreeSet::new();
    let mut sheets = Vec::new();
    for group in &plan.groups {
        for (i, layout) in group.sheets.iter().enumerate() {
            sheets.push(sheet_program(
                &model.project,
                group,
                i,
                layout,
                &parts,
                &plan,
                prof,
                &mut unverified,
            ));
        }
    }
    if !sheets.is_empty() && !prof.verified {
        unverified.insert("nesting.header");
    }

    let cx100 = crate::render(&plan.cx100_model(model), xcs);
    unverified.extend(cx100.unverified.iter().copied());
    let cnc_run = prof
        .cnc_run
        .as_ref()
        .map(|c| cnc_run(model, &plan, &sheets, &cx100, &parts, c));
    Ok(NestOutput {
        sheets,
        cx100,
        cnc_run,
        plan,
        unverified,
    })
}

/// Jedno vŕtanie na tabuli.
struct Drill {
    x: f64,
    y: f64,
    d: f64,
    depth: f64,
    tool: String,
    kind: String,
    /// Poradie skupiny podľa `drill.order`.
    rank: usize,
}

#[allow(clippy::too_many_arguments)]
fn sheet_program(
    project: &str,
    group: &SheetGroup,
    index: usize,
    layout: &SheetLayout,
    parts: &BTreeMap<&str, &Part>,
    plan: &NestPlan,
    prof: &NestingProfile,
    unverified: &mut BTreeSet<&'static str>,
) -> NestSheetFile {
    let n = |v: f64| fmt_num(v, prof.decimals);
    let t = group.thickness;
    let stem = file_stem(&substitute(
        &prof.sheet_name,
        &[
            ("mat", format!("{:02}", group.number)),
            ("sheet", format!("{:02}", index + 1)),
            ("material", file_stem(&group.material)),
        ],
    ));
    let vars = [
        ("project", project.to_owned()),
        ("program", stem.clone()),
        ("material", group.material.clone()),
        ("l", n(group.sheet_length)),
        ("w", n(group.sheet_width)),
        ("t", n(t)),
    ];
    let mut lines: Vec<String> = prof.header.iter().map(|l| substitute(l, &vars)).collect();

    // 1. vŕtanie plochy hore
    let drills = order_drills(collect_drills(layout, parts, plan, prof, t));
    for (i, d) in drills.iter().enumerate() {
        let name = substitute(
            &prof.drill.name,
            &[
                ("n", (i + 1).to_string()),
                ("d", n(d.d)),
                ("depth", n(d.depth)),
            ],
        );
        lines.push(prof.drill.select.clone());
        lines.push(substitute(
            &prof.drill.line,
            &[
                ("name", name),
                ("x", n(d.x)),
                ("y", n(d.y)),
                ("d", n(d.d)),
                ("depth", n(d.depth)),
                ("tool", d.tool.clone()),
                ("kind", d.kind.clone()),
            ],
        ));
    }
    if !drills.is_empty() && !prof.drill.verified {
        unverified.insert("nesting.CreateDrill");
    }

    // 2. obrysy: onion skin malých dielov, potom prerez malých a nakoniec veľkých
    let r = &prof.route;
    let mut order: Vec<(bool, &PartPlacement)> = layout
        .parts
        .iter()
        .map(|p| (is_small(p.length, p.width, prof), p))
        .collect();
    order.sort_by(|(sa, a), (sb, b)| {
        sb.cmp(sa)
            .then(a.area().total_cmp(&b.area()))
            .then(a.y.total_cmp(&b.y))
            .then(a.x.total_cmp(&b.x))
    });
    let mut k = r.first_number;
    let mut routed = false;
    let skin_depth = t - r.skin;
    if skin_depth > 0.0 {
        for (_, p) in order.iter().filter(|(small, _)| *small) {
            let runout = r.skin_runout_factor * (skin_depth - r.lead_in_depth);
            contour(&mut lines, p, skin_depth, &[r.overlap, runout], k, prof);
            k += 1;
            routed = true;
        }
    }
    let through = t + r.through_extra;
    for (_, p) in &order {
        contour(&mut lines, p, through, &[r.overlap], k, prof);
        k += 1;
        routed = true;
    }

    // 3. odrezanie zvyšku tabule
    let mut remnant_cut = false;
    let rem = &prof.remnant;
    if rem.enabled && !layout.parts.is_empty() {
        let y = layout.top() + rem.offset;
        let left = group.sheet_width - (y + r.tool_diameter / 2.0);
        if left >= rem.min_width {
            lines.extend(r.start_centerline.iter().cloned());
            lines.push(point(
                &r.first_point,
                group.sheet_length + rem.overrun,
                y,
                prof,
            ));
            lines.push(substitute(&r.depth, &[("z", n(through))]));
            lines.push(point(&r.point, -rem.overrun, y, prof));
            lines.push(substitute(&r.depth, &[("z", n(through))]));
            finish(&mut lines, k, through, &r.comp_centerline, prof);
            remnant_cut = true;
            routed = true;
        }
    }
    if routed && !r.verified {
        unverified.insert("nesting.CreateRoughFinish");
    }

    lines.extend(prof.footer.iter().map(|l| substitute(l, &vars)));
    let text = join_lines(&lines, prof.line_ending == LineEnding::Crlf);
    let area = group.sheet_length * group.sheet_width;
    NestSheetFile {
        name: format!("{stem}.xcs"),
        material: group.material.clone(),
        thickness: t,
        sheet_length: group.sheet_length,
        sheet_width: group.sheet_width,
        parts: layout.parts.iter().map(|p| p.part_id.clone()).collect(),
        cut_order: order.iter().map(|(_, p)| p.part_id.clone()).collect(),
        placements: layout.parts.clone(),
        utilisation: if area > 0.0 {
            layout.used_area() / area
        } else {
            0.0
        },
        remnant_cut,
        content: to_ascii(&text),
    }
}

fn collect_drills(
    layout: &SheetLayout,
    parts: &BTreeMap<&str, &Part>,
    plan: &NestPlan,
    prof: &NestingProfile,
    t: f64,
) -> Vec<Drill> {
    let dp = &prof.drill;
    let mut out = Vec::new();
    for pl in &layout.parts {
        let (Some(_), Some(pp)) = (parts.get(pl.part_id.as_str()), plan.parts.get(&pl.part_id))
        else {
            continue;
        };
        for op in &pp.nest_ops {
            let Operation::Hole {
                x,
                y,
                diameter,
                depth,
                through,
                ..
            } = *op
            else {
                continue;
            };
            let depth = if through {
                t + prof.route.through_extra
            } else {
                depth
            };
            let variant = dp.variants.iter().find(|v| {
                (v.diameter - diameter).abs() < 1e-3
                    && v.max_depth.is_none_or(|m| depth <= m + 1e-9)
            });
            let (sx, sy) = pl.to_sheet(x, y);
            let rank = dp
                .order
                .iter()
                .position(|d| (d - diameter).abs() < 1e-3)
                .unwrap_or(dp.order.len());
            out.push(Drill {
                x: sx,
                y: sy,
                d: diameter,
                depth,
                tool: variant
                    .and_then(|v| v.tool.clone())
                    .unwrap_or_else(|| dp.default_tool.clone()),
                kind: variant
                    .and_then(|v| v.kind.clone())
                    .unwrap_or_else(|| dp.default_kind.clone()),
                rank,
            });
        }
    }
    out
}

/// Skupiny podľa (poradie, Ø, nástroj, typ vrtáka); v skupine najbližší sused.
fn order_drills(drills: Vec<Drill>) -> Vec<Drill> {
    let mut groups: Vec<Vec<Drill>> = Vec::new();
    let mut sorted = drills;
    sorted.sort_by(|a, b| {
        a.rank
            .cmp(&b.rank)
            .then(a.d.total_cmp(&b.d))
            .then(a.tool.cmp(&b.tool))
            .then(a.kind.cmp(&b.kind))
    });
    for d in sorted {
        let same = groups.last().and_then(|g| g.first()).is_some_and(|f| {
            f.rank == d.rank && (f.d - d.d).abs() < 1e-9 && f.tool == d.tool && f.kind == d.kind
        });
        match groups.last_mut() {
            Some(g) if same => g.push(d),
            _ => groups.push(vec![d]),
        }
    }
    let mut out = Vec::new();
    let (mut cx, mut cy) = (0.0_f64, 0.0_f64);
    for mut g in groups {
        while !g.is_empty() {
            let mut best = 0;
            let mut best_d = f64::INFINITY;
            for (i, d) in g.iter().enumerate() {
                let dist = (d.x - cx).powi(2) + (d.y - cy).powi(2);
                if dist < best_d - 1e-9 {
                    best = i;
                    best_d = dist;
                }
            }
            let d = g.remove(best);
            cx = d.x;
            cy = d.y;
            out.push(d);
        }
    }
    out
}

fn point(template: &str, x: f64, y: f64, prof: &NestingProfile) -> String {
    substitute(
        template,
        &[
            ("x", fmt_num(x, prof.decimals)),
            ("y", fmt_num(y, prof.decimals)),
        ],
    )
}

fn finish(lines: &mut Vec<String>, k: u32, z: f64, comp: &str, prof: &NestingProfile) {
    let r = &prof.route;
    let name = substitute(&r.name, &[("k", k.to_string())]);
    let vars = [
        ("name", name),
        ("z", fmt_num(z, prof.decimals)),
        ("tool", r.tool.clone()),
        ("comp", comp.to_owned()),
        ("d", fmt_num(r.tool_diameter, prof.decimals)),
    ];
    lines.extend(r.finish.iter().map(|l| substitute(l, &vars)));
}

/// Uzavretý obrys dielu v smere hodín so štartom v strede ľavej hrany.
///
/// `extra`: úseky za štartom po uzavretí (prekrytie, prípadne výbeh); idú ďalej po obryse,
/// nikdy nie rovno za roh (fréza by vošla do susedného dielu).
fn contour(
    lines: &mut Vec<String>,
    p: &PartPlacement,
    z: f64,
    extra: &[f64],
    k: u32,
    prof: &NestingProfile,
) {
    let r = &prof.route;
    let (x0, y0, x1, y1) = (p.x, p.y, p.x + p.dx, p.y + p.dy);
    let ym = (y0 + y1) / 2.0;
    let lead = r.lead_in.min(p.dy / 2.0);
    let depth = substitute(&r.depth, &[("z", fmt_num(z, prof.decimals))]);

    lines.extend(r.start.iter().cloned());
    lines.push(point(&r.first_point, x0, ym - lead, prof));
    lines.push(substitute(
        &r.depth,
        &[("z", fmt_num(r.lead_in_depth, prof.decimals))],
    ));
    // obeh v smere hodín: hore po ľavej hrane, vpravo po hornej, dole, vľavo, späť na štart
    let loop_pts = [(x0, y1), (x1, y1), (x1, y0), (x0, y0), (x0, ym)];
    let mut path = vec![(x0, ym)];
    path.extend(loop_pts);
    for &(x, y) in &path {
        lines.push(point(&r.point, x, y, prof));
        lines.push(depth.clone());
    }
    // pokračovanie po obryse od štartu
    let mut walker = LoopWalker::new(&path);
    for &len in extra {
        if len <= 0.0 {
            continue;
        }
        for (x, y) in walker.advance(len) {
            lines.push(point(&r.point, x, y, prof));
            lines.push(depth.clone());
        }
    }
    finish(lines, k, z, &r.comp_outside, prof);
}

/// Prechádza uzavretý obrys od štartu a vracia rohy a koncový bod.
struct LoopWalker {
    /// Body obrysu: štart, rohy…, štart.
    pts: Vec<(f64, f64)>,
    /// Index úseku `pts[seg] → pts[seg + 1]`.
    seg: usize,
    /// Prejdená dĺžka v úseku.
    along: f64,
}

impl LoopWalker {
    fn new(pts: &[(f64, f64)]) -> Self {
        LoopWalker {
            pts: pts.to_vec(),
            seg: 0,
            along: 0.0,
        }
    }

    fn advance(&mut self, mut len: f64) -> Vec<(f64, f64)> {
        let mut out = Vec::new();
        let segs = self.pts.len().saturating_sub(1);
        if segs == 0 {
            return out;
        }
        // obmedzenie na jeden obeh navyše (ochrana pred nekonečným cyklom)
        let mut guard = 0;
        while len > 1e-9 && guard < 4 * segs + 4 {
            guard += 1;
            let (Some(&a), Some(&b)) = (self.pts.get(self.seg), self.pts.get(self.seg + 1)) else {
                break;
            };
            let seg_len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
            let rest = seg_len - self.along;
            if len < rest - 1e-9 {
                self.along += len;
                let f = self.along / seg_len;
                out.push((a.0 + (b.0 - a.0) * f, a.1 + (b.1 - a.1) * f));
                return out;
            }
            len -= rest;
            out.push(b);
            self.seg = (self.seg + 1) % segs;
            self.along = 0.0;
        }
        out
    }
}

/// `CncRun.txt`: hlavička, tabule podľa mena, programy CX100 podľa mena.
fn cnc_run(
    model: &Model,
    plan: &NestPlan,
    sheets: &[NestSheetFile],
    cx100: &XcsOutput,
    parts: &BTreeMap<&str, &Part>,
    c: &cc_model::NestCncRun,
) -> (String, Vec<u8>) {
    let n = |v: f64| fmt_num(v, 4);
    let mut lines: Vec<String> = c
        .header
        .iter()
        .map(|l| {
            substitute(
                l,
                &[
                    ("project", model.project.clone()),
                    ("parts", cx100.files.len().to_string()),
                    ("sheets", sheets.len().to_string()),
                ],
            )
        })
        .collect();
    let mut sorted: Vec<&NestSheetFile> = sheets.iter().collect();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));
    for (i, s) in sorted.iter().enumerate() {
        // plocha hore na tabuli: väčšina dielov (pri zhode F)
        let b_up = s
            .parts
            .iter()
            .filter(|id| plan.parts.get(*id).is_some_and(|p| p.face_up == Face::B))
            .count();
        let face = if 2 * b_up > s.parts.len() { "B" } else { "F" };
        let grain = s
            .parts
            .first()
            .and_then(|id| parts.get(id.as_str()))
            .is_some_and(|p| p.grain != cc_model::Grain::None);
        lines.push(substitute(
            &c.sheet_line,
            &[
                ("program", s.name.trim_end_matches(".xcs").to_owned()),
                ("n", (i + 1).to_string()),
                ("l", n(s.sheet_length)),
                ("w", n(s.sheet_width)),
                ("t", n(s.thickness)),
                ("material", s.material.clone()),
                ("grain", u8::from(grain).to_string()),
                ("face", face.to_owned()),
            ],
        ));
    }
    let mut files: Vec<_> = cx100.files.iter().collect();
    files.sort_by(|a, b| a.name.cmp(&b.name));
    for f in files {
        let Some(p) = parts.get(f.part_id.as_str()) else {
            continue;
        };
        let stem = f.name.trim_end_matches(".xcs");
        let face = if stem.ends_with("_B") { "B" } else { "F" };
        lines.push(substitute(
            &c.part_line,
            &[
                ("program", stem.to_owned()),
                ("part_id", p.id.clone()),
                ("code", p.role.id_code().to_owned()),
                ("l", n(p.length)),
                ("w", n(p.width)),
                ("t", n(p.thickness)),
                ("material", p.material.clone()),
                ("cabinet", p.cabinet.clone()),
                (
                    "grain",
                    u8::from(p.grain != cc_model::Grain::None).to_string(),
                ),
                ("face", face.to_owned()),
            ],
        ));
    }
    let text = join_lines(&lines, true);
    let mut bytes = vec![0xFF, 0xFE];
    for u in text.encode_utf16() {
        bytes.extend_from_slice(&u.to_le_bytes());
    }
    (c.file_name.clone(), bytes)
}
