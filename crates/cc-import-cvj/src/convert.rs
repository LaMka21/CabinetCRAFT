//! Prevod neutrálneho obrazu úlohy CV do projektu CabinetCRAFT (`project.toml`).
//!
//! Súradnice: CV má os Y nahor a pôdorys X–Z, my máme pôdorys `x, y` a `z` nahor.
//! Prevod `(X, Y, Z)_CV → (x, y, z) = (X, −Z, Y)` je otočenie (zachová pravotočivosť).
//! Stena CV má miestnosť na strane svojej lokálnej +Z; po prevode leží miestnosť
//! napravo od smeru steny, presne ako v našom modeli.
//!
//! Skrinka: rozmery `w, h, d` sú obálka dielov korpusu (bez dvierok, čiel, sokla,
//! nožičiek a kovania), takže `h` neobsahuje sokel. `at`, `elevation` a `outset` sa
//! odvodia z tej istej obálky v rámci plochy steny.

use std::collections::BTreeMap;

use cc_i18n::tr;
use cc_model::{
    CabinetSpec, CabinetType, Defaults, OpeningKind, OpeningSpec, ProjectFile, ProjectInfo,
    RoomSpec, Standard, WallSpec,
};
use glam::{DAffine3, DVec3};

use crate::features;
use crate::filler;
use crate::job::{CarcassSource, CvClass, CvjCabinet, CvjJob, CvjRoom, CvjWall, Face};

/// Tolerancia pre „rovnaké“ hodnoty a natočenia (mm, resp. kosínus).
const TOL: f64 = 0.05;
/// Najviac políc na skrinku pri importe.
const MAX_SHELVES: usize = 20;

/// Výsledok prevodu s väzbou na zdrojové skrinky.
#[derive(Debug, Clone)]
pub struct Imported {
    pub project: ProjectFile,
    /// poznámky pre používateľa (čo sa preskočilo, čo sa upravilo)
    pub notes: Vec<String>,
    /// pre každú skrinku projektu index skrinky v `CvjRoom::cabinets`
    pub sources: Vec<usize>,
    /// index importovanej miestnosti v `CvjJob::rooms`
    pub room: Option<usize>,
}

/// Bod CV (Y nahor) → náš bod (z nahor).
pub fn to_plan(p: DVec3) -> DVec3 {
    DVec3::new(p.x, -p.z, p.y)
}

/// Prevod úlohy na projekt; vráti projekt a poznámky.
pub fn to_project(job: &CvjJob, defaults: &Defaults) -> (ProjectFile, Vec<String>) {
    let imported = to_project_detailed(job, defaults);
    (imported.project, imported.notes)
}

/// Prevod úlohy na projekt s väzbou na zdrojové skrinky.
pub fn to_project_detailed(job: &CvjJob, defaults: &Defaults) -> Imported {
    to_project_with(job, defaults, None)
}

/// Prevod úlohy na projekt. So štandardom sa atribúty a vrch skrinky zhodné so
/// štandardom nezapíšu (skrinka nesie iba odlišné hodnoty).
pub fn to_project_with(job: &CvjJob, defaults: &Defaults, standard: Option<&Standard>) -> Imported {
    let mut notes: Vec<String> = job.notes.clone();
    let name = if job.name.trim().is_empty() {
        tr!("import_cvj.default_name")
    } else {
        job.name.trim().to_owned()
    };
    let mut project = ProjectFile {
        project: ProjectInfo::named(name),
        defaults: defaults.clone(),
        cabinets: Vec::new(),
        room: None,
        walls: Vec::new(),
        openings: Vec::new(),
    };
    let room_index = job
        .rooms
        .iter()
        .position(|r| !r.walls.is_empty() || !r.cabinets.is_empty());
    for (i, r) in job.rooms.iter().enumerate() {
        if Some(i) != room_index {
            notes.push(tr!("import_cvj.room_skipped", name = &r.name));
        }
    }
    let Some(room) = room_index.and_then(|i| job.rooms.get(i)) else {
        notes.push(tr!("import_cvj.no_room"));
        return Imported {
            project,
            notes,
            sources: Vec::new(),
            room: None,
        };
    };
    let mut sources = Vec::new();
    convert_room(room, standard, &mut project, &mut sources, &mut notes);
    Imported {
        project,
        notes,
        sources,
        room: room_index,
    }
}

