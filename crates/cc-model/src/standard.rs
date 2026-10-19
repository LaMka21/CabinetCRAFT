use serde::{Deserialize, Serialize};

use crate::FrontMount;

/// Konštrukčný štandard, súbor `standards/<meno>.toml` (kap. 4.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Standard {
    pub carcass: CarcassStandard,
    pub back: BackStandard,
    pub dowel: DowelStandard,
    pub line_bore: LineBoreStandard,
    #[serde(default, rename = "joint_rule")]
    pub joint_rules: Vec<JointRule>,
    /// Čelá: škáry, montáž, predvolený pánt, počet pántov.
    #[serde(default)]
    pub fronts: FrontStandard,
    /// Knižnica pántov; skrinka si vyberie `hinge = "id"`.
    #[serde(default, rename = "hinge", skip_serializing_if = "Vec::is_empty")]
    pub hinges: Vec<HingeSpec>,
    /// Knižnica systémov zásuviek; skrinka si vyberie `drawer = "id"`.
    #[serde(default, rename = "drawer", skip_serializing_if = "Vec::is_empty")]
    pub drawers: Vec<DrawerSpec>,
    /// Predvolené atribúty skriniek.
    #[serde(default)]
    pub cabinet_options: crate::ResolvedOptions,
    /// Tlmiace čočky v čelách.
    #[serde(default)]
    pub bumpers: BumperStandard,
    /// Nožičky.
    #[serde(default)]
    pub legs: LegStandard,
    /// Sokel.
    #[serde(default)]
    pub plinth: PlinthStandard,
    /// Krycí bok pri skrinke (atribút skrinky `cover_side_left/right`).
    #[serde(default)]
    pub cover_side: CoverSideStandard,
    /// Spojovacie kovanie skriniek medzi sebou (diery v susedných bokoch).
    #[serde(default)]
    pub cabinet_link: CabinetLinkStandard,
    /// Slepý roh: bok pri pevnom paneli a spoj panelu s korpusom.
    #[serde(default)]
    pub blind_corner: BlindCornerStandard,
    /// Blenda (výplň): lišta, čelo blendy a kolíky (findings 11 §5.3, 12 §2).
    #[serde(default)]
    pub filler: crate::FillerStandard,
    /// Mená kovania v súpise.
    #[serde(default)]
    pub fittings: FittingNames,
    /// Šikmá skrinka (šikmý bok, pohľadový chrbát).
    #[serde(default)]
    pub slanted: crate::SlantStandard,
    /// Otvorená nika (skrinka `niche = true`); bez sekcie sa nika nedá vyrobiť.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub niche: Option<crate::NicheStandard>,
    /// LED rampa pod hornými skrinkami (skrinka `led_rail`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub led_rail: Option<crate::LedRailStandard>,
}

/// Tlmiace (dorazové) čočky: diery v ploche F čela oproti pántom.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct BumperStandard {
    pub diameter: f64,
    pub depth: f64,
    /// Od bočnej hrany čela.
    pub edge: f64,
    /// Čelo zásuvky do tejto výšky má 2 diery v strede výšky, vyššie 4.
    pub small_front: f64,
    /// Od hornej a spodnej hrany čela podľa typu skrinky.
    pub base: BumperRows,
    pub wall: BumperRows,
    pub tall: BumperRows,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct BumperRows {
    pub top: f64,
    pub bottom: f64,
}

impl Default for BumperRows {
    fn default() -> Self {
        BumperRows {
            top: 9.0,
            bottom: 9.0,
        }
    }
}

impl Default for BumperStandard {
    fn default() -> Self {
        BumperStandard {
            diameter: 8.0,
            depth: 12.0,
            edge: 9.0,
            small_front: 150.0,
            base: BumperRows::default(),
            wall: BumperRows::default(),
            tall: BumperRows::default(),
        }
    }
}

impl BumperStandard {
    pub fn rows(&self, t: crate::CabinetType) -> &BumperRows {
        match t {
            crate::CabinetType::Base => &self.base,
            crate::CabinetType::Wall => &self.wall,
            crate::CabinetType::Tall => &self.tall,
        }
    }
}

