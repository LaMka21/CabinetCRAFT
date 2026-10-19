//! Čelá, zásuvky a atribúty skrinky CV → `CabinetSpec` (findings 09, 10, 11).
//!
//! Strom otvorov čela (`Face`) je binárny: delený otvor ostáva rodičom, jeho deti sú
//! delič (`TYPE 2` vodorovný, `TYPE 3` zvislý) a dva podotvory (findings 11 §3.2).
//! Náš model má riadky čiel zhora nadol a v riadku najviac jednu úroveň stĺpcov;
//! čo sa tak vyjadriť nedá, skončí poznámkou.

use std::collections::{BTreeMap, BTreeSet};

use cc_i18n::tr;
use cc_model::{
    CabinetOptions, CabinetType, FrontAdjust, FrontKind, FrontReveals, FrontSpec, HingeSide,
    JointType, LinkHole, ResolvedOptions, ShelfSupport, Standard, TipOn, TipOnColor, TipOnKind,
    TopStyle,
};

use glam::DVec3;

use crate::job::{
    Bbox, CarcassSource, CvjCabinet, CvjLink, CvjSection, FaceStyle, PartRole, SectionGroup,
    SectionKind,
};

/// Najväčšia hĺbka stromu otvorov, ktorú prejdeme.
const MAX_LAYOUT_DEPTH: usize = 32;
/// Najviac riadkov čiel (rovnako ako generátor).
const MAX_ROWS: usize = 12;
/// Najväčšia rozumná úprava rozmeru čela a škára (mm).
const MAX_ADJUST: f64 = 500.0;
const MAX_REVEAL: f64 = 200.0;

/// Čelá skrinky a dĺžka výsuvu.
#[derive(Debug, Default)]
pub(crate) struct FrontsImport {
    pub fronts: Vec<FrontSpec>,
    pub drawer_length: Option<f64>,
    /// škáry čiel pri skrinke z polohy čiel v CV
    pub reveals: FrontReveals,
}

/// Rozloženie otvorov čela.
enum Layout<'a> {
    /// koncový otvor (dvojité dvierka môžu byť dva uzly s rovnakým `ID`)
    Leaf(Vec<&'a CvjSection>),
    Split {
        vertical: bool,
        parts: Vec<Layout<'a>>,
    },
}

impl Layout<'_> {
    fn bbox(&self) -> Option<Bbox> {
        match self {
            Layout::Leaf(nodes) => union(nodes.iter().map(|n| n.bbox)),
            Layout::Split { parts, .. } => union(parts.iter().filter_map(|p| p.bbox())),
        }
    }
}

fn union(it: impl Iterator<Item = Bbox>) -> Option<Bbox> {
    it.reduce(|a, b| Bbox {
        min: a.min.min(b.min),
        max: a.max.max(b.max),
    })
}

fn center(l: &Layout) -> (f64, f64) {
    l.bbox().map_or((0.0, 0.0), |b| {
        let c = (b.min + b.max) * 0.5;
        (c.x, c.y)
    })
}

/// Riadok čiel: jeden otvor alebo stĺpce.
enum Row<'a> {
    Single(Vec<&'a CvjSection>),
    Columns(Vec<Vec<&'a CvjSection>>),
}

struct Tree<'a> {
    by_id: BTreeMap<i64, Vec<&'a CvjSection>>,
}

