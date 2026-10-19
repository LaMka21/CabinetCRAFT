//! Blenda (výplň) pri skrinke (`[cabinet.filler]`, model v `cc_model::FillerSpec`).
//!
//! Podľa `findings/11-atributy-skriniek.md` §5.3 a `findings/12-parita-ref.md` §2:
//! - **pri skrinke** (REF skr. 9, horná s blendou vpravo): lišta z materiálu korpusu za
//!   bokom (hĺbka boku, 2 pod vrchom boku, predná hrana na čele blendy), čelo blendy z
//!   materiálu čiel 2 pred korpusom vo výške čiel skrinky, voliteľne dno blendy pod lištou
//!   (hrúbka čela, od zadnej plochy dvierok po zadok skrinky) a vrch na bokoch predĺžený cez
//!   blendu;
//! - **samostatná blenda** (REF skr. 5, katalógová BLENDA): skrinka bez korpusu je lišta na
//!   okraji pri susednej skrinke, slepé čelo (šírka lišty + presah) leží priamo na lište.
//!
//! Kolíky medzi lištou a čelom/dnom robí `cc-joints` z kovania [`Hardware::Filler`].

use cc_i18n::tr;
use cc_model::{
    CabinetModel, CabinetSpec, Edge, Edges, FillerSide, Hardware, Material, Part, Role, Standard,
    Transform, EPS,
};
use glam::DVec3;

/// Najväčšia šírka blendy a jej čela (ochrana pred nezmyselnými číslami zo vstupu).
const MAX_FILLER: f64 = 1500.0;

/// Os-zarovnaný kváder dielu v súradniciach skrinky.
struct Box3 {
    min: DVec3,
    max: DVec3,
}

fn bbox(p: &Part) -> Box3 {
    let a = p.transform.point_to_cabinet(DVec3::ZERO);
    let b = p
        .transform
        .point_to_cabinet(DVec3::new(p.length, p.width, p.thickness));
    Box3 {
        min: a.min(b),
        max: a.max(b),
    }
}

/// Olepenie dielu blendy.
enum Band {
    /// predná hrana (lišta, dno blendy)
    Front,
    /// dookola (čelo blendy)
    All,
}

/// Pridá diely blendy do postavenej skrinky (ak ju skrinka má).
pub(crate) fn add(
    cab: &mut CabinetModel,
    spec: &CabinetSpec,
    standard: &Standard,
    panel: &Material,
    front: &Material,
) -> Result<(), String> {
    let Some(f) = &spec.filler else {
        return Ok(());
    };
    let fs = &standard.filler;
    let in_range = |v: f64| v.is_finite() && (0.0..=MAX_FILLER).contains(&v);
    for (name, v) in [
        ("filler.width", f.width),
        ("filler.front_gap", fs.front_gap),
        ("filler.standalone_front_gap", fs.standalone_front_gap),
        ("filler.strip_top_gap", fs.strip_top_gap),
        ("filler.front_overhang", fs.front_overhang),
        (
            "filler.standalone_front_overhang",
            fs.standalone_front_overhang,
        ),
    ] {
        if !in_range(v) {
            return Err(tr!(
                "generate.filler.out_of_range",
                name = name,
                value = v,
                max = MAX_FILLER
            ));
        }
    }
    let strip = if spec.carcass {
        attached(cab, spec, standard, panel, front)?
    } else {
        standalone(cab, spec, standard, panel, front)?
    };
    if f.dowels {
        cab.hardware.push(Hardware::Filler { strip });
    }
    Ok(())
}