/// Rozmiestnenie nožičiek pod spodnými a vysokými skrinkami.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct LegStandard {
    /// Os nožičky od boku skrinky.
    pub from_side: f64,
    /// Od čela a od zadnej hrany.
    pub from_front: f64,
    pub from_back: f64,
    /// Najväčšia vzdialenosť susedných nožičiek po šírke.
    pub max_span: f64,
}

/// Spojovacie kovanie susedných skriniek (findings 12 §8): diery v ploche F ľavého boku
/// pravej skrinky a na tom istom mieste v pravom boku ľavej susedy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct CabinetLinkStandard {
    pub diameter: f64,
    pub depth: f64,
    /// Od prednej a zadnej hrany boku (spodná a vysoká skrinka; horná má dieru v strede hĺbky).
    pub from_edge: f64,
    /// Od spodnej a hornej hrany boku (vysoká aj v strede výšky).
    pub from_end: f64,
}

impl Default for CabinetLinkStandard {
    fn default() -> Self {
        CabinetLinkStandard {
            diameter: 5.0,
            depth: 16.0,
            from_edge: 100.0,
            from_end: 80.0,
        }
    }
}

/// Slepý roh (pevný panel v krajnom stĺpci čiel, CV `S_KUC_ROH_P/_L`): bok na strane
/// panelu vystupuje dopredu až k panelu a panel je s ním (a s dnom) spojený kolíkmi.
/// Predvolené hodnoty nič nemenia.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields, default)]
pub struct BlindCornerStandard {
    /// O koľko bok na strane panelu vystupuje pred čelo korpusu (0 = nevystupuje).
    pub side_extend: f64,
    /// Panel na prednej hrane boku: kolíky (a konfirmáty) podľa `[dowel]` po dĺžke boku.
    pub panel_side_joint: bool,
    /// Kolíky medzi panelom a prednou hranou dna (cez medzeru pred korpusom): polohy
    /// od vnútornej plochy boku na strane panelu; prázdne = bez spoja.
    pub panel_bottom_dowels: Vec<f64>,
}

impl Default for LegStandard {
    fn default() -> Self {
        LegStandard {
            from_side: 50.0,
            from_front: 108.0,
            from_back: 50.0,
            max_span: 800.0,
        }
    }
}

impl LegStandard {
    /// Počet nožičiek pod skrinkou šírky `w` (2 rady po hĺbke).
    pub fn count(&self, w: f64) -> u32 {
        let span = (w - 2.0 * self.from_side).max(0.0);
        if !span.is_finite() || self.max_span <= 0.0 {
            return 4;
        }
        let per_row = ((span / self.max_span - 1e-6).ceil() + 1.0).clamp(2.0, 50.0) as u32;
        2 * per_row
    }
}

/// Sokel: doska pred nožičkami od podlahy po spodok skrinky.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct PlinthStandard {
    /// Medzera medzi soklom a spodkom skrinky.
    pub gap_top: f64,
    /// Zadná plocha sokla za čelom korpusu (CV: `Z = hĺbka − 68`, findings 12 §6).
    pub setback: f64,
    /// Najnižší sokel (nižšia skrinka nad podlahou ho nemá).
    pub min_height: f64,
    /// Materiál sokla (id z knižnice); bez hodnoty materiál korpusu.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub material: Option<String>,
    /// Jeden sokel pod celou zostavou susedných spodných a vysokých skriniek pri stene
    /// (rovnaká predná línia a výška); `false` = sokel pod každou skrinkou zvlášť.
    pub runs: bool,
    /// Najdlhší kus sokla zostavy (tabuľa); dlhšia zostava sa delí na styku skriniek.
    /// 0 = bez obmedzenia.
    pub max_length: f64,
    /// Bočný sokel pod bokom skrinky na odkrytom konci zostavy (nie pri susednej skrinke,
    /// v rohu ani pri krycom boku).
    pub side: bool,
    /// Medzera bočného sokla od zadnej hrany korpusu.
    pub side_back_gap: f64,
    /// Kolíky medzi bočným a predným soklom.
    pub side_dowel: PlinthDowel,
}