impl<'a> Tree<'a> {
    fn build(&self, id: i64, depth: usize, seen: &mut BTreeSet<i64>) -> Result<Layout<'a>, String> {
        if depth > MAX_LAYOUT_DEPTH || !seen.insert(id) {
            return Err(tr!("import_cvj.fronts.tree_cycle"));
        }
        let nodes = self.by_id.get(&id).cloned().unwrap_or_default();
        let mut kids: Vec<i64> = Vec::new();
        let mut divider: Option<bool> = None;
        for list in self.by_id.values() {
            for s in list {
                if s.parent != id || s.id == id {
                    continue;
                }
                match s.kind {
                    SectionKind::HorizontalDivider => divider = Some(false),
                    SectionKind::VerticalDivider => divider = Some(true),
                    _ => {
                        if !kids.contains(&s.id) {
                            kids.push(s.id);
                        }
                    }
                }
            }
        }
        if kids.is_empty() {
            return Ok(Layout::Leaf(nodes));
        }
        let mut parts = Vec::new();
        for k in kids {
            parts.push(self.build(k, depth + 1, seen)?);
        }
        if parts.len() == 1 {
            return parts
                .pop()
                .ok_or_else(|| tr!("import_cvj.fronts.empty_opening"));
        }
        // smer podľa deliča, inak podľa polohy podotvorov
        let vertical = divider.unwrap_or_else(|| {
            let cs: Vec<(f64, f64)> = parts.iter().map(center).collect();
            let spread = |f: fn(&(f64, f64)) -> f64| {
                let v: Vec<f64> = cs.iter().map(f).collect();
                v.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
                    - v.iter().cloned().fold(f64::INFINITY, f64::min)
            };
            spread(|c| c.0) > spread(|c| c.1)
        });
        if vertical {
            parts.sort_by(|a, b| center(a).0.total_cmp(&center(b).0));
        } else {
            // zhora nadol
            parts.sort_by(|a, b| center(b).1.total_cmp(&center(a).1));
        }
        Ok(Layout::Split { vertical, parts })
    }
}

fn rows<'a>(l: Layout<'a>, out: &mut Vec<Row<'a>>) -> Result<(), String> {
    match l {
        Layout::Leaf(n) => out.push(Row::Single(n)),
        Layout::Split {
            vertical: false,
            parts,
        } => {
            for p in parts {
                rows(p, out)?;
            }
        }
        Layout::Split {
            vertical: true,
            parts,
        } => {
            let mut cols = Vec::new();
            for p in parts {
                columns(p, &mut cols)?;
            }
            out.push(Row::Columns(cols));
        }
    }
    Ok(())
}

fn columns<'a>(l: Layout<'a>, out: &mut Vec<Vec<&'a CvjSection>>) -> Result<(), String> {
    match l {
        Layout::Leaf(n) => out.push(n),
        Layout::Split {
            vertical: true,
            parts,
        } => {
            for p in parts {
                columns(p, out)?;
            }
        }
        Layout::Split {
            vertical: false, ..
        } => return Err(tr!("import_cvj.fronts.column_split")),
    }
    Ok(())
}

/// Kladné konečné číslo.
fn positive(v: Option<f64>) -> Option<f64> {
    v.filter(|x| x.is_finite() && *x > 0.0)
}

/// Zaokrúhlenie na desatiny mm.
fn r1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0 + 0.0
}

/// Údaje koncového otvoru.
struct Leaf<'a> {
    nodes: Vec<&'a CvjSection>,
}

impl Leaf<'_> {
    fn first(&self) -> Option<&CvjSection> {
        self.nodes.first().copied()
    }
    fn bbox(&self) -> Option<Bbox> {
        union(self.nodes.iter().map(|n| n.bbox))
    }
    /// Šírka otvoru po čiaru delenia.
    fn width(&self) -> Option<f64> {
        positive(self.bbox().map(|b| b.size().x))
    }
    fn equal(&self) -> bool {
        self.nodes.iter().any(|n| n.equal)
    }
    fn has_fronts(&self) -> bool {
        self.nodes.iter().any(|n| !n.fronts.is_empty())
    }
    fn front_boxes(&self) -> Vec<Bbox> {
        let mut v: Vec<Bbox> = self
            .nodes
            .iter()
            .flat_map(|n| n.fronts.iter())
            .map(|f| f.bbox)
            .filter(|b| b.min.is_finite() && b.max.is_finite())
            .collect();
        v.sort_by(|a, b| a.min.x.total_cmp(&b.min.x));
        v
    }
    /// Úprava rozmeru čela (Size Adjustments `OLL/OLR/OLT/OLB`) otvoru s čelom.
    fn adjust(&self) -> FrontAdjust {
        let Some(p) = self
            .first()
            .map(|f| &f.params)
            .filter(|_| self.has_fronts())
        else {
            return FrontAdjust::default();
        };
        let v = |x: Option<f64>| {
            x.filter(|x| x.is_finite() && x.abs() < MAX_ADJUST)
                .map_or(0.0, r1)
        };
        FrontAdjust {
            left: v(p.overlay_left),
            right: v(p.overlay_right),
            top: v(p.overlay_top),
            bottom: v(p.overlay_bottom),
        }
    }
    /// Miesto čela pred úpravou rozmeru (v súradniciach skrinky).
    fn slot(&self) -> Option<Bbox> {
        let b = union(self.front_boxes().into_iter())?;
        let a = self.adjust();
        let s = Bbox {
            min: DVec3::new(b.min.x + a.left, b.min.y + a.bottom, b.min.z),
            max: DVec3::new(b.max.x - a.right, b.max.y - a.top, b.max.z),
        };
        (s.size().x > 0.0 && s.size().y > 0.0).then_some(s)
    }
    /// Výška miesta čela, bez čela výška otvoru.
    fn height(&self) -> Option<f64> {
        positive(self.slot().map(|b| b.size().y))
            .or_else(|| positive(self.bbox().map(|b| b.size().y)))
    }
    /// Škáry medzi čelami v otvore (dvojité dvierka).
    fn inner_gaps(&self) -> Vec<f64> {
        gaps_between(&self.front_boxes())
    }
}

