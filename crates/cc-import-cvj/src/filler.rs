//! Blenda (výplň) v zákazke CV → `filler` skrinky (findings 11 §5.3, 12 §2).
//!
//! Dva tvary z REF:
//! - **samostatná blenda** (katalógová BLENDA, skr. 5): jediný diel korpusu je zvislá lišta
//!   (tenká v X) a čelo otvoru je širšie (presah `OLR`/`OLL`) → skrinka bez korpusu s `filler`;
//!   čelo blendy robí generátor z `front_width` (otvor CV užší ako najmenšie čelo sa ako
//!   `fronts` nedá postaviť);
//! - **blenda pri skrinke** (skr. 9): za bokom skrinky sú voľné diely (mimo skupiny `Case`):
//!   zvislá lišta pri boku, čelo blendy v rovine čiel pred lištou a voliteľne dno blendy pod
//!   korpusom; vrch korpusu môže blendu prekrývať (626,5 = 576,5 + 50). Korpus sa vtedy
//!   zúži po vonkajšie plochy bokov a blenda ide do `filler`.
//!
//! Rozpoznáva sa podľa geometrie (rám skrinky CV: X doprava, Y hore, Z dopredu), nie podľa
//! mien dielov (tie zadáva projektant).

use cc_i18n::tr;
use cc_model::{FillerSide, FillerSpec};

use crate::job::{Bbox, CarcassSource, CvjCabinet, CvjPart, PartRole};

/// Tolerancia dotyku a zarovnania (mm).
const TOL: f64 = 0.5;
/// Diel s menším rozmerom nie je doska (model kovania).
const MIN_SOLID: f64 = 0.5;

/// Rozpoznaná blenda.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Detected {
    pub spec: FillerSpec,
    /// Obálka korpusu bez blendy (blenda pri skrinke); `None` pri samostatnej blende.
    pub carcass: Option<Bbox>,
}

impl Detected {
    /// Skrinka so zúženým korpusom (bez blendy) pre ďalší prevod; `None`, ak sa nemení.
    pub fn narrowed(&self, c: &CvjCabinet) -> Option<CvjCabinet> {
        let carcass = self.carcass?;
        let mut n = c.clone();
        n.carcass = carcass;
        Some(n)
    }

    pub fn is_standalone(&self) -> bool {
        self.carcass.is_none()
    }

    /// Popis do poznámok importu.
    pub fn describe(&self) -> String {
        let side = match self.spec.side {
            FillerSide::Left => tr!("import_cvj.filler.left"),
            FillerSide::Right => tr!("import_cvj.filler.right"),
        };
        if self.is_standalone() {
            return tr!("import_cvj.filler.standalone", side = side);
        }
        tr!(
            "import_cvj.filler.attached",
            side = side,
            width = format!("{:.1}", self.spec.width),
            front = format!("{:.1}", self.spec.front_width.unwrap_or(self.spec.width)),
            bottom = if self.spec.bottom {
                tr!("import_cvj.filler.bottom")
            } else {
                String::new()
            }
        )
    }
}

/// Blenda skrinky, ak ju skrinka má.
pub(crate) fn detect(c: &CvjCabinet) -> Option<Detected> {
    standalone(c).or_else(|| attached(c))
}

fn solid(p: &CvjPart) -> bool {
    let s = p.bbox.size();
    s.is_finite() && s.min_element() >= MIN_SOLID
}

/// Zvislá doska tenká v X (bok, lišta).
fn thin_x(b: &Bbox) -> bool {
    let s = b.size();
    s.x < s.y && s.x < s.z
}

fn union(boxes: impl Iterator<Item = Bbox>) -> Option<Bbox> {
    boxes.reduce(|a, b| Bbox {
        min: a.min.min(b.min),
        max: a.max.max(b.max),
    })
}

