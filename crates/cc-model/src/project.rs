use serde::{Deserialize, Serialize};

use crate::CabinetOptions;

/// Obsah súboru `project.toml` (kap. 4.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectFile {
    pub project: ProjectInfo,
    pub defaults: Defaults,
    #[serde(default, rename = "cabinet")]
    pub cabinets: Vec<CabinetSpec>,
    /// Miestnosť (kap. 13). Bez nej sa skrinky v náhľade zoradia do radu.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub room: Option<RoomSpec>,
    #[serde(default, rename = "wall", skip_serializing_if = "Vec::is_empty")]
    pub walls: Vec<WallSpec>,
    #[serde(default, rename = "opening", skip_serializing_if = "Vec::is_empty")]
    pub openings: Vec<OpeningSpec>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectInfo {
    pub name: String,
    /// MVP podporuje iba `"mm"`.
    #[serde(default = "default_units")]
    pub units: String,
    /// Číslo zákazky (do popisového poľa výkresov), napr. `REF2026`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number: Option<String>,
    /// Zákazník.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub customer: Option<String>,
    /// Kto výkresy kreslil.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub designer: Option<String>,
    /// Firma (hlavička popisového poľa).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub company: Option<String>,
    /// Logo firmy do popisového poľa (PNG alebo JPEG); relatívna cesta je od priečinka
    /// projektu.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo: Option<String>,
    /// Zákazka CABINET VISION, z ktorej sa projekt importoval: šablóna pre zápis `.cvj`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cv_template: Option<String>,
    /// ObjectID skriniek zákazky CV, ktoré import vynechal (náš generátor ich odmietol):
    /// zápis `.cvj` ich v zákazke ponechá.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cv_skipped: Vec<u32>,
}

impl ProjectInfo {
    /// Nová zákazka s menom; ostatné údaje prázdne.
    pub fn named(name: impl Into<String>) -> ProjectInfo {
        ProjectInfo {
            name: name.into(),
            units: default_units(),
            number: None,
            customer: None,
            designer: None,
            company: None,
            logo: None,
            cv_template: None,
            cv_skipped: Vec::new(),
        }
    }
}

