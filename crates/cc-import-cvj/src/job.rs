//! Neutrálny obraz úlohy CV: miestnosti, steny, skrinky, diely a pracovné dosky
//! v milimetroch a stupňoch, v súradniciach miestnosti CV (os Y nahor, pôdorys X–Z).
//!
//! Súradnice a transformácie podľa popisu formátu (§7): rámec objektu voči rodičovi
//! je `T(X,Y,Z) · Rz(AZ) · Ry(AY) · Rx(AX)`, teleso objektu je kváder
//! `[0,DX]×[0,DY]×[0,DZ]` v jeho rámci. Pri skrinke je počiatok ľavý-zadný-spodný
//! roh, +X doprava pozdĺž steny, +Y hore, +Z dopredu do miestnosti.

use cc_i18n::tr;
use glam::{DAffine3, DMat3, DVec3};

use crate::classes::{Extra, Node, Value};

/// Milimetre na palec (CV ukladá dĺžky v palcoch).
pub const MM_PER_INCH: f64 = 25.4;

/// Najväčšia rozumná dĺžka/súradnica (mm); väčšie hodnoty považujeme za poškodené.
const MAX_MM: f64 = 1.0e6;
/// Diel s menším rozmerom (mm) nemá objem (značka kovania, nulová lišta).
const MIN_SOLID: f64 = 0.5;

/// Celá úloha.
#[derive(Debug, Clone, PartialEq)]
pub struct CvjJob {
    pub name: String,
    pub description: String,
    /// prúd `Version`: rok, ?, build
    pub version: Option<[u32; 3]>,
    pub rooms: Vec<CvjRoom>,
    /// počet objektov v prúde `Contents`
    pub objects: usize,
    /// všetky bajty `Contents` sa prečítali
    pub consumed_all: bool,
    /// dôvod, prečo sa nedočítali objekty za koreňom (nevadí importu)
    pub trailing_error: Option<String>,
    /// upozornenia z čítania (poškodené hodnoty a pod.)
    pub notes: Vec<String>,
}

/// Miestnosť (`CVFloorLevel`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CvjRoom {
    pub name: String,
    pub description: String,
    /// počet neprázdnych tvarov obrysu podlahy (obrys sa nečíta)
    pub floor_shapes: usize,
    pub walls: Vec<CvjWall>,
    pub cabinets: Vec<CvjCabinet>,
    pub counters: Vec<CvjCounter>,
}

/// Stena (`CVWall`).
#[derive(Debug, Clone, PartialEq)]
pub struct CvjWall {
    pub number: u16,
    pub name: String,
    pub description: String,
    /// začiatok referenčnej čiary (vnútorné líce), súradnice miestnosti CV, mm
    pub start: DVec3,
    pub end: DVec3,
    pub length: f64,
    pub height: f64,
    pub thickness: f64,
    /// uhol v pôdoryse (°), parameter `AY`
    pub angle: f64,
    /// vnútorný uhol k susednej stene (°), parameter `_AI`
    pub inner_angle: Option<f64>,
    pub prev: Option<u16>,
    pub next: Option<u16>,
    /// rámec steny v miestnosti (+X pozdĺž steny, +Y hore, +Z do miestnosti)
    pub frame: DAffine3,
}

/// Plocha steny, na ktorej objekt stojí.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Face {
    /// predná (vnútorná) plocha
    Front,
    /// zadná (vonkajšia) plocha
    Back,
}

/// Trieda objektu CV (`CLASS`, §6.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CvClass {
    Base,
    Upper,
    Tall,
    Vanity,
    Appliance,
    Accessory,
    Decorative,
    Top,
    Molding,
    Wall,
    WallFace,
    Floor,
    Ceiling,
    Door,
    NgAccessory,
    Splash,
    UserLight,
    Fixture,
    Bath,
    Closet,
    ClosetBase,
    ClosetUpper,
    Other(u16),
}

impl CvClass {
    pub fn from_code(code: u16) -> CvClass {
        match code {
            1 => CvClass::Base,
            2 => CvClass::Upper,
            3 => CvClass::Tall,
            4 => CvClass::Vanity,
            5 => CvClass::Appliance,
            6 => CvClass::Accessory,
            7 => CvClass::Decorative,
            8 => CvClass::Top,
            9 => CvClass::Molding,
            10 => CvClass::Wall,
            11 => CvClass::WallFace,
            12 => CvClass::Floor,
            13 => CvClass::Ceiling,
            14 => CvClass::Door,
            15 => CvClass::NgAccessory,
            16 => CvClass::Splash,
            17 => CvClass::UserLight,
            18 => CvClass::Fixture,
            19 => CvClass::Bath,
            21 => CvClass::Closet,
            22 => CvClass::ClosetBase,
            23 => CvClass::ClosetUpper,
            c => CvClass::Other(c),
        }
    }

    pub fn name(self) -> String {
        match self {
            CvClass::Base => "Base".into(),
            CvClass::Upper => "Upper".into(),
            CvClass::Tall => "Tall".into(),
            CvClass::Vanity => "Vanity".into(),
            CvClass::Appliance => "Appliance".into(),
            CvClass::Accessory => "Accessory".into(),
            CvClass::Decorative => "Decorative".into(),
            CvClass::Top => "Top".into(),
            CvClass::Molding => "Molding".into(),
            CvClass::Wall => "Wall".into(),
            CvClass::WallFace => "WallFace".into(),
            CvClass::Floor => "Floor".into(),
            CvClass::Ceiling => "Ceiling".into(),
            CvClass::Door => "Door".into(),
            CvClass::NgAccessory => "NGAccessory".into(),
            CvClass::Splash => "Splash".into(),
            CvClass::UserLight => "UserLight".into(),
            CvClass::Fixture => "Fixture".into(),
            CvClass::Bath => "Bath".into(),
            CvClass::Closet => "Closet".into(),
            CvClass::ClosetBase => "ClosetBase".into(),
            CvClass::ClosetUpper => "ClosetUpper".into(),
            CvClass::Other(c) => tr!("import_cvj.class_other", code = c),
        }
    }
}

