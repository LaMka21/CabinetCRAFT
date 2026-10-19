//! Sokel, bočný sokel a krycí bok (findings 11 §5.5, 12 §6, 10 KRYCI BOK).
//!
//! V CV má každá spodná položka vlastný sokel `97 × šírka`, dielňa ho však v zostave
//! nahrádza **jedným kusom** pod celou radou susedných skriniek (REF: 2680 na stene 1,
//! 1944 na stene 2). Tu sa preto sokel tvorí po zostavách:
//! - zostava = susedné spodné a vysoké skrinky pri tej istej stene (dotýkajú sa, rovnaká
//!   predná línia a výška osadenia), so zapnutým soklom; patrí do nej aj skrinka iba
//!   s čelom (umývačka), zostava však potrebuje aspoň jednu skrinku s korpusom,
//! - sokel patrí prvej skrinke zostavy s korpusom, ide od ľavého po pravý koniec zostavy,
//! - skrinka s ručne zadaným soklom (`plinth_span`, import z CV) má vlastný kus,
//! - bočný sokel je pod bokom skrinky na odkrytom konci (nie pri susednej skrinke,
//!   v rohu so skrinkami susednej steny ani pri krycom boku) a s predným soklom ho
//!   spájajú 2 kolíky (`966f0031`, `9660501N` riadky 39–41).
//!
//! Rám skrinky: `x` doprava, `y` od čela korpusu dozadu, `z` nahor od spodku korpusu
//! (podlaha je `z = −osadenie`). Plocha F je k skrinke (pri prednom sokle zadná plocha).

use cc_model::{
    CabinetModel, CabinetSpec, CabinetType, Edges, Face, Material, MaterialLibrary, Operation,
    Part, PlacementLevels, ProjectFile, Role, RoomSpec, Standard, Transform, WallSpec,
};
use glam::{DVec2, DVec3};

use crate::{material, GenerateError};

/// Tolerancia „skrinky sa dotýkajú“ a „rovnaká predná línia / výška“ (mm).
const TOUCH: f64 = 1.0;

/// Strana skrinky.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Left,
    Right,
}

/// Doplní sokle, bočné sokle a krycie boky do skriniek modelu (`cabinets` je v poradí
/// `project.cabinets`).
pub(crate) fn add(
    cabinets: &mut [CabinetModel],
    project: &ProjectFile,
    standard: &Standard,
    materials: &MaterialLibrary,
    panel: &Material,
) -> Result<(), GenerateError> {
    let room = project.room.clone().unwrap_or_default();
    let levels = PlacementLevels::of(project);
    let ctx = Ctx {
        project,
        standard,
        room: &room,
        levels,
    };
    let any_plinth = cabinets.iter().any(|c| c.options.plinth);
    if any_plinth {
        let mat = match &standard.plinth.material {
            Some(id) => material(materials, id)?,
            None => panel,
        };
        for run in ctx.runs(cabinets) {
            ctx.add_run(cabinets, &run, mat);
        }
    }
    let wants_cover = project.cabinets.iter().any(|s| {
        s.options.cover_side_left == Some(true) || s.options.cover_side_right == Some(true)
    });
    if wants_cover {
        let mat = match &standard.cover_side.material {
            Some(id) => material(materials, id)?,
            None => panel,
        };
        for (cab, spec) in cabinets.iter_mut().zip(&project.cabinets) {
            for (side, on) in [
                (Side::Left, spec.options.cover_side_left),
                (Side::Right, spec.options.cover_side_right),
            ] {
                if on == Some(true) {
                    ctx.add_cover_side(cab, spec, side, mat);
                }
            }
        }
    }
    Ok(())
}

/// Zostava skriniek pod jedným soklom (indexy do `project.cabinets` zľava doprava).
#[derive(Debug, Clone)]
struct Run {
    members: Vec<usize>,
    /// skrinka, ktorej patrí predný sokel
    owner: usize,
    /// ručne zadaný sokel (`plinth_span`) skrinky `owner`
    manual: bool,
}