/// Škáry medzi susednými obálkami zoradenými zľava doprava.
fn gaps_between(boxes: &[Bbox]) -> Vec<f64> {
    boxes
        .windows(2)
        .filter_map(|w| match w {
            [a, b] => Some(b.min.x - a.max.x),
            _ => None,
        })
        .collect()
}

/// Riadok čiel s miestom v CV.
struct RowOut {
    spec: FrontSpec,
    height: Option<f64>,
    equal: bool,
    slot: Option<Bbox>,
    /// škáry medzi čelami vedľa seba
    gaps: Vec<f64>,
}

/// Čelá skrinky zo stromu otvorov čela. `front_only`: skrinka bez korpusu (umývačka).
pub(crate) fn fronts(
    c: &CvjCabinet,
    label: &str,
    front_only: bool,
    notes: &mut Vec<String>,
) -> FrontsImport {
    let face: Vec<&CvjSection> = c.sections_in(SectionGroup::Face).collect();
    if face.is_empty() {
        return FrontsImport::default();
    }
    let mut by_id: BTreeMap<i64, Vec<&CvjSection>> = BTreeMap::new();
    for s in &face {
        by_id.entry(s.id).or_default().push(s);
    }
    let tree = Tree { by_id };
    // korene: bez rodiča alebo s rodičom, ktorý v strome nie je
    let roots: Vec<i64> = tree
        .by_id
        .iter()
        .filter(|(id, list)| {
            list.iter().any(|s| {
                s.kind == SectionKind::Opening
                    && (s.parent == 0 || s.parent == **id || !tree.by_id.contains_key(&s.parent))
            })
        })
        .map(|(id, _)| *id)
        .collect();
    let layout = match roots.as_slice() {
        [] => Err(tr!("import_cvj.fronts.no_root")),
        [root] => tree.build(*root, 0, &mut BTreeSet::new()),
        many => {
            // viac koreňov: rozloženie podľa polohy
            let mut seen = BTreeSet::new();
            many.iter()
                .map(|r| tree.build(*r, 1, &mut seen))
                .collect::<Result<Vec<_>, _>>()
                .map(|parts| split_by_position(parts))
        }
    };
    let mut row_list = Vec::new();
    if let Err(e) = layout.and_then(|l| rows(l, &mut row_list)) {
        notes.push(tr!(
            "import_cvj.fronts.not_imported",
            label = label,
            reason = e
        ));
        return FrontsImport::default();
    }
    if row_list.len() > MAX_ROWS {
        notes.push(tr!(
            "import_cvj.fronts.too_many_rows",
            label = label,
            count = row_list.len(),
            max = MAX_ROWS
        ));
        return FrontsImport::default();
    }

    let mut drawer_lengths: Vec<f64> = Vec::new();
    let mut out: Vec<RowOut> = Vec::new();
    for row in row_list {
        match row {
            Row::Single(nodes) => {
                let leaf = Leaf { nodes };
                let mut spec =
                    leaf_spec(&leaf, false, front_only, label, &mut drawer_lengths, notes);
                spec.adjust = leaf.adjust();
                out.push(RowOut {
                    spec,
                    height: leaf.height(),
                    equal: leaf.equal(),
                    slot: leaf.slot(),
                    gaps: leaf.inner_gaps(),
                });
            }
            Row::Columns(cols) => {
                let leaves: Vec<Leaf> = cols.into_iter().map(|nodes| Leaf { nodes }).collect();
                let mut col_specs: Vec<FrontSpec> = Vec::new();
                for l in &leaves {
                    let mut s = leaf_spec(l, true, front_only, label, &mut drawer_lengths, notes);
                    s.width = if l.equal() { None } else { l.width().map(r1) };
                    s.adjust = l.adjust();
                    col_specs.push(s);
                }
                if col_specs.iter().all(|s| s.width.is_some()) {
                    if let Some(last) = col_specs.last_mut() {
                        last.width = None;
                    }
                }
                let slots: Vec<Bbox> = leaves.iter().filter_map(Leaf::slot).collect();
                let mut gaps = gaps_between(&slots);
                gaps.extend(leaves.iter().flat_map(Leaf::inner_gaps));
                let height = leaves
                    .iter()
                    .filter_map(Leaf::height)
                    .fold(None, |a: Option<f64>, h| Some(a.map_or(h, |a| a.max(h))));
                let mut spec = FrontSpec::new(FrontKind::Door);
                spec.columns = col_specs;
                out.push(RowOut {
                    spec,
                    height,
                    equal: false,
                    slot: union(slots.into_iter()),
                    gaps,
                });
            }
        }
    }
    let reveals = reveals(c, &out, label, notes);

    // výšky: zadané z CV; rovnaké (EQ) a posledný riadok si delia zvyšok
    let n = out.len();
    let mut fronts: Vec<FrontSpec> = Vec::with_capacity(n);
    for r in out {
        let mut spec = r.spec;
        spec.height = if n == 1 || r.equal {
            None
        } else {
            r.height.map(r1)
        };
        fronts.push(spec);
    }
    if n > 1 && fronts.iter().all(|f| f.height.is_some()) {
        if let Some(last) = fronts.last_mut() {
            last.height = None;
        }
    }
    // samé otvorené riadky = skrinka bez čiel
    if fronts
        .iter()
        .all(|f| f.kind == FrontKind::Open && f.columns.is_empty())
    {
        fronts.clear();
    }

    let mut lengths = drawer_lengths;
    lengths.sort_by(f64::total_cmp);
    lengths.dedup_by(|a, b| (*a - *b).abs() < 0.5);
    let drawer_length = lengths.last().copied();
    if lengths.len() > 1 {
        notes.push(tr!(
            "import_cvj.fronts.drawer_lengths",
            label = label,
            lengths = lengths
                .iter()
                .map(|l| format!("{l:.0}"))
                .collect::<Vec<_>>()
                .join(", "),
            longest = format!("{:.0}", drawer_length.unwrap_or(0.0))
        ));
    }
    FrontsImport {
        reveals: if fronts.is_empty() {
            FrontReveals::default()
        } else {
            reveals
        },
        fronts,
        drawer_length: drawer_length.map(r1),
    }
}