/// Meno typu skrinky (`TYPE`, §6.3).
pub fn cabinet_type_name(code: u16) -> Option<&'static str> {
    Some(match code {
        1 => "Standard",
        2 => "BlindRight",
        3 => "BlindLeft",
        4 => "Corner45",
        5 => "Corner90",
        6 => "Filler",
        7 => "Desk",
        8 => "Hood",
        9 => "Refrigerator",
        10 => "Valance",
        11 => "UCValance",
        12 => "GarageAppliance",
        13 => "DishWasher",
        14 => "TrashCompactor",
        15 => "Range",
        16 => "CookTop",
        17 => "Sink",
        18 => "Microwave",
        19 => "Window",
        20 => "EntryDoor",
        21 => "Picture",
        22 => "ToeAssembly",
        23 => "Miscellaneous",
        24 => "Closet",
        25 => "Part",
        26 => "Opening",
        27 => "TWall",
        _ => return None,
    })
}

/// Osovo zarovnaný kváder (mm).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bbox {
    pub min: DVec3,
    pub max: DVec3,
}

impl Bbox {
    pub fn size(&self) -> DVec3 {
        self.max - self.min
    }

    fn from_points(points: impl IntoIterator<Item = DVec3>) -> Option<Bbox> {
        let mut it = points.into_iter();
        let first = it.next()?;
        Some(it.fold(
            Bbox {
                min: first,
                max: first,
            },
            |b, p| Bbox {
                min: b.min.min(p),
                max: b.max.max(p),
            },
        ))
    }

    fn union(self, other: Bbox) -> Bbox {
        Bbox {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }

    /// 8 rohov.
    pub fn corners(&self) -> [DVec3; 8] {
        let (a, b) = (self.min, self.max);
        [
            DVec3::new(a.x, a.y, a.z),
            DVec3::new(b.x, a.y, a.z),
            DVec3::new(a.x, b.y, a.z),
            DVec3::new(b.x, b.y, a.z),
            DVec3::new(a.x, a.y, b.z),
            DVec3::new(b.x, a.y, b.z),
            DVec3::new(a.x, b.y, b.z),
            DVec3::new(b.x, b.y, b.z),
        ]
    }
}

/// Úloha dielu v skrinke.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartRole {
    /// korpus a vnútro (boky, dno, vrch, chrbát, police…)
    Carcass,
    /// dvierka a čelá zásuviek
    Front,
    /// korpus zásuvky
    DrawerBox,
    /// samostatný diel priamo pod skrinkou (sokel, lišta…)
    Loose,
}

/// Diel skrinky (ASM 16).
#[derive(Debug, Clone, PartialEq)]
pub struct CvjPart {
    /// krátky kód (meno), napr. `LU`
    pub code: String,
    pub description: String,
    /// číselný kód pozície `PID`
    pub pid: Option<f64>,
    /// `DX, DY, DZ` (dĺžka, šírka, hrúbka) v mm
    pub size: DVec3,
    /// `X, Y, Z` voči rodičovi (mm)
    pub position: DVec3,
    /// `AX, AY, AZ` (°)
    pub rotation: DVec3,
    /// rámec dielu v súradniciach skrinky
    pub to_cabinet: DAffine3,
    /// obálka dielu v súradniciach skrinky
    pub bbox: Bbox,
    pub role: PartRole,
    /// olepenie (`BAND`)
    pub band: Option<String>,
    /// číslo dielu v CNC výstupoch (`NCID`, findings 09 §0.1)
    pub ncid: Option<i64>,
}

/// Skupina stromu sekcií skrinky (findings 11 §3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionGroup {
    /// `Face`: otvory čela a deliče čela
    Face,
    /// `Interior`: otvory vnútra a vnútorné deliče (police)
    Interior,
}

/// Druh uzla stromu sekcií (`TYPE`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionKind {
    /// 1 = otvor
    Opening,
    /// 2 = vodorovný delič (MR; vo vnútri polica AS/FS/DS)
    HorizontalDivider,
    /// 3 = zvislý delič (MS; vo vnútri PT)
    VerticalDivider,
    /// iná hodnota (0 = sekcia vnútri zásuvky)
    Other(i64),
}

impl SectionKind {
    pub fn from_code(code: i64) -> SectionKind {
        match code {
            1 => SectionKind::Opening,
            2 => SectionKind::HorizontalDivider,
            3 => SectionKind::VerticalDivider,
            c => SectionKind::Other(c),
        }
    }
}

/// Výplň otvoru čela (`STYPE` pri otvore v skupine `Face`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaceStyle {
    /// 0 = všeobecný otvor (ďalej delený)
    General,
    /// 1 = dvierka
    Door,
    /// 2 = zásuvka
    Drawer,
    /// 3 = otvorený, bez čela
    Open,
    /// 4 = dvojité dvierka
    PairDoor,
    /// 5 = falošné (pevné) čelo
    FalseFront,
    /// 8 = slepý otvor (pevný panel slepého rohu)
    Blind,
    /// 9 = panel / spotrebič
    Panel,
    Other(i64),
}

