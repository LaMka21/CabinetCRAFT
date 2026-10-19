//! Blenda (výplň) pri skrinke: kolmá lišta za čelom blendy a čelo blendy, ktoré je širšie
//! ako lišta (presah do strany, `OLR` v CV). Podľa `findings/11-atributy-skriniek.md` §5.3
//! (katalógová BLENDA, zákazka REF skr. 5) a `findings/12-parita-ref.md` §2 (horná skrinka
//! REF skr. 9 s blendou vpravo: lišta, čelo blendy, dno blendy pod LED rampu, predĺžený vrch).
//!
//! Dva prípady:
//! - **pri skrinke s korpusom** (`carcass = true`): blenda leží za bokom skrinky na strane
//!   `side`, široká `width`; generátor pridá lištu, čelo blendy, voliteľne dno blendy a predĺži
//!   vrch, ktorý leží na bokoch;
//! - **samostatná blenda** (skrinka bez korpusu, `carcass = false`): skrinka je sama lišta
//!   (šírka skrinky = hrúbka lišty), slepé čelo blendy je širšie o presah do strany `side`
//!   (v CV `OLR 60`); so zadanými `fronts` sa použijú tie.

use serde::{Deserialize, Serialize};

/// Strana skrinky, na ktorej je blenda (pri samostatnej blende strana presahu čela).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FillerSide {
    Left,
    #[default]
    Right,
}

/// Blenda pri skrinke (`[cabinet.filler]`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FillerSpec {
    /// Strana skrinky s blendou.
    #[serde(default)]
    pub side: FillerSide,
    /// Šírka blendy za bokom korpusu (medzera, ktorú blenda vypĺňa; REF skr. 9: 50).
    /// Pri samostatnej blende sa nepoužije (lišta je v šírke skrinky).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub width: f64,
    /// Šírka čela blendy od vonkajšej plochy boku (REF skr. 9: 60), pri samostatnej blende od
    /// plochy lišty pri susedovi (REF skr. 5: 78); bez hodnoty `width + [filler] front_overhang`,
    /// resp. šírka skrinky + `standalone_front_overhang`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub front_width: Option<f64>,
    /// Dno blendy pod lištou (horná skrinka s LED rampou: doska hrúbky čela v rovine pod dnom).
    #[serde(default, skip_serializing_if = "is_false")]
    pub bottom: bool,
    /// Kolíky lišty do čela a do dna blendy (`false`: diely bez dier, napr. keď sú menšie
    /// ako minimum stroja a vŕtajú sa ručne).
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub dowels: bool,
}

impl Default for FillerSpec {
    fn default() -> Self {
        FillerSpec {
            side: FillerSide::Right,
            width: 0.0,
            front_width: None,
            bottom: false,
            dowels: true,
        }
    }
}

fn yes() -> bool {
    true
}

fn is_true(v: &bool) -> bool {
    *v
}

fn is_false(v: &bool) -> bool {
    !*v
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

/// Rad kolíkov lišty blendy: krajné `end` od koncov spoja, medzi nimi rovnomerne.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct FillerDowels {
    /// Krajný kolík od začiatku a konca spoja.
    pub end: f64,
    /// Pevný počet kolíkov; 0 = podľa `max_spacing`.
    pub count: u32,
    /// Najväčší rozostup kolíkov pri `count = 0`.
    pub max_spacing: f64,
}

impl Default for FillerDowels {
    fn default() -> Self {
        FillerDowels {
            end: 50.0,
            count: 0,
            max_spacing: 200.0,
        }
    }
}

/// Najviac kolíkov v jednom rade (ochrana pred nezmyselnými číslami).
const MAX_FILLER_DOWELS: usize = 64;