fn convert_room(
    room: &CvjRoom,
    standard: Option<&Standard>,
    project: &mut ProjectFile,
    sources: &mut Vec<usize>,
    notes: &mut Vec<String>,
) {
    let mut spec = RoomSpec::default();
    // steny
    let mut walls: Vec<&CvjWall> = room.walls.iter().collect();
    walls.sort_by_key(|w| w.number);
    if let Some(h) = mode(walls.iter().map(|w| w.height).filter(|v| *v > 0.0)) {
        spec.wall_height = h;
    }
    if let Some(t) = mode(walls.iter().map(|w| w.thickness).filter(|v| *v > 0.0)) {
        spec.wall_thickness = t;
    }
    let mut wall_ids: BTreeMap<u16, String> = BTreeMap::new();
    for w in &walls {
        if let Some(ws) = convert_wall(w, &spec, notes) {
            if wall_ids.contains_key(&w.number) {
                notes.push(tr!("import_cvj.wall_duplicate", wall = w.number));
                continue;
            }
            wall_ids.insert(w.number, ws.id.clone());
            project.walls.push(ws);
        }
    }

    // poloha skriniek na stenách
    let placed: Vec<Option<Placement>> = room
        .cabinets
        .iter()
        .map(|c| placement(c, &wall_ids))
        .collect();
    let our_type = |c: &CvjCabinet| match c.class {
        CvClass::Base | CvClass::Vanity => Some(CabinetType::Base),
        CvClass::Upper => Some(CabinetType::Wall),
        CvClass::Tall => Some(CabinetType::Tall),
        _ => None,
    };
    // výška sokla: najčastejšia výška spodku spodných a vysokých skriniek
    if let Some(t) = mode(room.cabinets.iter().zip(&placed).filter_map(|(c, p)| {
        let p = p.as_ref()?;
        matches!(our_type(c), Some(CabinetType::Base | CabinetType::Tall)).then_some(p.elevation)
    })) {
        spec.toe_height = t.max(0.0);
    }
    // horná hrana horných skriniek: najčastejšia
    if let Some(top) = mode(room.cabinets.iter().zip(&placed).filter_map(|(c, p)| {
        let p = p.as_ref()?;
        (our_type(c) == Some(CabinetType::Wall)).then_some(p.elevation + p.size.y)
    })) {
        if top > 0.0 {
            spec.upper_top = Some(top);
        }
    }

    let mut used_ids: Vec<String> = Vec::new();
    for (i, (c, p)) in room.cabinets.iter().zip(&placed).enumerate() {
        let label = cabinet_label(c);
        if c.class == CvClass::Decorative {
            if let Some(o) = opening(c, &wall_ids) {
                project.openings.push(o);
                continue;
            }
        }
        // blenda (filler.rs): korpus bez blendy, samostatná blenda = skrinka bez korpusu
        let blenda = filler::detect(c);
        let narrowed = blenda.as_ref().and_then(|b| b.narrowed(c));
        let narrowed_place = narrowed.as_ref().map(|n| placement(n, &wall_ids));
        let (c, p) = match (&narrowed, &narrowed_place) {
            (Some(n), Some(np)) => (n, np),
            _ => (c, p),
        };
        let standalone = blenda.as_ref().is_some_and(filler::Detected::is_standalone);
        let Some(ty) = our_type(c) else {
            notes.push(tr!(
                "import_cvj.class_unsupported",
                label = &label,
                class = c.class.name(),
                kind = c.type_name().map(|t| format!(", {t}")).unwrap_or_default()
            ));
            continue;
        };
        // bez dielov korpusu, ale s čelom (umývačka): skrinka iba s čelami
        let front_only = standalone
            || (c.carcass_source == CarcassSource::CvBox && c.face_fronts().next().is_some());
        match c.carcass_source {
            CarcassSource::CvBox if !c.parts.is_empty() && !front_only => {
                notes.push(tr!("import_cvj.fronts_only", label = &label));
                continue;
            }
            CarcassSource::Parts if c.carcass_part_count < 2 && !standalone => {
                notes.push(tr!(
                    "import_cvj.too_few_parts",
                    label = &label,
                    count = c.carcass_part_count
                ));
                continue;
            }
            _ => {}
        }
        let size = c.carcass.size();
        if !(size.x.is_finite() && size.y.is_finite() && size.z.is_finite())
            || size.x < 1.0
            || size.y < 1.0
            || size.z < 1.0
        {
            notes.push(tr!(
                "import_cvj.no_size",
                label = &label,
                w = format!("{:.1}", size.x),
                h = format!("{:.1}", size.y),
                d = format!("{:.1}", size.z)
            ));
            continue;
        }
        if let Some(b) = &blenda {
            notes.push(format!("{label}: {}", b.describe()));
        } else if front_only {
            notes.push(tr!("import_cvj.front_only", label = &label));
        } else if c.carcass_source == CarcassSource::CvBox {
            notes.push(tr!("import_cvj.size_from_box", label = &label));
        }
        let id = unique_id(ty, c.number, i, &mut used_ids);
        let mut cab = CabinetSpec {
            id: id.clone(),
            cabinet_type: ty,
            w: round(size.x),
            h: round(size.y),
            // slepý roh: bok vystupujúci k panelu sa do hĺbky nepočíta
            d: round(crate::blind::carcass_depth(c).unwrap_or(size.z)),
            adjustable_shelves: c.shelves.min(MAX_SHELVES) as u32,
            fixed_shelves: Vec::new(),
            wall: None,
            at: 0.0,
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
            carcass: !front_only,
            lock_width: false,
            comment: None,
            placement: None,
            // pôvod v CV pre spätný zápis do tej istej zákazky (cc-out-cvj)
            catalog: Some(c.name.trim().to_owned()).filter(|n| !n.is_empty()),
            cv_id: Some(c.object_id).filter(|id| *id != 0),
            cv_fronts: None,
            filler: None,
            slant: None,
            niche: false,
            led_rail: None,
            options: Default::default(),
        };
        let f = features::fronts(c, &label, front_only, notes);
        cab.fronts = f.fronts;
        cab.drawer_length = f.drawer_length;
        cab.reveals = f.reveals;
        if let Some(s) = standard {
            features::drop_same_reveals(&mut cab.reveals, s, ty);
        }
        if ty == CabinetType::Wall && !front_only {
            // otvorená nika a LED rampa (findings 12 §2–3)
            cab.niche = crate::niche::is_niche(c);
            cab.led_rail = crate::niche::led_rail(c, &label, standard, notes);
        }
        if !front_only {
            cab.slant = crate::slant::slant(c);
            if cab.slant.is_some() {
                notes.push(tr!("import_cvj.slant", label = &label));
            }
            cab.back = features::has_back(c).unwrap_or(true);
            if ty == CabinetType::Base {
                cab.top_style =
                    features::top_style(c).filter(|t| Some(*t) != features::standard_top(standard));
                cab.stretcher_drop = features::stretcher_drop(c);
            }
        }
        cab.options = features::options(
            c,
            ty,
            front_only,
            &label,
            standard.map(|s| &s.cabinet_options),
            notes,
        );
        if standalone {
            // čelo samostatnej blendy je v `filler.front_width` (generátor ho postaví sám)
            cab.fronts.clear();
            cab.reveals = Default::default();
            cab.drawer_length = None;
        }
        cab.filler = blenda.map(|b| b.spec);
        // sokel cez zostavu, bočný sokel a krycí bok (findings 12 §6)
        if !front_only && matches!(ty, CabinetType::Base | CabinetType::Tall) {
            let gap = standard.map_or(cc_model::PlinthStandard::default().gap_top, |s| {
                s.plinth.gap_top
            });
            crate::plinth::options(c, gap, &mut cab.options);
        }
        // čelá tak, ako sú v CV: spätný zápis ich delenie nezmení, kým ich nezmeníme my
        cab.cv_fronts = cab.cv_id.map(|_| cab.front_fingerprint());
        match p {
            Some(p) => {
                cab.wall = Some(p.wall_id.clone());
                cab.at = round(p.at);
                let mut outset = p.outset;
                if outset < -0.5 {
                    notes.push(tr!(
                        "import_cvj.behind_wall",
                        label = &label,
                        mm = format!("{:.1}", -outset)
                    ));
                }
                if outset < 0.0 {
                    outset = 0.0;
                }
                cab.outset = round(outset);
                let mut elevation = p.elevation;
                if elevation < 0.0 {
                    if elevation < -0.5 {
                        notes.push(tr!(
                            "import_cvj.below_floor",
                            label = &label,
                            mm = format!("{:.1}", -elevation)
                        ));
                    }
                    elevation = 0.0;
                }
                let default = match ty {
                    CabinetType::Base | CabinetType::Tall => spec.toe_height,
                    CabinetType::Wall => spec.upper_top() - cab.h,
                };
                if (elevation - default).abs() > TOL {
                    cab.elevation = Some(round(elevation));
                }
            }
            None => {
                notes.push(format!("{label}: {}", unplaced_reason(c, &wall_ids)));
            }
        }
        if ty == CabinetType::Tall && c.shelves > 0 {
            notes.push(tr!(
                "import_cvj.tall_shelves",
                label = &label,
                count = c.shelves
            ));
        }
        project.cabinets.push(cab);
        sources.push(i);
    }
    for t in &room.counters {
        notes.push(tr!(
            "import_cvj.countertop",
            name = &t.name,
            l = format!("{:.0}", t.size.x),
            w = format!("{:.0}", t.size.y),
            t = format!("{:.0}", t.size.z),
            wall = t
                .wall
                .map(|w| tr!("import_cvj.countertop_wall", wall = w))
                .unwrap_or_default()
        ));
    }
    if room.floor_shapes > 0 {
        notes.push(tr!("import_cvj.floor_outline"));
    }
    spec.wall_height = round(spec.wall_height);
    spec.wall_thickness = round(spec.wall_thickness);
    spec.toe_height = round(spec.toe_height);
    spec.upper_top = spec.upper_top.map(round);
    project.room = Some(spec);
}