impl FaceStyle {
    pub fn from_code(code: i64) -> FaceStyle {
        match code {
            0 => FaceStyle::General,
            1 => FaceStyle::Door,
            2 => FaceStyle::Drawer,
            3 => FaceStyle::Open,
            4 => FaceStyle::PairDoor,
            5 => FaceStyle::FalseFront,
            8 => FaceStyle::Blind,
            9 => FaceStyle::Panel,
            c => FaceStyle::Other(c),
        }
    }
}

/// Parametre otvoru (mm, kódy); chýbajúce = `None` (findings 09 §4.2, 10 §3.4).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OpeningParams {
    /// `REVL`, `REVR`, `REVT`, `REVB`: škára hrany čela od hrany otvoru
    pub reveal_left: Option<f64>,
    pub reveal_right: Option<f64>,
    pub reveal_top: Option<f64>,
    pub reveal_bottom: Option<f64>,
    /// `OLL`, `OLR`, `OLT`, `OLB`: presah čela (záporný = stiahnutie)
    pub overlay_left: Option<f64>,
    pub overlay_right: Option<f64>,
    pub overlay_top: Option<f64>,
    pub overlay_bottom: Option<f64>,
    /// `DOPEN`: odstup čela od líca korpusu
    pub front_gap: Option<f64>,
    /// `HNG`: strana závesov (1 ľavé, 2 pravé, 4 žiadne, 0 zásuvka)
    pub hinge: Option<i64>,
    /// `HNGT`, `HNGB`: ručná poloha horného a spodného závesu
    pub hinge_top: Option<f64>,
    pub hinge_bottom: Option<f64>,
    /// `HNGQ`: počet závesov (0 = automaticky)
    pub hinge_count: Option<i64>,
    /// `BOXH`, `BOXD`, `BOXW`: výška, hĺbka (= dĺžka výsuvu NL) a šírka korpusu zásuvky
    pub box_height: Option<f64>,
    pub box_depth: Option<f64>,
    pub box_width: Option<f64>,
    /// `BDC`: skrátenie hĺbky korpusu zásuvky (drez)
    pub box_depth_cut: Option<f64>,
    /// `BTC`: vôľa nad korpusom zásuvky
    pub box_top_clearance: Option<f64>,
    /// `NL`: dĺžka výsuvu, ak je zapísaná zvlášť
    pub runner_length: Option<f64>,
}

/// Čelo (`CVDoor`: dvierka `DOR`, panel `PANEL`, čelo zásuvky `DWR`).
#[derive(Debug, Clone, PartialEq)]
pub struct CvjFront {
    pub code: String,
    /// čelo zásuvky (ASM 12), inak dvierka/panel (ASM 11)
    pub drawer: bool,
    /// `DX × DY × DZ` (šírka, výška, hrúbka) v mm
    pub size: DVec3,
    /// obálka čela v súradniciach skrinky
    pub bbox: Bbox,
    /// počet závesov (`CVHinge` `HNG`)
    pub hinges: usize,
    /// z toho slepé závesy (`Typ_Zavesov` = „Slepy zaves“)
    pub blind_hinges: usize,
}

/// Uzol stromu sekcií: otvor alebo delič (findings 11 §3.2). Delenie je binárne,
/// uzly sú v skupine vedľa seba a strom tvoria `ID`/`PARID`.
#[derive(Debug, Clone, PartialEq)]
pub struct CvjSection {
    pub group: SectionGroup,
    /// meno uzla, napr. `FO`, `DOR_OPEN`, `DWR_OPEN`, `MR`, `MS`, `CO`, `AS`
    pub code: String,
    pub asm_type: u16,
    /// `ID`
    pub id: i64,
    /// `PARID` (0 = koreň)
    pub parent: i64,
    /// `TYPE`
    pub kind: SectionKind,
    /// `STYPE` (význam podľa skupiny a druhu)
    pub style: i64,
    /// `X, Y` voči skupine a `DX, DY, DZ` (mm)
    pub position: DVec3,
    pub size: DVec3,
    /// obálka uzla v súradniciach skrinky
    pub bbox: Bbox,
    /// `EQ > 0`: delí sa rovnakým dielom so súrodencami
    pub equal: bool,
    pub params: OpeningParams,
    /// čelá v podstrome otvoru
    pub fronts: Vec<CvjFront>,
}

impl CvjSection {
    /// Výplň otvoru čela podľa `STYPE`.
    pub fn face_style(&self) -> FaceStyle {
        FaceStyle::from_code(self.style)
    }
}

/// Atribúty skrinky z panela *Attributes* (findings 11 §1.2); hodnoty ako v CV.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CvjAttributes {
    /// `APPLY_HW_CAB`: 1 spoje áno, 0 nie, 2 reset
    pub apply_hardware: Option<i64>,
    /// `UCS_3DHWcabinet`: 2 kolík + konfirmát, 1 kolíky, 7/3/5 excentre, 10 Blum, 11 MaxiFix, 0 nič
    pub joint_type: Option<i64>,
    /// `MAKEDADO`: 0 drážka, 1 vsadený chrbát, 2 nič
    pub back_dado: Option<i64>,
    /// `Cab_FRD`: drážka frézou (inak pílou)
    pub dado_router: Option<bool>,
    /// `Zmazat_nohu`
    pub delete_legs: Option<bool>,
    /// `Zmazat_nohu_1` … `Zmazat_nohu_6`
    pub deleted_legs: [Option<bool>; 6],
    /// `Policove_kovanie`: 7 Sekura 7, 8 Tab 18, 0 nič
    pub shelf_support: Option<i64>,
    /// `Zapnut_cocky`
    pub bumpers: Option<bool>,
    /// `tip_on_zap_cab_*`: 0 vypnutý, 1 závrtný, 2 s adaptérom
    pub tip_on: Option<i64>,
    /// `tip_on_bar_cab_*`: 0 biely, 1 šedý, 2 čierny
    pub tip_on_color: Option<i64>,
    /// `tip_on_typ_cab_*`: 0 s dorazom, 1 s magnetom
    pub tip_on_kind: Option<i64>,
    /// `BKTYPE`: typ chrbta (význam neznámy)
    pub back_type: Option<i64>,
    /// `REVL`, `REVR`, `REVT`, `REVB` skrinky (mm)
    pub reveal_left: Option<f64>,
    pub reveal_right: Option<f64>,
    pub reveal_top: Option<f64>,
    pub reveal_bottom: Option<f64>,
    /// Spojenie s ľavou susednou skrinkou z ľavého boku `LU`/`LF` (findings 12 §8).
    pub link: Option<CvjLink>,
}

