//! Spojovacie kovanie susedných skriniek (findings 12 §8): skrinka so „spojiť s ľavou
//! susedou“ dostane diery v ploche F ľavého boku a ľavá suseda na tom istom mieste
//! v priestore diery v ploche F pravého boku. Polohy sú od čela a spodku ľavého boku.

use cc_model::{
    CabinetModel, CabinetSpec, CabinetType, Face, Model, Operation, Part, ProjectFile, Role,
    Standard,
};
use cc_room::Room;
use glam::DVec3;

/// Tolerancia „skrinky sa dotýkajú“ pozdĺž steny (mm).
const TOUCH: f64 = 1.0;

/// Doplní diery spojovacieho kovania do `model` podľa polôh skriniek v `project`.
pub fn apply(model: &mut Model, project: &ProjectFile, standard: &Standard) {
    let room = Room::new(project);
    let ls = &standard.cabinet_link;
    // (index pravej skrinky v modeli, index ľavej susedy, body v ráme pravej skrinky)
    let mut plans = Vec::new();
    for (ri, right) in model.cabinets.iter().enumerate() {
        if !right.options.link_left {
            continue;
        }
        let Some(rs) = spec(project, &right.id) else {
            continue;
        };
        let Some(wall) = rs.wall.as_deref() else {
            continue;
        };
        let Some(side) = part(right, Role::SideLeft) else {
            continue;
        };
        // ľavá suseda: tá istá stena, končí tam, kde pravá začína, výškovo sa prekrývajú
        let z_r = room.elevation(rs);
        let neighbor = model.cabinets.iter().position(|c| {
            spec(project, &c.id).is_some_and(|ls_| {
                c.id != right.id
                    && ls_.wall.as_deref() == Some(wall)
                    && (ls_.at + ls_.w - rs.at).abs() <= TOUCH
                    && room.elevation(ls_) < z_r + rs.h
                    && room.elevation(ls_) + ls_.h > z_r
            })
        });
        let Some(li) = neighbor else {
            continue;
        };
        let points = hole_points(rs, right, side, ls);
        plans.push((ri, li, points));
    }
    for (ri, li, points) in plans {
        let (Some(rs), Some(lspec)) = (
            model.cabinets.get(ri).and_then(|c| spec(project, &c.id)),
            model.cabinets.get(li).and_then(|c| spec(project, &c.id)),
        ) else {
            continue;
        };
        // rám pravej → rám ľavej: x o rozdiel polôh na stene, y (od čela) podľa hĺbky
        // od steny, z podľa výšky osadenia
        let shift = DVec3::new(
            rs.at - lspec.at,
            (lspec.d + lspec.outset) - (rs.d + rs.outset),
            room.elevation(rs) - room.elevation(lspec),
        );
        if let Some(c) = model.cabinets.get_mut(ri) {
            drill(c, Role::SideLeft, &points, ls);
        }
        let moved: Vec<DVec3> = points.iter().map(|p| *p + shift).collect();
        if let Some(c) = model.cabinets.get_mut(li) {
            drill(c, Role::SideRight, &moved, ls);
        }
    }
}

fn spec<'a>(project: &'a ProjectFile, id: &str) -> Option<&'a CabinetSpec> {
    project.cabinets.iter().find(|c| c.id == id)
}

fn part(c: &CabinetModel, role: Role) -> Option<&Part> {
    c.parts.iter().find(|p| p.role == role)
}

/// Obálka dielu v ráme skrinky.
fn bbox(p: &Part) -> (DVec3, DVec3) {
    let a = p.transform.point_to_cabinet(DVec3::ZERO);
    let b = p
        .transform
        .point_to_cabinet(DVec3::new(p.length, p.width, p.thickness));
    (a.min(b), a.max(b))
}

/// Body dier na vnútornej ploche ľavého boku pravej skrinky (rám skrinky; x = plocha F).
fn hole_points(
    rs: &CabinetSpec,
    cab: &CabinetModel,
    side: &Part,
    ls: &cc_model::CabinetLinkStandard,
) -> Vec<DVec3> {
    let (lo, hi) = bbox(side);
    let (dx, dy) = (hi.y - lo.y, hi.z - lo.z);
    let manual = &rs.options.link_holes;
    let pairs: Vec<(f64, f64)> = if manual.is_empty() {
        let (xs, ys): (Vec<f64>, Vec<f64>) = match cab.cabinet_type {
            CabinetType::Base => (
                vec![ls.from_edge, dx - ls.from_edge],
                vec![ls.from_end, dy - ls.from_end],
            ),
            CabinetType::Wall => (vec![dx / 2.0], vec![ls.from_end, dy - ls.from_end]),
            CabinetType::Tall => (
                vec![ls.from_edge, dx - ls.from_edge],
                vec![ls.from_end, dy / 2.0, dy - ls.from_end],
            ),
        };
        ys.iter()
            .flat_map(|y| xs.iter().map(move |x| (*x, *y)))
            .collect()
    } else {
        manual
            .iter()
            .map(|h| (h.from_front, h.from_bottom))
            .collect()
    };
    pairs
        .into_iter()
        .filter(|(x, y)| x.is_finite() && y.is_finite())
        .map(|(x, y)| DVec3::new(hi.x, lo.y + x, lo.z + y))
        .collect()
}

/// Diery Ø × hĺbka do plochy F boku s rolou `role` v bodoch `points` (rám skrinky;
/// x sa berie z plochy F boku). Body mimo boku sa vynechajú.
fn drill(c: &mut CabinetModel, role: Role, points: &[DVec3], ls: &cc_model::CabinetLinkStandard) {
    let Some(side) = c.parts.iter_mut().find(|p| p.role == role) else {
        return;
    };
    let r = ls.diameter / 2.0;
    for p in points {
        let local = side.transform.point_to_part(*p);
        let (x, y) = (local.x, local.y);
        if x - r < -1e-6 || x + r > side.length + 1e-6 || y - r < -1e-6 || y + r > side.width + 1e-6
        {
            continue;
        }
        let op = Operation::Hole {
            face: Face::F,
            x: clean(x),
            y: clean(y),
            z: side.thickness,
            diameter: ls.diameter,
            depth: ls.depth,
            through: false,
            dowel: false,
        };
        if !side.operations.contains(&op) {
            side.operations.push(op);
        }
    }
}

fn clean(v: f64) -> f64 {
    let r = (v * 1e6).round() / 1e6;
    if r == 0.0 {
        0.0
    } else {
        r
    }
}