/// Viac koreňov stromu: rozloženie podľa polohy.
fn split_by_position(mut parts: Vec<Layout<'_>>) -> Layout<'_> {
    let (xs, ys): (Vec<f64>, Vec<f64>) = parts.iter().map(center).unzip();
    let vertical = spread(&xs) > spread(&ys);
    if vertical {
        parts.sort_by(|a, b| center(a).0.total_cmp(&center(b).0));
    } else {
        parts.sort_by(|a, b| center(b).1.total_cmp(&center(a).1));
    }
    Layout::Split { vertical, parts }
}

fn spread(v: &[f64]) -> f64 {
    v.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
        - v.iter().cloned().fold(f64::INFINITY, f64::min)
}

/// Škáry čiel pri skrinke odvodené z polohy čiel v CV (miesto čela pred úpravou
/// rozmeru voči obálke korpusu), ako Reveals v CV.
fn reveals(c: &CvjCabinet, rows: &[RowOut], label: &str, notes: &mut Vec<String>) -> FrontReveals {
    let Some(all) = union(rows.iter().filter_map(|r| r.slot)) else {
        return FrontReveals::default();
    };
    let cb = c.carcass;
    let pick = |values: Vec<f64>, rows: bool, notes: &mut Vec<String>| -> Option<f64> {
        let first = *values.first()?;
        if values.iter().any(|v| (v - first).abs() > 0.5) {
            let list = values
                .iter()
                .map(|v| format!("{v:.1}"))
                .collect::<Vec<_>>()
                .join(", ");
            notes.push(if rows {
                tr!(
                    "import_cvj.fronts.reveals_rows",
                    label = label,
                    values = list
                )
            } else {
                tr!(
                    "import_cvj.fronts.reveals_columns",
                    label = label,
                    values = list
                )
            });
        }
        Some(first)
    };
    let horizontal: Vec<f64> = rows
        .windows(2)
        .filter_map(|w| match w {
            [a, b] => Some(a.slot?.min.y - b.slot?.max.y),
            _ => None,
        })
        .collect();
    let vertical: Vec<f64> = rows.iter().flat_map(|r| r.gaps.iter().copied()).collect();
    let r = FrontReveals {
        top: Some(cb.max.y - all.max.y),
        bottom: Some(all.min.y - cb.min.y),
        left: Some(all.min.x - cb.min.x),
        right: Some(cb.max.x - all.max.x),
        horizontal: pick(horizontal, true, notes),
        vertical: pick(vertical, false, notes),
    };
    let values = [r.top, r.bottom, r.left, r.right, r.horizontal, r.vertical];
    if values
        .iter()
        .flatten()
        .any(|v| !v.is_finite() || v.abs() > MAX_REVEAL)
    {
        notes.push(tr!("import_cvj.fronts.reveals_invalid", label = label));
        return FrontReveals::default();
    }
    let round = |v: Option<f64>| v.map(r1);
    FrontReveals {
        top: round(r.top),
        bottom: round(r.bottom),
        left: round(r.left),
        right: round(r.right),
        horizontal: round(r.horizontal),
        vertical: round(r.vertical),
    }
}

/// Škáry zhodné so štandardom sa vynechajú.
pub(crate) fn drop_same_reveals(r: &mut FrontReveals, standard: &Standard, ty: CabinetType) {
    let fs = &standard.fronts;
    let rows = fs.rows(ty);
    let same = |v: &mut Option<f64>, base: f64| {
        if v.is_some_and(|x| (x - base).abs() < 0.05) {
            *v = None;
        }
    };
    same(&mut r.top, rows.top);
    same(&mut r.bottom, rows.bottom);
    same(&mut r.horizontal, rows.gap);
    same(&mut r.left, fs.reveal_side);
    same(&mut r.right, fs.reveal_side);
    same(&mut r.vertical, fs.pair_gap);
}

/// Čelo jedného otvoru.
fn leaf_spec(
    leaf: &Leaf,
    in_column: bool,
    front_only: bool,
    label: &str,
    drawer_lengths: &mut Vec<f64>,
    notes: &mut Vec<String>,
) -> FrontSpec {
    let Some(first) = leaf.first() else {
        return FrontSpec::new(FrontKind::Open);
    };
    let p = &first.params;
    let id = first.id;
    let mut style = first.face_style();
    if leaf.nodes.len() > 1 && style != FaceStyle::PairDoor {
        notes.push(tr!(
            "import_cvj.fronts.opening_repeated",
            label = label,
            id = id,
            count = leaf.nodes.len()
        ));
    }
    // všeobecný alebo neznámy otvor: podľa druhu objektu (otvor dvierok/zásuvky)
    if let FaceStyle::General | FaceStyle::Other(_) = style {
        if let FaceStyle::Other(code) = style {
            notes.push(tr!(
                "import_cvj.fronts.unknown_style",
                label = label,
                code = code,
                object = &first.code
            ));
        }
        style = match first.asm_type {
            29 => FaceStyle::Door,
            30 => FaceStyle::Drawer,
            _ if leaf.has_fronts() => FaceStyle::FalseFront,
            _ => FaceStyle::Open,
        };
    }
    let mut spec = match style {
        FaceStyle::Door => {
            let mut s = FrontSpec::new(FrontKind::Door);
            match p.hinge {
                Some(1) => s.hinge_side = Some(HingeSide::Left),
                Some(2) => s.hinge_side = Some(HingeSide::Right),
                Some(4) => {
                    // dvierka bez závesov = pevné čelo (umývačka)
                    if !front_only {
                        notes.push(tr!("import_cvj.fronts.door_no_hinges", label = label));
                    }
                    s = FrontSpec::new(FrontKind::FalseFront);
                }
                Some(0) | None => {}
                Some(h) => notes.push(tr!(
                    "import_cvj.fronts.hinge_side",
                    label = label,
                    value = h
                )),
            }
            s
        }
        FaceStyle::PairDoor => FrontSpec::new(FrontKind::Pair),
        FaceStyle::Drawer => {
            let mut s = FrontSpec::new(FrontKind::Drawer);
            s.box_height = positive(p.box_height).map(r1);
            if let Some(l) = positive(p.runner_length).or(positive(p.box_depth)) {
                drawer_lengths.push(l);
            }
            s
        }
        FaceStyle::Open => FrontSpec::new(FrontKind::Open),
        FaceStyle::FalseFront => FrontSpec::new(FrontKind::FalseFront),
        FaceStyle::Blind => FrontSpec::new(FrontKind::Panel),
        // panel spotrebiča v stĺpci nie je slepý roh (pánt suseda by bol slepý)
        FaceStyle::Panel if in_column => FrontSpec::new(FrontKind::FalseFront),
        FaceStyle::Panel => FrontSpec::new(FrontKind::Panel),
        FaceStyle::General | FaceStyle::Other(_) => FrontSpec::new(FrontKind::Open),
    };
    if matches!(spec.kind, FrontKind::Door | FrontKind::Pair) && !front_only {
        let manual: Vec<String> = [("HNGT", p.hinge_top), ("HNGB", p.hinge_bottom)]
            .iter()
            .filter_map(|(n, v)| positive(*v).map(|v| format!("{n} {v:.0}")))
            .collect();
        if !manual.is_empty() {
            notes.push(tr!(
                "import_cvj.fronts.hinge_positions",
                label = label,
                values = manual.join(", ")
            ));
        }
        if let Some(q) = p.hinge_count.filter(|q| *q > 0) {
            notes.push(tr!(
                "import_cvj.fronts.hinge_count",
                label = label,
                count = q
            ));
        }
    }
    if in_column {
        spec.height = None;
    }
    spec
}

/// Atribúty skrinky → `CabinetOptions`; hodnoty zhodné so štandardom (`base`) sa vynechajú.
pub(crate) fn options(
    c: &CvjCabinet,
    ty: CabinetType,
    front_only: bool,
    label: &str,
    base: Option<&ResolvedOptions>,
    notes: &mut Vec<String>,
) -> CabinetOptions {
    let a = &c.attributes;
    let mut o = CabinetOptions::default();
    match a.apply_hardware {
        Some(1) => o.joints = Some(true),
        Some(0) => o.joints = Some(false),
        Some(2) => notes.push(tr!("import_cvj.options.joints_reset", label = label)),
        Some(v) => notes.push(tr!(
            "import_cvj.options.apply_hw_unknown",
            label = label,
            value = v
        )),
        None => {}
    }
    match a.joint_type {
        // „1-K+Konfirmat“; či je to konfirmát alebo vrut, určuje zákazka: názov prvku HW2
        Some(2) => {
            let screw = c.connectors.iter().any(|n| {
                let n = n.to_lowercase();
                n.contains("vrut") || n.contains("screw")
            });
            o.joint_type = Some(if screw {
                JointType::Screw
            } else {
                JointType::Confirmat
            });
        }
        Some(1) => o.joint_type = Some(JointType::Dowels),
        Some(0) => {
            o.joint_type = Some(JointType::Dowels);
            notes.push(tr!("import_cvj.options.joints_none", label = label));
        }
        Some(v) => {
            let name = match v {
                7 => tr!("import_cvj.options.joint_cam15"),
                3 => tr!("import_cvj.options.joint_vb35"),
                5 => tr!("import_cvj.options.joint_vb36"),
                10 => tr!("import_cvj.options.joint_blum"),
                11 => tr!("import_cvj.options.joint_maxifix"),
                _ => tr!("import_cvj.options.joint_unknown"),
            };
            notes.push(tr!(
                "import_cvj.options.joint_type",
                label = label,
                value = v,
                name = name
            ));
        }
        None => {}
    }
    match a.shelf_support {
        Some(7) => o.shelf_support = Some(ShelfSupport::Sekura7),
        Some(0) => o.shelf_support = Some(ShelfSupport::None),
        Some(8) => notes.push(tr!(
            "import_cvj.options.shelf_support",
            label = label,
            value = "Tab 18"
        )),
        Some(v) => notes.push(tr!(
            "import_cvj.options.shelf_support",
            label = label,
            value = v
        )),
        None => {}
    }
    o.bumpers = a.bumpers;
    match a.delete_legs {
        Some(false) => o.legs = Some(true),
        Some(true) => {
            let present: Vec<(usize, bool)> = a
                .deleted_legs
                .iter()
                .enumerate()
                .filter_map(|(i, d)| d.map(|d| (i + 1, d)))
                .collect();
            if !present.is_empty() && present.iter().all(|(_, d)| *d) {
                o.legs = Some(false);
            } else if present.iter().any(|(_, d)| *d) {
                let which: Vec<String> = present
                    .iter()
                    .filter(|(_, d)| *d)
                    .map(|(i, _)| i.to_string())
                    .collect();
                notes.push(tr!(
                    "import_cvj.options.legs_partial",
                    label = label,
                    legs = which.join(", ")
                ));
                o.legs = Some(true);
            }
        }
        None => {}
    }
    match a.tip_on {
        Some(0) => o.tip_on = Some(TipOn::Off),
        Some(1) => o.tip_on = Some(TipOn::Drilled),
        Some(2) => o.tip_on = Some(TipOn::Adapter),
        Some(v) => notes.push(tr!(
            "import_cvj.options.tip_on_unknown",
            label = label,
            value = v
        )),
        None => {}
    }
    if matches!(o.tip_on, Some(TipOn::Drilled | TipOn::Adapter)) {
        o.tip_on_color = match a.tip_on_color {
            Some(0) => Some(TipOnColor::White),
            Some(1) => Some(TipOnColor::Grey),
            Some(2) => Some(TipOnColor::Black),
            _ => None,
        };
        o.tip_on_kind = match a.tip_on_kind {
            Some(0) => Some(TipOnKind::Stop),
            Some(1) => Some(TipOnKind::Magnet),
            _ => None,
        };
    }
    if !front_only {
        link_options(a.link.as_ref(), &mut o, label, notes);
    }
    if !front_only && c.carcass_source == CarcassSource::Parts {
        if matches!(ty, CabinetType::Base | CabinetType::Tall) {
            o.plinth = Some(has_plinth(c));
        }
        match a.back_dado {
            Some(1) => notes.push(tr!("import_cvj.options.back_inset", label = label)),
            Some(2) => notes.push(tr!("import_cvj.options.back_no_groove", label = label)),
            _ => {
                if a.dado_router == Some(true) {
                    notes.push(tr!("import_cvj.options.back_router", label = label));
                }
            }
        }
    }
    if let Some(b) = base {
        drop_same(&mut o, b);
    }
    o
}

/// Najviac ručných dier spojenia na skrinku (vysoká skrinka má 6).
const MAX_LINK_HOLES: usize = 12;

/// Spojovacie kovanie skriniek z ľavého boku (findings 12 §8): stav, posun protikusu a pri
/// režime „Zap/Edit“ ručné polohy dier. Diery vŕta `cc-pipeline` (link.rs).
pub(crate) fn link_options(
    link: Option<&CvjLink>,
    o: &mut CabinetOptions,
    label: &str,
    notes: &mut Vec<String>,
) {
    let Some(l) = link else {
        return;
    };
    match l.state {
        0 => {
            o.link_left = Some(false);
            return;
        }
        1 => o.link_left = Some(true),
        2 => {
            o.link_left = Some(true);
            o.link_holes = l
                .holes
                .iter()
                .filter(|(x, y)| x.is_finite() && y.is_finite() && *x >= 0.0 && *y >= 0.0)
                .take(MAX_LINK_HOLES)
                .map(|&(from_front, from_bottom)| LinkHole {
                    from_front,
                    from_bottom,
                })
                .collect();
        }
        v => {
            notes.push(tr!(
                "import_cvj.options.link_unknown",
                label = label,
                value = v
            ));
            return;
        }
    }
    o.link_shift = l.shift.filter(|s| s.is_finite() && *s != 0.0);
    if let Some(s) = o.link_shift {
        notes.push(tr!(
            "import_cvj.options.link_shift",
            label = label,
            mm = s.to_string()
        ));
    }
}

/// Vynechá hodnoty zhodné so štandardom.
fn drop_same(o: &mut CabinetOptions, b: &ResolvedOptions) {
    fn same<T: PartialEq>(v: &mut Option<T>, base: T) {
        if v.as_ref() == Some(&base) {
            *v = None;
        }
    }
    same(&mut o.joints, b.joints);
    same(&mut o.joint_type, b.joint_type);
    same(&mut o.shelf_support, b.shelf_support);
    same(&mut o.bumpers, b.bumpers);
    same(&mut o.legs, b.legs);
    same(&mut o.plinth, b.plinth);
    same(&mut o.tip_on, b.tip_on);
    same(&mut o.tip_on_color, b.tip_on_color);
    same(&mut o.tip_on_kind, b.tip_on_kind);
    same(&mut o.link_left, b.link_left);
}

fn carcass_codes(c: &CvjCabinet) -> impl Iterator<Item = String> + '_ {
    c.parts
        .iter()
        .filter(|p| p.role == PartRole::Carcass)
        .map(|p| p.code.trim().to_uppercase())
}