impl Default for PlinthStandard {
    fn default() -> Self {
        PlinthStandard {
            gap_top: 3.0,
            setback: 68.0,
            min_height: 20.0,
            material: None,
            runs: false,
            max_length: 0.0,
            side: false,
            side_back_gap: 0.0,
            side_dowel: PlinthDowel::default(),
        }
    }
}

/// Kolíky bočného sokla: do čela bočného sokla a do zadnej plochy predného sokla.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct PlinthDowel {
    pub diameter: f64,
    /// Hĺbka do zadnej plochy predného sokla.
    pub face_depth: f64,
    /// Hĺbka do čela bočného sokla.
    pub edge_depth: f64,
    /// Od spodnej a hornej hrany bočného sokla (2 kolíky).
    pub from_edge: f64,
}

impl Default for PlinthDowel {
    fn default() -> Self {
        PlinthDowel {
            diameter: 8.0,
            face_depth: 13.0,
            edge_depth: 26.0,
            from_edge: 18.0,
        }
    }
}

/// Krycí bok pri boku spodnej alebo vysokej skrinky: od podlahy po pracovnú dosku,
/// pred čelá a za korpus (REF skr. 13, KRYCI BOK, findings 10 §2178).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct CoverSideStandard {
    /// Materiál (id z knižnice); bez hodnoty materiál korpusu.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub material: Option<String>,
    /// Nad vrch korpusu (mm).
    pub top: f64,
    /// Pred čelom korpusu (mm).
    pub front: f64,
    /// Za zadnou hranou korpusu (mm).
    pub back: f64,
}

/// Mená kovania v súpise (z knižnice dielne).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct FittingNames {
    pub dowel: String,
    pub confirmat: String,
    #[serde(default = "default_screw_name")]
    pub screw: String,
    pub leg: String,
    pub shelf_support: String,
    pub bumper: String,
    pub tip_on_stop: String,
    pub tip_on_magnet: String,
}

impl Default for FittingNames {
    fn default() -> Self {
        FittingNames {
            dowel: "Kolík 8×35".into(),
            confirmat: "Konfirmát 5×50".into(),
            screw: default_screw_name(),
            leg: "Nožička".into(),
            shelf_support: "Podperka police".into(),
            bumper: "Dorazová čočka".into(),
            tip_on_stop: "TIP-ON s dorazom".into(),
            tip_on_magnet: "TIP-ON s magnetom".into(),
        }
    }
}

impl Standard {
    pub fn hinge(&self, id: &str) -> Option<&HingeSpec> {
        self.hinges.iter().find(|h| h.id == id)
    }

    pub fn drawer(&self, id: &str) -> Option<&DrawerSpec> {
        self.drawers.iter().find(|d| d.id == id)
    }
}

/// Systém zásuvky (kovové bočnice + výsuvy; dno a zadok z dosky).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DrawerSpec {
    pub id: String,
    /// Meno v súpise kovania.
    pub name: String,
    /// Dĺžky výsuvu (NL); vyberie sa najdlhšia, ktorá sa zmestí do hĺbky skrinky.
    pub lengths: Vec<f64>,
    /// O koľko je skrinka hlbšia ako NL aspoň.
    pub depth_reserve: f64,
    /// Výšky korpusu (bočníc); vyberie sa najvyššia, ktorá sa zmestí pod vrch čela.
    pub heights: Vec<f64>,
    /// Najmenšia medzera medzi vrchom korpusu zásuvky a vrchom čela.
    pub top_reserve: f64,
    /// Bočnica korpusu od vnútornej plochy boku skrinky.
    pub side_clearance: f64,
    /// Spodok korpusu nad spodkom čela.
    pub above_front: f64,
    /// Spodok korpusu nad dnom skrinky (najnižšia zásuvka).
    pub above_bottom: f64,
    /// Diery výsuvu v boku (`x` od prednej hrany boku, `z` nad spodkom korpusu).
    pub runner_holes: Vec<HingeHole>,
    /// Iné diery výsuvu pre kratšie NL (prvý vyhovujúci záznam; inak `runner_holes`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub runner_variants: Vec<RunnerVariant>,
    /// Diery upevnenia čela (`x` od roviny bočnice dovnútra, `z` nad spodkom korpusu).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub front_holes: Vec<DrawerHole>,
    /// Materiál dna a zadnej steny korpusu; bez hodnoty bez týchto dielov.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub box_material: Option<String>,
    /// Dno: dĺžka = NL − `bottom_shorter`, šírka = svetlá šírka skrinky − `bottom_narrower`.
    #[serde(default)]
    pub bottom_shorter: f64,
    #[serde(default)]
    pub bottom_narrower: f64,
    /// Horná plocha dna nad spodkom korpusu.
    #[serde(default)]
    pub bottom_top: f64,
    /// Zadná stena: výška korpusu − `back_lower`, šírka ako dno.
    #[serde(default)]
    pub back_lower: f64,
}