/// Spojovacie kovanie skriniek z parametrov ľavého boku (findings 12 §8).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CvjLink {
    /// `Spojovacie_kovanie_zap_2` (alebo `_zap_1`): 0 vypnuté, 1 zapnuté, 2 zapnuté s ručnými polohami
    pub state: i64,
    /// `Posun_vrtania` (mm)
    pub shift: Option<f64>,
    /// zapnuté diery `Spojkov_{F|B}_{X|Y}_i`: (od prednej hrany boku, od spodku boku) v mm
    pub holes: Vec<(f64, f64)>,
}

/// Najviac dier spojenia na stĺpec (`Spojkov_F_*_1…`), ochrana pred poškodeným súborom.
const MAX_LINK_HOLES: usize = 8;

/// Odkiaľ je obálka korpusu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CarcassSource {
    /// z dielov korpusu
    Parts,
    /// z rozmerov CV `DX/DY/DZ` (skrinka nemá diely korpusu)
    CvBox,
}

/// Skrinka alebo iný objekt triedy `CVCabinet` (spotrebič, okno, panel…).
#[derive(Debug, Clone, PartialEq)]
pub struct CvjCabinet {
    pub object_id: u32,
    /// meno z katalógu
    pub name: String,
    pub description: String,
    pub comment: Option<String>,
    pub class: CvClass,
    pub type_code: u16,
    pub number: Option<i64>,
    /// `X, Y, Z` voči rodičovi (mm)
    pub position: DVec3,
    /// `DX, DY, DZ` (mm)
    pub size: DVec3,
    /// `AX, AY, AZ` (°)
    pub rotation: DVec3,
    /// výška sokla `TOEH` (mm)
    pub toe_height: Option<f64>,
    pub wall: Option<u16>,
    pub face: Option<Face>,
    /// rámec skrinky v miestnosti
    pub world: DAffine3,
    /// rámec skrinky v rámci plochy steny (ak stojí na stene)
    pub in_face: Option<DAffine3>,
    pub doors: usize,
    pub drawer_fronts: usize,
    pub drawer_boxes: usize,
    pub openings: usize,
    pub hardware: usize,
    /// názvy spojovacieho prvku `HW2` v zákazke (napr. „Konfirmát 5x50“)
    pub connectors: Vec<String>,
    /// police (heuristika)
    pub shelves: usize,
    pub parts: Vec<CvjPart>,
    /// obálka korpusu v súradniciach skrinky
    pub carcass: Bbox,
    pub carcass_source: CarcassSource,
    /// počet dielov korpusu s objemom, z ktorých je obálka
    pub carcass_part_count: usize,
    /// strom sekcií (otvory a deliče čela aj vnútra)
    pub sections: Vec<CvjSection>,
    pub attributes: CvjAttributes,
}

impl CvjCabinet {
    /// Uzly stromu sekcií danej skupiny.
    pub fn sections_in(&self, group: SectionGroup) -> impl Iterator<Item = &CvjSection> {
        self.sections.iter().filter(move |s| s.group == group)
    }

    /// Všetky čelá v otvoroch čela.
    pub fn face_fronts(&self) -> impl Iterator<Item = &CvjFront> {
        self.sections_in(SectionGroup::Face)
            .flat_map(|s| s.fronts.iter())
    }
}

impl CvjCabinet {
    pub fn type_name(&self) -> Option<&'static str> {
        cabinet_type_name(self.type_code)
    }

    /// Počet dielov (ASM 16) vrátane dvierok a zásuviek.
    pub fn part_count(&self) -> usize {
        self.parts.len()
    }
}

/// Pracovná doska (`CVCounter`); iba na poznámky.
#[derive(Debug, Clone, PartialEq)]
pub struct CvjCounter {
    pub name: String,
    pub description: String,
    pub wall: Option<u16>,
    pub face: Option<Face>,
    pub position: DVec3,
    pub size: DVec3,
    pub rotation: DVec3,
}

/// Kontext pri prechode miestnosti.
#[derive(Clone, Copy)]
struct Ctx {
    world: DAffine3,
    wall: Option<u16>,
    face: Option<Face>,
    face_world: Option<DAffine3>,
}

/// Zostaví neutrálny obraz z prečítaného stromu.
pub(crate) fn build(root: &Node) -> (Vec<CvjRoom>, Vec<String>) {
    let mut notes = Vec::new();
    let mut rooms = Vec::new();
    for child in &root.children {
        if child.class == "CVFloorLevel" || child.asm_type == 1 {
            let mut room = CvjRoom {
                name: child.name.clone(),
                description: child.description.clone(),
                floor_shapes: match child.extra {
                    Extra::FloorLevel { shapes } => shapes,
                    _ => 0,
                },
                ..CvjRoom::default()
            };
            let ctx = Ctx {
                world: DAffine3::IDENTITY,
                wall: None,
                face: None,
                face_world: None,
            };
            for c in &child.children {
                visit(c, ctx, &mut room, &mut notes);
            }
            rooms.push(room);
        }
    }
    (rooms, notes)
}