fn default_units() -> String {
    "mm".to_owned()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Defaults {
    pub panel_material: String,
    pub back_material: String,
    /// Názov konštrukčného štandardu, súbor `standards/<construction>.toml`.
    pub construction: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CabinetType {
    Base,
    Wall,
    Tall,
}

impl CabinetType {
    pub fn as_str(self) -> &'static str {
        match self {
            CabinetType::Base => "base",
            CabinetType::Wall => "wall",
            CabinetType::Tall => "tall",
        }
    }
}

/// Zadanie jednej skrinky. Rozmery sú vonkajšie rozmery korpusu (kap. 3.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CabinetSpec {
    pub id: String,
    #[serde(rename = "type")]
    pub cabinet_type: CabinetType,
    pub w: f64,
    pub h: f64,
    pub d: f64,
    #[serde(default)]
    pub adjustable_shelves: u32,
    /// Iba `tall`: výšky hornej plochy pevných políc od `z = 0`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fixed_shelves: Vec<f64>,
    /// Stena, pri ktorej skrinka stojí (kap. 13.3). Bez steny nemá polohu v miestnosti.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wall: Option<String>,
    /// Vzdialenosť ľavej hrany skrinky od ľavého konca steny (pri pohľade z miestnosti).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub at: f64,
    /// Výška spodku skrinky nad podlahou; bez hodnoty podľa typu (kap. 13.3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elevation: Option<f64>,
    /// Odsadenie zadnej strany od steny.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub outset: f64,
    /// Čelá zhora nadol (dvierka, zásuvky…); prázdne = otvorená skrinka.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fronts: Vec<FrontSpec>,
    /// Škáry čiel na korpus pri tejto skrinke (ako Reveals v CV); chýbajúce zo štandardu.
    #[serde(default, skip_serializing_if = "FrontReveals::is_empty")]
    pub reveals: FrontReveals,
    /// Montáž čiel; bez hodnoty podľa štandardu (`[fronts] mount`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub front_mount: Option<FrontMount>,
    /// Pánt z knižnice štandardu (`[[hinge]] id`); bez hodnoty `[fronts] hinge`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hinge: Option<String>,
    /// Systém zásuviek (`[[drawer]] id`); bez hodnoty `[fronts] drawer`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drawer: Option<String>,
    /// Najdlhší výsuv (NL), napr. 450 za drezom; bez hodnoty podľa hĺbky skrinky.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drawer_length: Option<f64>,
    /// Vrch spodnej skrinky (`panel` = celá doska, `stretchers` = 2 vlysy); bez hodnoty
    /// podľa štandardu. Horná a vysoká majú vždy celú dosku.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_style: Option<crate::TopStyle>,
    /// Ležaté vlysy vrchu znížené o toľko pod vrch bokov (varná doska, CV REF VARNA: 154,5);
    /// bez hodnoty hore ako zvyčajne.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stretcher_drop: Option<f64>,
    /// Zadná stena (`false`: drezová skrinka bez chrbta).
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub back: bool,
    /// Korpus (`false`: iba čelá, napr. čelo umývačky riadu).
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub carcass: bool,
    /// Zámok šírky (Lock Width v CV): šírka sa nemení ťahaním ani vypĺňaním steny,
    /// zadať ju ručne sa dá.
    #[serde(default, skip_serializing_if = "is_false")]
    pub lock_width: bool,
    /// Výškové umiestnenie ako Placement v CV (pravidlo voči miestnosti); `elevation`
    /// má prednosť (voľne, Float). Bez oboch podľa typu skrinky.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placement: Option<Placement>,
    /// Katalógová položka, z ktorej skrinka vznikla (v katalógu dielne = meno objektu CV,
    /// napr. `S_KUCH_D`); pri zápise do `.cvj` vyberá vzorovú skrinku.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog: Option<String>,
    /// ObjectID skrinky v zákazke CV, z ktorej sa importovala (spätný zápis do tej istej
    /// zákazky ju nahradí a ostatné objekty CV zachová).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cv_id: Option<u32>,
    /// Odtlačok čiel pri importe z CV ([`CabinetSpec::front_fingerprint`]). Kým sa čelá
    /// nezmenia, spätný zápis ponechá delenie čiel zo zákazky CV (import mohol čelá
    /// zjednodušiť kvôli nášmu generátoru, napr. čelo pod minimom stroja).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cv_fronts: Option<String>,
    /// Komentár ku skrinke (záložka Comment vo vlastnostiach CV).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// Blenda (výplň): lišta a čelo blendy pri skrinke, pri skrinke bez korpusu samostatná
    /// blenda (`filler.rs`, findings 11 §5.3, 12 §2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filler: Option<crate::FillerSpec>,
    /// Atribúty skrinky (spoje, policové kovanie, čočky, nožičky, TIP-ON); chýbajúce zo štandardu.
    #[serde(default, skip_serializing_if = "CabinetOptions::is_empty")]
    pub options: CabinetOptions,
    /// Šikmý bok (šikmá skrinka CV REF „SIKMA“); bez hodnoty rovná skrinka.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slant: Option<crate::Slant>,
    /// Iba `wall`: otvorená nika podľa `[niche]` štandardu (CV REF „S_KUCH_NASTENNA NIKA“):
    /// celé diely z MDF 19, vrch na bokoch, dno pod bokmi, pohľadový chrbát medzi bokmi.
    #[serde(default, skip_serializing_if = "is_false")]
    pub niche: bool,
    /// Iba `wall`: LED rampa pod skrinkou (doska podľa `[led_rail]` štandardu).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub led_rail: Option<LedRail>,
}

/// LED rampa pod hornou skrinkou (CV: ručne pridaný diel `BO` „LED RAMPA“ bez `PID`,
/// findings 12 §2). Doska leží pod dnom, zadnou hranou pri stene.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LedRail {
    /// Ľavý koniec od ľavej hrany skrinky (záporný = presahuje doľava).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub x: f64,
    /// Dĺžka; bez hodnoty šírka skrinky (CV REF: 1153 = pod dvoma skrinkami 576,5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub length: Option<f64>,
    /// Hĺbka od zadnej strany skrinky; bez hodnoty `[led_rail] depth_minus` (310 pri 330).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<f64>,
}

fn yes() -> bool {
    true
}

