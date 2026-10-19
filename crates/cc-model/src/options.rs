//! Atribúty skrinky (ako panel Attributes v CV, findings 11 §1): spoje, typ spojov,
//! policové kovanie, tlmiace čočky, nožičky, TIP-ON. Skrinka nesie iba zmenené hodnoty,
//! ostatné platia zo štandardu (`[cabinet_options]`).

use serde::{Deserialize, Serialize};

/// Typ spojovacieho kovania medzi kolíkmi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JointType {
    /// Iba kolíky.
    #[default]
    Dowels,
    /// Kolíky + konfirmát v strede každej dvojice kolíkov (diera do boku aj do hrany).
    Confirmat,
    /// Kolíky + vrut 3,5 × 50 v strede každej dvojice (iba diera do boku).
    Screw,
}

/// Podperky nastaviteľných políc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShelfSupport {
    /// Häfele Sekura 7: rad dier Ø5 a 4 podperky na policu.
    #[default]
    Sekura7,
    /// Bez podperiek a bez radu dier.
    None,
}

/// TIP-ON (otváranie zatlačením).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TipOn {
    #[default]
    Off,
    /// Závrtný.
    Drilled,
    /// S adaptérom.
    Adapter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TipOnColor {
    #[default]
    White,
    Grey,
    Black,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TipOnKind {
    /// S dorazom.
    #[default]
    Stop,
    /// S magnetom.
    Magnet,
}

/// Ručne zadaná diera spojenia so susednou skrinkou v ľavom boku (findings 12 §8):
/// poloha v ploche F boku, protikus je na tom istom mieste v pravom boku ľavej susedy.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinkHole {
    /// Od prednej hrany boku (mm).
    pub from_front: f64,
    /// Od spodnej hrany boku (mm).
    pub from_bottom: f64,
}

/// Atribúty pri skrinke; `None` = podľa štandardu.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CabinetOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub joints: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub joint_type: Option<JointType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shelf_support: Option<ShelfSupport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bumpers: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legs: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plinth: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tip_on: Option<TipOn>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tip_on_color: Option<TipOnColor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tip_on_kind: Option<TipOnKind>,
    /// Spojovacie kovanie skriniek medzi sebou: spojiť s **ľavou** susednou skrinkou
    /// (diery v ploche F ľavého boku a protikus v pravom boku suseda, findings 12 §8).
    /// Zatiaľ iba atribút (import z CV), generátor diery nerobí.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link_left: Option<bool>,
    /// Posun protikusu v susedovi (mm, napr. pri medzere medzi skrinkami); `None` = 0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link_shift: Option<f64>,
    /// Ručné polohy dier spojenia v ľavom boku; prázdne = podľa štandardu.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub link_holes: Vec<LinkHole>,
    /// Sokel s ručne zadanou polohou a dĺžkou (ako v CV: jeden kus pod celou zostavou,
    /// dĺžka bez rovnice, findings 12 §6). Bez hodnoty sokel podľa zostavy (`[plinth] runs`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plinth_span: Option<PlinthSpan>,
    /// Bočný sokel pod ľavým bokom skrinky; `None` = na odkrytom konci zostavy (`[plinth] side`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub side_plinth_left: Option<bool>,
    /// Bočný sokel pod pravým bokom skrinky; `None` = na odkrytom konci zostavy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub side_plinth_right: Option<bool>,
    /// Výška bočného sokla (mm); `None` = ako predný sokel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub side_plinth_height: Option<f64>,
    /// Krycí bok (KRYCI BOK) pri ľavom boku skrinky, od podlahy po pracovnú dosku (`[cover_side]`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover_side_left: Option<bool>,
    /// Krycí bok pri pravom boku skrinky.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover_side_right: Option<bool>,
}