/// Sokel: samostatný diel „SOKEL“ (findings 11 §5.5).
fn has_plinth(c: &CvjCabinet) -> bool {
    c.parts
        .iter()
        .any(|p| p.role == PartRole::Loose && p.description.trim().eq_ignore_ascii_case("sokel"))
}

/// Zadná stena: diel chrbta `UB` (nepohľadový) alebo `FB` (pohľadový); `None` bez dielov.
pub(crate) fn has_back(c: &CvjCabinet) -> Option<bool> {
    (c.carcass_source == CarcassSource::Parts)
        .then(|| carcass_codes(c).any(|k| k == "UB" || k == "FB"))
}

/// Vrch spodnej skrinky: `TO` = celá doska, iba `TS` = vlysy.
pub(crate) fn top_style(c: &CvjCabinet) -> Option<TopStyle> {
    if c.carcass_source != CarcassSource::Parts {
        return None;
    }
    let codes: Vec<String> = carcass_codes(c).collect();
    if codes.iter().any(|k| k == "TO") {
        Some(TopStyle::Panel)
    } else if codes.iter().any(|k| k == "TS") {
        // vlysy na výšku (výška > hĺbka) = drezová skrinka (findings 12 §4)
        let vertical = c
            .parts
            .iter()
            .filter(|p| p.role == PartRole::Carcass && p.code.trim().eq_ignore_ascii_case("TS"))
            .any(|p| {
                let e = p.bbox.max - p.bbox.min;
                e.y > e.z + 1.0
            });
        Some(if vertical {
            TopStyle::Sink
        } else {
            TopStyle::Stretchers
        })
    } else {
        None
    }
}

