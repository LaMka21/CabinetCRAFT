//! Miestnosť (špecifikácia, kap. 13): steny, poloha skriniek, kontrola umiestnenia,
//! automatické vyplnenie steny a kóty pre pôdorys a pohľad na stenu.
//!
//! Súradnice miestnosti: pôdorys `x, y` v mm, `z` nahor. Stena je úsečka svojej
//! vnútornej plochy; miestnosť leží napravo od smeru `start → koniec`, telo steny naľavo.
//! Skrinka stojí zadnou stranou na vnútornej ploche steny, jej `x` ide pozdĺž steny
//! (pri pohľade z miestnosti zľava doprava), predok smeruje do miestnosti.

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
    CabinetSpec, CabinetType, Issue, IssueCode, OpeningKind, ProjectFile, RoomSpec, Transform,
    WallSpec,
};
use glam::{DVec2, DVec3};

const TOL: f64 = 0.5;

/// Stena s odvodenou geometriou.
#[derive(Debug, Clone)]
pub struct Wall {
    pub id: String,
    pub start: DVec2,
    pub end: DVec2,
    /// jednotkový smer pozdĺž steny
    pub dir: DVec2,
    /// jednotková normála do miestnosti (napravo od smeru)
    pub inward: DVec2,
    pub length: f64,
    pub height: f64,
    pub thickness: f64,
    /// stena, ktorá končí v našom začiatku
    pub prev: Option<usize>,
    /// stena, ktorá začína v našom konci
    pub next: Option<usize>,
}

impl Wall {
    /// Bod na vnútornej ploche vo vzdialenosti `at` od začiatku, posunutý o `offset` do miestnosti.
    pub fn point(&self, at: f64, offset: f64) -> DVec2 {
        self.start + self.dir * at + self.inward * offset
    }
}

/// Miestnosť projektu.
#[derive(Debug, Clone)]
pub struct Room {
    pub spec: RoomSpec,
    pub walls: Vec<Wall>,
    /// Vrch spodných a spodok horných skriniek pre výškové umiestnenie.
    pub levels: cc_model::PlacementLevels,
}

/// Rad, do ktorého skrinka patrí pri vypĺňaní a kótovaní.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    /// spodné a vysoké
    Base,
    /// horné a vysoké
    Upper,
}

impl Row {
    pub fn contains(self, t: CabinetType) -> bool {
        match self {
            Row::Base => t != CabinetType::Wall,
            Row::Upper => t != CabinetType::Base,
        }
    }
}

impl Room {
    pub fn new(project: &ProjectFile) -> Room {
        let spec = project.room.clone().unwrap_or_default();
        let mut walls: Vec<Wall> = project
            .walls
            .iter()
            .map(|w| {
                let a = w.angle.to_radians();
                let dir = DVec2::new(a.cos(), a.sin());
                let start = DVec2::new(w.x, w.y);
                Wall {
                    id: w.id.clone(),
                    start,
                    end: start + dir * w.length,
                    dir,
                    inward: DVec2::new(dir.y, -dir.x),
                    length: w.length,
                    height: w.height.unwrap_or(spec.wall_height),
                    thickness: w.thickness.unwrap_or(spec.wall_thickness),
                    prev: None,
                    next: None,
                }
            })
            .collect();
        for i in 0..walls.len() {
            for j in 0..walls.len() {
                if i != j && walls[i].end.distance(walls[j].start) < 1.0 {
                    walls[i].next = Some(j);
                    walls[j].prev = Some(i);
                }
            }
        }
        Room {
            spec,
            walls,
            levels: cc_model::PlacementLevels::of(project),
        }
    }

    pub fn wall_index(&self, id: &str) -> Option<usize> {
        self.walls.iter().position(|w| w.id == id)
    }

    /// Obrys tela steny v pôdoryse: začiatok a koniec vnútornej plochy, potom koniec
    /// a začiatok vonkajšej plochy (v rohoch zrezané na pokos so susednou stenou).
    pub fn wall_outline(&self, i: usize) -> [DVec2; 4] {
        let Some(w) = self.walls.get(i) else {
            return [DVec2::ZERO; 4];
        };
        let outward = -w.inward;
        let back = |wall: &Wall| (wall.start - wall.inward * wall.thickness, wall.dir);
        let (bp, bd) = (w.start + outward * w.thickness, w.dir);
        let back_start = w
            .prev
            .and_then(|p| self.walls.get(p))
            .and_then(|pw| {
                let (pp, pd) = back(pw);
                intersect(pp, pd, bp, bd)
            })
            .unwrap_or(bp);
        let back_end = w
            .next
            .and_then(|n| self.walls.get(n))
            .and_then(|nw| {
                let (np, nd) = back(nw);
                intersect(bp, bd, np, nd)
            })
            .unwrap_or(w.end + outward * w.thickness);
        [w.start, w.end, back_end, back_start]
    }

    /// Výška spodku skrinky nad podlahou.
    pub fn elevation(&self, c: &CabinetSpec) -> f64 {
        c.elevation_in(&self.spec, &self.levels)
    }

    /// Súradnice skrinky (predný ľavý spodný roh = 0) → súradnice miestnosti, ak stojí pri stene.
    pub fn placement(&self, c: &CabinetSpec) -> Option<Transform> {
        let w = self.walls.get(self.wall_index(c.wall.as_deref()?)?)?;
        let front = w.point(c.at, c.outset + c.d);
        Some(Transform::from_axes(
            w.dir.extend(0.0),
            (-w.inward).extend(0.0),
            front.extend(self.elevation(c)),
        ))
    }