struct Ctx<'a> {
    project: &'a ProjectFile,
    standard: &'a Standard,
    room: &'a RoomSpec,
    levels: PlacementLevels,
}

impl Ctx<'_> {
    fn spec(&self, i: usize) -> Option<&CabinetSpec> {
        self.project.cabinets.get(i)
    }

    fn elevation(&self, s: &CabinetSpec) -> f64 {
        s.elevation_in(self.room, &self.levels)
    }

    /// Výška predného sokla skrinky stojacej na podlahe; `None` = sokel nemá.
    fn plinth_height(&self, s: &CabinetSpec) -> Option<f64> {
        let ps = &self.standard.plinth;
        let h = match s.options.plinth_span.and_then(|p| p.height) {
            Some(h) => h,
            None => self.elevation(s) - ps.gap_top,
        };
        (h.is_finite() && h >= ps.min_height).then_some(h)
    }

    /// Skrinka môže mať sokel (spodná alebo vysoká so zapnutým soklom a výškou nad podlahou).
    fn eligible(&self, cab: &CabinetModel, s: &CabinetSpec) -> bool {
        cab.options.plinth
            && matches!(s.cabinet_type, CabinetType::Base | CabinetType::Tall)
            && [s.w, s.d, s.at, s.outset].iter().all(|v| v.is_finite())
            && s.w > 0.0
            && self.plinth_height(s).is_some()
    }

    /// Zostavy skriniek; skrinky bez steny a s ručným soklom sú samostatne.
    fn runs(&self, cabinets: &[CabinetModel]) -> Vec<Run> {
        let mut single = Vec::new();
        let mut placed: Vec<usize> = Vec::new();
        for (i, (cab, s)) in cabinets.iter().zip(&self.project.cabinets).enumerate() {
            if !self.eligible(cab, s) {
                continue;
            }
            if s.options.plinth_span.is_some() {
                single.push(Run {
                    members: vec![i],
                    owner: i,
                    manual: true,
                });
            } else if self.standard.plinth.runs && s.wall.is_some() {
                placed.push(i);
            } else if s.carcass {
                single.push(Run {
                    members: vec![i],
                    owner: i,
                    manual: false,
                });
            }
        }
        let key = |i: &usize| {
            self.spec(*i)
                .map(|s| (s.wall.clone().unwrap_or_default(), s.at))
                .unwrap_or_default()
        };
        placed.sort_by(|a, b| {
            let (ka, kb) = (key(a), key(b));
            ka.0.cmp(&kb.0).then(ka.1.total_cmp(&kb.1))
        });
        let max = self.standard.plinth.max_length;
        let mut groups: Vec<Vec<usize>> = Vec::new();
        for i in placed {
            let joins = groups
                .last()
                .and_then(|g| Some((g.first()?, g.last()?)))
                .and_then(|(&f, &p)| Some((self.spec(f)?, self.spec(p)?, self.spec(i)?)))
                .is_some_and(|(first, a, b)| {
                    // dlhší sokel sa delí na styku skriniek (tabuľa)
                    self.continues(a, b) && (max <= 0.0 || b.at + b.w - first.at <= max + TOUCH)
                });
            match groups.last_mut() {
                Some(g) if joins => g.push(i),
                _ => groups.push(vec![i]),
            }
        }
        let mut runs: Vec<Run> = groups
            .into_iter()
            .filter_map(|members| {
                let owner = *members
                    .iter()
                    .find(|&&i| self.spec(i).is_some_and(|s| s.carcass))?;
                Some(Run {
                    members,
                    owner,
                    manual: false,
                })
            })
            .collect();
        runs.extend(single);
        runs.sort_by_key(|r| r.owner);
        runs
    }

    /// `b` nadväzuje na `a` (zľava doprava) v jednej zostave.
    fn continues(&self, a: &CabinetSpec, b: &CabinetSpec) -> bool {
        a.wall == b.wall
            && (a.at + a.w - b.at).abs() <= TOUCH
            && ((a.outset + a.d) - (b.outset + b.d)).abs() <= TOUCH
            && (self.elevation(a) - self.elevation(b)).abs() <= TOUCH
    }

    fn add_run(&self, cabinets: &mut [CabinetModel], run: &Run, mat: &Material) {
        let ps = &self.standard.plinth;
        let (Some(owner), Some(first), Some(last)) = (
            self.spec(run.owner),
            run.members.first().and_then(|&i| self.spec(i)),
            run.members.last().and_then(|&i| self.spec(i)),
        ) else {
            return;
        };
        let Some(height) = self.plinth_height(owner) else {
            return;
        };
        let elevation = self.elevation(owner);
        let (x0, length) = match (run.manual, owner.options.plinth_span) {
            (true, Some(span)) => (span.from, span.length),
            _ if owner.wall.is_some() => (first.at - owner.at, last.at + last.w - first.at),
            _ => (0.0, owner.w),
        };
        if !(x0.is_finite() && length.is_finite() && length > 0.0) {
            return;
        }
        let t = mat.thickness;
        // x = +X (dĺžka), y = −Z (zhora nadol), z = +Y: F = zadná plocha (k skrinke)
        let mut front = part(
            format!("{}-{}", owner.id, Role::Plinth.id_code()),
            &owner.id,
            Role::Plinth,
            mat,
            length,
            height,
            Transform::from_axes(
                DVec3::X,
                DVec3::NEG_Z,
                DVec3::new(x0, ps.setback - t, height - elevation),
            ),
        );
        // bočné sokle na koncoch zostavy (pri ručnom sokle pod bokmi tej istej skrinky)
        let ends = [
            (Side::Left, run.members.first()),
            (Side::Right, run.members.last()),
        ];
        let mut sides = Vec::new();
        for (side, end) in ends {
            let Some(&end) = end else {
                continue;
            };
            let Some(es) = self.spec(end) else {
                continue;
            };
            let explicit = match side {
                Side::Left => es.options.side_plinth_left,
                Side::Right => es.options.side_plinth_right,
            };
            let wanted = explicit.unwrap_or_else(|| {
                ps.side && es.wall.is_some() && self.exposed(es, side, &run.members)
            });
            if !wanted {
                continue;
            }
            let side_h = es
                .options
                .side_plinth_height
                .filter(|h| h.is_finite() && *h >= ps.min_height)
                .unwrap_or(height);
            // kolíky iba pri zapnutých spojoch skrinky („Spoje skrinky: nie“ = bez vŕtania)
            let joints = cabinets.get(end).is_some_and(|c| c.options.joints);
            let Some(sp) = self.side_plinth(es, side, side_h, mat, joints) else {
                continue;
            };
            // stred hrúbky bočného sokla v ráme dielu predného sokla
            let shift = if es.wall.is_some() && owner.wall.is_some() {
                es.at - owner.at
            } else {
                0.0
            };
            let x_mid = match side {
                Side::Left => shift + t / 2.0,
                Side::Right => shift + es.w - t / 2.0,
            } - x0;
            let heights = if joints {
                dowel_heights(side_h, &ps.side_dowel)
            } else {
                Vec::new()
            };
            for z in heights {
                // oba sokle stoja na podlahe: výška od podlahy → y predného sokla (zhora nadol)
                let y = height - z;
                if x_mid > 0.0 && x_mid < length && y > 0.0 && y < height {
                    front.operations.push(Operation::Hole {
                        face: Face::F,
                        x: clean(x_mid),
                        y: clean(y),
                        z: t,
                        diameter: ps.side_dowel.diameter,
                        depth: ps.side_dowel.face_depth,
                        through: false,
                        dowel: true,
                    });
                }
            }
            sides.push((end, sp));
        }
        if let Some(c) = cabinets.get_mut(run.owner) {
            c.parts.push(front);
        }
        for (end, sp) in sides {
            if let Some(c) = cabinets.get_mut(end) {
                c.parts.push(sp);
            }
        }
    }

    /// Bočný sokel pod bokom skrinky: od zadnej plochy predného sokla dozadu, kolíky
    /// do čela pri prednom sokle (hrana `X0`).
    fn side_plinth(
        &self,
        s: &CabinetSpec,
        side: Side,
        height: f64,
        mat: &Material,
        joints: bool,
    ) -> Option<Part> {
        let ps = &self.standard.plinth;
        let length = s.d - ps.side_back_gap - ps.setback;
        if !(length.is_finite() && length > 0.0) {
            return None;
        }
        let elevation = self.elevation(s);
        // dĺžka dielu = hĺbka (minimum stroja počíta dĺžku × šírku), čelo pri prednom sokle = X0
        let (code, transform) = match side {
            // x = +Y (dozadu), y = +Z (nahor), z = +X (F = vnútorná plocha)
            Side::Left => (
                "L",
                Transform::from_axes(DVec3::Y, DVec3::Z, DVec3::new(0.0, ps.setback, -elevation)),
            ),
            // x = +Y, y = −Z (zhora nadol), z = −X
            Side::Right => (
                "R",
                Transform::from_axes(
                    DVec3::Y,
                    DVec3::NEG_Z,
                    DVec3::new(s.w, ps.setback, height - elevation),
                ),
            ),
        };
        let mut p = part(
            format!("{}-{}-{code}", s.id, Role::Plinth.id_code()),
            &s.id,
            Role::Plinth,
            mat,
            length,
            height,
            transform,
        );
        let heights = if joints {
            dowel_heights(height, &ps.side_dowel)
        } else {
            Vec::new()
        };
        for z in heights {
            let y = match side {
                Side::Left => z,
                Side::Right => height - z,
            };
            p.operations.push(Operation::Hole {
                face: Face::X0,
                x: 0.0,
                y: clean(y),
                z: clean(mat.thickness / 2.0),
                diameter: ps.side_dowel.diameter,
                depth: ps.side_dowel.edge_depth,
                through: false,
                dowel: true,
            });
        }
        Some(p)
    }

    /// Koniec zostavy pri skrinke `s` na strane `side` je odkrytý.
    fn exposed(&self, s: &CabinetSpec, side: Side, members: &[usize]) -> bool {
        let Some(wall_id) = s.wall.as_deref() else {
            return false;
        };
        let cover = match side {
            Side::Left => s.options.cover_side_left,
            Side::Right => s.options.cover_side_right,
        };
        if cover == Some(true) {
            return false;
        }
        let edge = match side {
            Side::Left => s.at,
            Side::Right => s.at + s.w,
        };
        let floor =
            |o: &CabinetSpec| matches!(o.cabinet_type, CabinetType::Base | CabinetType::Tall);
        let neighbour = self.project.cabinets.iter().enumerate().any(|(i, o)| {
            !members.contains(&i)
                && o.id != s.id
                && floor(o)
                && o.wall.as_deref() == Some(wall_id)
                && match side {
                    Side::Left => (o.at + o.w - edge).abs() <= TOUCH,
                    Side::Right => (o.at - edge).abs() <= TOUCH,
                }
        });
        if neighbour {
            return false;
        }
        let walls = &self.project.walls;
        let Some(wall) = walls.iter().find(|w| w.id == wall_id) else {
            return true;
        };
        // roh: susedná stena začína v našom konci (alebo končí v našom začiatku); koniec
        // zostavy siaha po skrinky jej radu v rohu (hĺbka od steny)
        let joined = match side {
            Side::Left => walls
                .iter()
                .find(|o| o.id != wall.id && near(end(o), start(wall))),
            Side::Right => walls
                .iter()
                .find(|o| o.id != wall.id && near(start(o), end(wall))),
        };
        let Some(other) = joined else {
            return true;
        };
        let depth = self
            .project
            .cabinets
            .iter()
            .filter(|o| floor(o) && o.wall.as_deref() == Some(other.id.as_str()))
            .filter(|o| match side {
                Side::Left => o.at + o.w > other.length - o.d - o.outset - TOUCH,
                Side::Right => o.at < o.d + o.outset + TOUCH,
            })
            .map(|o| o.d + o.outset)
            .fold(0.0, f64::max);
        match side {
            Side::Left => edge > depth + TOUCH,
            Side::Right => edge < wall.length - depth - TOUCH,
        }
    }

    /// Krycí bok pri boku skrinky: od podlahy po vrch korpusu + `top`, od `front` pred
    /// čelom po `back` za zadnou hranou.
    fn add_cover_side(&self, cab: &mut CabinetModel, s: &CabinetSpec, side: Side, mat: &Material) {
        let cs = &self.standard.cover_side;
        if !s.carcass || !matches!(s.cabinet_type, CabinetType::Base | CabinetType::Tall) {
            return;
        }
        let elevation = self.elevation(s);
        let height = elevation + s.h + cs.top;
        let depth = cs.front + s.d + cs.back;
        if !(height.is_finite() && depth.is_finite() && height > 0.0 && depth > 0.0) {
            return;
        }
        let t = mat.thickness;
        let (code, transform) = match side {
            // x = −Z, y = +Y, z = +X: F = plocha pri boku skrinky
            Side::Left => (
                "L",
                Transform::from_axes(
                    DVec3::NEG_Z,
                    DVec3::Y,
                    DVec3::new(-t, -cs.front, s.h + cs.top),
                ),
            ),
            // x = +Z, y = +Y, z = −X
            Side::Right => (
                "R",
                Transform::from_axes(
                    DVec3::Z,
                    DVec3::Y,
                    DVec3::new(s.w + t, -cs.front, -elevation),
                ),
            ),
        };
        cab.parts.push(part(
            format!("{}-{}-{code}", s.id, Role::CoverSide.id_code()),
            &s.id,
            Role::CoverSide,
            mat,
            height,
            depth,
            transform,
        ));
    }
}