/// Samostatná blenda: lišta na okraji skrinky pri susedovi, slepé čelo priamo na lište.
/// Bez `fronts` je čelo široké `front_width` od plochy lišty pri susedovi (CV: šírka + `OLR`),
/// s `fronts` sa použijú tie a iba sa prisunú na lištu.
fn standalone(
    cab: &mut CabinetModel,
    spec: &CabinetSpec,
    standard: &Standard,
    panel: &Material,
    front: &Material,
) -> Result<String, String> {
    let fs = &standard.filler;
    let f = spec.filler.clone().unwrap_or_default();
    let side = f.side;
    let t = panel.thickness;
    if spec.w + EPS < t {
        return Err(tr!(
            "generate.filler.standalone_narrow",
            width = spec.w,
            strip = t
        ));
    }
    let tf = front.thickness;
    let y_back = -fs.standalone_front_gap;
    if spec.fronts.is_empty() {
        let fw = f
            .front_width
            .unwrap_or(spec.w + fs.standalone_front_overhang);
        if !(fw.is_finite() && fw + EPS >= t && fw <= MAX_FILLER) {
            return Err(tr!(
                "generate.filler.standalone_front_width",
                width = fw,
                strip = t,
                max = MAX_FILLER
            ));
        }
        let x0 = match side {
            FillerSide::Right => 0.0,
            FillerSide::Left => spec.w - fw,
        };
        // x = +Z (nahor), y = +X, z = +Y (F = zadná plocha k lište), ako čelá skrinky
        push(
            cab,
            spec,
            Role::FalseFront,
            front,
            spec.h,
            fw,
            Transform::from_axes(DVec3::Z, DVec3::X, DVec3::new(x0, y_back - tf, 0.0)),
            Band::All,
        );
    }
    let transform = match side {
        // blenda vpravo od suseda: lišta pri ľavom okraji (ako ľavý bok, F k presahu čela)
        FillerSide::Right => {
            Transform::from_axes(DVec3::NEG_Z, DVec3::Y, DVec3::new(0.0, 0.0, spec.h))
        }
        // blenda vľavo od suseda: lišta pri pravom okraji (ako pravý bok)
        FillerSide::Left => Transform::from_axes(DVec3::Z, DVec3::Y, DVec3::new(spec.w, 0.0, 0.0)),
    };
    let id = push(
        cab,
        spec,
        Role::FillerStrip,
        panel,
        spec.h,
        spec.d,
        transform,
        Band::Front,
    );
    // [P] REF skr. 5: čelo blendy leží priamo na prednej hrane lišty (bez odstupu čiel)
    for p in cab.parts.iter_mut().filter(|p| p.role.is_front()) {
        let b = bbox(p);
        // iba čelá na korpus (pred prednou rovinou)
        if b.max.y <= EPS {
            p.transform.origin.y += y_back - b.max.y;
        }
    }
    Ok(id)
}