    /// Obdĺžnik pôdorysu skrinky (4 rohy: predný ľavý, predný pravý, zadný pravý, zadný ľavý).
    pub fn footprint(t: &Transform, c: &CabinetSpec) -> [DVec2; 4] {
        [
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(c.w, 0.0, 0.0),
            DVec3::new(c.w, c.d, 0.0),
            DVec3::new(0.0, c.d, 0.0),
        ]
        .map(|p| t.point_to_cabinet(p).truncate())
    }

    /// Obrys podlahy: referenčné čiary stien uzavretej reťaze, ktorá obsahuje prvú stenu (body
    /// v poradí stien). `None`, ak reťaz nie je uzavretá.
    pub fn floor_polygon(&self) -> Option<Vec<DVec2>> {
        let mut out = Vec::new();
        let mut i = 0usize;
        for _ in 0..self.walls.len() {
            let w = self.walls.get(i)?;
            out.push(w.start);
            i = w.next?;
            if i == 0 {
                return (out.len() >= 3).then_some(out);
            }
        }
        None
    }

    /// Obdĺžnik (min, max) všetkých stien v pôdoryse.
    pub fn bounds(&self) -> Option<(DVec2, DVec2)> {
        let mut it = (0..self.walls.len()).flat_map(|i| self.wall_outline(i));
        let first = it.next()?;
        Some(it.fold((first, first), |(lo, hi), p| (lo.min(p), hi.max(p))))
    }
}

/// Uzavreté miestnosti (reťaze stien), v ktorých je vnútro naľavo od smeru stien (nakreslené
/// proti smeru hodinových ručičiek): indexy stien každej takej reťaze.
pub fn reversed_loops(project: &ProjectFile) -> Vec<Vec<usize>> {
    let room = Room::new(project);
    let mut seen = vec![false; room.walls.len()];
    let mut out = Vec::new();
    for first in 0..room.walls.len() {
        if seen[first] {
            continue;
        }
        let mut chain = Vec::new();
        let mut i = first;
        let closed = loop {
            if chain.contains(&i) || chain.len() > room.walls.len() {
                break false;
            }
            chain.push(i);
            match room.walls.get(i).and_then(|w| w.next) {
                Some(n) if n == first => break true,
                Some(n) => i = n,
                None => break false,
            }
        };
        for &k in &chain {
            if let Some(s) = seen.get_mut(k) {
                *s = true;
            }
        }
        if !closed || chain.len() < 3 {
            continue;
        }
        // plocha > 0 = proti smeru hodín (os y nahor); vnútro je potom naľavo, nie napravo
        let area: f64 = chain
            .iter()
            .filter_map(|&k| room.walls.get(k))
            .map(|w| w.start.perp_dot(w.end))
            .sum::<f64>()
            / 2.0;
        if area > 0.0 {
            out.push(chain);
        }
    }
    out
}

/// Otočí smer stien uzavretých miestností nakreslených proti smeru hodinových ručičiek, aby
/// bola miestnosť napravo od smeru (ako v CV). Skrinky a otvory na týchto stenách ostanú na
/// mieste: ich poloha sa prepočíta od nového začiatku steny. Vráti ID otočených stien.
pub fn orient_rooms(project: &mut ProjectFile) -> Vec<String> {
    let loops = reversed_loops(project);
    let mut turned = Vec::new();
    for chain in loops {
        for k in chain {
            let Some(w) = project.walls.get_mut(k) else {
                continue;
            };
            let a = w.angle.to_radians();
            w.x += w.length * a.cos();
            w.y += w.length * a.sin();
            w.x = (w.x * 1000.0).round() / 1000.0;
            w.y = (w.y * 1000.0).round() / 1000.0;
            let mut ang = w.angle + 180.0;
            while ang > 180.0 {
                ang -= 360.0;
            }
            w.angle = ang;
            let (id, len) = (w.id.clone(), w.length);
            for c in project
                .cabinets
                .iter_mut()
                .filter(|c| c.wall.as_deref() == Some(id.as_str()))
            {
                c.at = len - c.at - c.w;
            }
            for o in project.openings.iter_mut().filter(|o| o.wall == id) {
                o.at = len - o.at - o.width;
            }
            turned.push(id);
        }
    }
    turned
}

/// Priesečník priamok `p + t·d` a `q + s·e`.
fn intersect(p: DVec2, d: DVec2, q: DVec2, e: DVec2) -> Option<DVec2> {
    let den = d.perp_dot(e);
    if den.abs() < 1e-9 {
        return None;
    }
    let t = (q - p).perp_dot(e) / den;
    Some(p + d * t)
}

/// Poloha každej skrinky projektu v súradniciach miestnosti. Skrinky bez steny sa
/// zoradia do radu pred miestnosťou (spodné a vysoké dole, horné nad nimi).
pub fn layout(project: &ProjectFile) -> Vec<Transform> {
    let room = Room::new(project);
    let max_d = project.cabinets.iter().map(|c| c.d).fold(0.0, f64::max);
    let base_h = project
        .cabinets
        .iter()
        .filter(|c| c.cabinet_type == CabinetType::Base)
        .map(|c| c.h)
        .fold(720.0, f64::max);
    let (origin_x, origin_y) = match room.bounds() {
        Some((lo, _)) => (lo.x, lo.y - 1500.0 - max_d),
        None => (0.0, 0.0),
    };
    let (mut x_low, mut x_high) = (origin_x, origin_x);
    project
        .cabinets
        .iter()
        .map(|c| {
            if let Some(t) = room.placement(c) {
                return t;
            }
            let y = origin_y + max_d - c.d;
            let origin = match c.cabinet_type {
                CabinetType::Wall => {
                    let o = DVec3::new(x_high, y, base_h + 550.0);
                    x_high += c.w;
                    o
                }
                _ => {
                    let o = DVec3::new(x_low, y, 0.0);
                    x_low += c.w;
                    o
                }
            };
            Transform::from_axes(DVec3::X, DVec3::Y, origin)
        })
        .collect()
}

