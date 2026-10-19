//! Slepý roh (`S_KUC_ROH_P/_L`, findings 11 §5.2, 12 §1): bok na strane pevného panelu
//! vystupuje 3 mm pred čelo korpusu (`Z = CAB.DZ + 3`, REF: LU 391 pri RU, dne a vrchu 388).
//! Hĺbka skrinky je preto po čelo ostatných dielov korpusu (392), nie po vystupujúci bok
//! (395); vystupovanie boku odvodí generátor z panelu (`[blind_corner] side_extend`).

use crate::job::{Bbox, CarcassSource, CvjCabinet, PartRole};

/// Najväčšie vystupovanie jedného boku, ktoré sa ešte berie ako slepý roh (mm).
const MAX_PROTRUSION: f64 = 10.0;
/// Menšie rozdiely sú zaokrúhlenie, nie vystupovanie (mm).
const MIN_PROTRUSION: f64 = 0.5;
/// Diel s menším rozmerom nemá objem (model kovania).
const MIN_SOLID: f64 = 0.5;

/// Hĺbka korpusu bez boku, ktorý vystupuje dopredu; `None`, ak žiadny bok nevystupuje.
pub(crate) fn carcass_depth(c: &CvjCabinet) -> Option<f64> {
    if c.carcass_source != CarcassSource::Parts {
        return None;
    }
    let parts: Vec<(String, Bbox)> = c
        .parts
        .iter()
        .filter(|p| p.role == PartRole::Carcass && p.bbox.size().min_element() >= MIN_SOLID)
        .map(|p| (p.code.trim().to_uppercase(), p.bbox))
        .collect();
    depth_without_protruding_side(&parts, c.carcass.min.z)
}

/// Z dielov korpusu (kód, obálka): ak práve jeden bok siaha pred všetky ostatné diely
/// o 0,5 … 10 mm, hĺbka po čelo ostatných dielov od `rear` (zadok korpusu).
fn depth_without_protruding_side(parts: &[(String, Bbox)], rear: f64) -> Option<f64> {
    let is_side = |code: &str| matches!(code, "LU" | "RU" | "LF" | "RF");
    let front = |side: bool| {
        parts
            .iter()
            .filter(|(code, _)| is_side(code) == side)
            .map(|(_, b)| b.max.z)
            .fold(f64::NEG_INFINITY, f64::max)
    };
    let rest = front(false);
    if !rest.is_finite() || !rear.is_finite() {
        return None;
    }
    let protruding = parts
        .iter()
        .filter(|(code, b)| is_side(code) && b.max.z > rest + MIN_PROTRUSION)
        .collect::<Vec<_>>();
    match protruding.as_slice() {
        [(_, b)] if b.max.z <= rest + MAX_PROTRUSION && rest - rear > 1.0 => Some(rest - rear),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec3;

    fn part(code: &str, min: [f64; 3], max: [f64; 3]) -> (String, Bbox) {
        (
            code.to_owned(),
            Bbox {
                min: DVec3::from(min),
                max: DVec3::from(max),
            },
        )
    }

    /// Diely skrinky 10 zo zákazky REF (rám skrinky CV, Z dopredu).
    fn roh(lu_front: f64) -> Vec<(String, Bbox)> {
        vec![
            part("LU", [0.0, 100.0, 4.0], [18.0, 862.0, lu_front]),
            part("RU", [982.0, 100.0, 4.0], [1000.0, 862.0, 392.0]),
            part("UB", [1.0, 101.5, 0.0], [999.0, 860.5, 4.0]),
            part("TO", [18.0, 843.5, 4.0], [982.0, 861.5, 392.0]),
            part("DE", [18.0, 100.5, 4.0], [982.0, 118.5, 392.0]),
            part("AS", [18.5, 472.0, 4.0], [981.5, 490.0, 389.0]),
        ]
    }

    #[test]
    fn protruding_blind_side_is_not_depth() {
        assert_eq!(depth_without_protruding_side(&roh(395.0), 0.0), Some(392.0));
        // bez vystupujúceho boku, priveľa (iný diel), oba boky vpredu
        assert_eq!(depth_without_protruding_side(&roh(392.0), 0.0), None);
        assert_eq!(depth_without_protruding_side(&roh(450.0), 0.0), None);
        let mut both = roh(395.0);
        both[1].1.max.z = 395.0;
        assert_eq!(depth_without_protruding_side(&both, 0.0), None);
        assert_eq!(depth_without_protruding_side(&[], 0.0), None);
        assert_eq!(depth_without_protruding_side(&roh(395.0), f64::NAN), None);
    }

    /// Voliteľne na zákazke REF (`CABINETCRAFT_CVJ_REF`): roh má hĺbku 392 ako v CV.
    #[test]
    fn optional_ref_blind_corner_depth() {
        let Some(path) = std::env::var_os("CABINETCRAFT_CVJ_REF").map(std::path::PathBuf::from)
        else {
            return;
        };
        let job = crate::read_job(&path).unwrap();
        let defaults = cc_model::Defaults {
            panel_material: "P".into(),
            back_material: "B".into(),
            construction: "C".into(),
        };
        let p = crate::to_project(&job, &defaults).0;
        let roh = p.cabinets.iter().find(|c| c.id == "SP10").unwrap();
        assert_eq!(roh.d, 392.0);
    }
}
