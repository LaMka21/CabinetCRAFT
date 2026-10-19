//! Otvorená nika a LED rampa hornej skrinky z CV (zákazka REF, findings 12 §2–3).

use cc_i18n::tr;
use cc_model::{LedRail, Standard};

use crate::job::{CarcassSource, CvClass, CvjCabinet, PartRole};

/// Tolerancia rozmerov (mm).
const TOL: f64 = 0.5;

/// Otvorená nika (CV REF skr. 7 „S_KUCH_NASTENNA NIKA“): horná skrinka s pohľadovým chrbtom
/// `FB` a dnom `BO` pod bokmi cez celú šírku korpusu (bežná horná má dno medzi bokmi).
pub(crate) fn is_niche(c: &CvjCabinet) -> bool {
    if c.class != CvClass::Upper || c.carcass_source != CarcassSource::Parts {
        return false;
    }
    let carcass = || c.parts.iter().filter(|p| p.role == PartRole::Carcass);
    let code = |p: &&crate::job::CvjPart, k: &str| p.code.trim().eq_ignore_ascii_case(k);
    let width = c.carcass.size().x;
    let finished_back = carcass().any(|p| code(&p, "FB"));
    let full_bottom = carcass().any(|p| {
        code(&p, "BO") && p.bbox.min.y <= c.carcass.min.y + TOL && p.bbox.size().x >= width - TOL
    });
    finished_back && full_bottom
}

/// LED rampa: samostatný diel `BO` s menom „LED RAMPA“ (aj „NES_LED RAMPA“) pod skrinkou,
/// ručne pridaný v CV (bez `PID`). Poloha voči ľavej hrane korpusu, hĺbka od zadnej strany;
/// hodnoty zhodné s predvolenými štandardu sa vynechajú.
pub(crate) fn led_rail(
    c: &CvjCabinet,
    label: &str,
    standard: Option<&Standard>,
    notes: &mut Vec<String>,
) -> Option<LedRail> {
    if c.class != CvClass::Upper {
        return None;
    }
    let mut rails = c.parts.iter().filter(|p| {
        p.role == PartRole::Loose
            && p.code.trim().eq_ignore_ascii_case("BO")
            && p.description.to_uppercase().contains("LED RAMPA")
    });
    let p = rails.next()?;
    if rails.next().is_some() {
        notes.push(tr!(
            "import_cvj.led.several",
            label = label,
            name = p.description.trim()
        ));
    }
    let size = p.bbox.size();
    let (x, length, depth) = (r2(p.bbox.min.x - c.carcass.min.x), r2(size.x), r2(size.z));
    if ![x, length, depth].iter().all(|v| v.is_finite()) || length < 1.0 || depth < 1.0 {
        notes.push(tr!("import_cvj.led.bad_size", label = label));
        return None;
    }
    if (p.bbox.min.z - c.carcass.min.z).abs() > TOL || (p.bbox.max.y - c.carcass.min.y).abs() > TOL
    {
        notes.push(tr!("import_cvj.led.moved", label = label));
    }
    let w = r2(c.carcass.size().x);
    let d = r2(c.carcass.size().z);
    let default_depth = standard
        .and_then(|s| s.led_rail.as_ref())
        .map(|l| d - l.depth_minus);
    Some(LedRail {
        x: if x.abs() < 0.01 { 0.0 } else { x },
        length: Some(length).filter(|l| (l - w).abs() > 0.01),
        depth: Some(depth).filter(|v| default_depth.is_none_or(|dd| (v - dd).abs() > 0.01)),
    })
}