/// Kontrola umiestnenia (kap. 13.4); iba varovania.
pub fn check(project: &ProjectFile) -> Vec<Issue> {
    let room = Room::new(project);
    let mut issues = Vec::new();
    let placed: Vec<(usize, Transform)> = project
        .cabinets
        .iter()
        .enumerate()
        .filter_map(|(i, c)| room.placement(c).map(|t| (i, t)))
        .collect();
    for &(i, _) in &placed {
        let Some(c) = project.cabinets.get(i) else {
            continue;
        };
        let Some(w) = c
            .wall
            .as_deref()
            .and_then(|id| room.wall_index(id))
            .and_then(|wi| room.walls.get(wi))
        else {
            continue;
        };
        if c.at < -TOL || c.at + c.w > w.length + TOL {
            issues.push(Issue::warning(
                IssueCode::PlacementOutsideWall,
                None,
                tr!(
                    "room.check.outside_wall",
                    id = &c.id,
                    wall = &w.id,
                    from = format!("{:.0}", c.at),
                    to = format!("{:.0}", c.at + c.w),
                    length = format!("{:.0}", w.length)
                ),
            ));
        }
        let top = room.elevation(c) + c.h;
        if top > w.height + TOL {
            issues.push(Issue::warning(
                IssueCode::PlacementAboveCeiling,
                None,
                tr!(
                    "room.check.above_ceiling",
                    id = &c.id,
                    top = format!("{:.0}", top),
                    wall = &w.id,
                    height = format!("{:.0}", w.height)
                ),
            ));
        }
        for o in project
            .openings
            .iter()
            .filter(|o| Some(&o.wall) == c.wall.as_ref())
        {
            let z0 = room.elevation(c);
            let horizontal = c.at < o.at + o.width - TOL && o.at < c.at + c.w - TOL;
            let vertical = z0 < o.elevation + o.height - TOL && o.elevation < z0 + c.h - TOL;
            if horizontal && vertical {
                let (id, wall) = (&c.id, &o.wall);
                let (from, to) = (format!("{:.0}", o.at), format!("{:.0}", o.at + o.width));
                // celá veta podľa druhu otvoru (pád slova sa v jazykoch líši)
                let message = match o.kind {
                    OpeningKind::Window => {
                        tr!(
                            "room.check.into_window",
                            id = id,
                            wall = wall,
                            from = &from,
                            to = &to
                        )
                    }
                    OpeningKind::Door => {
                        tr!(
                            "room.check.into_door",
                            id = id,
                            wall = wall,
                            from = &from,
                            to = &to
                        )
                    }
                    OpeningKind::Opening => {
                        tr!(
                            "room.check.into_opening",
                            id = id,
                            wall = wall,
                            from = &from,
                            to = &to
                        )
                    }
                };
                issues.push(Issue::warning(IssueCode::PlacementOpening, None, message));
            }
        }
    }
    for (k, &(i, ti)) in placed.iter().enumerate() {
        for &(j, tj) in placed.iter().skip(k + 1) {
            let (Some(a), Some(b)) = (project.cabinets.get(i), project.cabinets.get(j)) else {
                continue;
            };
            let (za, zb) = (room.elevation(a), room.elevation(b));
            if za >= zb + b.h - TOL || zb >= za + a.h - TOL {
                continue;
            }
            if rects_overlap(&Room::footprint(&ti, a), &Room::footprint(&tj, b)) {
                issues.push(Issue::warning(
                    IssueCode::PlacementOverlap,
                    None,
                    tr!("room.check.overlap", a = &a.id, b = &b.id),
                ));
            }
        }
    }
    issues
}

/// Prekryv dvoch konvexných štvoruholníkov (vety o oddeľujúcej osi), dotyk sa nepočíta.
fn rects_overlap(a: &[DVec2; 4], b: &[DVec2; 4]) -> bool {
    for poly in [a, b] {
        for i in 0..4 {
            let edge = poly[(i + 1) % 4] - poly[i];
            let axis = DVec2::new(-edge.y, edge.x).normalize_or_zero();
            let proj = |p: &[DVec2; 4]| {
                p.iter()
                    .map(|v| v.dot(axis))
                    .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), d| {
                        (lo.min(d), hi.max(d))
                    })
            };
            let (a0, a1) = proj(a);
            let (b0, b1) = proj(b);
            if a1 <= b0 + TOL || b1 <= a0 + TOL {
                return false;
            }
        }
    }
    true
}

/// Parametre automatického vyplnenia (kap. 13.5).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FillParams {
    /// najmenšia vypĺňaná medzera (užšia ostane prázdna)
    pub min: f64,
    /// bežná šírka skrinky
    pub nominal: f64,
    /// najväčšia šírka skrinky
    pub max: f64,
    /// krok šírky
    pub increment: f64,
}

impl Default for FillParams {
    fn default() -> Self {
        FillParams {
            min: 150.0,
            nominal: 600.0,
            max: 900.0,
            increment: 50.0,
        }
    }
}