fn cabinet_label(c: &CvjCabinet) -> String {
    match c.number {
        Some(n) => tr!("import_cvj.label_cabinet", number = n, name = &c.name),
        None => tr!("import_cvj.label_object", name = &c.name),
    }
}

fn unique_id(ty: CabinetType, number: Option<i64>, index: usize, used: &mut Vec<String>) -> String {
    let prefix = match ty {
        CabinetType::Base => "SP",
        CabinetType::Wall => "HO",
        CabinetType::Tall => "VY",
    };
    let n = number.filter(|n| *n > 0).unwrap_or(index as i64 + 1);
    let mut id = format!("{prefix}{n}");
    let mut k = 2;
    while used.contains(&id) {
        id = format!("{prefix}{n}_{k}");
        k += 1;
    }
    used.push(id.clone());
    id
}

fn convert_wall(w: &CvjWall, room: &RoomSpec, notes: &mut Vec<String>) -> Option<WallSpec> {
    let id = format!("W{}", w.number);
    if !(w.length.is_finite() && w.length > 0.5) {
        notes.push(tr!("import_cvj.wall_zero_length", wall = w.number));
        return None;
    }
    let m = w.frame.matrix3;
    if m.y_axis.dot(DVec3::Y) < 1.0 - 1e-6 {
        notes.push(tr!("import_cvj.wall_not_vertical", wall = w.number));
    }
    if w.start.y.abs() > TOL {
        notes.push(tr!(
            "import_cvj.wall_raised",
            wall = w.number,
            mm = format!("{:.0}", w.start.y)
        ));
    }
    let start = to_plan(w.start);
    let end = to_plan(w.end);
    let dir = end - start;
    let angle = normalize_angle(dir.y.atan2(dir.x).to_degrees());
    let height =
        (w.height > 0.0 && (w.height - room.wall_height).abs() > TOL).then(|| round(w.height));
    let thickness = if w.thickness <= 0.0 {
        notes.push(tr!("import_cvj.wall_zero_thickness", wall = w.number));
        None
    } else {
        ((w.thickness - room.wall_thickness).abs() > TOL).then(|| round(w.thickness))
    };
    Some(WallSpec {
        id,
        x: round(start.x),
        y: round(start.y),
        length: round(w.length),
        angle: round(angle),
        height,
        thickness,
    })
}

