//! Strom otvorov čela CV (sekcie, findings 11 §3.2 a 13 §10.4) z čiel skrinky projektu.
//!
//! Pravidlá podľa zákaziek CV dielne (REF):
//! - jeden otvor = jeden uzol (ID 1, rodič 0) cez celé čelo,
//! - delenie = uzol `FO` (rodič) a za ním traja potomkovia: horný (pri stĺpcoch ľavý) otvor,
//!   delič `MR` (vodorovný) / `MS` (zvislý) na čiare delenia, zvyšok; uzly sú v poradí
//!   prechodu do hĺbky,
//! - výška otvoru = miesto čela + polovice škár (pri krajných otvoroch škára hore/dole),
//! - šírky stĺpcov sú šírky po čiaru delenia (súčet = šírka skrinky).

use cc_model::{CabinetSpec, FrontAdjust, FrontKind, FrontMount, FrontSpec, HingeSide, Standard};

/// Druh uzla sekcie (vzor sa hľadá podľa mena a STYPE v šablóne).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    /// delený otvor `FO` (STYPE 0)
    Split,
    /// vodorovný delič `MR`
    Rail,
    /// zvislý delič `MS`
    Stile,
    Door,
    /// dvojkrídlové dvierka: dva `DOR_OPEN` s rovnakým ID a STYPE 4
    Pair,
    Drawer,
    /// slepé čelo (`DWR_OPEN` STYPE 5)
    FalseFront,
    /// slepý panel (`DOR_OPEN` STYPE 8)
    Panel,
    /// otvorený bez čela (`FO` STYPE 3)
    Open,
}

impl NodeKind {
    /// meno objektu CV a STYPE
    pub fn cv(self) -> (&'static str, i32) {
        match self {
            NodeKind::Split => ("FO", 0),
            NodeKind::Rail => ("MR", 0),
            NodeKind::Stile => ("MS", 0),
            NodeKind::Door => ("DOR_OPEN", 1),
            NodeKind::Pair => ("DOR_OPEN", 4),
            NodeKind::Drawer => ("DWR_OPEN", 2),
            NodeKind::FalseFront => ("DWR_OPEN", 5),
            NodeKind::Panel => ("DOR_OPEN", 8),
            NodeKind::Open => ("FO", 3),
        }
    }

    /// TYPE uzla: 1 otvor, 2 vodorovný delič, 3 zvislý delič
    pub fn cv_type(self) -> i32 {
        match self {
            NodeKind::Rail => 2,
            NodeKind::Stile => 3,
            _ => 1,
        }
    }
}

/// Uzol sekcie na zápis (rozmery v mm v ráme skupiny `Face`, Y od spodku).
#[derive(Debug, Clone, PartialEq)]
pub struct SectionNode {
    pub kind: NodeKind,
    pub id: i32,
    pub parent: i32,
    pub x: f64,
    pub y: f64,
    pub dx: f64,
    pub dy: f64,
    /// strana pántov dvierok: 1 vľavo, 2 vpravo
    pub hinge: Option<i32>,
    /// výška korpusu zásuvky (zo skrinky, inak najvyššia zo systému, ktorá sa zmestí)
    pub box_height: Option<f64>,
    /// hĺbka korpusu zásuvky = dĺžka výsuvu NL (`BOXD`)
    pub box_depth: Option<f64>,
    /// riadok bez pevnej výšky medzi viacerými takými: CV ich delí rovnakým dielom (`EQ`)
    pub equal: bool,
    /// úprava rozmeru čela (OLL/OLR/OLT/OLB)
    pub adjust: FrontAdjust,
}

struct Builder {
    next_id: i32,
    out: Vec<SectionNode>,
}

impl Builder {
    fn id(&mut self) -> i32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn node(&mut self, kind: NodeKind, parent: i32, r: [f64; 4]) -> i32 {
        let id = self.id();
        self.push(kind, id, parent, r, None, false);
        id
    }