fn visit(node: &Node, ctx: Ctx, room: &mut CvjRoom, notes: &mut Vec<String>) {
    if let Extra::Wall { number, prev, next } = node.extra {
        let frame = ctx.world * local(node, notes);
        let length = len(node, "DX", notes).unwrap_or(0.0);
        let wall = CvjWall {
            number,
            name: node.name.clone(),
            description: node.description.clone(),
            start: frame.transform_point3(DVec3::ZERO),
            end: frame.transform_point3(DVec3::new(length, 0.0, 0.0)),
            length,
            height: len(node, "DY", notes).unwrap_or(0.0),
            thickness: len(node, "DZ", notes).unwrap_or(0.0),
            angle: angle(node, "AY", notes).unwrap_or(0.0),
            inner_angle: angle(node, "_AI", notes),
            prev: (prev != 0).then_some(prev),
            next: (next != 0).then_some(next),
            frame,
        };
        let ctx = Ctx {
            world: frame,
            wall: Some(number),
            face: None,
            face_world: None,
        };
        room.walls.push(wall);
        for c in &node.children {
            visit(c, ctx, room, notes);
        }
        return;
    }
    if let Extra::WallFace { code } = node.extra {
        let face = match code & 0xFFFF {
            2 => Face::Back,
            1 => Face::Front,
            _ if node.name.to_lowercase().contains("back") => Face::Back,
            _ => Face::Front,
        };
        // predná plocha = rámec steny; zadná nesie vlastnú polohu v súradniciach miestnosti
        let world = match face {
            Face::Front => ctx.world,
            Face::Back => local(node, notes),
        };
        let ctx = Ctx {
            world,
            wall: ctx.wall,
            face: Some(face),
            face_world: Some(world),
        };
        for c in &node.children {
            visit(c, ctx, room, notes);
        }
        return;
    }
    if node.class == "CVCabinet" || node.asm_type == 9 {
        let mut nested = Vec::new();
        let cab = cabinet(node, ctx, &mut nested, notes);
        let world = cab.world;
        room.cabinets.push(cab);
        for (n, to_cab) in nested {
            let inner = Ctx {
                world: world * to_cab,
                ..ctx
            };
            // `n` je vnorená skrinka; jej rámec už obsahuje `to_cab`, preto rodič = world·to_cab·local⁻¹
            let parent = inner.world * local(n, notes).inverse();
            visit(
                n,
                Ctx {
                    world: parent,
                    ..ctx
                },
                room,
                notes,
            );
        }
        return;
    }
    if node.class == "CVCounter" || node.asm_type == 7 {
        room.counters.push(CvjCounter {
            name: node.name.clone(),
            description: node.description.clone(),
            wall: ctx.wall,
            face: ctx.face,
            position: vec3(node, ["X", "Y", "Z"], true, notes),
            size: vec3(node, ["DX", "DY", "DZ"], true, notes),
            rotation: vec3(node, ["AX", "AY", "AZ"], false, notes),
        });
        return;
    }
    // iné objekty (skupiny, lišty…): pokračujeme do detí
    let ctx = Ctx {
        world: ctx.world * local(node, notes),
        ..ctx
    };
    for c in &node.children {
        visit(c, ctx, room, notes);
    }
}

/// Príznaky predkov dielu vnútri skrinky.
#[derive(Clone, Copy, Default)]
struct PartFlags {
    in_front: bool,
    in_drawer_box: bool,
    in_group: bool,
}

/// Počty a diely, ktoré sa zbierajú pri prechode skrinky.
#[derive(Default)]
struct CabAcc {
    doors: usize,
    drawer_fronts: usize,
    drawer_boxes: usize,
    openings: usize,
    hardware: usize,
    connectors: Vec<String>,
    parts: Vec<CvjPart>,
    link: Option<CvjLink>,
}

const HARDWARE: &[&str] = &[
    "CVHardware",
    "CVHinge",
    "CVHingePlate",
    "CVGuide",
    "CVLegLeveler",
    "CVLeg",
    "CVPull",
    "CVSlideRail",
];

