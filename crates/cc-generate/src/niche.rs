//! Otvorená nika (štandard `[niche]`, skrinka `niche = true`) a LED rampa pod hornou
//! skrinkou (`[led_rail]`, skrinka `led_rail`) podľa zákazky CV REF (findings 12 §2–3,
//! dataset `tests/fixtures/cv-ref`, skrinky 6, 7 a 8).
//!
//! Nika (CV „S_KUCH_NASTENNA NIKA“): všetky diely z jednej dosky (MDF 19), vrch naložený na
//! boky a dno pod bokmi cez celú šírku a hĺbku, boky medzi nimi, pohľadový chrbát (`FB`)
//! vložený medzi boky, vrch a dno, so zadnou plochou `back_from_rear` pred zadnou hranou bokov
//! (medzera pre záves). Spoje robí `cc-joints` podľa pravidiel štandardu; tu vznikajú iba
//! diery závesu, otvory v chrbte a drážka pre LED v dne.

use cc_i18n::tr;
use cc_model::{
    CabinetModel, CabinetSpec, CabinetType, Face, Material, MaterialLibrary, Operation, Part, Role,
    ShelfSpacing, Standard, Transform, EPS, P2,
};
use glam::DVec3;

use crate::{build_fronts, material, Dims, GenerateError, PartBuilder};

/// Najviac polohovateľných políc v nike.
const MAX_SHELVES: u32 = 20;