/// Diery výsuvu pre NL do `max_length`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunnerVariant {
    pub max_length: f64,
    pub holes: Vec<HingeHole>,
}

impl DrawerSpec {
    /// Diery výsuvu pre dĺžku `nl`.
    pub fn runner_holes_for(&self, nl: f64) -> &[HingeHole] {
        self.runner_variants
            .iter()
            .find(|v| nl <= v.max_length + 1e-6)
            .map_or(self.runner_holes.as_slice(), |v| v.holes.as_slice())
    }
}

/// Diera kovania zásuvky, voliteľne iba pri vysokom korpuse.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DrawerHole {
    #[serde(default, skip_serializing_if = "is_zero")]
    pub x: f64,
    pub z: f64,
    pub diameter: f64,
    pub depth: f64,
    /// Iba ak je korpus aspoň taký vysoký.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_box_height: Option<f64>,
}

/// Pravidlá čiel (dvierka, čelá zásuviek).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct FrontStandard {
    /// Materiál čiel; bez hodnoty materiál korpusu.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub material: Option<String>,
    /// Predvolená montáž.
    pub mount: FrontMount,
    /// Na korpus: medzera bočnej hrany čela od hrany korpusu (polovica škáry medzi skrinkami).
    pub reveal_side: f64,
    /// Škára medzi dvojitými dvierkami.
    pub pair_gap: f64,
    /// Na korpus: odstup čela od líca korpusu.
    pub gap_to_carcass: f64,
    /// Vložené: škára okolo čela v otvore korpusu aj medzi čelami.
    pub inset_gap: f64,
    /// Na korpus, podľa typu skrinky: vrch, spodok a škára medzi čelami nad sebou.
    pub base: FrontRows,
    pub wall: FrontRows,
    pub tall: FrontRows,
    /// Predvolený pánt (`[[hinge]] id`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hinge: Option<String>,
    /// Predvolený systém zásuviek (`[[drawer]] id`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drawer: Option<String>,
    /// Slepý pánt pre dvierka vedľa pevného panelu (`[[hinge]] id`); podložka ide na panel.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blind_hinge: Option<String>,
    /// Os krajného pántu od hornej a dolnej hrany dvierok (pánt môže mať vlastnú).
    pub hinge_end_offset: f64,
    /// Najväčší rozostup pántov (pánt môže mať vlastný); počet = max(2, ⌈L / rozostup⌉ + 1),
    /// L = výška dvierok − 2 × odsadenie krajných pántov.
    pub hinge_max_spacing: f64,
}

impl Default for FrontStandard {
    fn default() -> Self {
        FrontStandard {
            material: None,
            mount: FrontMount::Overlay,
            reveal_side: 1.0,
            pair_gap: 2.0,
            gap_to_carcass: 0.0,
            inset_gap: 2.0,
            base: FrontRows::default(),
            wall: FrontRows::default(),
            tall: FrontRows::default(),
            hinge: None,
            drawer: None,
            blind_hinge: None,
            hinge_end_offset: 100.0,
            hinge_max_spacing: 609.6,
        }
    }
}

