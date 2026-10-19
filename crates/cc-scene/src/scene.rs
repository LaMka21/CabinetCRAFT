//! Geometria scény: diely ako kvádre, vŕtanie ako kruhy, drážky ako obdĺžniky na ploche
//! dielu, steny miestnosti a podlaha.

use cc_model::{Face, Model, Operation, Part, ProjectFile, Role};
use cc_room::Room;
use glam::{DMat3, DVec2, DVec3};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SceneOptions {
    pub backs: bool,
    pub shelves: bool,
    /// dvierka a čelá zásuviek
    pub fronts: bool,
    pub operations: bool,
    /// steny miestnosti a podlaha
    pub walls: bool,
    /// 0 = zložené, 1 = diely od seba.
    pub explode: f64,
}

impl Default for SceneOptions {
    fn default() -> Self {
        SceneOptions {
            backs: true,
            shelves: true,
            fronts: true,
            operations: true,
            walls: true,
            explode: 0.0,
        }
    }
}

/// Trojuholník plochy.
#[derive(Debug, Clone, Copy)]
pub struct Tri {
    pub p: [DVec3; 3],
    /// vonkajšia normála
    pub n: DVec3,
    pub color: [f64; 3],
    /// index dielu + 1 (0 = nič)
    pub part: u32,
    /// false: farba bez tieňovania a mierne zvýhodnená v hĺbke (otvory, drážky)
    pub lit: bool,
    /// stena: normála do miestnosti; ak je kamera za stenou (mimo miestnosti), stena sa nekreslí
    pub cull: Option<DVec3>,
}

/// Hrana.
#[derive(Debug, Clone, Copy)]
pub struct Line {
    pub a: DVec3,
    pub b: DVec3,
    /// index dielu + 1 (0 = nič)
    pub part: u32,
    pub cull: Option<DVec3>,
}

/// Obálka dielu vo svete: rovnobežnosten `origin + u·axes[0] + v·axes[1] + w·axes[2]`,
/// `u, v, w ∈ [0, 1]` (dĺžka, šírka, hrúbka dielu).
#[derive(Debug, Clone, Copy)]
pub struct PartBox {
    pub origin: DVec3,
    pub axes: [DVec3; 3],
}

impl PartBox {
    /// Vzdialenosť `t` (v jednotkách `dir`) k prvému priesečníku lúča s obálkou.
    pub fn hit(&self, origin: DVec3, dir: DVec3) -> Option<f64> {
        let m = DMat3::from_cols(self.axes[0], self.axes[1], self.axes[2]);
        if m.determinant().abs() < 1e-9 {
            return None;
        }
        let inv = m.inverse();
        let o = inv * (origin - self.origin);
        let d = inv * dir;
        let (mut t0, mut t1) = (f64::NEG_INFINITY, f64::INFINITY);
        for k in 0..3 {
            let (ok, dk) = (o[k], d[k]);
            if dk.abs() < 1e-12 {
                if !(0.0..=1.0).contains(&ok) {
                    return None;
                }
                continue;
            }
            let (a, b) = ((0.0 - ok) / dk, (1.0 - ok) / dk);
            t0 = t0.max(a.min(b));
            t1 = t1.min(a.max(b));
        }
        if t0 > t1 || t1 < 0.0 {
            return None;
        }
        Some(t0.max(0.0))
    }

    /// Rohy (index: bit 0 = dĺžka, bit 1 = šírka, bit 2 = hrúbka).
    pub fn corners(&self) -> [DVec3; 8] {
        std::array::from_fn(|i| {
            let mut p = self.origin;
            for (k, axis) in self.axes.iter().enumerate() {
                if i & (1 << k) != 0 {
                    p += *axis;
                }
            }
            p
        })
    }
}

/// Čo je vybrané (zvýraznenie v CPU rastri aj na GPU).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Selection {
    /// index dielu + 1: plochy oranžovo
    pub part: Option<u32>,
    /// index v `Scene::cabinet_boxes`: drôtený obrys skrinky
    pub cabinet: Option<usize>,
}