impl FillerDowels {
    /// Polohy kolíkov od začiatku spoja dĺžky `len`; prázdne, ak sa spoj nedá rozmiestniť.
    pub fn positions(&self, len: f64) -> Vec<f64> {
        if !(len.is_finite() && len > 0.0 && self.end.is_finite() && self.end >= 0.0) {
            return Vec::new();
        }
        let span = len - 2.0 * self.end;
        if span < 0.0 {
            return Vec::new();
        }
        let n = if self.count > 0 {
            self.count as usize
        } else if self.max_spacing.is_finite() && self.max_spacing > 0.0 {
            // počet medzier = ⌈rozpätie / max. rozostup⌉, aspoň 1
            let gaps = (span / self.max_spacing - 1e-6).ceil().max(1.0);
            if gaps > MAX_FILLER_DOWELS as f64 {
                MAX_FILLER_DOWELS
            } else {
                gaps as usize + 1
            }
        } else {
            return Vec::new();
        };
        let n = n.min(MAX_FILLER_DOWELS);
        match n {
            0 => Vec::new(),
            1 => vec![len / 2.0],
            n => (0..n)
                .map(|i| self.end + span * i as f64 / (n - 1) as f64)
                .collect(),
        }
    }
}

/// Blenda v štandarde dielne (`[filler]`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct FillerStandard {
    /// Čelo blendy pri skrinke je o toľko pred lícom korpusu; lišta siaha až po čelo.
    pub front_gap: f64,
    /// Čelo samostatnej blendy od prednej hrany lišty.
    pub standalone_front_gap: f64,
    /// Lišta končí o toľko pod vrchom boku.
    pub strip_top_gap: f64,
    /// Čelo blendy je o toľko širšie ako blenda (bez `front_width` pri skrinke).
    pub front_overhang: f64,
    /// Presah čela samostatnej blendy cez lištu (bez `front_width`).
    pub standalone_front_overhang: f64,
    /// Vrch, ktorý leží na bokoch, sa predĺži cez blendu.
    pub extend_top: bool,
    /// Kolíky lišty do čela a do dna blendy pri skrinke.
    pub dowels: FillerDowels,
    /// Kolíky lišty do čela samostatnej blendy.
    pub standalone_dowels: FillerDowels,
}

impl Default for FillerStandard {
    fn default() -> Self {
        FillerStandard {
            front_gap: 2.0,
            standalone_front_gap: 0.0,
            strip_top_gap: 2.0,
            front_overhang: 10.0,
            standalone_front_overhang: 60.0,
            extend_top: true,
            dowels: FillerDowels::default(),
            standalone_dowels: FillerDowels {
                end: 32.0,
                count: 3,
                max_spacing: 0.0,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dowel_rows_like_cv_ref() {
        // [P] REF skr. 9: lišta 309,5 → 50 / 154,75 / 259,5, 661 → 50 / 237 / 424 / 611
        let d = FillerDowels::default();
        assert_eq!(d.positions(309.5), vec![50.0, 154.75, 259.5]);
        assert_eq!(d.positions(661.0), vec![50.0, 237.0, 424.0, 611.0]);
        // [P] REF skr. 5: lišta 762 → 32 / 381 / 730
        let s = FillerStandard::default().standalone_dowels;
        assert_eq!(s.positions(762.0), vec![32.0, 381.0, 730.0]);
    }

    #[test]
    fn hostile_numbers_give_no_dowels() {
        let d = FillerDowels::default();
        assert!(d.positions(f64::NAN).is_empty());
        assert!(d.positions(-5.0).is_empty());
        assert!(d.positions(60.0).is_empty()); // kratší ako 2 × end
        let huge = FillerDowels {
            end: 0.0,
            count: 0,
            max_spacing: 1e-9,
        };
        assert_eq!(huge.positions(1e6).len(), MAX_FILLER_DOWELS);
        let fixed = FillerDowels {
            end: 10.0,
            count: u32::MAX,
            max_spacing: 0.0,
        };
        assert_eq!(fixed.positions(100.0).len(), MAX_FILLER_DOWELS);
    }

    #[test]
    fn spec_toml_defaults() {
        let s: FillerSpec = toml::from_str("width = 50\nfront_width = 60\nbottom = true").unwrap();
        assert_eq!(s.side, FillerSide::Right);
        assert!(s.dowels);
        let text = toml::to_string(&s).unwrap();
        assert!(!text.contains("dowels"));
        let back: FillerSpec = toml::from_str(&text).unwrap();
        assert_eq!(back, s);
        let std: FillerStandard = toml::from_str("front_gap = 3").unwrap();
        assert_eq!(std.front_gap, 3.0);
        assert_eq!(std.standalone_dowels.count, 3);
    }
}