/// Rozdelenie medzery `gap` na skrinky: (šírky zľava doprava, výplň vľavo, výplň vpravo).
pub fn fill_widths(gap: f64, p: &FillParams) -> (Vec<f64>, f64, f64) {
    let inc = p.increment.max(1.0);
    if gap + 1e-9 < p.min {
        return (Vec::new(), 0.0, 0.0);
    }
    let mut n = if gap <= p.max + 1e-9 {
        1
    } else {
        ((gap / p.nominal).floor() as usize).max(2)
    };
    while gap / n as f64 > p.max + 1e-9 {
        n += 1;
    }
    let base = ((gap / n as f64) / inc + 1e-9).floor() * inc;
    let mut widths = vec![base; n];
    let mut rest = gap - base * n as f64;
    // rozšírenie o krok symetricky od stredu
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| {
        let c = (n as f64 - 1.0) / 2.0;
        (a as f64 - c)
            .abs()
            .total_cmp(&(b as f64 - c).abs())
            .then(a.cmp(&b))
    });
    for &i in order.iter().cycle().take(n * 4) {
        if rest + 1e-9 < inc || widths[i] + inc > p.max + 1e-9 {
            if rest + 1e-9 < inc {
                break;
            }
            continue;
        }
        widths[i] += inc;
        rest -= inc;
    }
    let rest = (rest * 1e6).round() / 1e6;
    (widths, rest / 2.0, rest / 2.0)
}

/// Voľné úseky steny `wall` v danom rade: (od, do) v mm od ľavého konca. Odpočíta
/// existujúce skrinky radu, dvere (oba rady), okná (rad, do ktorého výškou zasahujú)
/// a v rohoch hĺbku skriniek toho istého radu na susednej stene.
pub fn free_gaps(project: &ProjectFile, wall: &str, row: Row, min: f64) -> Vec<(f64, f64)> {
    let room = Room::new(project);
    let Some(w) = room.wall_index(wall).and_then(|wi| room.walls.get(wi)) else {
        return Vec::new();
    };
    let row_depth = |other: usize, near_end: bool| -> f64 {
        let Some(ow) = room.walls.get(other) else {
            return 0.0;
        };
        project
            .cabinets
            .iter()
            .filter(|c| c.wall.as_deref() == Some(ow.id.as_str()) && row.contains(c.cabinet_type))
            .filter(|c| {
                if near_end {
                    c.at + c.w > ow.length - c.d - 1.0
                } else {
                    c.at < c.d + 1.0
                }
            })
            .map(|c| c.d + c.outset)
            .fold(0.0, f64::max)
    };
    let left = w.prev.map_or(0.0, |p| row_depth(p, true));
    let right = w.length - w.next.map_or(0.0, |n| row_depth(n, false));
    // výškový rozsah radu: spodný po pracovnú dosku (najvyššia spodná skrinka + 40 mm),
    // horný od spodku najvyššej hornej skrinky po hornú líniu
    let tallest = |t: CabinetType| {
        project
            .cabinets
            .iter()
            .filter(|c| c.cabinet_type == t)
            .map(|c| c.h)
            .fold(f64::NEG_INFINITY, f64::max)
    };
    let base_h = Some(tallest(CabinetType::Base))
        .filter(|h| h.is_finite())
        .unwrap_or(720.0);
    let upper_h = Some(tallest(CabinetType::Wall))
        .filter(|h| h.is_finite())
        .unwrap_or(720.0);
    let (z0, z1) = match row {
        Row::Base => (0.0, room.spec.toe_height + base_h + 40.0),
        Row::Upper => (room.spec.upper_top() - upper_h, room.spec.upper_top()),
    };
    let mut blocked: Vec<(f64, f64)> = project
        .cabinets
        .iter()
        .filter(|c| c.wall.as_deref() == Some(wall) && row.contains(c.cabinet_type))
        .map(|c| (c.at, c.at + c.w))
        .collect();
    for o in project.openings.iter().filter(|o| o.wall == wall) {
        if o.elevation < z1 && o.elevation + o.height > z0 {
            blocked.push((o.at, o.at + o.width));
        }
    }
    blocked.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut gaps = Vec::new();
    let mut cursor = left;
    for (a, b) in blocked {
        if a > cursor + min - 1e-9 {
            gaps.push((cursor, a.min(right)));
        }
        cursor = cursor.max(b);
    }
    if right > cursor + min - 1e-9 {
        gaps.push((cursor, right));
    }
    gaps.retain(|(a, b)| b - a + 1e-9 >= min);
    gaps
}

/// Rohové limity skrinky `index` (Snap to Corner): (ľavý, pravý) koniec steny zmenšený
/// o hĺbku skriniek jej radu na susednej stene v rohu.
pub fn corner_limits(project: &ProjectFile, index: usize) -> Option<(f64, f64)> {
    let c = project.cabinets.get(index)?;
    let wall_id = c.wall.as_deref()?;
    let room = Room::new(project);
    let w = room.walls.get(room.wall_index(wall_id)?)?;
    let shares_row = |t: CabinetType| {
        [Row::Base, Row::Upper]
            .into_iter()
            .any(|r| r.contains(c.cabinet_type) && r.contains(t))
    };
    let depth = |other: Option<usize>, near_end: bool| -> f64 {
        let Some(ow) = other.and_then(|o| room.walls.get(o)) else {
            return 0.0;
        };
        project
            .cabinets
            .iter()
            .filter(|o| o.wall.as_deref() == Some(ow.id.as_str()) && shares_row(o.cabinet_type))
            .filter(|o| {
                if near_end {
                    o.at + o.w > ow.length - o.d - 1.0
                } else {
                    o.at < o.d + 1.0
                }
            })
            .map(|o| o.d + o.outset)
            .fold(0.0, f64::max)
    };
    Some((depth(w.prev, true), w.length - depth(w.next, false)))
}