/// Ručne zadaný sokel skrinky (v ráme skrinky pozdĺž osi x).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlinthSpan {
    /// Začiatok od ľavej hrany korpusu (mm; záporný = vľavo od skrinky).
    pub from: f64,
    /// Dĺžka sokla (mm).
    pub length: f64,
    /// Výška sokla (mm); bez hodnoty od podlahy po spodok skrinky mínus `[plinth] gap_top`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
}

impl CabinetOptions {
    pub fn is_empty(&self) -> bool {
        *self == CabinetOptions::default()
    }

    /// Hodnoty skrinky, chýbajúce zo štandardu.
    pub fn resolve(&self, d: &ResolvedOptions) -> ResolvedOptions {
        ResolvedOptions {
            joints: self.joints.unwrap_or(d.joints),
            joint_type: self.joint_type.unwrap_or(d.joint_type),
            shelf_support: self.shelf_support.unwrap_or(d.shelf_support),
            bumpers: self.bumpers.unwrap_or(d.bumpers),
            legs: self.legs.unwrap_or(d.legs),
            plinth: self.plinth.unwrap_or(d.plinth),
            tip_on: self.tip_on.unwrap_or(d.tip_on),
            tip_on_color: self.tip_on_color.unwrap_or(d.tip_on_color),
            tip_on_kind: self.tip_on_kind.unwrap_or(d.tip_on_kind),
            link_left: self.link_left.unwrap_or(d.link_left),
        }
    }
}

/// Platné atribúty skrinky (aj predvolené hodnoty štandardu `[cabinet_options]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct ResolvedOptions {
    /// Spojovacie kovanie (kolíky, konfirmáty); `false` napr. pri samostatnej doske.
    pub joints: bool,
    pub joint_type: JointType,
    pub shelf_support: ShelfSupport,
    /// Tlmiace (dorazové) čočky v dvierkach a čelách.
    pub bumpers: bool,
    /// Nožičky pod spodnými a vysokými skrinkami.
    pub legs: bool,
    /// Sokel (doska pred nožičkami) pod spodnými a vysokými skrinkami.
    pub plinth: bool,
    pub tip_on: TipOn,
    pub tip_on_color: TipOnColor,
    pub tip_on_kind: TipOnKind,
    /// Spojovacie kovanie skriniek s ľavou susedou (findings 12 §8); diery vŕta `cc-pipeline` (link.rs).
    pub link_left: bool,
}

impl Default for ResolvedOptions {
    fn default() -> Self {
        ResolvedOptions {
            joints: true,
            joint_type: JointType::Dowels,
            shelf_support: ShelfSupport::Sekura7,
            bumpers: false,
            legs: true,
            plinth: false,
            tip_on: TipOn::Off,
            tip_on_color: TipOnColor::White,
            tip_on_kind: TipOnKind::Stop,
            link_left: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_options_without_link_load() {
        let o: CabinetOptions = toml::from_str("joints = true\nlegs = false\n").unwrap();
        assert_eq!(o.link_left, None);
        assert_eq!(o.link_shift, None);
        assert!(o.link_holes.is_empty());
        let r: ResolvedOptions = toml::from_str("joints = true\n").unwrap();
        assert!(!r.link_left);
        assert!(!o.resolve(&r).link_left);
    }

    #[test]
    fn link_options_roundtrip_and_resolve() {
        let o = CabinetOptions {
            link_left: Some(true),
            link_shift: Some(2.5),
            link_holes: vec![
                LinkHole {
                    from_front: 100.0,
                    from_bottom: 100.0,
                },
                LinkHole {
                    from_front: 406.0,
                    from_bottom: 702.0,
                },
            ],
            ..Default::default()
        };
        assert!(!o.is_empty());
        let text = toml::to_string(&o).unwrap();
        let back: CabinetOptions = toml::from_str(&text).unwrap();
        assert_eq!(back, o);
        assert!(o.resolve(&ResolvedOptions::default()).link_left);
        // prázdne ručné polohy sa nezapisujú
        let empty = toml::to_string(&CabinetOptions::default()).unwrap();
        assert!(!empty.contains("link"));
    }
}