/// Poloha korpusu v rámci prednej plochy steny.
struct Placement {
    wall_id: String,
    at: f64,
    elevation: f64,
    outset: f64,
    /// rozmery korpusu
    size: DVec3,
}

fn placement(c: &CvjCabinet, walls: &BTreeMap<u16, String>) -> Option<Placement> {
    if c.face != Some(Face::Front) {
        return None;
    }
    let wall_id = walls.get(&c.wall?)?.clone();
    let in_face = c.in_face?;
    if !axis_aligned(&in_face) {
        return None;
    }
    let pts = c.carcass.corners().map(|p| in_face.transform_point3(p));
    let min = pts
        .iter()
        .fold(DVec3::splat(f64::INFINITY), |a, p| a.min(*p));
    let max = pts
        .iter()
        .fold(DVec3::splat(f64::NEG_INFINITY), |a, p| a.max(*p));
    let size = max - min;
    if !(min.is_finite() && max.is_finite()) {
        return None;
    }
    Some(Placement {
        wall_id,
        at: min.x,
        elevation: min.y,
        outset: min.z,
        size,
    })
}

/// Rámec skrinky je rovnobežný s rámcom plochy (bez natočenia).
fn axis_aligned(t: &DAffine3) -> bool {
    let m = t.matrix3;
    m.x_axis.dot(DVec3::X) > 1.0 - 1e-6
        && m.y_axis.dot(DVec3::Y) > 1.0 - 1e-6
        && m.z_axis.dot(DVec3::Z) > 1.0 - 1e-6
}