    fn push(
        &mut self,
        kind: NodeKind,
        id: i32,
        parent: i32,
        r: [f64; 4],
        f: Option<&FrontSpec>,
        equal: bool,
    ) {
        self.out.push(SectionNode {
            kind,
            id,
            parent,
            x: r[0],
            y: r[1],
            dx: r[2],
            dy: r[3],
            // bez strany ostane hodnota vzoru (napr. umývačka bez pántov)
            hinge: f
                .filter(|f| f.kind == FrontKind::Door)
                .and_then(|f| f.hinge_side)
                .map(|h| match h {
                    HingeSide::Left => 1,
                    HingeSide::Right => 2,
                }),
            box_height: f
                .filter(|f| f.kind == FrontKind::Drawer)
                .and_then(|f| f.box_height),
            box_depth: None,
            equal,
            adjust: f.map(|f| f.adjust.clone()).unwrap_or_default(),
        });
    }

    /// Otvor s čelom `f` v obdĺžniku `r` pod rodičom `parent` (aj stĺpce riadku).
    fn leaf(&mut self, f: &FrontSpec, parent: i32, r: [f64; 4], equal: bool) {
        if !f.columns.is_empty() {
            let widths = column_widths(&f.columns, r[2]);
            self.columns(&f.columns, &widths, parent, r);
            return;
        }
        let kind = match f.kind {
            FrontKind::Door => NodeKind::Door,
            FrontKind::Pair => NodeKind::Pair,
            FrontKind::Drawer => NodeKind::Drawer,
            FrontKind::FalseFront => NodeKind::FalseFront,
            FrontKind::Panel => NodeKind::Panel,
            FrontKind::Open => NodeKind::Open,
        };
        let id = self.id();
        self.push(kind, id, parent, r, Some(f), equal);
        if kind == NodeKind::Pair {
            // druhé krídlo: rovnaké ID (findings 13 §10.4)
            self.push(kind, id, parent, r, Some(f), equal);
        }
    }

    /// Riadky zhora nadol v obdĺžniku `r`; `heights` = výšky otvorov.
    fn rows(&mut self, rows: &[FrontSpec], heights: &[f64], equal: bool, parent: i32, r: [f64; 4]) {
        let eq = |f: &FrontSpec| equal && f.height.is_none();
        match (rows, heights) {
            ([f], _) => self.leaf(f, parent, r, eq(f)),
            ([first, rest @ ..], [h, hrest @ ..]) => {
                let fo = self.node(NodeKind::Split, parent, r);
                let boundary = r[1] + r[3] - h;
                self.leaf(first, fo, [r[0], boundary, r[2], *h], eq(first));
                self.node(NodeKind::Rail, fo, [r[0], boundary, 0.0, r[2]]);
                self.rows(rest, hrest, equal, fo, [r[0], r[1], r[2], boundary - r[1]]);
            }
            _ => {}
        }
    }

    /// Stĺpce zľava doprava v obdĺžniku `r`; `widths` = šírky po čiaru delenia.
    fn columns(&mut self, cols: &[FrontSpec], widths: &[f64], parent: i32, r: [f64; 4]) {
        match (cols, widths) {
            ([c], _) => self.leaf(c, parent, r, false),
            ([first, rest @ ..], [w, wrest @ ..]) => {
                let fo = self.node(NodeKind::Split, parent, r);
                let line = r[0] + w;
                self.leaf(first, fo, [r[0], r[1], *w, r[3]], false);
                self.node(NodeKind::Stile, fo, [line, r[1], 0.0, r[3]]);
                self.columns(rest, wrest, fo, [line, r[1], r[0] + r[2] - line, r[3]]);
            }
            _ => {}
        }
    }
}