/// Geometria celého projektu vo svetových súradniciach (mm, z nahor, predok y = 0).
#[derive(Debug, Clone, Default)]
pub struct Scene {
    pub(crate) tris: Vec<Tri>,
    pub(crate) lines: Vec<Line>,
    /// index → ID dielu
    pub parts: Vec<String>,
    /// index → ID skrinky dielu
    pub part_cabinets: Vec<String>,
    /// index → obálka dielu
    pub boxes: Vec<PartBox>,
    /// skrinky: ID a kváder `w × d × h` v polohe miestnosti
    pub cabinet_boxes: Vec<(String, PartBox)>,
    /// popisy skriniek: bod nad prednou hranou a ID
    pub labels: Vec<(DVec3, String)>,
    pub(crate) min: DVec3,
    pub(crate) max: DVec3,
}

pub fn role_color(role: Role) -> [f64; 3] {
    match role {
        Role::Back | Role::FinishedBack => [0.80, 0.72, 0.60],
        Role::AdjustableShelf | Role::FixedShelf => [0.86, 0.89, 0.93],
        Role::StretcherFront | Role::StretcherRear => [0.90, 0.89, 0.86],
        Role::Door | Role::DrawerFront | Role::FalseFront => [0.97, 0.96, 0.93],
        Role::DrawerBottom | Role::DrawerBack => [0.88, 0.86, 0.82],
        _ => [0.94, 0.93, 0.90],
    }
}

/// Vonkajšia normála plochy v súradniciach dielu (plocha F je hore, nástroj ide v −z).
fn face_normal(face: Face) -> DVec3 {
    match face {
        Face::F => DVec3::Z,
        Face::B => DVec3::NEG_Z,
        Face::X0 => DVec3::NEG_X,
        Face::X1 => DVec3::X,
        Face::Y0 => DVec3::NEG_Y,
        Face::Y1 => DVec3::Y,
    }
}

const BOX_EDGES: [(usize, usize); 12] = [
    (0, 1),
    (2, 3),
    (4, 5),
    (6, 7),
    (0, 2),
    (1, 3),
    (4, 6),
    (5, 7),
    (0, 4),
    (1, 5),
    (2, 6),
    (3, 7),
];

impl Scene {
    /// Skrinky v polohe podľa miestnosti (`cc_room::layout`), steny s otvormi a podlaha.
    pub fn build(model: &Model, project: &ProjectFile, opts: &SceneOptions) -> Scene {
        let mut scene = Scene {
            min: DVec3::splat(f64::INFINITY),
            max: DVec3::splat(f64::NEG_INFINITY),
            ..Scene::default()
        };
        let layout = cc_room::layout(project);
        for cabinet in &model.cabinets {
            let Some(place) = project
                .cabinets
                .iter()
                .position(|c| c.id == cabinet.id)
                .and_then(|i| layout.get(i))
                .copied()
            else {
                continue;
            };
            let center = DVec3::new(cabinet.w, cabinet.d, cabinet.h) / 2.0;
            let origin = place.point_to_cabinet(DVec3::ZERO);
            scene.cabinet_boxes.push((
                cabinet.id.clone(),
                PartBox {
                    origin,
                    axes: [
                        place.point_to_cabinet(DVec3::new(cabinet.w, 0.0, 0.0)) - origin,
                        place.point_to_cabinet(DVec3::new(0.0, cabinet.d, 0.0)) - origin,
                        place.point_to_cabinet(DVec3::new(0.0, 0.0, cabinet.h)) - origin,
                    ],
                },
            ));
            for part in &cabinet.parts {
                let hidden = match part.role {
                    Role::Back | Role::FinishedBack => !opts.backs,
                    Role::AdjustableShelf | Role::FixedShelf => !opts.shelves,
                    Role::Door | Role::DrawerFront | Role::FalseFront => !opts.fronts,
                    _ => false,
                };
                if hidden {
                    continue;
                }
                let t = &part.transform;
                let part_center =
                    t.point_to_cabinet(DVec3::new(part.length, part.width, part.thickness) / 2.0);
                let shift = (part_center - center) * 0.55 * opts.explode;
                let id = scene.parts.len() as u32 + 1;
                scene.parts.push(part.id.clone());
                scene.part_cabinets.push(cabinet.id.clone());
                let to_world = |p: DVec3| place.point_to_cabinet(t.point_to_cabinet(p) + shift);
                let dir = |v: DVec3| place.dir_to_cabinet(t.dir_to_cabinet(v));
                scene.add_box(part, id, &to_world, &dir);
                if opts.operations {
                    scene.add_operations(part, id, &to_world, &dir);
                }
            }
            scene.labels.push((
                place.point_to_cabinet(DVec3::new(cabinet.w / 2.0, 0.0, cabinet.h + 40.0)),
                cabinet.id.clone(),
            ));
        }
        if opts.walls {
            scene.add_room(project);
        }
        if !scene.min.is_finite() || !scene.max.is_finite() {
            scene.min = DVec3::ZERO;
            scene.max = DVec3::splat(100.0);
        }
        scene
    }