/// Čelá na korpus v jednom type skrinky (mm; kladné = čelo je vo vnútri obrysu korpusu,
/// záporné = presahuje).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct FrontRows {
    /// Vrch najvyššieho čela pod vrchom korpusu.
    pub top: f64,
    /// Spodok najnižšieho čela nad spodkom korpusu.
    pub bottom: f64,
    /// Škára medzi čelami nad sebou.
    pub gap: f64,
    /// Vrch pevného panelu pod vrchom korpusu (panel nemá úchytku).
    pub panel_top: f64,
}

impl Default for FrontRows {
    fn default() -> Self {
        FrontRows {
            top: 1.0,
            bottom: 1.0,
            gap: 2.0,
            panel_top: 1.0,
        }
    }
}

impl FrontStandard {
    /// Pravidlá čiel pre typ skrinky.
    pub fn rows(&self, t: crate::CabinetType) -> &FrontRows {
        match t {
            crate::CabinetType::Base => &self.base,
            crate::CabinetType::Wall => &self.wall,
            crate::CabinetType::Tall => &self.tall,
        }
    }

    /// Počet pántov pre dvierka výšky `h`: max(2, ⌈L / `max_spacing`⌉ + 1), najviac 12.
    pub fn hinge_count(h: f64, end_offset: f64, max_spacing: f64) -> usize {
        let l = (h - 2.0 * end_offset).max(0.0);
        if !l.is_finite() || !max_spacing.is_finite() || max_spacing <= 0.0 {
            return 2;
        }
        // presný násobok rozostupu nepridá pánt (tolerancia na zaokrúhlenie)
        let n = (l / max_spacing - 1e-6).ceil() + 1.0;
        n.clamp(2.0, 12.0) as usize
    }
}

/// Pánt: miska v dvierkach a podložka na boku.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HingeSpec {
    pub id: String,
    /// Meno v rozpise kovania.
    pub name: String,
    pub cup_diameter: f64,
    pub cup_depth: f64,
    /// Stred misky od hrany dvierok (K + polomer misky).
    pub cup_center: f64,
    /// Ďalšie diery pri miske (`x` od stredu misky k stredu dvierok, `z` pozdĺž hrany).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cup_holes: Vec<HingeHole>,
    /// Odsadenie krajných pántov od hrán dvierok; bez hodnoty `[fronts] hinge_end_offset`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_offset: Option<f64>,
    /// Najväčší rozostup pántov; bez hodnoty `[fronts] hinge_max_spacing`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_spacing: Option<f64>,
    /// Os podložky od prednej hrany boku (pri vložených dvierkach + hrúbka dvierok);
    /// pri slepom pánte od čiary delenia na pevnom paneli.
    pub plate_setback: f64,
    /// Diery podložky (`x` dozadu od osi podložky, `z` od osi pántu).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub plate_holes: Vec<HingeHole>,
}