/// Výšky kolíkov bočného sokla od jeho spodku.
fn dowel_heights(height: f64, d: &cc_model::PlinthDowel) -> Vec<f64> {
    let (a, b) = (d.from_edge, height - d.from_edge);
    if !(a.is_finite() && b.is_finite() && a > 0.0 && b > a) || d.diameter <= 0.0 {
        return Vec::new();
    }
    vec![a, b]
}

fn part(
    id: String,
    cabinet: &str,
    role: Role,
    mat: &Material,
    length: f64,
    width: f64,
    transform: Transform,
) -> Part {
    debug_assert!(transform.is_right_handed_orthonormal());
    Part {
        id,
        cabinet: cabinet.to_owned(),
        role,
        material: mat.id.clone(),
        length,
        width,
        thickness: mat.thickness,
        grain: mat.grain,
        edges: Edges::default(),
        transform,
        operations: Vec::new(),
    }
}

fn start(w: &WallSpec) -> DVec2 {
    DVec2::new(w.x, w.y)
}

fn end(w: &WallSpec) -> DVec2 {
    let a = w.angle.to_radians();
    start(w) + DVec2::new(a.cos(), a.sin()) * w.length
}

fn near(a: DVec2, b: DVec2) -> bool {
    a.distance(b) < 1.0
}

/// Zaokrúhli numerický šum (napr. 9.000000000000002 → 9).
fn clean(v: f64) -> f64 {
    let r = (v * 1e6).round() / 1e6;
    if r == 0.0 {
        0.0
    } else {
        r
    }
}
