//! Šikmá skrinka v CV (REF „S_KUCH_D SIKMA“, „S_KUCH_NASTENNA SIKMA“): bok korpusu je
//! natočený v čelnom pohľade (`AX` ≠ 0 pri `AY` = −90, napr. −30,52°).
//!
//! - Obálka dielov takej skrinky nie je jej rozmer: šikmý bok má zrezané rohy mimo korpusu
//!   a môže presahovať dozadu (REF: 60 mm k stene). Korpus je preto kváder z rozmerov CV
//!   `DX × DY × DZ` bez sokla `TOEH`, ako pri skrinke bez dielov.
//! - Bok sa prevedie na [`Slant`]: priamka jeho vonkajšej plochy pri spodku a vrchu korpusu,
//!   presah dozadu a pohľadový chrbát (`FB`).

use cc_model::{Slant, SlantSide};
use glam::DVec3;

use crate::job::{Bbox, CvjCabinet, CvjPart, PartRole};

/// Tolerancia smeru (kosínus).
const DIR_TOL: f64 = 1e-6;

fn is_side(p: &CvjPart) -> bool {
    p.role == PartRole::Carcass
        && matches!(
            p.code.trim().to_uppercase().as_str(),
            "LU" | "LF" | "RU" | "RF"
        )
}

/// Diel je natočený mimo osí skrinky.
pub(crate) fn is_tilted(p: &CvjPart) -> bool {
    let m = p.to_cabinet.matrix3;
    [m.x_axis, m.y_axis, m.z_axis].iter().any(|a| {
        let n = a.normalize_or_zero();
        n != DVec3::ZERO && n.abs().max_element() < 1.0 - DIR_TOL
    })
}

/// Šikmý bok korpusu, ak ho skrinka má.
fn slanted_side(parts: &[CvjPart]) -> Option<&CvjPart> {
    parts.iter().find(|p| is_side(p) && is_tilted(p))
}

/// Obálka korpusu skrinky so šikmým bokom: rozmery CV bez sokla.
pub(crate) fn carcass_box(parts: &[CvjPart], size: DVec3, toe: Option<f64>) -> Option<Bbox> {
    slanted_side(parts)?;
    if !(size.is_finite() && size.min_element() > 0.0) {
        return None;
    }
    let toe = toe.filter(|t| *t > 0.0 && *t < size.y).unwrap_or(0.0);
    Some(Bbox {
        min: DVec3::new(0.0, toe, 0.0),
        max: size,
    })
}

/// Šikmý bok skrinky v našom modeli (súradnice CV: X doprava, Y hore, Z dopredu).
pub(crate) fn slant(c: &CvjCabinet) -> Option<Slant> {
    let p = slanted_side(&c.parts)?;
    let m = p.to_cabinet;
    let n = m.transform_vector3(DVec3::Z).normalize_or_zero();
    // sklon: os dielu v čelnej rovine, ktorá nie je vodorovná
    let u = [DVec3::X, DVec3::Y]
        .map(|a| m.transform_vector3(a).normalize_or_zero())
        .into_iter()
        .find(|a| a.z.abs() < DIR_TOL && a.y.abs() > DIR_TOL)?;
    // hrúbka v čelnej rovine (bok je kolmý na čelo)
    if n.z.abs() > DIR_TOL || !p.size.z.is_finite() || p.size.z <= 0.0 {
        return None;
    }
    let cb = c.carcass;
    let faces = [
        m.transform_point3(DVec3::ZERO),
        m.transform_point3(DVec3::new(0.0, 0.0, p.size.z)),
    ];
    let x_at = |q: DVec3, y: f64| q.x + (y - q.y) * u.x / u.y;
    // strana podľa kódu dielu CV (`LU`/`LF` ľavý, `RU`/`RF` pravý); poloha by pri trojuholníku
    // hornej REF nerozhodla (bok ide z ľavého spodného do pravého horného rohu)
    let left = p.code.trim().to_uppercase().starts_with('L');
    // vonkajšia plocha je pri spodku ďalej od stredu skrinky
    let end_y = cb.min.y;
    let [a, b] = faces;
    let outer = match (left, x_at(a, end_y) < x_at(b, end_y)) {
        (true, true) | (false, false) => a,
        _ => b,
    };
    let (side, bottom, top) = if left {
        (
            SlantSide::Left,
            x_at(outer, cb.min.y) - cb.min.x,
            x_at(outer, cb.max.y) - cb.min.x,
        )
    } else {
        (
            SlantSide::Right,
            cb.max.x - x_at(outer, cb.min.y),
            cb.max.x - x_at(outer, cb.max.y),
        )
    };
    let extend_back = (cb.min.z - p.bbox.min.z).max(0.0);
    let finished_back = c
        .parts
        .iter()
        .any(|q| q.role == PartRole::Carcass && q.code.trim().eq_ignore_ascii_case("FB"));
    let values = [bottom, top, extend_back];
    if values.iter().any(|v| !v.is_finite()) {
        return None;
    }
    Some(Slant {
        side,
        bottom: round(bottom),
        top: round(top),
        extend_back: round(extend_back),
        finished_back,
    })
}

/// Desaťtisíciny mm (sklon boku 30,5° pri výške 762: 0,0001 mm ≈ 0,00001°), bez −0.
fn round(v: f64) -> f64 {
    (v * 10_000.0).round() / 10_000.0 + 0.0
}