    pub fn tris(&self) -> &[Tri] {
        &self.tris
    }

    pub fn lines(&self) -> &[Line] {
        &self.lines
    }

    /// Obálka celej scény (min, max).
    pub fn bounds(&self) -> (DVec3, DVec3) {
        (self.min, self.max)
    }

    /// Index dielu + 1 podľa ID.
    pub fn part_index(&self, id: &str) -> Option<u32> {
        self.parts
            .iter()
            .position(|p| p == id)
            .map(|i| i as u32 + 1)
    }

    /// Obálka vybraných dielov (podľa indexu + 1) vo svete.
    pub fn bounds_of(&self, mut select: impl FnMut(u32) -> bool) -> Option<(DVec3, DVec3)> {
        let mut lo = DVec3::splat(f64::INFINITY);
        let mut hi = DVec3::splat(f64::NEG_INFINITY);
        for (i, b) in self.boxes.iter().enumerate() {
            if !select(i as u32 + 1) {
                continue;
            }
            for c in b.corners() {
                lo = lo.min(c);
                hi = hi.max(c);
            }
        }
        (lo.is_finite() && hi.is_finite()).then_some((lo, hi))
    }

    /// Obálka skrinky (kváder skrinky vo svete).
    pub fn cabinet_bounds(&self, cabinet: &str) -> Option<(DVec3, DVec3)> {
        let (_, b) = self.cabinet_boxes.iter().find(|(id, _)| id == cabinet)?;
        let c = b.corners();
        Some(
            c.iter()
                .fold((c[0], c[0]), |(lo, hi), p| (lo.min(*p), hi.max(*p))),
        )
    }

    /// Výber podľa ID dielu a skrinky.
    pub fn selection(&self, part: Option<&str>, cabinet: Option<&str>) -> Selection {
        Selection {
            part: part.and_then(|p| self.part_index(p)),
            cabinet: cabinet.and_then(|c| self.cabinet_boxes.iter().position(|(id, _)| id == c)),
        }
    }

    /// ID skrinky dielu (index + 1).
    pub fn cabinet_of(&self, part: u32) -> Option<&str> {
        self.part_cabinets
            .get((part as usize).checked_sub(1)?)
            .map(String::as_str)
    }

    /// Hrany drôteného obrysu skrinky (index v `cabinet_boxes`).
    pub fn cabinet_outline(&self, index: usize) -> Vec<[DVec3; 2]> {
        let Some((_, b)) = self.cabinet_boxes.get(index) else {
            return Vec::new();
        };
        let c = b.corners();
        BOX_EDGES.iter().map(|&(a, b)| [c[a], c[b]]).collect()
    }

