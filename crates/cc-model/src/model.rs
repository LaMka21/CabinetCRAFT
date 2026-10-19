use serde::{Deserialize, Serialize};

use crate::{CabinetType, Edge, Grain, Operation, ResolvedOptions, Transform};

/// Rola dielu v skrinke.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    SideLeft,
    SideRight,
    Bottom,
    Top,
    StretcherFront,
    StretcherRear,
    Back,
    AdjustableShelf,
    FixedShelf,
    Door,
    DrawerFront,
    FalseFront,
    /// Dno korpusu zásuvky.
    DrawerBottom,
    /// Zadná stena korpusu zásuvky.
    DrawerBack,
    /// Sokel pred nožičkami (aj bočný sokel).
    Plinth,
    /// Lišta blendy: kolmá doska za čelom blendy (findings 11 §5.3, 12 §2).
    FillerStrip,
    /// Čelo blendy pri skrinke (v rovine čiel, presah do strany).
    FillerFront,
    /// Dno blendy pod lištou (horná skrinka s LED rampou).
    FillerBottom,
    /// Pohľadový chrbát z dosky korpusu (CV `FB`) spojený kolíkmi: šikmá skrinka (REF diel
    /// 0029) alebo vložený medzi boky otvorenej niky (REF skr. 7, findings 12 §3).
    FinishedBack,
    /// LED rampa: doska pod hornou skrinkou s drážkou pre LED profil (CV `BO` „LED RAMPA“,
    /// findings 12 §2); bez spojov s korpusom.
    LedRail,
    /// Krycí bok pri boku skrinky (KRYCI BOK).
    CoverSide,
}

impl Role {
    /// Mená rolí použiteľné v pravidlách spojov (kap. 4.3).
    pub const RULE_NAMES: [&'static str; 9] = [
        "side",
        "bottom",
        "top",
        "stretcher_front",
        "stretcher_rear",
        "back",
        "adjustable_shelf",
        "fixed_shelf",
        "finished_back",
    ];

    /// Meno v rozpise dielov.
    pub fn as_str(self) -> &'static str {
        match self {
            Role::SideLeft => "side_left",
            Role::SideRight => "side_right",
            Role::Bottom => "bottom",
            Role::Top => "top",
            Role::StretcherFront => "stretcher_front",
            Role::StretcherRear => "stretcher_rear",
            Role::Back => "back",
            Role::AdjustableShelf => "adjustable_shelf",
            Role::FixedShelf => "fixed_shelf",
            Role::Door => "door",
            Role::DrawerFront => "drawer_front",
            Role::FalseFront => "false_front",
            Role::DrawerBottom => "drawer_bottom",
            Role::DrawerBack => "drawer_back",
            Role::Plinth => "plinth",
            Role::FillerStrip => "filler_strip",
            Role::FillerFront => "filler_front",
            Role::FillerBottom => "filler_bottom",
            Role::FinishedBack => "finished_back",
            Role::LedRail => "led_rail",
            Role::CoverSide => "cover_side",
        }
    }

    /// Čelo (dvierka, čelo zásuvky, slepé čelo): nie je spojené s korpusom spojmi.
    pub fn is_front(self) -> bool {
        matches!(self, Role::Door | Role::DrawerFront | Role::FalseFront)
    }

    /// Diel korpusu zásuvky (nie je spojený s korpusom skrinky).
    pub fn is_drawer_box(self) -> bool {
        matches!(self, Role::DrawerBottom | Role::DrawerBack)
    }

    /// Diel blendy (lišta, čelo a dno blendy); spoje im robí `cc-joints` zvlášť.
    pub fn is_filler(self) -> bool {
        matches!(
            self,
            Role::FillerStrip | Role::FillerFront | Role::FillerBottom
        )
    }

    /// Diel bez spojov s korpusom podľa pravidiel (čelá, zásuvky, sokel, krycí bok, blenda,
    /// LED rampa).
    pub fn is_loose(self) -> bool {
        self.is_front()
            || self.is_drawer_box()
            || matches!(self, Role::Plinth | Role::CoverSide | Role::LedRail)
            || self.is_filler()
    }

    /// Meno v pravidlách spojov; ľavý a pravý bok sú obidva `side`.
    pub fn rule_name(self) -> &'static str {
        match self {
            Role::SideLeft | Role::SideRight => "side",
            other => other.as_str(),
        }
    }

    /// Skratka v ID dielu (`S1-SIDE-L`, `S1-ASHELF-1`).
    pub fn id_code(self) -> &'static str {
        match self {
            Role::SideLeft => "SIDE-L",
            Role::SideRight => "SIDE-R",
            Role::Bottom => "BOTTOM",
            Role::Top => "TOP",
            Role::StretcherFront => "STR-F",
            Role::StretcherRear => "STR-R",
            Role::Back => "BACK",
            Role::AdjustableShelf => "ASHELF",
            Role::FixedShelf => "FSHELF",
            Role::Door => "DOOR",
            Role::DrawerFront => "DRW",
            Role::FalseFront => "FF",
            Role::DrawerBottom => "DRWB",
            Role::DrawerBack => "DRWK",
            Role::Plinth => "SOKEL",
            Role::FillerStrip => "BL-LISTA",
            Role::FillerFront => "BL-CELO",
            Role::FillerBottom => "BL-DNO",
            Role::FinishedBack => "FBACK",
            Role::LedRail => "LED",
            Role::CoverSide => "COVER",
        }
    }
}