/// Diely, kovanie a obrábanie otvorenej niky.
pub(crate) fn build_niche(
    spec: &CabinetSpec,
    standard: &Standard,
    front: &Material,
    materials: &MaterialLibrary,
) -> Result<CabinetModel, GenerateError> {
    let invalid = |message: String| GenerateError::Invalid {
        cabinet: spec.id.clone(),
        message,
    };
    let ns = standard
        .niche
        .as_ref()
        .ok_or_else(|| invalid(tr!("generate.niche.no_section")))?;
    if spec.cabinet_type != CabinetType::Wall {
        return Err(invalid(tr!("generate.niche.wall_only")));
    }
    let mat = material(materials, &ns.material)?;
    let t = mat.thickness;
    let (w, h, d) = (spec.w, spec.h, spec.d);
    let rear = ns.back_from_rear;
    let cs = &standard.carcass;
    let finite = [w, h, d, t, rear].iter().all(|v| v.is_finite());
    if !finite || t <= EPS || rear < 0.0 {
        return Err(invalid(tr!("generate.niche.bad_values")));
    }
    let (wi, hi) = (w - 2.0 * t, h - 2.0 * t);
    // predná plocha chrbta = hĺbka použiteľná pre police a rad dier
    let yb = d - rear - t;
    if wi <= EPS || hi <= EPS || yb <= cs.shelf_front_setback + EPS {
        return Err(invalid(tr!(
            "generate.niche.too_small",
            w = w,
            h = h,
            d = d,
            t = t,
            rear = rear
        )));
    }

    let mut b = PartBuilder::new(spec, mat);
    // boky medzi dnom a vrchom (F = vnútorná plocha, Y0 = predná hrana)
    let left = b.front(
        Role::SideLeft,
        mat,
        hi,
        d,
        // x = −Z (zhora nadol), y = +Y, z = +X
        Transform::from_axes(DVec3::NEG_Z, DVec3::Y, DVec3::new(0.0, 0.0, h - t)),
    );
    let right = b.front(
        Role::SideRight,
        mat,
        hi,
        d,
        // x = +Z, y = +Y, z = −X
        Transform::from_axes(DVec3::Z, DVec3::Y, DVec3::new(w, 0.0, t)),
    );
    // vrch na bokoch a dno pod bokmi cez celú šírku a hĺbku (CV „FFFF“: olepené dookola)
    b.front(
        Role::Top,
        mat,
        w,
        d,
        // x = −X, y = +Y, z = −Z (F = spodná plocha)
        Transform::from_axes(DVec3::NEG_X, DVec3::Y, DVec3::new(w, 0.0, h)),
    );
    let bottom = b.front(
        Role::Bottom,
        mat,
        w,
        d,
        // x = +X, y = +Y, z = +Z (F = horná plocha)
        Transform::from_axes(DVec3::X, DVec3::Y, DVec3::ZERO),
    );
    // pohľadový chrbát medzi bokmi, vrchom a dnom; x = +X, y = +Z, z = −Y (F = predná plocha)
    let back = b.front(
        Role::FinishedBack,
        mat,
        wi,
        hi,
        Transform::from_axes(DVec3::X, DVec3::Z, DVec3::new(t, d - rear, t)),
    );

    // polohovateľné police: o `shelf_side_clearance` kratšie, po chrbát mínus `shelf_front_setback`
    let n = spec.adjustable_shelves;
    if n > MAX_SHELVES {
        return Err(invalid(tr!(
            "generate.niche.too_many_shelves",
            count = MAX_SHELVES
        )));
    }
    if n > 0 {
        let sc = cs.shelf_side_clearance;
        let setback = cs.shelf_front_setback;
        let length = wi - 2.0 * sc;
        let width = yb - setback;
        let opening = (hi - f64::from(n) * t) / f64::from(n + 1);
        if length <= EPS || width <= EPS || opening <= EPS {
            return Err(invalid(tr!(
                "generate.niche.shelf_does_not_fit",
                length = length,
                width = width,
                count = n
            )));
        }
        for i in 1..=n {
            let k = f64::from(i);
            let top_face = match cs.shelf_spacing {
                ShelfSpacing::InnerHeight => t + hi * k / f64::from(n + 1),
                // [P] CV: rovnaké otvory medzi dnom, policami a vrchom (REF skr. 7: 227,3 / 454,7)
                ShelfSpacing::EqualOpenings => t + k * (opening + t),
            };
            b.panel(
                Role::AdjustableShelf,
                length,
                width,
                Transform::from_axes(
                    DVec3::X,
                    DVec3::Y,
                    DVec3::new(t + sc, setback, top_face - t),
                ),
            );
        }
    }

    let dims = Dims {
        w,
        h,
        d,
        t,
        tb: t,
        wi,
        yb,
        gw: 0.0,
        floor: 0.0,
    };
    let (hardware, openings) =
        build_fronts(spec, standard, &dims, front, materials, &mut b).map_err(invalid)?;
    let mut parts = b.finish();

    // závesné kovanie: diery do vnútornej plochy bokov, `from_edge` od zadnej hrany
    for (side, inner_x) in [(&left, t), (&right, w - t)] {
        let Some(sp) = part_ref(&parts, side) else {
            continue;
        };
        let mut ops = Vec::new();
        for hh in &ns.hanger_holes {
            let entry = DVec3::new(inner_x, d - hh.from_edge, h - t - hh.from_top);
            ops.push(hole_of(sp, entry, hh).map_err(invalid)?);
        }
        push_ops(&mut parts, side, ops);
    }
    // otvory v chrbte pri oboch bokoch (zo zadnej plochy)
    if let Some(bp) = part_ref(&parts, &back) {
        let mut ops = Vec::new();
        for hh in &ns.back_holes {
            for x in [t + hh.from_edge, w - t - hh.from_edge] {
                let entry = DVec3::new(x, d - rear, h - t - hh.from_top);
                ops.push(hole_of(bp, entry, hh).map_err(invalid)?);
            }
        }
        push_ops(&mut parts, &back, ops);
    }
    // dno niky je zároveň LED rampa: drážka zospodu v línii LED pod susednými skrinkami
    if ns.led_groove {
        if let (Some(g), Some(p)) = (
            standard.led_rail.as_ref().and_then(|l| l.groove),
            part_ref(&parts, &bottom),
        ) {
            let op = led_groove(p, d, g).map_err(invalid)?;
            push_ops(&mut parts, &bottom, vec![op]);
        }
    }

    Ok(CabinetModel {
        id: spec.id.clone(),
        cabinet_type: spec.cabinet_type,
        w,
        h,
        d,
        parts,
        hardware,
        options: spec.options.resolve(&standard.cabinet_options),
        openings,
    })
}