/// Výškové umiestnenie skrinky ako Placement v CV (findings 06 §4.3): výška spodnej hrany
/// sa odvodí z miestnosti a pri zmene rozmerov sa prepočíta.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Placement {
    /// Na podlahe (0).
    Floor,
    /// Na sokli (výška sokla miestnosti).
    ToeHeight,
    /// Pod pracovnou doskou: vrch na vrchu spodných skriniek.
    UnderCounter,
    /// Na pracovnej doske: spodok na vrchu spodných + hrúbka dosky.
    CounterTop,
    /// Pod hornými skrinkami: vrch na spodku horných.
    UnderUppers,
    /// Pod podhľadom: vrch na podhľade.
    UnderSoffit,
    /// Pod stropom: vrch na strope.
    Ceiling,
    /// Nad podhľadom: spodok na podhľade.
    AboveSoffit,
    /// Spodok vo výške (mm), ako Above Elevation (horné skrinky dielne 1518).
    AboveElevation(f64),
    /// Vrch vo výške (mm), ako Below Elevation.
    BelowElevation(f64),
}

/// Hrúbka pracovnej dosky pri umiestnení „na pracovnej doske“ (CV: 1,49606″ = 38 mm).
pub const COUNTER_THICKNESS: f64 = 38.0;

/// Výšky miestnosti pre umiestnenie: vrch spodných skriniek a spodok horných.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlacementLevels {
    pub base_top: f64,
    pub upper_bottom: f64,
}

impl PlacementLevels {
    /// Z miestnosti a najvyšších spodných/horných skriniek projektu (bez nich 720).
    pub fn of(project: &ProjectFile) -> PlacementLevels {
        let room = project.room.clone().unwrap_or_default();
        let tallest = |t: CabinetType| {
            project
                .cabinets
                .iter()
                .filter(|c| c.cabinet_type == t && c.h.is_finite())
                .map(|c| c.h)
                .fold(f64::NEG_INFINITY, f64::max)
        };
        let or = |v: f64| if v.is_finite() { v } else { 720.0 };
        PlacementLevels {
            base_top: room.toe_height + or(tallest(CabinetType::Base)),
            upper_bottom: room.upper_top() - or(tallest(CabinetType::Wall)),
        }
    }
}

impl CabinetSpec {
    /// Riadky čiel (zhora), za ktorými môžu byť police ako v CV (Interior otvoru): dvierka,
    /// dvojité dvierka, otvorený otvor, pevný panel; riadok so stĺpcami, ak aspoň jeden stĺpec
    /// je taký. Za zásuvkou a slepým čelom police nie sú.
    pub fn shelf_rows(&self) -> Vec<bool> {
        let ok = |k: FrontKind| !matches!(k, FrontKind::Drawer | FrontKind::FalseFront);
        self.fronts
            .iter()
            .map(|f| {
                if f.columns.is_empty() {
                    ok(f.kind)
                } else {
                    f.columns.iter().any(|c| ok(c.kind))
                }
            })
            .collect()
    }

    /// Má skrinka otvor, do ktorého patria police? Skrinka bez čiel áno (otvorená).
    pub fn has_shelf_opening(&self) -> bool {
        self.fronts.is_empty() || self.shelf_rows().iter().any(|r| *r)
    }

    /// Odtlačok všetkého, čo určuje delenie čiel (čelá, škáry, osadenie): FNV-1a 64 z popisu.
    pub fn front_fingerprint(&self) -> String {
        let text = format!(
            "{:?}|{:?}|{:?}",
            self.fronts, self.reveals, self.front_mount
        );
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in text.bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
        format!("{h:016x}")
    }

    /// Výška spodnej hrany nad podlahou: zadaná `elevation`, inak podľa `placement`,
    /// inak podľa typu (spodné a vysoké na sokel, horné pod hornú líniu).
    pub fn elevation_in(&self, room: &RoomSpec, levels: &PlacementLevels) -> f64 {
        if let Some(e) = self.elevation {
            return e;
        }
        let h = self.h;
        let soffit = room.wall_height - room.soffit;
        match self.placement {
            Some(Placement::Floor) => 0.0,
            Some(Placement::ToeHeight) => room.toe_height,
            Some(Placement::UnderCounter) => levels.base_top - h,
            Some(Placement::CounterTop) => levels.base_top + COUNTER_THICKNESS,
            Some(Placement::UnderUppers) => levels.upper_bottom - h,
            Some(Placement::UnderSoffit) => soffit - h,
            Some(Placement::Ceiling) => room.wall_height - h,
            Some(Placement::AboveSoffit) => soffit,
            Some(Placement::AboveElevation(elevation)) => elevation,
            Some(Placement::BelowElevation(elevation)) => elevation - h,
            None => match self.cabinet_type {
                CabinetType::Base | CabinetType::Tall => room.toe_height,
                CabinetType::Wall => room.upper_top() - h,
            },
        }
    }
}