fn r2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::job::{Bbox, CvjAttributes, CvjPart};
    use glam::{DAffine3, DVec3};

    fn part(code: &str, desc: &str, role: PartRole, min: DVec3, max: DVec3) -> CvjPart {
        CvjPart {
            code: code.into(),
            description: desc.into(),
            pid: None,
            size: max - min,
            position: min,
            rotation: DVec3::ZERO,
            to_cabinet: DAffine3::IDENTITY,
            bbox: Bbox { min, max },
            role,
            band: None,
            ncid: None,
        }
    }

    fn cabinet(parts: Vec<CvjPart>) -> CvjCabinet {
        let carcass = parts
            .iter()
            .filter(|p| p.role == PartRole::Carcass)
            .map(|p| p.bbox)
            .fold(None::<Bbox>, |acc, b| {
                Some(match acc {
                    None => b,
                    Some(a) => Bbox {
                        min: a.min.min(b.min),
                        max: a.max.max(b.max),
                    },
                })
            })
            .unwrap();
        CvjCabinet {
            object_id: 1,
            name: "S_KUCH_NASTENNA NIKA".into(),
            description: String::new(),
            comment: None,
            class: CvClass::Upper,
            type_code: 0,
            number: Some(7),
            position: DVec3::ZERO,
            size: carcass.size(),
            rotation: DVec3::ZERO,
            toe_height: None,
            wall: None,
            face: None,
            world: DAffine3::IDENTITY,
            in_face: None,
            doors: 0,
            drawer_fronts: 0,
            drawer_boxes: 0,
            openings: 0,
            hardware: 0,
            connectors: Vec::new(),
            shelves: 0,
            carcass_part_count: parts.len(),
            parts,
            carcass,
            carcass_source: CarcassSource::Parts,
            sections: Vec::new(),
            attributes: CvjAttributes::default(),
        }
    }

    /// Nika REF skr. 7 (rám skrinky CV: X doprava, Y hore, Z dopredu) a bežná horná skrinka 8.
    #[test]
    fn niche_is_recognized_by_full_bottom_and_finished_back() {
        let c = PartRole::Carcass;
        let v = DVec3::new;
        let niche = cabinet(vec![
            part(
                "LF",
                "NES_Lavy BOK B",
                c,
                v(0.0, 19.0, -19.0),
                v(19.0, 682.0, 332.0),
            ),
            part(
                "RF",
                "NES_Pravy BOK B",
                c,
                v(466.0, 19.0, -19.0),
                v(485.0, 682.0, 332.0),
            ),
            part(
                "FB",
                "Pohladovy chrbat",
                c,
                v(19.0, 19.0, 0.0),
                v(466.0, 682.0, 19.0),
            ),
            part(
                "TO",
                "NES_Vrch",
                c,
                v(0.0, 682.0, -19.0),
                v(485.0, 701.0, 332.0),
            ),
            part(
                "BO",
                "NES_Dno",
                c,
                v(0.0, 0.0, -19.0),
                v(485.0, 19.0, 332.0),
            ),
        ]);
        assert!(is_niche(&niche));
        let upper = cabinet(vec![
            part(
                "LU",
                "Lavy BOK A",
                c,
                v(0.0, 0.0, 20.5),
                v(18.0, 663.0, 330.0),
            ),
            part(
                "RU",
                "Pravy BOK A",
                c,
                v(558.5, 0.0, 20.5),
                v(576.5, 663.0, 330.0),
            ),
            part(
                "UB",
                "Nepohladovy chrbat",
                c,
                v(1.0, 10.0, 16.5),
                v(575.5, 671.0, 20.5),
            ),
            part("TO", "Vrch", c, v(0.0, 663.0, 0.0), v(576.5, 682.0, 330.0)),
            part("BO", "Dno", c, v(18.0, 0.0, 0.0), v(558.5, 18.0, 330.0)),
        ]);
        assert!(!is_niche(&upper));
    }

    /// 0065 „NES_LED RAMPA“ 1153 × 310 × 19 pod skrinkou 8 (a 9): Y −19 … 0, Z 0 … 310.
    #[test]
    fn led_rail_from_loose_part() {
        let c = PartRole::Carcass;
        let v = DVec3::new;
        let mut parts = vec![
            part(
                "LU",
                "Lavy BOK A",
                c,
                v(0.0, 0.0, 20.5),
                v(18.0, 663.0, 330.0),
            ),
            part(
                "RU",
                "Pravy BOK A",
                c,
                v(558.5, 0.0, 20.5),
                v(576.5, 663.0, 330.0),
            ),
            part("TO", "Vrch", c, v(0.0, 663.0, 0.0), v(576.5, 682.0, 330.0)),
        ];
        parts.push(part(
            "BO",
            "NES_LED RAMPA",
            PartRole::Loose,
            v(0.0, -19.0, 0.0),
            v(1153.0, 0.0, 310.0),
        ));
        let cab = cabinet(parts);
        let mut notes = Vec::new();
        let rail = led_rail(&cab, "HO8", None, &mut notes).unwrap();
        assert_eq!(rail.x, 0.0);
        assert_eq!(rail.length, Some(1153.0));
        assert_eq!(rail.depth, Some(310.0));
        assert!(notes.is_empty(), "{notes:?}");
        // sokel ani iný samostatný diel nie je LED rampa
        let cab = cabinet(vec![
            part(
                "LU",
                "Lavy BOK A",
                c,
                v(0.0, 0.0, 20.5),
                v(18.0, 663.0, 330.0),
            ),
            part(
                "RU",
                "Pravy BOK A",
                c,
                v(558.5, 0.0, 20.5),
                v(576.5, 663.0, 330.0),
            ),
            part(
                "BO",
                "BLENDA LED DNO",
                PartRole::Loose,
                v(576.5, -19.0, 0.0),
                v(626.5, 0.0, 333.0),
            ),
        ]);
        assert!(led_rail(&cab, "HO9", None, &mut notes).is_none());
    }
}