/// LED rampa pod hornou skrinkou: doska pod dnom (`z = −T … 0`), zadnou hranou pri zadnej
/// strane skrinky, s drážkou pre LED profil zospodu.
pub(crate) fn add_led_rail(
    cab: &mut CabinetModel,
    spec: &CabinetSpec,
    standard: &Standard,
    materials: &MaterialLibrary,
) -> Result<(), GenerateError> {
    let Some(rail) = spec.led_rail else {
        return Ok(());
    };
    let invalid = |message: String| GenerateError::Invalid {
        cabinet: spec.id.clone(),
        message,
    };
    let ls = standard
        .led_rail
        .as_ref()
        .ok_or_else(|| invalid(tr!("generate.led.no_section")))?;
    if spec.cabinet_type != CabinetType::Wall {
        return Err(invalid(tr!("generate.led.wall_only")));
    }
    let mat = material(materials, &ls.material)?;
    let length = rail.length.unwrap_or(spec.w);
    let depth = rail.depth.unwrap_or(spec.d - ls.depth_minus);
    let t = mat.thickness;
    if ![rail.x, length, depth, t].iter().all(|v| v.is_finite())
        || length <= EPS
        || depth <= EPS
        || depth > spec.d + EPS
        || rail.x.abs() > 10_000.0
        || length > 10_000.0
    {
        return Err(invalid(tr!(
            "generate.led.bad_size",
            length = length,
            depth = depth,
            x = rail.x
        )));
    }
    let mut b = PartBuilder::new(spec, mat);
    // x = +X, y = +Y, z = +Z (F = horná plocha k skrinke); olepená dookola (CV „CCCC“)
    let id = b.front(
        Role::LedRail,
        mat,
        length,
        depth,
        Transform::from_axes(DVec3::X, DVec3::Y, DVec3::new(rail.x, spec.d - depth, -t)),
    );
    let mut parts = b.finish();
    if let (Some(g), Some(p)) = (ls.groove, part_ref(&parts, &id)) {
        let op = led_groove(p, spec.d, g).map_err(invalid)?;
        push_ops(&mut parts, &id, vec![op]);
    }
    cab.parts.extend(parts);
    Ok(())
}

/// Drážka pre LED profil v spodnej ploche (B) dielu ležiaceho naplocho (`y` od čela):
/// zadná hrana drážky `from_rear` od zadnej strany skrinky hĺbky `cab_d`, cez celú dĺžku.
fn led_groove(p: &Part, cab_d: f64, g: cc_model::LedGroove) -> Result<Operation, String> {
    // zadná strana skrinky v ráme dielu (diel môže začínať za čelom skrinky)
    let rear = p.transform.point_to_part(DVec3::new(0.0, cab_d, 0.0)).y;
    let y = rear - g.from_rear - g.width / 2.0;
    let ok = [g.from_rear, g.width, g.depth]
        .iter()
        .all(|v| v.is_finite())
        && g.width > EPS
        && g.depth > EPS
        && y - g.width / 2.0 >= -EPS
        && y + g.width / 2.0 <= p.width + EPS;
    if !ok {
        return Err(tr!(
            "generate.led.groove_does_not_fit",
            width = g.width,
            depth = g.depth,
            from_rear = g.from_rear,
            id = &p.id,
            part_width = p.width
        ));
    }
    Ok(Operation::Groove {
        face: Face::B,
        start: P2 { x: 0.0, y },
        end: P2 { x: p.length, y },
        width: g.width,
        depth: g.depth,
    })
}

/// Diera niky; bez hĺbky cez celý diel.
fn hole_of(p: &Part, entry: DVec3, hh: &cc_model::NicheHole) -> Result<Operation, String> {
    match hh.depth {
        Some(depth) => face_hole(p, entry, hh.diameter, depth, false),
        None => face_hole(p, entry, hh.diameter, p.thickness, true),
    }
}

/// Diera do plochy dielu: bod vstupu v súradniciach skrinky leží na ploche F alebo B.
fn face_hole(
    p: &Part,
    entry: DVec3,
    diameter: f64,
    depth: f64,
    through: bool,
) -> Result<Operation, String> {
    let q = p.transform.point_to_part(entry);
    let face = if (q.z - p.thickness).abs() < 1e-6 {
        Face::F
    } else if q.z.abs() < 1e-6 {
        Face::B
    } else {
        return Err(tr!("generate.niche.hole_off_face", id = &p.id));
    };
    let ok = [q.x, q.y, diameter, depth].iter().all(|v| v.is_finite())
        && diameter > EPS
        && depth > EPS
        && (0.0..=p.length).contains(&q.x)
        && (0.0..=p.width).contains(&q.y);
    if !ok {
        return Err(tr!(
            "generate.niche.hole_outside",
            diameter = diameter,
            depth = depth,
            id = &p.id,
            x = format!("{:.1}", q.x),
            y = format!("{:.1}", q.y)
        ));
    }
    Ok(Operation::Hole {
        face,
        x: round6(q.x),
        y: round6(q.y),
        z: if face == Face::F { p.thickness } else { 0.0 },
        diameter,
        depth,
        through,
        dowel: false,
    })
}

fn round6(v: f64) -> f64 {
    let r = (v * 1e6).round() / 1e6;
    if r == 0.0 {
        0.0
    } else {
        r
    }
}

fn part_ref<'a>(parts: &'a [Part], id: &str) -> Option<&'a Part> {
    parts.iter().find(|p| p.id == id)
}

fn push_ops(parts: &mut [Part], id: &str, ops: Vec<Operation>) {
    if let Some(p) = parts.iter_mut().find(|p| p.id == id) {
        p.operations.extend(ops);
    }
}