    /// Najbližší diel na lúči (index + 1). Steny a podlaha sa nevyberajú.
    pub fn pick(&self, origin: DVec3, dir: DVec3) -> Option<u32> {
        let mut best: Option<(f64, u32)> = None;
        for (i, b) in self.boxes.iter().enumerate() {
            if let Some(t) = b.hit(origin, dir) {
                if best.is_none_or(|(bt, _)| t < bt) {
                    best = Some((t, i as u32 + 1));
                }
            }
        }
        best.map(|(_, id)| id)
    }

    /// Steny (kvádre po úsekoch medzi otvormi) a podlaha.
    fn add_room(&mut self, project: &ProjectFile) {
        let room = Room::new(project);
        if room.walls.is_empty() {
            return;
        }
        const WALL: [f64; 3] = [0.91, 0.90, 0.88];
        for (wi, w) in room.walls.iter().enumerate() {
            let inward = w.inward.extend(0.0);
            let dir = w.dir.extend(0.0);
            // obrys steny ako v pôdoryse: v rohoch zrezaný na pokos so susednou stenou
            let outline = room.wall_outline(wi);
            let (back_start, back_end) = (outline[3], outline[2]);
            let (a0, a1) = (0.0, w.length);
            // bod vonkajšej plochy v polohe `along` (konce podľa pokosu)
            let outer = |along: f64| -> DVec3 {
                if along <= a0 + 1e-6 {
                    back_start.extend(0.0)
                } else if along >= a1 - 1e-6 {
                    back_end.extend(0.0)
                } else {
                    (w.start - w.inward * w.thickness + w.dir * along).extend(0.0)
                }
            };
            let openings: Vec<_> = project.openings.iter().filter(|o| o.wall == w.id).collect();
            let mut cuts = vec![a0, a1];
            for o in &openings {
                cuts.extend([o.at.clamp(a0, a1), (o.at + o.width).clamp(a0, a1)]);
            }
            cuts.sort_by(f64::total_cmp);
            cuts.dedup_by(|x, y| (*x - *y).abs() < 0.01);
            for seg in cuts.windows(2) {
                let (s0, s1) = (seg[0], seg[1]);
                let mid = (s0 + s1) / 2.0;
                let mut spans = vec![(0.0, w.height)];
                for o in openings
                    .iter()
                    .filter(|o| o.at < mid && mid < o.at + o.width)
                {
                    spans = spans
                        .into_iter()
                        .flat_map(|(z0, z1): (f64, f64)| {
                            let lo = (z0, o.elevation.min(z1));
                            let hi = ((o.elevation + o.height).max(z0), z1);
                            [lo, hi].into_iter().filter(|(a, b)| b - a > 0.5)
                        })
                        .collect();
                }
                let base = w.start.extend(0.0);
                for (z0, z1) in spans {
                    let corner = |along: f64, across: f64, z: f64| {
                        let p = if across > 0.5 {
                            outer(along)
                        } else {
                            base + dir * along
                        };
                        p + DVec3::Z * z
                    };
                    let c: Vec<DVec3> = (0..8)
                        .map(|i| {
                            corner(
                                if i & 1 != 0 { s1 } else { s0 },
                                if i & 2 != 0 { 1.0 } else { 0.0 },
                                if i & 4 != 0 { z1 } else { z0 },
                            )
                        })
                        .collect();
                    self.add_solid(&c, WALL, Some(inward));
                }
            }
        }
        // podlaha v tvare miestnosti (uzavretá reťaz stien), inak obdĺžnik pod stenami
        let floor: Vec<[DVec3; 3]> = match room.floor_polygon() {
            Some(poly) => triangulate(&poly)
                .into_iter()
                .map(|t| {
                    let [a, b, c] = t.map(|i| poly[i]);
                    // trojuholník proti smeru hodín zhora (normála +Z)
                    let [a, b, c] = if (b - a).perp_dot(c - a) < 0.0 {
                        [a, c, b]
                    } else {
                        [a, b, c]
                    };
                    [a.extend(-1.0), b.extend(-1.0), c.extend(-1.0)]
                })
                .collect(),
            None => match room.bounds() {
                Some((lo, hi)) => {
                    let q = [
                        DVec3::new(lo.x, lo.y, -1.0),
                        DVec3::new(hi.x, lo.y, -1.0),
                        DVec3::new(hi.x, hi.y, -1.0),
                        DVec3::new(lo.x, hi.y, -1.0),
                    ];
                    vec![[q[0], q[1], q[2]], [q[0], q[2], q[3]]]
                }
                None => Vec::new(),
            },
        };
        {
            for tri in floor.iter().copied() {
                self.tris.push(Tri {
                    p: tri,
                    n: DVec3::Z,
                    color: [0.82, 0.78, 0.72],
                    part: 0,
                    lit: true,
                    cull: None,
                });
            }
            for p in floor.iter().flatten() {
                self.min = self.min.min(*p);
                self.max = self.max.max(*p);
            }
        }
    }