fn is_true(v: &bool) -> bool {
    *v
}

fn is_false(v: &bool) -> bool {
    !*v
}

/// Škáry čiel na korpus (mm): vrch a spodok voči korpusu (kladné = čelo je vo vnútri
/// obrysu), boky, medzi čelami nad sebou a vedľa seba. `None` = podľa štandardu.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrontReveals {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bottom: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub right: Option<f64>,
    /// Medzi čelami nad sebou.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub horizontal: Option<f64>,
    /// Medzi čelami vedľa seba (dvojité dvierka, stĺpce).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vertical: Option<f64>,
}

impl FrontReveals {
    pub fn is_empty(&self) -> bool {
        *self == FrontReveals::default()
    }
}

/// Druh čela v otvore skrinky.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrontKind {
    /// Jedny dvierka.
    Door,
    /// Dvojité dvierka (ľavé a pravé).
    Pair,
    /// Čelo zásuvky.
    Drawer,
    /// Pevné (slepé) čelo.
    FalseFront,
    /// Otvorené, bez čela.
    Open,
    /// Pevný panel slepého rohu (nesie podložku slepého pántu susedných dvierok).
    Panel,
}

impl FrontKind {
    pub fn as_str(self) -> &'static str {
        match self {
            FrontKind::Door => "door",
            FrontKind::Pair => "pair",
            FrontKind::Drawer => "drawer",
            FrontKind::FalseFront => "false_front",
            FrontKind::Open => "open",
            FrontKind::Panel => "panel",
        }
    }
}

/// Strana pántov jedných dvierok (pri pohľade spredu).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HingeSide {
    #[default]
    Left,
    Right,
}

/// Montáž čiel: na korpus (prekrývajú ho) alebo vložené do korpusu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FrontMount {
    #[default]
    Overlay,
    Inset,
}

/// Jedno čelo (riadok) skrinky.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrontSpec {
    pub kind: FrontKind,
    /// Výška čela; bez hodnoty sa zvyšok výšky delí rovnako medzi čelá bez výšky.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
    /// Iba `door`: strana pántov.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hinge_side: Option<HingeSide>,
    /// Stĺpec: šírka otvoru po čiaru delenia; bez hodnoty delí zvyšok rovnako.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    /// Zvislé delenie riadku na stĺpce (zľava doprava); `kind` riadku sa potom nepoužije.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub columns: Vec<FrontSpec>,
    /// Iba `drawer`: výška korpusu zásuvky (napr. N 80,5 pod varnou doskou); bez hodnoty
    /// najvyšší, ktorý sa zmestí.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub box_height: Option<f64>,
    /// Úprava rozmeru čela pri hranách (Size Adjustments v CV): kladné zväčší čelo za hranu
    /// otvoru (napr. blenda vpravo +60), záporné zmenší (úchytka hore −30).
    #[serde(default, skip_serializing_if = "FrontAdjust::is_empty")]
    pub adjust: FrontAdjust,
}

/// Úprava rozmeru čela pri ľavej, pravej, hornej a spodnej hrane (mm).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrontAdjust {
    #[serde(default, skip_serializing_if = "is_zero")]
    pub left: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub right: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub top: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub bottom: f64,
}

impl FrontAdjust {
    pub fn is_empty(&self) -> bool {
        *self == FrontAdjust::default()
    }
}

impl FrontSpec {
    pub fn new(kind: FrontKind) -> FrontSpec {
        FrontSpec {
            kind,
            height: None,
            hinge_side: None,
            width: None,
            columns: Vec::new(),
            box_height: None,
            adjust: FrontAdjust::default(),
        }
    }
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

/// Spoločné hodnoty miestnosti (kap. 13.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoomSpec {
    /// Výška stien (strop).
    #[serde(default = "default_wall_height")]
    pub wall_height: f64,
    /// Predvolená hrúbka stien.
    #[serde(default = "default_wall_thickness")]
    pub wall_thickness: f64,
    /// Pás pod stropom (meria sa od stropu nadol); horné skrinky končia pod ním.
    #[serde(default)]
    pub soffit: f64,
    /// Výška sokla/nožičiek: spodok spodných a vysokých skriniek.
    #[serde(default = "default_toe_height")]
    pub toe_height: f64,
    /// Horná hrana horných skriniek; bez hodnoty `wall_height − soffit`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upper_top: Option<f64>,
}