/// Limity skrinky `index` pozdĺž jej steny pre polia Clearance (ako v CV): (ľavý limit,
/// pravý limit, dĺžka steny). Limit je hrana najbližšej skrinky toho istého radu, inak
/// koniec steny zmenšený o hĺbku skriniek radu na susednej stene v rohu.
pub fn row_limits(project: &ProjectFile, index: usize) -> Option<(f64, f64, f64)> {
    let c = project.cabinets.get(index)?;
    let wall_id = c.wall.as_deref()?;
    let room = Room::new(project);
    let w = room.walls.get(room.wall_index(wall_id)?)?;
    let shares_row = |t: CabinetType| {
        [Row::Base, Row::Upper]
            .into_iter()
            .any(|r| r.contains(c.cabinet_type) && r.contains(t))
    };
    let corner = |other: Option<usize>, near_end: bool| -> f64 {
        let Some(ow) = other.and_then(|o| room.walls.get(o)) else {
            return 0.0;
        };
        project
            .cabinets
            .iter()
            .filter(|o| o.wall.as_deref() == Some(ow.id.as_str()) && shares_row(o.cabinet_type))
            .filter(|o| {
                if near_end {
                    o.at + o.w > ow.length - o.d - 1.0
                } else {
                    o.at < o.d + 1.0
                }
            })
            .map(|o| o.d + o.outset)
            .fold(0.0, f64::max)
    };
    let mut left = corner(w.prev, true);
    let mut right = w.length - corner(w.next, false);
    for (j, o) in project.cabinets.iter().enumerate() {
        if j == index || o.wall.as_deref() != Some(wall_id) || !shares_row(o.cabinet_type) {
            continue;
        }
        if o.at + o.w <= c.at + 1e-6 {
            left = left.max(o.at + o.w);
        } else if o.at >= c.at + c.w - 1e-6 {
            right = right.min(o.at);
        }
    }
    Some((left, right, w.length))
}

