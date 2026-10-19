//! Otvorená nika a LED rampa horných skriniek (zákazka CV REF, findings 12 §2–3):
//! časti štandardu `[niche]`, `[led_rail]` a skupiny kolíkov pri koncoch hrany v pravidle spoja.

use serde::{Deserialize, Serialize};

/// Otvorená nika dielne (CV REF skr. 7 „S_KUCH_NASTENNA NIKA“): všetky diely z jednej dosky,
/// vrch naložený na boky, dno pod bokmi (obe cez celú šírku a hĺbku), pohľadový chrbát
/// vložený medzi boky, vrch a dno, odsadený od zadnej hrany bokov (miesto pre záves).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NicheStandard {
    /// Materiál všetkých dielov niky (boky, vrch, dno, chrbát, police).
    pub material: String,
    /// Zadná plocha pohľadového chrbta pred zadnou hranou bokov.
    pub back_from_rear: f64,
    /// Závesné kovanie: diery do vnútornej plochy oboch bokov (`from_edge` od zadnej hrany boku).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hanger_holes: Vec<NicheHole>,
    /// Otvory v chrbte pri oboch bokoch (`from_edge` od bočnej hrany chrbta), bez hĺbky cez
    /// celý chrbát (prístup ku skrutkám závesu).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub back_holes: Vec<NicheHole>,
    /// Dno niky je zároveň LED rampa: drážka `[led_rail] groove` zospodu.
    #[serde(default, skip_serializing_if = "is_false")]
    pub led_groove: bool,
}

/// Diera dielu niky: `from_top` od hornej hrany dielu, `from_edge` od hrany podľa použitia.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NicheHole {
    pub from_top: f64,
    pub from_edge: f64,
    pub diameter: f64,
    /// Bez hodnoty cez celý diel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<f64>,
}

/// LED rampa pod hornými skrinkami (CV REF: „LED RAMPA“, „NES_LED RAMPA“; findings 12 §2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LedRailStandard {
    pub material: String,
    /// Predvolená hĺbka rampy = hĺbka skrinky − `depth_minus` (zadná hrana pri stene).
    #[serde(default)]
    pub depth_minus: f64,
    /// Drážka pre LED profil zospodu, cez celú dĺžku.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub groove: Option<LedGroove>,
}

/// Drážka pre LED profil: zadná hrana drážky `from_rear` od zadnej strany skrinky (steny),
/// takže drážka ide v jednej línii pod skrinkami rôznej hĺbky aj pod nikou.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LedGroove {
    pub from_rear: f64,
    pub width: f64,
    pub depth: f64,
}

/// Kolíky pravidla spoja v dvoch skupinách kolík – konfirmát – kolík (rozostup `pair_pitch`)
/// pri koncoch hrany dielu `edge` namiesto vzoru štandardu. Vzdialenosť po najbližší kolík
/// od začiatku hrany (spodný, ľavý alebo predný koniec) a od jej konca.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EndGroups {
    /// Rola dielu, ktorého hrana nesie skupiny (meno z pravidiel, napr. `finished_back`).
    pub edge: String,
    pub from_start: f64,
    pub from_end: f64,
    /// Iba pre tento druh skrinky (napr. `wall` = nika); inak pre všetky. Pravidlá spojov tej
    /// istej dvojice rolí sa prepisujú, preto sa skupiny dajú obmedziť (šikmá spodná skrinka
    /// má pohľadový chrbát so vzorom 3/3/3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cabinet: Option<crate::CabinetType>,
}

fn is_false(v: &bool) -> bool {
    !*v
}