/// Olepenie hrán dielu (id hranovej pásky alebo `None`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edges {
    pub x0: Option<String>,
    pub x1: Option<String>,
    pub y0: Option<String>,
    pub y1: Option<String>,
}

impl Edges {
    pub fn get(&self, edge: Edge) -> Option<&str> {
        match edge {
            Edge::X0 => self.x0.as_deref(),
            Edge::X1 => self.x1.as_deref(),
            Edge::Y0 => self.y0.as_deref(),
            Edge::Y1 => self.y1.as_deref(),
        }
    }

    pub fn set(&mut self, edge: Edge, band: Option<String>) {
        let slot = match edge {
            Edge::X0 => &mut self.x0,
            Edge::X1 => &mut self.x1,
            Edge::Y0 => &mut self.y0,
            Edge::Y1 => &mut self.y1,
        };
        *slot = band;
    }
}

/// Obdĺžnikový doskový diel `length × width × thickness` (kap. 3.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Part {
    pub id: String,
    pub cabinet: String,
    pub role: Role,
    pub material: String,
    pub length: f64,
    pub width: f64,
    pub thickness: f64,
    pub grain: Grain,
    pub edges: Edges,
    /// Súradnice dielu → súradnice skrinky. Pri polohovateľnej polici je
    /// poloha iba nominálna (police nie sú v MVP obrábané).
    pub transform: Transform,
    #[serde(default)]
    pub operations: Vec<Operation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CabinetModel {
    pub id: String,
    pub cabinet_type: CabinetType,
    pub w: f64,
    pub h: f64,
    pub d: f64,
    pub parts: Vec<Part>,
    /// Kovanie skrinky (pánty…) s polohou; obrábanie z neho robí `cc-joints`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hardware: Vec<Hardware>,
    /// Platné atribúty skrinky (pre obrábanie a súpis kovania; v `model.json` nie sú).
    #[serde(skip)]
    pub options: ResolvedOptions,
    /// Otvory čiel (sekcie): kde je ktorý riadok a stĺpec `fronts` v ráme skrinky, aj otvorený
    /// bez čela. Pre výber otvoru v pohľade (editor sekcií); v `model.json` nie sú.
    #[serde(skip)]
    pub openings: Vec<FrontOpening>,
}

/// Otvor čela v ráme skrinky: riadok `fronts` (zhora od 0), stĺpec riadku (ak je delený),
/// ľavý dolný roh a rozmer plochy čela (bez úprav rozmeru).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrontOpening {
    pub row: usize,
    pub column: Option<usize>,
    pub x: f64,
    pub z: f64,
    pub w: f64,
    pub h: f64,
}

/// Kus kovania v skrinke (súradnice skrinky).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Hardware {
    /// Pánt: miska v dvierkach, podložka na boku.
    Hinge {
        /// `[[hinge]] id` zo štandardu.
        hinge: String,
        /// ID dvierok.
        door: String,
        /// ID boku s podložkou.
        side: String,
        /// Výška osi pántu (z).
        z: f64,
        /// Stred misky (x).
        cup_x: f64,
        /// Os podložky od prednej hrany boku (y).
        plate_y: f64,
        /// Slepý pánt: podložka na pevnom paneli `side` v tejto polohe x (inak na boku).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        plate_x: Option<f64>,
    },
    /// Zásuvka s výsuvmi: diery výsuvov v bokoch, upevnenie čela.
    Drawer {
        /// `[[drawer]] id` zo štandardu.
        drawer: String,
        /// ID čela zásuvky.
        front: String,
        /// ID ľavého a pravého boku skrinky.
        side_left: String,
        side_right: String,
        /// Spodok korpusu zásuvky (z).
        box_bottom: f64,
        /// Výška korpusu zásuvky (výška bočnice systému).
        box_height: f64,
        /// Dĺžka výsuvu (NL).
        length: f64,
        /// Roviny bočníc korpusu zásuvky (x).
        box_left: f64,
        box_right: f64,
    },
    /// Kolíky lišty blendy do čela a do dna blendy, ktorých sa lišta dotýka.
    Filler {
        /// ID lišty blendy.
        strip: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Model {
    pub project: String,
    pub cabinets: Vec<CabinetModel>,
}

impl Model {
    pub fn parts(&self) -> impl Iterator<Item = &Part> {
        self.cabinets.iter().flat_map(|c| c.parts.iter())
    }
}