/// Diera kovania voči referenčnému bodu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HingeHole {
    #[serde(default, skip_serializing_if = "is_zero")]
    pub x: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub z: f64,
    pub diameter: f64,
    pub depth: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TopStyle {
    /// Dve spojky (predná a zadná), iba spodná skrinka.
    Stretchers,
    /// Celá doska.
    Panel,
    /// Drezová skrinka (findings 12 §4): predný vlys zvislý hore pri čele, zadný vlys
    /// („CHRBAT“) zvislý na dne pri zadnej hrane, obyčajne bez zadnej steny.
    Sink,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CarcassStandard {
    /// Vrch spodnej skrinky; horná a vysoká skrinka majú vždy celú dosku.
    pub top_style: TopStyle,
    pub stretcher_width: f64,
    pub shelf_side_clearance: f64,
    pub shelf_front_setback: f64,
    #[serde(default, skip_serializing_if = "is_default_spacing")]
    pub shelf_spacing: ShelfSpacing,
    /// Materiál vrchu hornej skrinky; bez hodnoty materiál korpusu.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wall_top_material: Option<String>,
    /// Spodná skrinka: dno a vrch (vlysy) sú o túto hodnotu dnu od koncov bokov
    /// (CV: dno 0,5 nad spodkom bokov, vrch 0,5 pod ich vrchom).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub base_end_inset: f64,
    /// Horná skrinka: vrch naložený na boky cez celú šírku a hĺbku (boky sú o jeho hrúbku nižšie).
    #[serde(default, skip_serializing_if = "is_false")]
    pub wall_top_on_sides: bool,
    /// Horná skrinka s chrbtom v drážke: boky končia pred chrbtom (bez drážky), dno a vrch idú
    /// cez celú hĺbku a chrbát v ich drážkach má šírku W − 2 × `fit_clearance` (ako v CV).
    #[serde(default, skip_serializing_if = "is_false")]
    pub wall_back_behind_sides: bool,
}

fn is_false(v: &bool) -> bool {
    !*v
}

fn is_default_spacing(s: &ShelfSpacing) -> bool {
    *s == ShelfSpacing::InnerHeight
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BackMount {
    /// V drážke bokov, dna a vrchu.
    Groove,
    /// Naložená zozadu na korpus (korpus je o hrúbku chrbta plytší).
    Applied,
}

/// Rozmiestnenie polohovateľných políc v skrinke.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShelfSpacing {
    /// Horné plochy políc delia vnútornú výšku rovnako.
    #[default]
    InnerHeight,
    /// Otvory medzi policami sú rovnaké (ako v CV).
    EqualOpenings,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackStandard {
    pub mount: BackMount,
    /// Montáž chrbta podľa typu skrinky; bez hodnoty `mount`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mount_base: Option<BackMount>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mount_wall: Option<BackMount>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mount_tall: Option<BackMount>,
    /// Naložený chrbát: o koľko je korpus plytší ako skrinka; bez hodnoty hrúbka chrbta.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applied_depth: Option<f64>,
    /// Naložený chrbát: odsadenie od bočných hrán, od vrchu a od spodku korpusu.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applied_inset_side: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applied_inset_top: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applied_inset_bottom: Option<f64>,
    /// `e`: zadná hrana dielu → zadná stena drážky.
    pub groove_offset_from_rear: f64,
    /// Šírka drážky `gw = T_back + groove_width_clearance`.
    pub groove_width_clearance: f64,
    /// `gd`
    pub groove_depth: f64,
    /// `c`: vôľa zadnej steny v drážke (na stranu) a hore.
    pub fit_clearance: f64,
    pub groove_through: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DowelStandard {
    pub diameter: f64,
    pub face_depth: f64,
    pub edge_depth: f64,
    pub end_offset: f64,
    /// `even`: max. rozstup jednotlivých kolíkov.
    pub max_spacing: f64,
    /// `even`: vnútorné kolíky zarovnať na mriežku 32 mm.
    pub snap_32: bool,
    /// Rozmiestnenie: `even` (jednotlivé kolíky rovnomerne) alebo `pairs` (dvojice ako v CV).
    #[serde(default)]
    pub layout: DowelLayout,
    /// `pairs`: vzdialenosť kolíkov v dvojici.
    #[serde(default = "default_pair_pitch")]
    pub pair_pitch: f64,
    /// `pairs`: max. vzdialenosť začiatkov susedných dvojíc.
    #[serde(default = "default_max_pair_spacing")]
    pub max_pair_spacing: f64,
    /// `pairs`: pri krátkom spoji (dve dvojice sa nezmestia) 2 kolíky v tejto vzdialenosti od koncov.
    #[serde(default = "default_short_end_offset")]
    pub short_end_offset: f64,
    /// `cv333`: rozostup skupín kolík-konfirmát-kolík na dlhej hrane (bližší, ak sa zmestí, inak vzdialenejší).
    #[serde(default = "default_group_spacing")]
    pub group_spacing: [f64; 2],
    /// Konfirmát (vrut) medzi kolíkmi: diera do plochy dielu, ktorý nesie hranu druhého.
    #[serde(default = "default_confirmat_diameter")]
    pub confirmat_diameter: f64,
    #[serde(default = "default_confirmat_depth")]
    pub confirmat_depth: f64,
    /// Konfirmát: diera do hrany druhého dielu (priemer, hĺbka); bez hodnoty iba do boku.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirmat_edge: Option<[f64; 2]>,
    /// `true`: spoj končí pred drážkou zadnej steny (`yb`, kap. 6); `false`: po celej hĺbke
    /// dielu, ako v programoch CV (kolíky môžu byť aj za drážkou, ak do nej nezasahujú).
    #[serde(default = "default_true")]
    pub stop_at_back: bool,
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

fn default_screw_name() -> String {
    "Vrut 3,5×50".into()
}

fn default_confirmat_diameter() -> f64 {
    5.0
}

fn default_confirmat_depth() -> f64 {
    13.0
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DowelLayout {
    #[default]
    Even,
    Pairs,
    /// Vzor „spoj 3/3/3“ z CV: počet a poloha kolíkov aj konfirmátov podľa pásma dĺžky hrany
    /// (findings 11 §1.3).
    Cv333,
}

fn default_group_spacing() -> [f64; 2] {
    [160.0, 256.0]
}

fn default_pair_pitch() -> f64 {
    64.0
}

fn default_max_pair_spacing() -> f64 {
    250.0
}

fn default_short_end_offset() -> f64 {
    18.0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LineBoreStandard {
    pub diameter: f64,
    pub depth: f64,
    pub front_offset: f64,
    pub rear_offset: f64,
    pub pitch: f64,
    pub start_above_bottom: f64,
    pub end_below_top: f64,
    /// `full`: súvislý rad po celej výške; `per_shelf`: skupina dier pri každej polici.
    #[serde(default)]
    pub layout: LineBoreLayout,
    /// `per_shelf`: počet dier v skupine (s roztečou `pitch`, stredná pri polici).
    #[serde(default = "default_holes_per_shelf")]
    pub holes_per_shelf: u32,
    /// `per_shelf`: stredná diera presne toľko pod spodnou plochou police (Sekura 7: 6);
    /// bez hodnoty najbližšia diera mriežky `pitch`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub center_below_shelf: Option<f64>,
    /// Diery pre čap podperky v spodnej ploche polohovateľnej police: [priemer, hĺbka,
    /// vzdialenosť od konca police]; v radoch radu dier boku. Bez hodnoty sa police nevŕtajú.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shelf_pin_holes: Option<[f64; 3]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineBoreLayout {
    #[default]
    Full,
    PerShelf,
}

fn default_holes_per_shelf() -> u32 {
    3
}

impl BackStandard {
    pub fn mount_for(&self, t: crate::CabinetType) -> BackMount {
        match t {
            crate::CabinetType::Base => self.mount_base,
            crate::CabinetType::Wall => self.mount_wall,
            crate::CabinetType::Tall => self.mount_tall,
        }
        .unwrap_or(self.mount)
    }
}

/// Typ spoja v pravidle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionKind {
    Dowel,
    BackGroove,
    LineBore,
    /// Diely sa iba dotýkajú, bez obrábania (napr. polica po pohľadový chrbát niky).
    Touch,
}

/// Jedna hodnota alebo zoznam hodnôt v TOML (`a = "side"` alebo `a = ["side", "bottom"]`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OneOrMany {
    One(String),
    Many(Vec<String>),
}

impl OneOrMany {
    pub fn items(&self) -> Vec<&str> {
        match self {
            OneOrMany::One(s) => vec![s.as_str()],
            OneOrMany::Many(v) => v.iter().map(String::as_str).collect(),
        }
    }
}

/// Pravidlo spoja: dvojica rolí → typ spoja (kap. 4.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JointRule {
    pub a: OneOrMany,
    pub b: OneOrMany,
    pub connection: ConnectionKind,
    /// Kolíky: odsadenie krajného kolíka od konca hrany pre tento spoj (inak `[dowel] end_offset`),
    /// napr. drez REF: zadný vlys na dne od 64 (findings 12 §4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_offset: Option<f64>,
    /// Kolíky: dve skupiny pri koncoch hrany dielu `edge` namiesto vzoru (CV REF nika:
    /// chrbát medzi bokmi 64 od spodku a 150 od vrchu).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_groups: Option<crate::EndGroups>,
}
