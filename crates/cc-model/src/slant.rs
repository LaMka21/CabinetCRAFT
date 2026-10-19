//! Šikmá skrinka (CV REF „S_KUCH_D SIKMA“, „S_KUCH_NASTENNA SIKMA“): jeden bok je
//! v čelnom pohľade zošikmený, dno a vrch končia na jeho vnútornej ploche.
//!
//! Tvar skrinky je kváder `w × h × d` ako pri rovnej skrinke; šikmý bok je daný priamkou
//! svojej vonkajšej plochy v čelnom pohľade: vzdialenosť od hrany skrinky pri spodku
//! (`bottom`) a pri vrchu korpusu (`top`). Ak priamka pri vrchu siaha za druhý bok,
//! skrinka nemá vrch a bok končí v rohu pri druhom boku (trojuholník, horná REF).

use serde::{Deserialize, Serialize};

/// Ktorý bok skrinky je šikmý.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlantSide {
    #[default]
    Left,
    Right,
}

/// Šikmý bok skrinky (údaje skrinky v `project.toml`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Slant {
    #[serde(default, skip_serializing_if = "is_left")]
    pub side: SlantSide,
    /// Vonkajšia plocha šikmého boku pri spodku korpusu: vzdialenosť od hrany skrinky na
    /// strane šikmého boku (mm, záporná = bok mierne presahuje skrinku).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub bottom: f64,
    /// To isté pri vrchu korpusu (mm); `top − bottom` je vodorovný posun boku po výške.
    pub top: f64,
    /// O koľko šikmý bok presahuje dozadu za skrinku (mm; CV REF spodná: 60 až k stene).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub extend_back: f64,
    /// Pohľadový chrbát z dosky korpusu (CV `FB`, `FINBK`): korpus je o jeho hrúbku plytší
    /// a dno, bok a vrch sú k nemu spojené kolíkmi. Bez neho chrbát podľa štandardu.
    #[serde(default, skip_serializing_if = "is_false")]
    pub finished_back: bool,
}

/// Pravidlá šikmej skrinky v štandarde dielne (`[slanted]`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct SlantStandard {
    /// Materiál šikmého boku (pohľadový); bez hodnoty materiál korpusu.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub side_material: Option<String>,
    /// Dno a vrch nad spodkom a pod vrchom rovného boku (mm).
    pub end_inset: f64,
    /// Chrbát v drážke: o koľko zasahuje do dna (a vrchu), mm.
    pub back_insert_bottom: f64,
    /// Chrbát v drážke: o koľko zasahuje do šikmého boku (kolmo na bok), mm.
    pub back_insert_side: f64,
    /// Najkratší vrch: ak by bol kratší, skrinka vrch nemá (bok končí v rohu), mm.
    pub min_top: f64,
    /// Najväčší sklon boku od zvislice (°).
    pub max_angle: f64,
}

impl Default for SlantStandard {
    fn default() -> Self {
        SlantStandard {
            side_material: None,
            end_inset: 0.0,
            back_insert_bottom: 8.0,
            back_insert_side: 8.0,
            min_top: 50.0,
            max_angle: 75.0,
        }
    }
}

fn is_left(s: &SlantSide) -> bool {
    *s == SlantSide::Left
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

fn is_false(v: &bool) -> bool {
    !*v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toml_roundtrip() {
        let s = Slant {
            side: SlantSide::Left,
            bottom: -0.265,
            top: 449.022,
            extend_back: 60.0,
            finished_back: true,
        };
        let text = toml::to_string(&s).unwrap();
        assert!(!text.contains("side"));
        let back: Slant = toml::from_str(&text).unwrap();
        assert_eq!(back, s);
        let std: SlantStandard = toml::from_str("end_inset = 0.5").unwrap();
        assert_eq!(std.end_inset, 0.5);
        assert_eq!(std.min_top, 50.0);
    }
}