fn cabinet<'n>(
    node: &'n Node,
    ctx: Ctx,
    nested: &mut Vec<(&'n Node, DAffine3)>,
    notes: &mut Vec<String>,
) -> CvjCabinet {
    let world = ctx.world * local(node, notes);
    let in_face = ctx.face_world.map(|f| f.inverse() * world);
    let size = vec3(node, ["DX", "DY", "DZ"], true, notes);
    let mut acc = CabAcc::default();
    collect(
        node,
        DAffine3::IDENTITY,
        PartFlags::default(),
        &mut acc,
        nested,
        notes,
    );
    // diely korpusu s objemom (bez „modelov“ kovania s nulovým rozmerom)
    let carcass_parts: Vec<&CvjPart> = acc
        .parts
        .iter()
        .filter(|p| p.role == PartRole::Carcass && p.bbox.size().min_element() >= MIN_SOLID)
        .collect();
    let from_parts = carcass_parts.iter().map(|p| p.bbox).reduce(Bbox::union);
    let toe_height = len(node, "TOEH", notes);
    let (carcass, carcass_source) = match from_parts {
        Some(b) => (b, CarcassSource::Parts),
        None => {
            // bez dielov: obálka CV bez sokla
            let size = size.max(DVec3::ZERO);
            let toe = toe_height
                .filter(|t| *t > 0.0 && *t < size.y)
                .unwrap_or(0.0);
            (
                Bbox {
                    min: DVec3::new(0.0, toe, 0.0),
                    max: size,
                },
                CarcassSource::CvBox,
            )
        }
    };
    // šikmý bok: korpus z rozmerov CV, nie z obálky dielov (slant.rs)
    let carcass = crate::slant::carcass_box(&acc.parts, size, toe_height).unwrap_or(carcass);
    let shelves = carcass_parts
        .iter()
        .filter(|p| is_shelf(p, &carcass))
        .count();
    let carcass_part_count = carcass_parts.len();
    let sections = sections(node, notes);
    let mut attributes = attributes(node, notes);
    attributes.link = acc.link.take();
    CvjCabinet {
        object_id: node.object_id,
        name: node.name.clone(),
        description: node.description.clone(),
        comment: match node.param("COMMENT") {
            Some(Value::Text(t)) if !t.is_empty() => Some(t.clone()),
            _ => None,
        },
        class: CvClass::from_code(node.cv_class),
        type_code: node.cv_type,
        number: node
            .param("CABNO")
            .and_then(Value::as_f64)
            .filter(|v| v.is_finite() && v.abs() < 1e9)
            .map(|v| v.round() as i64),
        position: vec3(node, ["X", "Y", "Z"], true, notes),
        size,
        rotation: vec3(node, ["AX", "AY", "AZ"], false, notes),
        toe_height,
        wall: ctx.wall,
        face: ctx.face,
        world,
        in_face,
        doors: acc.doors,
        drawer_fronts: acc.drawer_fronts,
        drawer_boxes: acc.drawer_boxes,
        openings: acc.openings,
        hardware: acc.hardware,
        connectors: acc.connectors,
        shelves,
        parts: acc.parts,
        carcass,
        carcass_source,
        carcass_part_count,
        sections,
        attributes,
    }
}

/// Najviac uzlov stromu sekcií na skrinku a čiel na otvor (ochrana pred poškodeným súborom).
const MAX_SECTIONS: usize = 256;
const MAX_SECTION_FRONTS: usize = 16;

/// Strom sekcií: uzly s parametrom `ID` priamo v skupinách `Face` a `Interior`.
fn sections(cab: &Node, notes: &mut Vec<String>) -> Vec<CvjSection> {
    let mut out = Vec::new();
    for group_node in &cab.children {
        let group = match group_node.name.as_str() {
            "Face" => SectionGroup::Face,
            "Interior" => SectionGroup::Interior,
            _ => continue,
        };
        if group_node.asm_type != 14 {
            continue;
        }
        let group_frame = local(group_node, notes);
        for n in &group_node.children {
            let Some(id) = int(n, "ID") else {
                continue;
            };
            if out.len() >= MAX_SECTIONS {
                notes.push(tr!(
                    "import_cvj.too_many_sections",
                    name = &cab.name,
                    max = MAX_SECTIONS
                ));
                return out;
            }
            let frame = group_frame * local(n, notes);
            let size = vec3(n, ["DX", "DY", "DZ"], true, notes);
            let mut fronts = Vec::new();
            find_fronts(n, frame, &mut fronts, notes);
            out.push(CvjSection {
                group,
                code: n.name.clone(),
                asm_type: n.asm_type,
                id,
                parent: int(n, "PARID").unwrap_or(0),
                kind: SectionKind::from_code(int(n, "TYPE").unwrap_or(0)),
                style: int(n, "STYPE").unwrap_or(0),
                position: vec3(n, ["X", "Y", "Z"], true, notes),
                size,
                bbox: box_of(size, frame),
                equal: int(n, "EQ").is_some_and(|v| v > 0),
                params: opening_params(n, notes),
                fronts,
            });
        }
    }
    out
}

/// Čelá (`CVDoor`, ASM 11/12) v podstrome otvoru; `frame` = rámec `node` v skrinke.
fn find_fronts(node: &Node, frame: DAffine3, out: &mut Vec<CvjFront>, notes: &mut Vec<String>) {
    for child in &node.children {
        if out.len() >= MAX_SECTION_FRONTS {
            return;
        }
        let t = frame * local(child, notes);
        if matches!(child.asm_type, 11 | 12) {
            let size = vec3(child, ["DX", "DY", "DZ"], true, notes);
            let (mut hinges, mut blind_hinges) = (0, 0);
            count_hinges(child, &mut hinges, &mut blind_hinges);
            out.push(CvjFront {
                code: child.name.clone(),
                drawer: child.asm_type == 12,
                size,
                bbox: box_of(size, t),
                hinges,
                blind_hinges,
            });
            continue;
        }
        // korpus zásuvky a iné skupiny: čelo zásuvky je vnorené v `DBOX`
        if child.asm_type != 16 {
            find_fronts(child, t, out, notes);
        }
    }
}

fn count_hinges(node: &Node, hinges: &mut usize, blind: &mut usize) {
    for c in &node.children {
        if c.class == "CVHinge" && c.name == "HNG" {
            *hinges += 1;
            if let Some(Value::Text(t)) = c.param("Typ_Zavesov") {
                if t.to_lowercase().contains("slep") {
                    *blind += 1;
                }
            }
        }
        count_hinges(c, hinges, blind);
    }
}

fn opening_params(n: &Node, notes: &mut Vec<String>) -> OpeningParams {
    let mut l = |name: &str| len(n, name, notes);
    OpeningParams {
        reveal_left: l("REVL"),
        reveal_right: l("REVR"),
        reveal_top: l("REVT"),
        reveal_bottom: l("REVB"),
        overlay_left: l("OLL"),
        overlay_right: l("OLR"),
        overlay_top: l("OLT"),
        overlay_bottom: l("OLB"),
        front_gap: l("DOPEN"),
        hinge: int(n, "HNG"),
        hinge_top: l("HNGT"),
        hinge_bottom: l("HNGB"),
        hinge_count: int(n, "HNGQ"),
        box_height: l("BOXH"),
        box_depth: l("BOXD"),
        box_width: l("BOXW"),
        box_depth_cut: l("BDC"),
        box_top_clearance: l("BTC"),
        runner_length: l("NL"),
    }
}