/// Blenda za bokom skrinky s korpusom; vráti ID lišty.
fn attached(
    cab: &mut CabinetModel,
    spec: &CabinetSpec,
    standard: &Standard,
    panel: &Material,
    front: &Material,
) -> Result<String, String> {
    let fs = &standard.filler;
    let Some(f) = &spec.filler else {
        return Err(tr!("generate.filler.missing"));
    };
    let t = panel.thickness;
    let width = f.width;
    if width + EPS < t {
        return Err(tr!("generate.filler.narrow", width = width, strip = t));
    }
    let fw = f.front_width.unwrap_or(width + fs.front_overhang);
    if !(fw.is_finite() && fw + EPS >= t && fw <= MAX_FILLER) {
        return Err(tr!(
            "generate.filler.front_width",
            width = fw,
            strip = t,
            max = MAX_FILLER
        ));
    }
    let side_role = match f.side {
        FillerSide::Left => Role::SideLeft,
        FillerSide::Right => Role::SideRight,
    };
    let side = cab
        .parts
        .iter()
        .find(|p| p.role == side_role)
        .map(bbox)
        .ok_or_else(|| tr!("generate.filler.no_side"))?;
    // vonkajšia plocha boku a smer von zo skrinky
    let (outer, out) = match f.side {
        FillerSide::Left => (side.min.x, -1.0),
        FillerSide::Right => (side.max.x, 1.0),
    };
    // interval x šírky `s` od vonkajšej plochy boku smerom von
    let span = |s: f64| {
        if out > 0.0 {
            (outer, outer + s)
        } else {
            (outer - s, outer)
        }
    };
    let y_front = -fs.front_gap;

    // lišta: hĺbka boku, spodok ako bok, vrch o `strip_top_gap` nižšie
    let strip_h = side.max.z - side.min.z - fs.strip_top_gap;
    let strip_d = side.max.y - side.min.y;
    if strip_h <= EPS || strip_d <= EPS {
        return Err(tr!(
            "generate.filler.bad_strip",
            height = strip_h,
            depth = strip_d
        ));
    }
    let (sx0, sx1) = span(t);
    let transform = match f.side {
        // ako pravý bok: x = +Z, y = +Y, z = −X
        FillerSide::Right => {
            Transform::from_axes(DVec3::Z, DVec3::Y, DVec3::new(sx1, y_front, side.min.z))
        }
        // ako ľavý bok: x = −Z, y = +Y, z = +X
        FillerSide::Left => Transform::from_axes(
            DVec3::NEG_Z,
            DVec3::Y,
            DVec3::new(sx0, y_front, side.min.z + strip_h),
        ),
    };

    // čelo blendy vo výške čiel skrinky (dvierka s presahom pod dno), bez čiel ako korpus
    let (mut z0, mut z1) = (f64::INFINITY, f64::NEG_INFINITY);
    for b in cab.parts.iter().filter(|p| p.role.is_front()).map(bbox) {
        z0 = z0.min(b.min.z);
        z1 = z1.max(b.max.z);
    }
    if !(z0.is_finite() && z1.is_finite() && z1 - z0 > EPS) {
        (z0, z1) = (0.0, spec.h);
    }
    let tf = front.thickness;
    let (fx0, _) = span(fw);

    let strip = push(
        cab,
        spec,
        Role::FillerStrip,
        panel,
        strip_h,
        strip_d,
        transform,
        Band::Front,
    );
    // x = +Z (nahor), y = +X, z = +Y (F = zadná plocha k lište), ako čelá skrinky
    push(
        cab,
        spec,
        Role::FillerFront,
        front,
        z1 - z0,
        fw,
        Transform::from_axes(DVec3::Z, DVec3::X, DVec3::new(fx0, y_front - tf, z0)),
        Band::All,
    );
    if f.bottom {
        // [P] REF skr. 9: od zadnej plochy dvierok po zadok skrinky, pod spodkom boku
        let (bx0, _) = span(width);
        let y0 = -standard.fronts.gap_to_carcass;
        let depth = spec.d - y0;
        if depth <= EPS {
            return Err(tr!("generate.filler.bad_bottom", depth = depth));
        }
        // x = +X, y = +Y, z = +Z (F = horná plocha k lište)
        push(
            cab,
            spec,
            Role::FillerBottom,
            front,
            width,
            depth,
            Transform::from_axes(
                DVec3::X,
                DVec3::Y,
                DVec3::new(bx0, y0, side.min.z - front.thickness),
            ),
            Band::Front,
        );
    }

    // [P] REF skr. 9: vrch na bokoch ide cez blendu (626,5 = 576,5 + 50)
    if fs.extend_top {
        for p in cab.parts.iter_mut().filter(|p| p.role == Role::Top) {
            let b = bbox(p);
            let reaches = if out > 0.0 {
                b.max.x >= outer - EPS
            } else {
                b.min.x <= outer + EPS
            };
            // dĺžka vrchu ide pozdĺž šírky skrinky
            if !reaches || p.transform.x_axis.x.abs() < 0.5 {
                continue;
            }
            p.length += width;
            // predĺženie proti osi x dielu posunie počiatok
            if (p.transform.x_axis.x > 0.0) != (out > 0.0) {
                p.transform.origin.x += out * width;
            }
        }
    }
    Ok(strip)
}

/// Pridá diel blendy a vráti jeho ID (`HO9-BL-LISTA`).
#[allow(clippy::too_many_arguments)]
fn push(
    cab: &mut CabinetModel,
    spec: &CabinetSpec,
    role: Role,
    material: &Material,
    length: f64,
    width: f64,
    transform: Transform,
    band: Band,
) -> String {
    let mut edges = Edges::default();
    match band {
        Band::Front => {
            if let Some(e) = transform.edge_facing(DVec3::NEG_Y) {
                edges.set(e, material.edge_band.clone());
            }
        }
        Band::All => {
            for e in Edge::ALL {
                edges.set(e, material.edge_band.clone());
            }
        }
    }
    let id = format!("{}-{}", spec.id, role.id_code());
    cab.parts.push(Part {
        id: id.clone(),
        cabinet: spec.id.clone(),
        role,
        material: material.id.clone(),
        length,
        width,
        thickness: material.thickness,
        grain: material.grain,
        edges,
        transform,
        operations: Vec::new(),
    });
    id
}