fn unplaced_reason(c: &CvjCabinet, walls: &BTreeMap<u16, String>) -> String {
    let base = tr!("import_cvj.unplaced");
    match (c.face, c.wall) {
        (Some(Face::Back), Some(w)) => {
            tr!("import_cvj.unplaced_back_face", wall = w, base = base)
        }
        (Some(Face::Front), Some(w)) if !walls.contains_key(&w) => {
            tr!("import_cvj.unplaced_wall_skipped", wall = w, base = base)
        }
        (Some(Face::Front), Some(_)) => tr!(
            "import_cvj.unplaced_rotated",
            angle = format!("{:.1}", c.rotation.y),
            base = base
        ),
        _ => tr!("import_cvj.unplaced_free", base = base),
    }
}

/// Okno, dvere alebo otvor (dekoratívny objekt na prednej ploche steny).
fn opening(c: &CvjCabinet, walls: &BTreeMap<u16, String>) -> Option<OpeningSpec> {
    let kind = match c.type_code {
        19 => OpeningKind::Window,
        20 => OpeningKind::Door,
        26 => OpeningKind::Opening,
        _ => return None,
    };
    // otvor berieme z obálky CV (rozmery DX/DY a poloha), nie z dielov
    if c.face != Some(Face::Front) {
        return None;
    }
    let wall = walls.get(&c.wall?)?.clone();
    let in_face = c.in_face?;
    if !axis_aligned(&in_face) {
        return None;
    }
    let o = in_face.transform_point3(DVec3::ZERO);
    let (w, h) = (c.size.x, c.size.y);
    if !(w > 0.0 && h > 0.0 && o.x.is_finite() && o.y.is_finite()) {
        return None;
    }
    Some(OpeningSpec {
        wall,
        kind,
        at: round(o.x.max(0.0)),
        width: round(w),
        height: round(h),
        elevation: round(o.y.max(0.0)),
    })
}

/// Najčastejšia hodnota (zaokrúhlená na 0,1 mm); pri zhode menšia.
fn mode(values: impl Iterator<Item = f64>) -> Option<f64> {
    let mut counts: BTreeMap<i64, usize> = BTreeMap::new();
    for v in values.filter(|v| v.is_finite() && v.abs() < 1e7) {
        *counts.entry((v * 10.0).round() as i64).or_default() += 1;
    }
    let best = counts.values().copied().max()?;
    counts
        .iter()
        .find(|(_, n)| **n == best)
        .map(|(k, _)| *k as f64 / 10.0)
}

/// Zaokrúhlenie na tisíciny mm (čistý zápis do TOML), bez −0.
fn round(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0 + 0.0
}

/// Uhol do intervalu (−180, 180].
fn normalize_angle(a: f64) -> f64 {
    let mut a = a % 360.0;
    if a <= -180.0 {
        a += 360.0;
    }
    if a > 180.0 {
        a -= 360.0;
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_mapping_is_rotation() {
        let x = to_plan(DVec3::X);
        let y = to_plan(DVec3::Y);
        let z = to_plan(DVec3::Z);
        // pravotočivosť: x × y = z aj po prevode
        assert!((x.cross(y) - z).length() < 1e-12);
        assert_eq!(y, DVec3::Z);
    }

    #[test]
    fn helpers() {
        assert_eq!(normalize_angle(270.0), -90.0);
        assert_eq!(normalize_angle(-180.0), 180.0);
        assert_eq!(mode([1.0, 2.0, 2.0, 3.0].into_iter()), Some(2.0));
        assert_eq!(mode([f64::NAN].into_iter()), None);
        assert_eq!(round(-0.0001), 0.0);
        assert!(round(-0.0001).is_sign_positive());
    }
}