fn column_widths(cols: &[FrontSpec], total: f64) -> Vec<f64> {
    let fixed: f64 = cols.iter().filter_map(|c| c.width).sum();
    let free = cols.iter().filter(|c| c.width.is_none()).count();
    let share = if free > 0 {
        (total - fixed) / free as f64
    } else {
        0.0
    };
    cols.iter().map(|c| c.width.unwrap_or(share)).collect()
}

/// Uzly sekcií čela skrinky v ráme skupiny `Face` (šírka `w`, výška `h` = korpus).
/// Výšky riadkov ako generátor CabinetCRAFT (škáry pri skrinke, inak zo štandardu);
/// `t` = hrúbka korpusu (pri vložených čelách).
pub fn face_sections(
    spec: &CabinetSpec,
    standard: &Standard,
    w: f64,
    h: f64,
    t: f64,
) -> Vec<SectionNode> {
    let mut b = Builder {
        next_id: 1,
        out: Vec::new(),
    };
    let whole = [0.0, 0.0, w, h];
    let fronts = &spec.fronts;
    if fronts.is_empty() {
        b.leaf(&FrontSpec::new(FrontKind::Open), 0, whole, false);
        return b.out;
    }
    let fs = &standard.fronts;
    let rows = fs.rows(spec.cabinet_type);
    let mount = spec.front_mount.unwrap_or(fs.mount);
    let (top, bottom, gap) = match mount {
        FrontMount::Overlay => (
            spec.reveals.top.unwrap_or(rows.top),
            spec.reveals.bottom.unwrap_or(rows.bottom),
            spec.reveals.horizontal.unwrap_or(rows.gap),
        ),
        FrontMount::Inset => (t + fs.inset_gap, t + fs.inset_gap, fs.inset_gap),
    };
    let n = fronts.len();
    let available = h - top - bottom - gap * (n.saturating_sub(1)) as f64;
    let fixed: f64 = fronts.iter().filter_map(|f| f.height).sum();
    let free = fronts.iter().filter(|f| f.height.is_none()).count();
    let share = if free > 0 {
        (available - fixed) / free as f64
    } else {
        0.0
    };
    // otvor = miesto čela + polovice škár; krajné otvory nesú škáru hore / dole
    let heights: Vec<f64> = fronts
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let slot = f.height.unwrap_or(share);
            let above = if i == 0 { top } else { gap / 2.0 };
            let below = if i + 1 == n { bottom } else { gap / 2.0 };
            slot + above + below
        })
        .collect();
    b.rows(fronts, &heights, free > 1, 0, whole);

    // korpus zásuvky ako generátor: najdlhší výsuv do hĺbky, najvyšší korpus pod vrch čela
    let drawer = spec
        .drawer
        .as_deref()
        .or(fs.drawer.as_deref())
        .and_then(|id| standard.drawer(id))
        .or(standard.drawers.first());
    if let Some(ds) = drawer {
        let length = ds
            .lengths
            .iter()
            .copied()
            .filter(|l| {
                l.is_finite()
                    && *l > 0.0
                    && l + ds.depth_reserve <= spec.d + 1e-6
                    && spec.drawer_length.is_none_or(|max| *l <= max + 1e-6)
            })
            .reduce(f64::max);
        for s in b.out.iter_mut().filter(|s| s.kind == NodeKind::Drawer) {
            s.box_depth = length;
            if s.box_height.is_none() {
                // čelo ≈ otvor bez škáry
                let front = s.dy - gap;
                s.box_height = ds
                    .heights
                    .iter()
                    .copied()
                    .filter(|h| {
                        h.is_finite() && *h > 0.0 && ds.above_front + h <= front - ds.top_reserve
                    })
                    .reduce(f64::max);
            }
        }
    }
    b.out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn standard() -> Standard {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/dielna/standards/dielna-cx100.toml");
        toml::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    fn cab(fronts: Vec<FrontSpec>) -> CabinetSpec {
        let mut c: CabinetSpec =
            toml::from_str("id = \"A\"\ntype = \"base\"\nw = 600\nh = 762\nd = 510").unwrap();
        c.fronts = fronts;
        c
    }

    /// VARNA v REF: miesta 160 / 298 / 298, škáry hore 2, dole 0, medzi 2 → otvory
    /// 163 / 300 / 299 a strom FO, DWR, MR, FO, DWR, MR, DWR ako v CV.
    #[test]
    fn varna_like_cv() {
        let mut c = cab(vec![
            FrontSpec {
                height: Some(160.0),
                box_height: Some(103.0),
                ..FrontSpec::new(FrontKind::Drawer)
            },
            FrontSpec::new(FrontKind::Drawer),
            FrontSpec::new(FrontKind::Drawer),
        ]);
        c.reveals.top = Some(2.0);
        c.reveals.bottom = Some(0.0);
        c.reveals.horizontal = Some(2.0);
        let n = face_sections(&c, &standard(), 600.0, 762.0, 18.0);
        let summary: Vec<(NodeKind, i32, i32, f64, f64)> = n
            .iter()
            .map(|s| (s.kind, s.id, s.parent, s.y, s.dy))
            .collect();
        use NodeKind::*;
        assert_eq!(
            summary,
            [
                (Split, 1, 0, 0.0, 762.0),
                (Drawer, 2, 1, 599.0, 163.0),
                (Rail, 3, 1, 599.0, 600.0),
                (Split, 4, 1, 0.0, 599.0),
                (Drawer, 5, 4, 299.0, 300.0),
                (Rail, 6, 4, 299.0, 600.0),
                (Drawer, 7, 4, 0.0, 299.0),
            ]
        );
        assert_eq!(n[1].box_height, Some(103.0));
    }

    /// Roh REF: panel 500 vľavo + dvierka → FO, DOR_OPEN(8), MS na 500, DOR_OPEN(1) HNG 1.
    #[test]
    fn blind_corner_like_cv() {
        let c = cab(vec![FrontSpec {
            columns: vec![
                FrontSpec {
                    width: Some(500.0),
                    ..FrontSpec::new(FrontKind::Panel)
                },
                FrontSpec {
                    hinge_side: Some(HingeSide::Left),
                    ..FrontSpec::new(FrontKind::Door)
                },
            ],
            ..FrontSpec::new(FrontKind::Door)
        }]);
        let n = face_sections(&c, &standard(), 1000.0, 762.0, 18.0);
        let summary: Vec<(NodeKind, i32, i32, f64, f64)> = n
            .iter()
            .map(|s| (s.kind, s.id, s.parent, s.x, s.dx))
            .collect();
        use NodeKind::*;
        assert_eq!(
            summary,
            [
                (Split, 1, 0, 0.0, 1000.0),
                (Panel, 2, 1, 0.0, 500.0),
                (Stile, 3, 1, 500.0, 0.0),
                (Door, 4, 1, 500.0, 500.0),
            ]
        );
        assert_eq!(n[3].hinge, Some(1));
    }

    /// Jedny dvierka vpravo cez celé čelo; dvojité = dva uzly s rovnakým ID.
    #[test]
    fn single_and_pair() {
        let c = cab(vec![FrontSpec {
            hinge_side: Some(HingeSide::Right),
            ..FrontSpec::new(FrontKind::Door)
        }]);
        let n = face_sections(&c, &standard(), 600.0, 762.0, 18.0);
        assert_eq!(n.len(), 1);
        assert_eq!(
            (n[0].id, n[0].parent, n[0].hinge, n[0].dy),
            (1, 0, Some(2), 762.0)
        );
        let p = face_sections(
            &cab(vec![FrontSpec::new(FrontKind::Pair)]),
            &standard(),
            600.0,
            762.0,
            18.0,
        );
        assert_eq!(p.len(), 2);
        assert_eq!((p[0].id, p[1].id, p[0].kind), (1, 1, NodeKind::Pair));
    }
}