/// Samostatná blenda: jediný diel korpusu je lišta a čelo je širšie.
fn standalone(c: &CvjCabinet) -> Option<Detected> {
    if c.carcass_source != CarcassSource::Parts || c.carcass_part_count != 1 {
        return None;
    }
    let strip = c
        .parts
        .iter()
        .find(|p| p.role == PartRole::Carcass && solid(p))?;
    if !thin_x(&strip.bbox) {
        return None;
    }
    let front = union(
        c.face_fronts()
            .map(|f| f.bbox)
            .filter(|b| b.min.is_finite() && b.max.is_finite()),
    )?;
    let s = strip.bbox;
    let side = if front.min.x < s.min.x - TOL && front.max.x <= s.max.x + TOL {
        FillerSide::Left
    } else {
        FillerSide::Right
    };
    // šírka čela od plochy lišty pri susedovi (78 = 18 + OLR 60)
    let front_width = match side {
        FillerSide::Right => front.max.x - s.min.x,
        FillerSide::Left => s.max.x - front.min.x,
    };
    Some(Detected {
        spec: FillerSpec {
            side,
            front_width: Some(round(front_width)).filter(|w| *w > 0.0),
            ..FillerSpec::default()
        },
        carcass: None,
    })
}

/// Blenda za bokom skrinky z voľných dielov.
fn attached(c: &CvjCabinet) -> Option<Detected> {
    let carcass: Vec<&CvjPart> = c
        .parts
        .iter()
        .filter(|p| p.role == PartRole::Carcass && solid(p))
        .collect();
    let code = |p: &CvjPart| p.code.trim().to_uppercase();
    let lefts = union(
        carcass
            .iter()
            .filter(|p| matches!(code(p).as_str(), "LU" | "LF"))
            .map(|p| p.bbox),
    )?;
    let rights = union(
        carcass
            .iter()
            .filter(|p| matches!(code(p).as_str(), "RU" | "RF"))
            .map(|p| p.bbox),
    )?;
    let (x_lo, x_hi) = (lefts.min.x, rights.max.x);
    let sides_bottom = lefts.min.y.min(rights.min.y);
    let side_h = lefts.size().y.max(rights.size().y);
    if !(x_hi - x_lo > 1.0 && side_h > 1.0) {
        return None;
    }
    let loose: Vec<&CvjPart> = c
        .parts
        .iter()
        .filter(|p| p.role == PartRole::Loose && solid(p))
        .collect();
    for side in [FillerSide::Right, FillerSide::Left] {
        // diel celý za bokom a pri ňom
        let next_to = |b: &Bbox| match side {
            FillerSide::Right => (b.min.x - x_hi).abs() <= TOL,
            FillerSide::Left => (b.max.x - x_lo).abs() <= TOL,
        };
        let Some(strip) = loose
            .iter()
            .map(|p| p.bbox)
            .find(|b| next_to(b) && thin_x(b) && b.size().y >= 0.5 * side_h)
        else {
            continue;
        };
        // čelo blendy: tenké v Z, pred lištou
        let Some(front) = loose.iter().map(|p| p.bbox).find(|b| {
            let s = b.size();
            next_to(b) && s.z < s.x && s.z < s.y && b.min.z >= strip.max.z - TOL
        }) else {
            continue;
        };
        // dno blendy: tenké v Y, pod spodkom bokov
        let bottom = loose.iter().map(|p| p.bbox).find(|b| {
            let s = b.size();
            next_to(b) && s.y < s.x && s.y < s.z && b.max.y <= sides_bottom + TOL
        });
        // vrch korpusu cez blendu
        let overlap = match side {
            FillerSide::Right => c.carcass.max.x - x_hi,
            FillerSide::Left => x_lo - c.carcass.min.x,
        }
        .max(0.0);
        let width = [overlap, bottom.map_or(0.0, |b| b.size().x), strip.size().x]
            .into_iter()
            .fold(0.0, f64::max);
        let mut narrowed = c.carcass;
        narrowed.min.x = x_lo;
        narrowed.max.x = x_hi;
        return Some(Detected {
            spec: FillerSpec {
                side,
                width: round(width),
                front_width: Some(round(front.size().x)),
                bottom: bottom.is_some(),
                dowels: true,
            },
            carcass: Some(narrowed),
        });
    }
    None
}

/// Zaokrúhlenie na desatiny mm (CV má polohy dielov s šumom, napr. lišta na X −0,018).
fn round(v: f64) -> f64 {
    (v * 10.0).round() / 10.0 + 0.0
}