impl Default for RoomSpec {
    fn default() -> Self {
        RoomSpec {
            wall_height: default_wall_height(),
            wall_thickness: default_wall_thickness(),
            soffit: 0.0,
            toe_height: default_toe_height(),
            upper_top: None,
        }
    }
}

impl RoomSpec {
    pub fn upper_top(&self) -> f64 {
        self.upper_top.unwrap_or(self.wall_height - self.soffit)
    }
}

fn default_wall_height() -> f64 {
    2600.0
}

fn default_wall_thickness() -> f64 {
    100.0
}

fn default_toe_height() -> f64 {
    100.0
}

/// Rovná stena (kap. 13.2). Referenčná čiara je vnútorná plocha steny; miestnosť je
/// napravo od smeru `start → koniec`, telo steny naľavo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WallSpec {
    pub id: String,
    /// Začiatok (pôdorys, mm).
    pub x: f64,
    pub y: f64,
    pub length: f64,
    /// Smer v pôdoryse v stupňoch, proti smeru hodinových ručičiek od osi +x.
    #[serde(default)]
    pub angle: f64,
    /// Bez hodnoty `room.wall_height`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
    /// Bez hodnoty `room.wall_thickness`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thickness: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OpeningKind {
    Window,
    Door,
    Opening,
}

impl OpeningKind {
    pub fn as_str(self) -> &'static str {
        match self {
            OpeningKind::Window => "window",
            OpeningKind::Door => "door",
            OpeningKind::Opening => "opening",
        }
    }
}

/// Okno, dvere alebo otvor v stene (kap. 13.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpeningSpec {
    pub wall: String,
    pub kind: OpeningKind,
    /// Ľavá hrana od ľavého konca steny.
    pub at: f64,
    pub width: f64,
    pub height: f64,
    /// Parapet: spodok otvoru nad podlahou.
    #[serde(default)]
    pub elevation: f64,
}

#[cfg(test)]
mod placement_tests {
    use super::*;

    fn cab(t: CabinetType, h: f64) -> CabinetSpec {
        let mut c: CabinetSpec = toml::from_str(
            "id = \"A\"
type = \"base\"
w = 600
h = 762
d = 510",
        )
        .unwrap();
        c.cabinet_type = t;
        c.h = h;
        c
    }

    /// Umiestnenie ako Placement v CV (findings 06 §4.3).
    #[test]
    fn placement_rules() {
        let room = RoomSpec {
            wall_height: 2600.0,
            soffit: 300.0,
            toe_height: 100.0,
            upper_top: Some(2200.0),
            ..RoomSpec::default()
        };
        let levels = PlacementLevels {
            base_top: 862.0,
            upper_bottom: 1500.0,
        };
        let mut c = cab(CabinetType::Wall, 700.0);
        let at = |c: &CabinetSpec| c.elevation_in(&room, &levels);
        assert_eq!(at(&c), 1500.0); // bez pravidla: pod hornú líniu 2200
        c.placement = Some(Placement::AboveElevation(1518.0));
        assert_eq!(at(&c), 1518.0);
        c.placement = Some(Placement::UnderSoffit);
        assert_eq!(at(&c), 2300.0 - 700.0);
        c.placement = Some(Placement::CounterTop);
        assert_eq!(at(&c), 862.0 + 38.0);
        c.placement = Some(Placement::UnderUppers);
        assert_eq!(at(&c), 1500.0 - 700.0);
        c.placement = Some(Placement::BelowElevation(2218.0));
        assert_eq!(at(&c), 1518.0);
        // zadaná výška (voľne) má prednosť
        c.elevation = Some(10.0);
        assert_eq!(at(&c), 10.0);
        let b = cab(CabinetType::Base, 762.0);
        assert_eq!(at(&b), 100.0);
        // TOML: horná skrinka dielne ako v CV
        let p: Placement = toml::from_str::<std::collections::BTreeMap<String, Placement>>(
            "p = { above_elevation = 1518.0 }",
        )
        .unwrap()["p"];
        assert_eq!(p, Placement::AboveElevation(1518.0));
    }
}