    /// Kváder z 8 rohov (index: bit 0 = x, bit 1 = y, bit 2 = z) s plochami podľa skutočných normál.
    fn add_solid(&mut self, c: &[DVec3], color: [f64; 3], cull: Option<DVec3>) {
        if c.len() != 8 {
            return;
        }
        let center = c.iter().fold(DVec3::ZERO, |a, p| a + *p) / 8.0;
        for p in c {
            self.min = self.min.min(*p);
            self.max = self.max.max(*p);
        }
        for q in [
            [0, 1, 3, 2],
            [4, 6, 7, 5],
            [0, 4, 5, 1],
            [2, 3, 7, 6],
            [0, 2, 6, 4],
            [1, 5, 7, 3],
        ] {
            let pts = q.map(|i| c[i]);
            let mut n = (pts[1] - pts[0]).cross(pts[2] - pts[0]).normalize_or_zero();
            let face_center = (pts[0] + pts[1] + pts[2] + pts[3]) / 4.0;
            if n.dot(face_center - center) < 0.0 {
                n = -n;
            }
            for tri in [[pts[0], pts[1], pts[2]], [pts[0], pts[2], pts[3]]] {
                self.tris.push(Tri {
                    p: tri,
                    n,
                    color,
                    part: 0,
                    lit: true,
                    cull,
                });
            }
        }
        for (a, b) in BOX_EDGES {
            self.lines.push(Line {
                a: c[a],
                b: c[b],
                part: 0,
                cull,
            });
        }
    }

    fn add_box(
        &mut self,
        part: &Part,
        id: u32,
        to_world: &dyn Fn(DVec3) -> DVec3,
        dir: &dyn Fn(DVec3) -> DVec3,
    ) {
        let (l, w, t) = (part.length, part.width, part.thickness);
        let c = |i: usize| {
            to_world(DVec3::new(
                if i & 1 != 0 { l } else { 0.0 },
                if i & 2 != 0 { w } else { 0.0 },
                if i & 4 != 0 { t } else { 0.0 },
            ))
        };
        let corners: Vec<DVec3> = (0..8).map(c).collect();
        for p in &corners {
            self.min = self.min.min(*p);
            self.max = self.max.max(*p);
        }
        self.boxes.push(PartBox {
            origin: corners[0],
            axes: [
                corners[1] - corners[0],
                corners[2] - corners[0],
                corners[4] - corners[0],
            ],
        });
        let color = role_color(part.role);
        // plochy: (rohy v poradí okolo, normála v súradniciach dielu)
        let faces: [([usize; 4], DVec3); 6] = [
            ([0, 1, 3, 2], DVec3::NEG_Z),
            ([4, 6, 7, 5], DVec3::Z),
            ([0, 4, 5, 1], DVec3::NEG_Y),
            ([2, 3, 7, 6], DVec3::Y),
            ([0, 2, 6, 4], DVec3::NEG_X),
            ([1, 5, 7, 3], DVec3::X),
        ];
        for (q, n) in faces {
            let n = dir(n);
            for tri in [[q[0], q[1], q[2]], [q[0], q[2], q[3]]] {
                self.tris.push(Tri {
                    p: tri.map(|i| corners[i]),
                    n,
                    color,
                    part: id,
                    lit: true,
                    cull: None,
                });
            }
        }
        for (a, b) in BOX_EDGES {
            self.lines.push(Line {
                a: corners[a],
                b: corners[b],
                part: id,
                cull: None,
            });
        }
    }