/// Atribúty skrinky; mená parametrov podľa findings 11 §1.2.
fn attributes(n: &Node, notes: &mut Vec<String>) -> CvjAttributes {
    // TIP-ON má tri varianty tej istej premennej, platí tá, ktorá existuje (findings 11 §1.2)
    let variant = |base: &str| (1..=3).find_map(|i| int(n, &format!("{base}_{i}")));
    let mut deleted_legs = [None; 6];
    for (i, d) in deleted_legs.iter_mut().enumerate() {
        *d = flag(n, &format!("Zmazat_nohu_{}", i + 1));
    }
    CvjAttributes {
        apply_hardware: int(n, "APPLY_HW_CAB"),
        joint_type: int(n, "UCS_3DHWcabinet"),
        back_dado: int(n, "MAKEDADO"),
        dado_router: flag(n, "Cab_FRD"),
        delete_legs: flag(n, "Zmazat_nohu"),
        deleted_legs,
        shelf_support: int(n, "Policove_kovanie"),
        bumpers: flag(n, "Zapnut_cocky"),
        tip_on: variant("tip_on_zap_cab"),
        tip_on_color: variant("tip_on_bar_cab"),
        tip_on_kind: variant("tip_on_typ_cab"),
        back_type: int(n, "BKTYPE"),
        reveal_left: len(n, "REVL", notes),
        reveal_right: len(n, "REVR", notes),
        reveal_top: len(n, "REVT", notes),
        reveal_bottom: len(n, "REVB", notes),
        link: None,
    }
}

/// Spojenie s ľavou susedou z parametrov ľavého boku; `None`, ak bok atribút nemá.
fn link(n: &Node, notes: &mut Vec<String>) -> Option<CvjLink> {
    // dielňa má dva varianty tej istej premennej podľa nastavenia `Spoj_kov_miest` (findings 12 §8.2)
    let state =
        int(n, "Spojovacie_kovanie_zap_2").or_else(|| int(n, "Spojovacie_kovanie_zap_1"))?;
    let mut holes = Vec::new();
    for col in ["F", "B"] {
        for i in 1..=MAX_LINK_HOLES {
            if flag(n, &format!("Spojkov_{col}_on_{i}")) == Some(false) {
                continue;
            }
            let x = len(n, &format!("Spojkov_{col}_X_{i}"), notes);
            let y = len(n, &format!("Spojkov_{col}_Y_{i}"), notes);
            if let (Some(x), Some(y)) = (x, y) {
                holes.push((x, y));
            }
        }
    }
    Some(CvjLink {
        state,
        shift: len(n, "Posun_vrtania", notes),
        holes,
    })
}

/// Obálka kvádra `[0, size]` v rámci `frame`.
fn box_of(size: DVec3, frame: DAffine3) -> Bbox {
    Bbox::from_points(
        Bbox {
            min: DVec3::ZERO,
            max: size,
        }
        .corners()
        .map(|p| frame.transform_point3(p)),
    )
    .unwrap_or(Bbox {
        min: DVec3::ZERO,
        max: DVec3::ZERO,
    })
}

/// Celočíselný parameter (kód, výber); nečíselné a nezmyselné hodnoty → `None`.
fn int(node: &Node, name: &str) -> Option<i64> {
    let v = node.param(name)?.as_f64()?;
    (v.is_finite() && v.abs() < 1e9).then(|| v.round() as i64)
}

/// Pravdivostný parameter (bool alebo číslo ≠ 0).
fn flag(node: &Node, name: &str) -> Option<bool> {
    match node.param(name)? {
        Value::Bool(b) => Some(*b),
        v => v.as_f64().filter(|x| x.is_finite()).map(|x| x != 0.0),
    }
}

fn collect<'n>(
    node: &'n Node,
    to_cab: DAffine3,
    flags: PartFlags,
    acc: &mut CabAcc,
    nested: &mut Vec<(&'n Node, DAffine3)>,
    notes: &mut Vec<String>,
) {
    for child in &node.children {
        let t = to_cab * local(child, notes);
        if child.class == "CVCabinet" || child.asm_type == 9 {
            nested.push((child, t));
            continue;
        }
        match child.asm_type {
            11 => acc.doors += 1,
            12 => acc.drawer_fronts += 1,
            45 => acc.drawer_boxes += 1,
            15 | 29 | 30 => acc.openings += 1,
            _ => {}
        }
        if HARDWARE.contains(&child.class.as_str()) {
            acc.hardware += 1;
            // spojovací prvok medzi kolíkmi (`HW2`): jeho názov v zákazke
            if child.name == "HW2"
                && acc.connectors.len() < 8
                && !acc.connectors.contains(&child.description)
            {
                acc.connectors.push(child.description.clone());
            }
        }
        if child.asm_type == 16 {
            let size = vec3(child, ["DX", "DY", "DZ"], true, notes);
            let bbox = Bbox::from_points(
                Bbox {
                    min: DVec3::ZERO,
                    max: size,
                }
                .corners()
                .map(|p| t.transform_point3(p)),
            )
            .unwrap_or(Bbox {
                min: DVec3::ZERO,
                max: DVec3::ZERO,
            });
            let role = if flags.in_front {
                PartRole::Front
            } else if flags.in_drawer_box {
                PartRole::DrawerBox
            } else if !flags.in_group {
                PartRole::Loose
            } else {
                PartRole::Carcass
            };
            // spojovacie kovanie skriniek je atribút ľavého boku korpusu
            if role == PartRole::Carcass
                && acc.link.is_none()
                && matches!(child.name.trim(), "LU" | "LF")
            {
                acc.link = link(child, notes);
            }
            acc.parts.push(CvjPart {
                code: child.name.clone(),
                description: child.description.clone(),
                pid: child
                    .param("PID")
                    .and_then(Value::as_f64)
                    .filter(|v| v.is_finite()),
                size,
                position: vec3(child, ["X", "Y", "Z"], true, notes),
                rotation: vec3(child, ["AX", "AY", "AZ"], false, notes),
                to_cabinet: t,
                bbox,
                role,
                band: match child.param("BAND") {
                    Some(Value::Text(t)) => Some(t.clone()),
                    _ => None,
                },
                ncid: int(child, "NCID"),
            });
        }
        let inner = PartFlags {
            in_front: flags.in_front || matches!(child.asm_type, 11 | 12),
            in_drawer_box: flags.in_drawer_box || child.asm_type == 45,
            in_group: true,
        };
        collect(child, t, inner, acc, nested, notes);
    }
}