/// Ležaté vlysy vrchu znížené pod vrch bokov (varná doska, findings 12 §5): o koľko je horná
/// plocha vlysov pod vrchom bokov, ak viac ako 5 mm.
pub(crate) fn stretcher_drop(c: &CvjCabinet) -> Option<f64> {
    let carcass = || c.parts.iter().filter(|p| p.role == PartRole::Carcass);
    let side_top = carcass()
        .filter(|p| {
            matches!(
                p.code.trim().to_uppercase().as_str(),
                "LU" | "RU" | "LF" | "RF"
            )
        })
        .map(|p| p.bbox.max.y)
        .fold(f64::NEG_INFINITY, f64::max);
    let ts: Vec<&crate::job::CvjPart> = carcass()
        .filter(|p| p.code.trim().eq_ignore_ascii_case("TS"))
        .collect();
    if ts.is_empty() || !side_top.is_finite() {
        return None;
    }
    // iba ležaté vlysy (výška menšia ako hĺbka)
    if ts.iter().any(|p| {
        let e = p.bbox.max - p.bbox.min;
        e.y > e.z
    }) {
        return None;
    }
    let top = ts
        .iter()
        .map(|p| p.bbox.max.y)
        .fold(f64::NEG_INFINITY, f64::max);
    let drop = side_top - top;
    (drop > 5.0).then(|| (drop * 10.0).round() / 10.0)
}

/// Predvolený vrch spodnej skrinky podľa štandardu.
pub(crate) fn standard_top(standard: Option<&Standard>) -> Option<TopStyle> {
    standard.map(|s| s.carcass.top_style)
}