/// Poloha pre novú skrinku typu `t` šírky `w` na stene `wall`: voľné miesto radu najbližšie
/// k `prefer` (ako v CV: prekrývajúca skrinka sa posunie, aby sa suseda dotýkala; pri
/// rovnakej vzdialenosti doprava). Vysoká skrinka potrebuje voľno v oboch radoch.
/// `None`, ak sa na stenu nezmestí.
pub fn place_on_wall(
    project: &ProjectFile,
    wall: &str,
    t: CabinetType,
    w: f64,
    prefer: f64,
) -> Option<f64> {
    if !(w.is_finite() && w > 0.0 && prefer.is_finite()) {
        return None;
    }
    let rows: &[Row] = match t {
        CabinetType::Base => &[Row::Base],
        CabinetType::Wall => &[Row::Upper],
        CabinetType::Tall => &[Row::Base, Row::Upper],
    };
    let gaps: Vec<Vec<(f64, f64)>> = rows
        .iter()
        .map(|r| free_gaps(project, wall, *r, w))
        .collect();
    let fits = |at: f64| {
        gaps.iter()
            .all(|g| g.iter().any(|(a, b)| at >= a - 1e-6 && at + w <= b + 1e-6))
    };
    // kandidáti: preferovaná poloha a začiatky/konce voľných úsekov
    let mut candidates: Vec<f64> = vec![prefer];
    for g in &gaps {
        for (a, b) in g {
            candidates.push(*a);
            candidates.push(b - w);
        }
    }
    candidates
        .iter()
        .copied()
        .filter(|&c| fits(c))
        .min_by(|a, b| {
            ((a - prefer).abs(), -a)
                .partial_cmp(&((b - prefer).abs(), -b))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

/// Kótovacie body pozdĺž steny (mm od ľavého konca), zoradené a bez duplicít:
/// konce steny, hrany skriniek radu a okraje otvorov, ktoré rad ovplyvňujú.
pub fn dimension_points(project: &ProjectFile, wall: &str, row: Row) -> Vec<f64> {
    let room = Room::new(project);
    let Some(w) = room.wall_index(wall).and_then(|wi| room.walls.get(wi)) else {
        return Vec::new();
    };
    let mut pts = vec![0.0, w.length];
    for c in project
        .cabinets
        .iter()
        .filter(|c| c.wall.as_deref() == Some(wall) && row.contains(c.cabinet_type))
    {
        pts.extend([c.at, c.at + c.w]);
    }
    for o in project.openings.iter().filter(|o| o.wall == wall) {
        let relevant = match row {
            Row::Base => o.kind == OpeningKind::Door,
            Row::Upper => o.kind != OpeningKind::Door,
        };
        if relevant {
            pts.extend([o.at, o.at + o.width]);
        }
    }
    pts.sort_by(f64::total_cmp);
    pts.dedup_by(|a, b| (*a - *b).abs() < 0.05);
    pts
}

/// Ďalšie voľné ID steny `W1`, `W2`…
pub fn next_wall_id(project: &ProjectFile) -> String {
    (1..=project.walls.len() + 1)
        .map(|n| format!("W{n}"))
        .find(|id| project.walls.iter().all(|w| &w.id != id))
        .unwrap_or_else(|| format!("W{}", project.walls.len() + 1))
}

/// Ďalšie voľné ID skrinky: spodné `SP`, horné `HO`, vysoké `VY` + číslo.
pub fn next_cabinet_id(cabinets: &[CabinetSpec], t: CabinetType) -> String {
    let prefix = match t {
        CabinetType::Base => "SP",
        CabinetType::Wall => "HO",
        CabinetType::Tall => "VY",
    };
    (1..=cabinets.len() + 1)
        .map(|n| format!("{prefix}{n}"))
        .find(|id| cabinets.iter().all(|c| &c.id != id))
        .unwrap_or_else(|| format!("{prefix}{}", cabinets.len() + 1))
}

/// Štyri steny obdĺžnikovej miestnosti `w × d` s ľavým dolným rohom v `origin`, kreslené
/// v smere hodinových ručičiek (miestnosť napravo); prvá je zadná stena (hore v pôdoryse).
pub fn rect_walls(project: &ProjectFile, origin: DVec2, w: f64, d: f64) -> Vec<WallSpec> {
    let mut tmp = project.clone();
    let mut out = Vec::new();
    for (x, y, length, angle) in [
        (origin.x, origin.y + d, w, 0.0),
        (origin.x + w, origin.y + d, d, -90.0),
        (origin.x + w, origin.y, w, 180.0),
        (origin.x, origin.y, d, 90.0),
    ] {
        let wall = WallSpec {
            id: next_wall_id(&tmp),
            x,
            y,
            length,
            angle,
            height: None,
            thickness: None,
        };
        tmp.walls.push(wall.clone());
        out.push(wall);
    }
    out
}

/// Skrinky, ktoré vyplnia voľné úseky radu `row` na stene `wall` (kap. 13.5).
/// Rozmery okrem šírky a police berie zo vzoru `template`.
pub fn autofill(
    project: &ProjectFile,
    wall: &str,
    row: Row,
    p: &FillParams,
    template: &CabinetSpec,
) -> Vec<CabinetSpec> {
    let t = match row {
        Row::Base => CabinetType::Base,
        Row::Upper => CabinetType::Wall,
    };
    let mut all = project.cabinets.clone();
    let mut added = Vec::new();
    for (a, b) in free_gaps(project, wall, row, p.min) {
        let (widths, left, _) = fill_widths(b - a, p);
        let mut at = a + left;
        for w in widths {
            let mut c = template.clone();
            c.id = next_cabinet_id(&all, t);
            c.cabinet_type = t;
            c.w = w;
            c.wall = Some(wall.to_owned());
            c.at = (at * 10.0).round() / 10.0;
            c.elevation = None;
            c.outset = 0.0;
            c.fixed_shelves.clear();
            all.push(c.clone());
            added.push(c);
            at += w;
        }
    }
    added
}

/// Skrinky radu `row` pri rohu na konci steny `wall` (a začiatku nasledujúcej steny):
/// (index skrinky na `wall` najbližšie ku koncu, index skrinky na ďalšej stene najbližšie
/// k začiatku, ID ďalšej steny). `None`, ak roh nemá dve skrinky.
pub fn corner_cabinets(
    project: &ProjectFile,
    wall: &str,
    row: Row,
) -> Option<(usize, usize, String)> {
    let room = Room::new(project);
    let w = room.walls.get(room.wall_index(wall)?)?;
    let next = room.walls.get(w.next?)?;
    let on = |id: &str| -> Vec<(usize, f64, f64)> {
        project
            .cabinets
            .iter()
            .enumerate()
            .filter(|(_, c)| c.wall.as_deref() == Some(id) && row.contains(c.cabinet_type))
            .map(|(i, c)| (i, c.at, c.at + c.w))
            .collect()
    };
    let a = on(&w.id)
        .into_iter()
        .max_by(|x, y| x.2.total_cmp(&y.2))
        .map(|x| x.0)?;
    let b = on(&next.id)
        .into_iter()
        .min_by(|x, y| x.1.total_cmp(&y.1))
        .map(|x| x.0)?;
    Some((a, b, next.id.clone()))
}

/// Slepý roh: predĺžená časť skrinky dostane v každom riadku čiel slepý panel (stĺpec
/// `Panel` šírky predĺženia) na strane rohu; pôvodné čelá ostanú. `left` = roh vľavo.
fn add_blind_panel(c: &mut CabinetSpec, ext: f64, left: bool) {
    if ext <= 1.0 || !ext.is_finite() {
        return;
    }
    for row in &mut c.fronts {
        if row.kind == cc_model::FrontKind::Open && row.columns.is_empty() {
            continue;
        }
        let panel = cc_model::FrontSpec {
            width: Some(ext),
            ..cc_model::FrontSpec::new(cc_model::FrontKind::Panel)
        };
        if row.columns.is_empty() {
            let mut front = row.clone();
            front.height = None;
            front.width = None;
            row.columns = if left {
                vec![panel, front]
            } else {
                vec![front, panel]
            };
        } else if left {
            row.columns.insert(0, panel);
        } else {
            row.columns.push(panel);
        }
    }
}

/// Pravý klik do rohu v CV „Extend Left/Right End“ (findings 06 §4.4): skrinka pri rohu
/// sa predĺži až do rohu (na stene `wall` po jej koniec, na ďalšej stene od začiatku) a druhá
/// skrinka sa podľa potreby odsunie tak, aby sa jej dotýkala. Vráti ID predĺženej skrinky.
pub fn extend_into_corner(
    project: &mut ProjectFile,
    wall: &str,
    row: Row,
    extend_next: bool,
) -> Result<String, String> {
    let (a, b, _) = corner_cabinets(project, wall, row)
        .ok_or_else(|| tr!("room.corner.no_cabinets", wall = wall))?;
    let len = {
        let room = Room::new(project);
        room.wall_index(wall)
            .and_then(|i| room.walls.get(i))
            .map(|w| w.length)
            .ok_or_else(|| tr!("room.corner.no_wall", wall = wall))?
    };
    let (ca, cb) = match (project.cabinets.get(a), project.cabinets.get(b)) {
        (Some(x), Some(y)) => (x.clone(), y.clone()),
        _ => return Err(tr!("room.corner.no_cabinet")),
    };
    let ext = if extend_next { &cb } else { &ca };
    if ext.lock_width {
        return Err(tr!("room.corner.locked_width", id = &ext.id));
    }
    // slepý roh: predĺžená časť je za slepým panelom; zásuvka vedľa panela by nemala
    // na čo namontovať výsuv (chýba medzistena)
    let has_drawer = ext.fronts.iter().any(|r| {
        r.kind == cc_model::FrontKind::Drawer
            || r.columns
                .iter()
                .any(|c| c.kind == cc_model::FrontKind::Drawer)
    });
    if has_drawer {
        return Err(tr!("room.corner.has_drawers", id = &ext.id));
    }
    if extend_next {
        // ďalšia stena: od začiatku steny, prvá skrinka steny `wall` končí pred jej hĺbkou
        let new_w = cb.w + cb.at;
        let limit = len - (cb.d + cb.outset);
        if let Some(c) = project.cabinets.get_mut(b) {
            add_blind_panel(c, cb.at, true);
            c.w = new_w;
            c.at = 0.0;
        }
        if ca.at + ca.w > limit + 1e-6 {
            if let Some(c) = project.cabinets.get_mut(a) {
                c.at = limit - c.w;
            }
        }
    } else {
        // stena `wall`: po jej koniec, skrinka na ďalšej stene začína za jej hĺbkou
        let new_w = len - ca.at;
        let start = ca.d + ca.outset;
        if let Some(c) = project.cabinets.get_mut(a) {
            add_blind_panel(c, new_w - ca.w, false);
            c.w = new_w;
        }
        if cb.at < start - 1e-6 {
            if let Some(c) = project.cabinets.get_mut(b) {
                c.at = start;
            }
        }
    }
    Ok(ext.id.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cc_model::{CabinetSpec, Defaults, OpeningSpec, ProjectInfo, WallSpec};

    fn cab(id: &str, t: CabinetType, wall: &str, at: f64, w: f64) -> CabinetSpec {
        let (h, d) = match t {
            CabinetType::Base => (720.0, 560.0),
            CabinetType::Wall => (720.0, 330.0),
            CabinetType::Tall => (2100.0, 560.0),
        };
        CabinetSpec {
            id: id.into(),
            cabinet_type: t,
            w,
            h,
            d,
            adjustable_shelves: 0,
            fixed_shelves: vec![],
            wall: Some(wall.into()),
            at,
            elevation: None,
            outset: 0.0,
            fronts: Vec::new(),
            reveals: Default::default(),
            front_mount: None,
            hinge: None,
            drawer: None,
            drawer_length: None,
            top_style: None,
            stretcher_drop: None,
            back: true,
            carcass: true,
            lock_width: false,
            comment: None,
            placement: None,
            catalog: None,
            cv_id: None,
            cv_fronts: None,
            filler: None,
            slant: None,
            niche: false,
            led_rail: None,
            options: Default::default(),
        }
    }

    /// Miestnosť 3000 × 2000 kreslená v smere hodinových ručičiek (miestnosť napravo).
    fn project() -> ProjectFile {
        let wall = |id: &str, x: f64, y: f64, length: f64, angle: f64| WallSpec {
            id: id.into(),
            x,
            y,
            length,
            angle,
            height: None,
            thickness: None,
        };
        ProjectFile {
            project: ProjectInfo::named("t"),
            defaults: Defaults {
                panel_material: "m".into(),
                back_material: "b".into(),
                construction: "c".into(),
            },
            cabinets: vec![
                cab("SP1", CabinetType::Base, "W1", 0.0, 600.0),
                cab("SP2", CabinetType::Base, "W1", 600.0, 800.0),
                cab("HO1", CabinetType::Wall, "W1", 0.0, 600.0),
            ],
            room: Some(RoomSpec::default()),
            walls: vec![
                wall("W1", 0.0, 2000.0, 3000.0, 0.0),
                wall("W2", 3000.0, 2000.0, 2000.0, -90.0),
                wall("W3", 3000.0, 0.0, 3000.0, 180.0),
                wall("W4", 0.0, 0.0, 2000.0, 90.0),
            ],
            openings: vec![],
        }
    }

    /// Miestnosť nakreslená proti smeru hodín sa otočí: skrinky ostanú na mieste, ale vnútri.
    #[test]
    fn counter_clockwise_room_is_turned_inside() {
        let mut p = project();
        assert!(reversed_loops(&p).is_empty());
        // tá istá miestnosť proti smeru hodín
        p.walls = vec![
            WallSpec {
                id: "W1".into(),
                x: 3000.0,
                y: 2000.0,
                length: 3000.0,
                angle: 180.0,
                height: None,
                thickness: None,
            },
            WallSpec {
                id: "W4".into(),
                x: 0.0,
                y: 2000.0,
                length: 2000.0,
                angle: -90.0,
                height: None,
                thickness: None,
            },
            WallSpec {
                id: "W3".into(),
                x: 0.0,
                y: 0.0,
                length: 3000.0,
                angle: 0.0,
                height: None,
                thickness: None,
            },
            WallSpec {
                id: "W2".into(),
                x: 3000.0,
                y: 0.0,
                length: 2000.0,
                angle: 90.0,
                height: None,
                thickness: None,
            },
        ];
        assert_eq!(reversed_loops(&p).len(), 1);
        let turned = orient_rooms(&mut p);
        assert_eq!(turned.len(), 4);
        assert!(reversed_loops(&p).is_empty());
        let w1 = p.walls.iter().find(|w| w.id == "W1").unwrap();
        assert!((w1.x - 0.0).abs() < 1e-6 && (w1.y - 2000.0).abs() < 1e-6 && w1.angle.abs() < 1e-9);
        // SP1 bol na 0…600 od pravého konca → teraz 2400…3000 od ľavého
        let sp1 = p.cabinets.iter().find(|c| c.id == "SP1").unwrap();
        assert!((sp1.at - 2400.0).abs() < 1e-6);
        let room = Room::new(&p);
        assert!(room.walls[0].inward.y < 0.0, "vnútro pod W1");
    }

    #[test]
    fn walls_chain_and_face_into_room() {
        let room = Room::new(&project());
        assert_eq!(room.walls[0].next, Some(1));
        assert_eq!(room.walls[0].prev, Some(3));
        // W1 ide po +x hore, miestnosť je pod ňou (−y)
        assert!((room.walls[0].inward - DVec2::NEG_Y).length() < 1e-9);
        // vonkajší roh W1/W2 na pokos: (3100, 2100)
        let o = room.wall_outline(0);
        assert!((o[2] - DVec2::new(3100.0, 2100.0)).length() < 1e-6, "{o:?}");
        assert!((o[3] - DVec2::new(-100.0, 2100.0)).length() < 1e-6, "{o:?}");
    }

    #[test]
    fn cabinet_placement_back_on_wall_front_into_room() {
        let p = project();
        let room = Room::new(&p);
        let t = room.placement(&p.cabinets[0]).unwrap();
        assert!(t.is_right_handed_orthonormal());
        // predný ľavý spodný roh: x 0, y 2000 − 560, z = sokel 100
        let o = t.point_to_cabinet(DVec3::ZERO);
        assert!(
            (o - DVec3::new(0.0, 1440.0, 100.0)).length() < 1e-9,
            "{o:?}"
        );
        // zadná strana na stene
        let back = t.point_to_cabinet(DVec3::new(0.0, 560.0, 0.0));
        assert!((back.y - 2000.0).abs() < 1e-9);
        // horná skrinka: horná hrana pod stropom 2600
        let up = room.placement(&p.cabinets[2]).unwrap();
        assert!((up.point_to_cabinet(DVec3::new(0.0, 0.0, 720.0)).z - 2600.0).abs() < 1e-9);
    }

    #[test]
    fn checks_overlap_outside_opening() {
        let mut p = project();
        assert!(check(&p).is_empty(), "{:?}", check(&p));
        p.cabinets[1].at = 500.0;
        p.cabinets
            .push(cab("SP3", CabinetType::Base, "W1", 2800.0, 600.0));
        // roh: skrinka na W2 hneď pri rohu narazí do SP3 (hĺbka 560)
        p.cabinets
            .push(cab("SP4", CabinetType::Base, "W2", 0.0, 600.0));
        p.openings.push(OpeningSpec {
            wall: "W1".into(),
            kind: OpeningKind::Window,
            at: 300.0,
            width: 900.0,
            height: 1200.0,
            elevation: 1000.0,
        });
        let codes: Vec<_> = check(&p).into_iter().map(|i| (i.code, i.message)).collect();
        let has = |c: IssueCode, s: &str| codes.iter().any(|(k, m)| *k == c && m.contains(s));
        assert!(
            has(IssueCode::PlacementOverlap, "`SP1` a `SP2`"),
            "{codes:?}"
        );
        assert!(has(IssueCode::PlacementOutsideWall, "SP3"), "{codes:?}");
        assert!(
            has(IssueCode::PlacementOverlap, "`SP3` a `SP4`"),
            "{codes:?}"
        );
        assert!(has(IssueCode::PlacementOpening, "HO1"), "{codes:?}");
        assert!(!has(IssueCode::PlacementOpening, "SP1"), "{codes:?}");
    }

    #[test]
    fn fill_rules() {
        let p = FillParams::default();
        assert_eq!(fill_widths(100.0, &p), (vec![], 0.0, 0.0));
        assert_eq!(fill_widths(830.0, &p), (vec![800.0], 15.0, 15.0));
        // 1610: 2 × 800 + výplň 10
        assert_eq!(fill_widths(1610.0, &p), (vec![800.0, 800.0], 5.0, 5.0));
        // 2000: n = 3 (2000/600), 650 + 50 navyše do stredu
        assert_eq!(
            fill_widths(2000.0, &p),
            (vec![650.0, 700.0, 650.0], 0.0, 0.0)
        );
        let (w, l, r) = fill_widths(3330.0, &p);
        assert!((w.iter().sum::<f64>() + l + r - 3330.0).abs() < 1e-9);
        assert!(w.iter().all(|&x| x <= 900.0 && x % 50.0 == 0.0));
    }

    #[test]
    fn gaps_respect_cabinets_corners_and_doors() {
        let mut p = project();
        p.cabinets
            .push(cab("SP5", CabinetType::Base, "W2", 0.0, 600.0));
        p.openings.push(OpeningSpec {
            wall: "W1".into(),
            kind: OpeningKind::Door,
            at: 2000.0,
            width: 800.0,
            height: 2000.0,
            elevation: 0.0,
        });
        // W1: 0–1400 obsadené, dvere 2000–2800, vpravo roh so SP5 (hĺbka 560) → 2440
        assert_eq!(
            free_gaps(&p, "W1", Row::Base, 150.0),
            vec![(1400.0, 2000.0)]
        );
        // horný rad: HO1 0–600, dvere sú vysoké → blokujú aj horný rad
        // horný rad: HO1 0–600, dvere sú vysoké → blokujú aj horný rad; v rohu so SP5
        // (spodná) horný rad pokračuje až po koniec steny
        assert_eq!(
            free_gaps(&p, "W1", Row::Upper, 150.0),
            vec![(600.0, 2000.0), (2800.0, 3000.0)]
        );
        assert_eq!(
            dimension_points(&p, "W1", Row::Base),
            vec![0.0, 600.0, 1400.0, 2000.0, 2800.0, 3000.0]
        );
    }
}