/// Heuristika police: vodorovný diel vnútri korpusu (nie dno ani vrch) cez väčšinu
/// šírky a hĺbky, alebo diel s menom „polica“/„shelf“.
fn is_shelf(p: &CvjPart, carcass: &Bbox) -> bool {
    let s = p.bbox.size();
    let c = carcass.size();
    // doska: hrúbka vo zvislom smere, plocha aspoň 50 × 50 mm
    let horizontal = s.y >= 3.0 && s.y < s.x && s.y < s.z && s.x >= 50.0 && s.z >= 50.0;
    if !horizontal {
        return false;
    }
    let text = format!("{} {}", p.code, p.description).to_lowercase();
    if text.contains("polic") || text.contains("shelf") {
        return true;
    }
    let inner =
        p.bbox.min.y >= carcass.min.y + s.y + 1.0 && p.bbox.max.y <= carcass.max.y - s.y - 1.0;
    inner && s.x >= 0.5 * c.x && s.z >= 0.5 * c.z
}

/// Číselný parameter (bez prevodu); NaN, ∞ a nezmyselne veľké hodnoty → `None` s poznámkou.
fn raw(node: &Node, name: &str, notes: &mut Vec<String>) -> Option<f64> {
    let v = node.param(name)?.as_f64()?;
    if v.is_finite() && v.abs() * MM_PER_INCH <= MAX_MM {
        Some(v)
    } else {
        notes.push(tr!(
            "import_cvj.bad_param",
            object = &node.name,
            param = name,
            value = v.to_string()
        ));
        None
    }
}

/// Dĺžka v mm.
fn len(node: &Node, name: &str, notes: &mut Vec<String>) -> Option<f64> {
    raw(node, name, notes).map(|v| v * MM_PER_INCH)
}

/// Uhol v stupňoch.
fn angle(node: &Node, name: &str, notes: &mut Vec<String>) -> Option<f64> {
    raw(node, name, notes)
}

fn vec3(node: &Node, names: [&str; 3], length: bool, notes: &mut Vec<String>) -> DVec3 {
    let [a, b, c] = names.map(|n| {
        if length {
            len(node, n, notes)
        } else {
            angle(node, n, notes)
        }
        .unwrap_or(0.0)
    });
    DVec3::new(a, b, c)
}

/// Rámec objektu voči rodičovi: `T(X,Y,Z) · Rz(AZ) · Ry(AY) · Rx(AX)`.
pub(crate) fn local(node: &Node, notes: &mut Vec<String>) -> DAffine3 {
    let t = vec3(node, ["X", "Y", "Z"], true, notes);
    let a = vec3(node, ["AX", "AY", "AZ"], false, notes);
    frame(t, a)
}

/// `T(t) · Rz(a.z) · Ry(a.y) · Rx(a.x)`, uhly v stupňoch.
pub fn frame(t: DVec3, a: DVec3) -> DAffine3 {
    let rot = DMat3::from_rotation_z(a.z.to_radians())
        * DMat3::from_rotation_y(a.y.to_radians())
        * DMat3::from_rotation_x(a.x.to_radians());
    DAffine3::from_mat3_translation(rot, t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_order_rz_ry_rx() {
        // ľavý bok: X = 18, AY = −90 → hrúbka (lokálne +Z) smeruje do −X
        let f = frame(DVec3::new(18.0, 0.0, 0.0), DVec3::new(0.0, -90.0, 0.0));
        let p = f.transform_point3(DVec3::new(0.0, 0.0, 18.0));
        assert!((p - DVec3::ZERO).length() < 1e-9, "{p}");
        // koniec steny = začiatok + L·(cos AY, 0, −sin AY)
        let w = frame(DVec3::ZERO, DVec3::new(0.0, 30.0, 0.0));
        let e = w.transform_point3(DVec3::new(10.0, 0.0, 0.0));
        let a = 30f64.to_radians();
        assert!((e - DVec3::new(10.0 * a.cos(), 0.0, -10.0 * a.sin())).length() < 1e-9);
    }

    #[test]
    fn class_codes() {
        assert_eq!(CvClass::from_code(2), CvClass::Upper);
        assert_eq!(CvClass::from_code(20), CvClass::Other(20));
        assert_eq!(cabinet_type_name(17), Some("Sink"));
        assert_eq!(cabinet_type_name(99), None);
    }
}