    fn add_operations(
        &mut self,
        part: &Part,
        id: u32,
        to_world: &dyn Fn(DVec3) -> DVec3,
        dir: &dyn Fn(DVec3) -> DVec3,
    ) {
        const LIFT: f64 = 0.4;
        for op in &part.operations {
            match *op {
                Operation::Hole {
                    face,
                    x,
                    y,
                    z,
                    diameter,
                    dowel,
                    ..
                } => {
                    let n = face_normal(face);
                    let (u, v) = perpendicular(n);
                    let center = DVec3::new(x, y, z) + n * LIFT;
                    let r = diameter / 2.0;
                    let color = if dowel {
                        [0.22, 0.22, 0.25]
                    } else {
                        [0.20, 0.35, 0.70]
                    };
                    const SEG: usize = 12;
                    let ring: Vec<DVec3> = (0..SEG)
                        .map(|i| {
                            let a = i as f64 / SEG as f64 * std::f64::consts::TAU;
                            to_world(center + (u * a.cos() + v * a.sin()) * r)
                        })
                        .collect();
                    let c = to_world(center);
                    let nw = dir(n);
                    for i in 0..SEG {
                        self.tris.push(Tri {
                            p: [c, ring[i], ring[(i + 1) % SEG]],
                            n: nw,
                            color,
                            part: id,
                            lit: false,
                            cull: None,
                        });
                    }
                }
                Operation::Groove {
                    face,
                    start,
                    end,
                    width,
                    ..
                } => {
                    let z = if face == Face::B {
                        -LIFT
                    } else {
                        part.thickness + LIFT
                    };
                    let d = DVec3::new(end.x - start.x, end.y - start.y, 0.0).normalize_or_zero();
                    let side = DVec3::new(-d.y, d.x, 0.0) * (width / 2.0);
                    let clamp = |p: DVec3| {
                        DVec3::new(p.x.clamp(0.0, part.length), p.y.clamp(0.0, part.width), z)
                    };
                    let s = DVec3::new(start.x, start.y, z);
                    let e = DVec3::new(end.x, end.y, z);
                    let q = [s - side, e - side, e + side, s + side].map(|p| to_world(clamp(p)));
                    let nw = dir(face_normal(face));
                    for tri in [[q[0], q[1], q[2]], [q[0], q[2], q[3]]] {
                        self.tris.push(Tri {
                            p: tri,
                            n: nw,
                            color: [0.45, 0.33, 0.22],
                            part: id,
                            lit: false,
                            cull: None,
                        });
                    }
                }
            }
        }
    }
}

fn perpendicular(n: DVec3) -> (DVec3, DVec3) {
    let helper = if n.z.abs() < 0.9 { DVec3::Z } else { DVec3::X };
    let u = n.cross(helper).normalize_or_zero();
    (u, n.cross(u))
}

/// Trojuholníky jednoduchého mnohouholníka (orezávanie uší); poradie bodov ľubovoľné.
/// Pri neúspechu (samopretínanie) vráti vejár z prvého bodu.
fn triangulate(poly: &[DVec2]) -> Vec<[usize; 3]> {
    let n = poly.len();
    if n < 3 {
        return Vec::new();
    }
    let area: f64 = (0..n)
        .map(|i| poly[i].perp_dot(poly[(i + 1) % n]))
        .sum::<f64>()
        / 2.0;
    let ccw = area > 0.0;
    let mut idx: Vec<usize> = (0..n).collect();
    let mut out = Vec::new();
    let convex = |a: DVec2, b: DVec2, c: DVec2| {
        let cr = (b - a).perp_dot(c - b);
        if ccw {
            cr > 1e-9
        } else {
            cr < -1e-9
        }
    };
    let inside = |p: DVec2, a: DVec2, b: DVec2, c: DVec2| {
        let d1 = (b - a).perp_dot(p - a);
        let d2 = (c - b).perp_dot(p - b);
        let d3 = (a - c).perp_dot(p - c);
        let neg = d1 < -1e-9 || d2 < -1e-9 || d3 < -1e-9;
        let pos = d1 > 1e-9 || d2 > 1e-9 || d3 > 1e-9;
        !(neg && pos)
    };
    let mut guard = 0;
    while idx.len() > 3 && guard < 10 * n {
        guard += 1;
        let m = idx.len();
        let mut cut = None;
        for k in 0..m {
            let (ia, ib, ic) = (idx[(k + m - 1) % m], idx[k], idx[(k + 1) % m]);
            let (a, b, c) = (poly[ia], poly[ib], poly[ic]);
            if !convex(a, b, c) {
                continue;
            }
            let ear = idx
                .iter()
                .filter(|&&j| j != ia && j != ib && j != ic)
                .all(|&j| !inside(poly[j], a, b, c));
            if ear {
                out.push([ia, ib, ic]);
                cut = Some(k);
                break;
            }
        }
        match cut {
            Some(k) => {
                idx.remove(k);
            }
            None => {
                // nejednoduchý obrys: vejár
                return (1..n - 1).map(|i| [0, i, i + 1]).collect();
            }
        }
    }
    if idx.len() == 3 {
        out.push([idx[0], idx[1], idx[2]]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Podlaha tvaru L: trojuholníky pokryjú presne plochu mnohouholníka (aj nekonvexného).
    #[test]
    fn floor_triangulation_covers_l_shape() {
        let l = [
            DVec2::new(0.0, 0.0),
            DVec2::new(2840.0, 0.0),
            DVec2::new(2840.0, 570.0),
            DVec2::new(3910.0, 570.0),
            DVec2::new(3960.0, 1290.0),
            DVec2::new(1930.0, 1610.0),
            DVec2::new(0.0, 1610.0),
        ];
        for poly in [l.to_vec(), l.iter().rev().copied().collect::<Vec<_>>()] {
            let area: f64 = (0..poly.len())
                .map(|i| poly[i].perp_dot(poly[(i + 1) % poly.len()]))
                .sum::<f64>()
                .abs()
                / 2.0;
            let tris = triangulate(&poly);
            assert_eq!(tris.len(), poly.len() - 2);
            let sum: f64 = tris
                .iter()
                .map(|t| {
                    (poly[t[1]] - poly[t[0]])
                        .perp_dot(poly[t[2]] - poly[t[0]])
                        .abs()
                        / 2.0
                })
                .sum();
            assert!((sum - area).abs() < 1e-6, "{sum} vs {area}");
        }
    }

    #[test]
    fn ray_hits_box_front_face() {
        let b = PartBox {
            origin: DVec3::new(0.0, 0.0, 0.0),
            axes: [
                DVec3::new(100.0, 0.0, 0.0),
                DVec3::new(0.0, 50.0, 0.0),
                DVec3::new(0.0, 0.0, 18.0),
            ],
        };
        let t = b
            .hit(DVec3::new(50.0, -100.0, 9.0), DVec3::new(0.0, 1.0, 0.0))
            .unwrap();
        assert!((t - 100.0).abs() < 1e-9);
        assert!(b
            .hit(DVec3::new(150.0, -100.0, 9.0), DVec3::new(0.0, 1.0, 0.0))
            .is_none());
        // lúč smerom od kvádra
        assert!(b
            .hit(DVec3::new(50.0, -100.0, 9.0), DVec3::new(0.0, -1.0, 0.0))
            .is_none());
        // plochý (zdegenerovaný) diel sa nevyberá
        let flat = PartBox {
            axes: [DVec3::X, DVec3::ZERO, DVec3::Z],
            ..b
        };
        assert!(flat.hit(DVec3::ZERO, DVec3::Y).is_none());
    }
}
