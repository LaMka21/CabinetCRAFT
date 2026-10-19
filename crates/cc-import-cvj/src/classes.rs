//! Prechod prúdu `Contents`: rozloženie polí jednotlivých tried podľa popisu formátu
//! (vlastný popis formátu `findings/07-cvj-format.md`, §2–§4).
//!
//! Archív MFC nemá dĺžky objektov, preto každú triedu treba prečítať pole po poli,
//! aj keď väčšinu hodnôt nepotrebujeme. Výsledkom je strom [`Node`] so všetkými
//! „assembly“ objektmi (job, miestnosť, steny, plochy, skrinky, skupiny, diely…)
//! a ich pomenovanými parametrami. Ostatné triedy (rovnice, pakety, 2D geometria)
//! sa iba prečítajú a zahodia.

use crate::archive::{ObjectTag, Reader, TagEvent};
use crate::CvjError;

/// Najväčšia hĺbka vnorenia objektov (ochrana zásobníka pred poškodeným súborom).
pub const MAX_DEPTH: usize = 64;

/// Hodnota parametra (`CXResult`).
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// dĺžka v palcoch (typ 1)
    Length(f64),
    /// uhol v stupňoch (typ 2)
    Angle(f64),
    /// celé číslo / výber (typ 4)
    Int(i32),
    /// pravdivostná hodnota (typ 5)
    Bool(bool),
    /// text (typ 8)
    Text(String),
    /// bez znamienka (typ 10)
    UInt(u32),
    /// ostatné čísla (typ 6 a iné)
    Number(f64),
}

impl Value {
    /// Číselná hodnota; dĺžky v palcoch (bez prevodu).
    pub fn as_f64(&self) -> Option<f64> {
        match *self {
            Value::Length(v) | Value::Angle(v) | Value::Number(v) => Some(v),
            Value::Int(v) => Some(v as f64),
            Value::UInt(v) => Some(v as f64),
            Value::Bool(b) => Some(if b { 1.0 } else { 0.0 }),
            Value::Text(_) => None,
        }
    }
}

/// Pomenovaný parameter objektu (`CVParameter`).
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub value: Value,
    /// začiatok objektu parametra (značka) a jeho koniec v prúde
    pub at: usize,
    pub end: usize,
    /// poloha hodnoty (za typom v `CXResult`) a typ hodnoty
    pub value_at: usize,
    pub value_type: u32,
    /// ObjectID parametra a jeho poloha (pri klonovaní treba nenulové ID zmeniť)
    pub object_id: u32,
    pub id_at: usize,
}

/// Údaje špecifické pre niektoré triedy.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Extra {
    #[default]
    None,
    /// `CVWall`: číslo steny, predchádzajúca a nasledujúca stena (0 = žiadna)
    Wall { number: u16, prev: u16, next: u16 },
    /// `CVWallFace`: vlastný kód plochy (číslo steny << 16 | 1 predná / 2 zadná)
    WallFace { code: u32 },
    /// `CVFloorLevel`: počet tvarov obrysu podlahy
    FloorLevel { shapes: usize },
}

/// Objekt typu „assembly“ (odvodený od `CVAsmBase`) so svojimi deťmi.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Node {
    /// meno triedy MFC (konštanta formátu, napr. `CVCabinet`)
    pub class: String,
    /// pozícia začiatku objektu v prúde
    pub pos: usize,
    pub object_id: u32,
    pub name: String,
    pub description: String,
    /// ASM_TYPE (§6.1)
    pub asm_type: u16,
    /// CLASS (§6.2)
    pub cv_class: u16,
    /// TYPE (§6.3)
    pub cv_type: u16,
    pub params: Vec<Param>,
    pub children: Vec<Node>,
    pub extra: Extra,
    /// koniec objektu v prúde
    pub end: usize,
    /// poloha ObjectID (u32)
    pub id_at: usize,
    /// poloha počtu parametrov (u16)
    pub params_count_at: usize,
    /// poloha počtu detí (u16) v `LoadDataAndChildren`, ak trieda deti má
    pub children_count_at: Option<usize>,
    /// poloha príznakov `CVAsmBase` +0x78 (u32: dolné 3 bity = natiahnutie W/H/D)
    pub flags_at: Option<usize>,
    /// poloha väzby na katalóg +0x44 (u32 za počtom detí v `LoadDataAndChildren`)
    pub link_at: Option<usize>,
    /// `CVCabinet`: poloha nominálneho rozmeru +0x150 (3× f64, palce)
    pub nominal_at: Option<usize>,
    /// poloha mena (`CString` za ObjectID) a jeho dĺžka v bajtoch; popis nasleduje hneď za
    /// menom (`name_at + name_len`) a má `desc_len` bajtov
    pub name_at: usize,
    pub name_len: usize,
    pub desc_len: usize,
    /// `CVWall`: poloha čísla steny (7× u16: číslo, typ, ?, ľavý sused, pravý sused, …);
    /// `CVWallFace`: poloha vlastného kódu plochy (3× u32: kód, ľavá a pravá pripojená plocha)
    pub extra_at: Option<usize>,
}

impl Node {
    pub fn param(&self, name: &str) -> Option<&Value> {
        self.params
            .iter()
            .find(|p| p.name == name)
            .map(|p| &p.value)
    }
}

/// Výsledok čítania jedného objektu.
enum Obj {
    Null,
    Reference,
    Node(Box<Node>),
    Param(Param),
    /// iný objekt (prečítaný a zahodený)
    Other,
}

/// Čo sa zistilo pri prechode celého prúdu.
#[derive(Debug, Clone, PartialEq)]
pub struct Walk {
    pub root: Node,
    /// pozícia konca koreňového objektu
    pub root_end: usize,
    /// triedy zástupných objektov za koreňom
    pub trailing: Vec<String>,
    /// chyba pri čítaní objektov za koreňom (koreň je napriek tomu celý)
    pub trailing_error: Option<String>,
    /// všetky bajty prúdu sa prečítali
    pub consumed_all: bool,
    pub objects: usize,
}

/// Zásobník vlákna, na ktorom beží rekurzívny prechod (hlboké vnorenie v ladiacom
/// preklade potrebuje viac než predvolený zásobník hlavného vlákna Windows).
const WALK_STACK: usize = 64 * 1024 * 1024;

/// Prejde celý prúd `Contents`. Koreň musí byť `CVJob`.
///
/// Prechod beží na samostatnom vlákne s veľkým zásobníkom, aby ani najhlbšie
/// povolené vnorenie ([`MAX_DEPTH`]) nevyčerpalo zásobník volajúceho.
pub fn walk(data: &[u8]) -> Result<Walk, CvjError> {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .name("cvj-walk".into())
            .stack_size(WALK_STACK)
            .spawn_scoped(scope, || walk_here(data))
            .map_err(CvjError::Io)?
            .join()
            .unwrap_or(Err(CvjError::Unsupported {
                what: "vnútorná chyba pri čítaní úlohy",
                pos: 0,
            }))
    })
}

/// Ako [`walk`], navyše vráti všetky značky objektov v poradí čítania (pre bezstratový
/// zápis `Contents`, crate `cc-out-cvj`).
pub fn walk_recorded(data: &[u8]) -> Result<(Walk, Vec<TagEvent>), CvjError> {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .name("cvj-walk".into())
            .stack_size(WALK_STACK)
            .spawn_scoped(scope, || {
                let mut r = Reader::recording(data);
                let walk = walk_with(&mut r)?;
                Ok((walk, r.take_events()))
            })
            .map_err(CvjError::Io)?
            .join()
            .unwrap_or(Err(CvjError::Unsupported {
                what: "vnútorná chyba pri čítaní úlohy",
                pos: 0,
            }))
    })
}

fn walk_here(data: &[u8]) -> Result<Walk, CvjError> {
    walk_with(&mut Reader::new(data))
}

fn walk_with(reader: &mut Reader<'_>) -> Result<Walk, CvjError> {
    let r = std::mem::replace(reader, Reader::new(&[]));
    let mut w = Walker { r };
    let out = walk_inner(&mut w);
    *reader = w.r;
    out
}

fn walk_inner(w: &mut Walker<'_>) -> Result<Walk, CvjError> {
    let root = match w.object(0)? {
        Obj::Node(n) if n.class == "CVJob" => *n,
        Obj::Node(n) => {
            return Err(CvjError::UnexpectedClass {
                pos: n.pos,
                expected: "CVJob",
                found: n.class,
            })
        }
        _ => {
            return Err(CvjError::UnexpectedClass {
                pos: 0,
                expected: "CVJob",
                found: "(nič)".to_owned(),
            })
        }
    };
    let root_end = w.r.pos();
    let mut trailing = Vec::new();
    let mut trailing_error = None;
    while w.r.remaining() > 0 {
        let at = w.r.pos();
        match w.r.object_tag() {
            Ok(ObjectTag::New { class, .. }) => {
                if PLACEHOLDERS.contains(&class.as_str()) {
                    trailing.push(class);
                } else {
                    // iný objekt za koreňom: skúsime ho prečítať bežným spôsobom
                    match w.body(&class, at, 1) {
                        Ok(_) => trailing.push(class),
                        Err(e) => {
                            trailing_error = Some(e.to_string());
                            break;
                        }
                    }
                }
            }
            Ok(_) => {}
            Err(e) => {
                trailing_error = Some(e.to_string());
                break;
            }
        }
    }
    Ok(Walk {
        root,
        root_end,
        trailing,
        consumed_all: trailing_error.is_none() && w.r.remaining() == 0,
        trailing_error,
        objects: w.r.object_count(),
    })
}

/// Zástupné objekty za koreňom s prázdnym telom (§4.12).
const PLACEHOLDERS: &[&str] = &[
    "CVCatalogDataList",
    "CVMatDataListX",
    "CVConstDataListX",
    "CVDoorDataList",
    "CVTemplateList",
    "CVDrawManager",
    "CVLightSchedList",
];

/// „Listové“ triedy: telo = AB + schéma (§4.7).
const LEAF_ASSEMBLIES: &[&str] = &[
    "CVHardware",
    "CVHinge",
    "CVHingePlate",
    "CVGuide",
    "CVLegLeveler",
    "CVLineboreOp",
    "CVDadoExOp",
    "CVLeg",
    "CVPull",
    "CVSlideRail",
];

/// Všetky triedy, ktorých rozloženie importér pozná.
pub fn known_classes() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = vec![
        "CVJob",
        "CVFloorLevel",
        "CVRoom",
        "CVWall",
        "CVWallFace",
        "CVCabinet",
        "CVAssembly",
        "CVWidget",
        "CVMolding",
        "CVDesignCat",
        "CVDoor",
        "CVCounter",
        "CVSplash",
        "CVRouteOp",
        "CVSmartFace",
        "CVParameter",
        "CVEquation",
        "CVOptOption",
        "CVOptEffect",
        "CVTemplate2d",
        "CVTemplate3d",
        "CVVertexNode",
        "CVTemplateNode",
        "CXVTemplate",
        "CVDataPacket",
        "CVShapePacket",
        "CVBandPacket",
        "CXLine2d",
        "CXArc2d",
        "CXCoincidentRel2d",
        "CXTangentRel2d",
        "CXFixedRel2d",
        "CXHorzVertRel2d",
        "CXParameter",
        "CXIdLst",
    ];
    v.extend_from_slice(LEAF_ASSEMBLIES);
    v.extend_from_slice(PLACEHOLDERS);
    v
}

struct Walker<'a> {
    r: Reader<'a>,
}

impl Walker<'_> {
    /// Načíta jeden objekt (značka + telo).
    fn object(&mut self, depth: usize) -> Result<Obj, CvjError> {
        let at = self.r.pos();
        if depth > MAX_DEPTH {
            return Err(CvjError::TooDeep { pos: at });
        }
        match self.r.object_tag()? {
            ObjectTag::Null => Ok(Obj::Null),
            ObjectTag::Reference(_) => Ok(Obj::Reference),
            ObjectTag::New { class, .. } => self.body(&class, at, depth),
        }
    }

    /// `n` objektov za sebou; vráti iba uzly „assembly“.
    fn objects(&mut self, what: &'static str, n: i64, depth: usize) -> Result<Vec<Node>, CvjError> {
        let n = self.r.check_count(what, n, 2)?;
        let mut out = Vec::new();
        for _ in 0..n {
            if let Obj::Node(node) = self.object(depth)? {
                out.push(*node);
            }
        }
        Ok(out)
    }

    /// Telo objektu triedy `class`, ktorého značka začala na `at`.
    fn body(&mut self, class: &str, at: usize, depth: usize) -> Result<Obj, CvjError> {
        let d = depth + 1;
        let mut node = Node {
            class: class.to_owned(),
            pos: at,
            ..Node::default()
        };
        match class {
            "CVJob" => {
                self.asm_base(&mut node, d)?;
                self.job_body()?;
                self.data_and_children(&mut node, d)?;
            }
            "CVFloorLevel" => {
                self.asm_base(&mut node, d)?;
                self.floor_level_body(&mut node, d)?;
                self.data_and_children(&mut node, d)?;
            }
            "CVRoom" => self.asm_base(&mut node, d)?,
            "CVWall" => {
                self.asm_base(&mut node, d)?;
                self.r.schema()?;
                node.extra_at = Some(self.r.pos());
                let number = self.r.u16()?;
                self.r.u16()?;
                self.r.u16()?;
                let prev = self.r.u16()?;
                let next = self.r.u16()?;
                self.r.u16()?;
                self.r.u16()?;
                self.r.u32()?;
                self.r.u32()?;
                self.r.u16()?;
                node.extra = Extra::Wall { number, prev, next };
                self.data_and_children(&mut node, d)?;
            }
            "CVWallFace" => {
                self.asm_base(&mut node, d)?;
                self.r.schema()?;
                node.extra_at = Some(self.r.pos());
                let code = self.r.u32()?;
                self.r.u32()?;
                self.r.u32()?;
                node.extra = Extra::WallFace { code };
                self.data_and_children(&mut node, d)?;
            }
            "CVCabinet" => {
                self.asm_base(&mut node, d)?;
                self.r.schema()?;
                self.r.u16()?;
                self.r.u32()?;
                self.r.u32()?;
                node.nominal_at = Some(self.r.pos());
                self.r.point3()?; // nominálny rozmer
                self.cat_options(d)?;
                self.r.u32()?;
                self.r.u32()?;
                self.r.u16()?;
                self.r.point3()?;
                self.r.point3()?;
                self.r.u32()?;
                self.data_and_children(&mut node, d)?;
            }
            "CVAssembly" | "CVWidget" | "CVMolding" => {
                self.asm_base(&mut node, d)?;
                self.r.schema()?;
                self.data_and_children(&mut node, d)?;
            }
            "CVDesignCat" => {
                // podľa popisu neoverené na dátach (§4.7)
                self.asm_base(&mut node, d)?;
                self.r.schema()?;
                self.cat_options(d)?;
                self.data_and_children(&mut node, d)?;
            }
            "CVDoor" => {
                self.asm_base(&mut node, d)?;
                self.r.schema()?;
                // cena COleCurrency: stav, nízke a vysoké slovo
                self.r.i32()?;
                self.r.u32()?;
                self.r.i32()?;
                self.data_and_children(&mut node, d)?;
            }
            "CVCounter" => {
                self.asm_base(&mut node, d)?;
                self.r.schema()?;
                self.r.u16()?;
                self.r.u32()?;
                let n = self.r.u16()?;
                let n = self
                    .r
                    .check_count("delenia pracovnej dosky", n.into(), 14)?;
                for _ in 0..n {
                    self.r.schema()?;
                    self.r.f64()?;
                    self.r.u32()?;
                }
                self.data_and_children(&mut node, d)?;
            }
            "CVSplash" => {
                self.asm_base(&mut node, d)?;
                self.r.schema()?;
                self.r.u16()?;
                self.data_and_children(&mut node, d)?;
            }
            "CVRouteOp" => {
                self.asm_base(&mut node, d)?;
                let s = self.r.schema()?;
                if s.minor() < 2 {
                    self.r.u32()?;
                    self.r.u32()?;
                } else {
                    self.r.schema()?;
                    self.r.u32()?;
                    self.r.u32()?;
                }
            }
            "CVSmartFace" => {
                self.asm_base(&mut node, d)?;
                self.r.schema()?;
                self.r.u32()?;
            }
            c if LEAF_ASSEMBLIES.contains(&c) => {
                self.asm_base(&mut node, d)?;
                self.r.schema()?;
            }
            "CVParameter" => {
                return self.parameter(d).map(|mut p| {
                    p.at = at;
                    p.end = self.r.pos();
                    Obj::Param(p)
                })
            }
            "CVEquation" => {
                let s = self.r.schema()?;
                if s.minor() < 2 {
                    self.r.short_ansi()?;
                    self.r.short_ansi()?;
                } else {
                    self.r.string()?;
                    self.r.string()?;
                    if s.minor() >= 3 {
                        self.r.u8()?;
                    }
                }
                return Ok(other());
            }
            "CVOptOption" => {
                self.r.schema()?;
                self.r.u32()?;
                self.r.u32()?;
                self.r.u16()?;
                self.r.string()?;
                self.r.string()?;
                return Ok(other());
            }
            "CVOptEffect" => {
                self.r.schema()?;
                self.r.u32()?;
                self.r.u32()?;
                self.r.u16()?;
                self.r.u16()?;
                self.r.f64()?;
                self.r.u16()?;
                return Ok(other());
            }
            "CVTemplate2d" => {
                self.template_base(&mut node)?;
                self.r.schema()?;
                self.r.f64()?;
                if let Obj::Null = self.object(d)? {
                    let n = self.r.i32()?;
                    self.objects("uzly šablóny", n.into(), d)?;
                }
                return Ok(other());
            }
            "CVTemplate3d" => {
                self.template_base(&mut node)?;
                self.r.schema()?;
                let n = self.r.i32()?;
                self.objects("vrcholy 3D šablóny", n.into(), d)?;
                let m = self.r.i32()?;
                if m != 0 {
                    // rozloženie `CVFaceData` nie je popísané (§11)
                    return Err(CvjError::Unsupported {
                        what: "CVFaceData (plochy 3D šablóny)",
                        pos: self.r.pos(),
                    });
                }
                return Ok(other());
            }
            "CVVertexNode" => {
                self.r.point3()?;
                self.r.u32()?;
                return Ok(other());
            }
            "CVTemplateNode" => {
                self.r.point3()?;
                self.r.u32()?;
                self.r.schema()?;
                self.r.f64()?;
                self.r.f64()?;
                return Ok(other());
            }
            "CXVTemplate" => {
                // profil
                let s = self.r.schema()?;
                self.r.u32()?;
                let n = self.r.i32()?;
                self.objects("entity profilu", n.into(), d)?;
                if s.minor() > 1 {
                    let m = self.r.i32()?;
                    self.objects("väzby profilu", m.into(), d)?;
                }
                self.r.schema()?;
                self.r.u32()?;
                self.r.u32()?;
                // zvyšok šablóny
                self.r.u32()?;
                self.r.u32()?;
                self.r.point3()?;
                self.r.point3()?;
                self.r.f64()?;
                self.r.u32()?;
                return Ok(other());
            }
            "CVDataPacket" | "CVShapePacket" => {
                self.r.schema()?;
                self.r.u32()?;
                return Ok(other());
            }
            "CVBandPacket" => {
                self.r.schema()?;
                self.r.u32()?;
                self.r.schema()?;
                let flags = self.r.u16()?;
                for _ in 0..4 {
                    self.r.f64()?;
                }
                if flags & 1 != 0 {
                    self.object(d)?;
                }
                return Ok(other());
            }
            "CXLine2d" => {
                self.curve2d(d)?;
                self.r.schema()?;
                return Ok(other());
            }
            "CXArc2d" => {
                self.curve2d(d)?;
                self.r.schema()?;
                self.r.point2()?;
                return Ok(other());
            }
            "CXCoincidentRel2d" | "CXTangentRel2d" => {
                self.rel2d(d)?;
                return Ok(other());
            }
            "CXFixedRel2d" | "CXHorzVertRel2d" => {
                self.rel2d(d)?;
                self.r.schema()?;
                self.r.u8()?;
                return Ok(other());
            }
            "CXParameter" => {
                self.r.schema()?;
                self.r.string()?;
                self.r.u32()?;
                self.result()?;
                return Ok(other());
            }
            "CXIdLst" => {
                self.r.schema()?;
                self.r.skip(4)?;
                let n = self.r.count()?;
                let n = self.r.check_count("zoznam identifikátorov", n.into(), 8)?;
                self.r.skip(n * 8)?;
                return Ok(other());
            }
            c if PLACEHOLDERS.contains(&c) => return Ok(other()),
            _ => {
                return Err(CvjError::UnknownClass {
                    class: class.to_owned(),
                    pos: at,
                })
            }
        }
        node.end = self.r.pos();
        Ok(Obj::Node(Box::new(node)))
    }

    /// `CXBase` + `CVBase` (§2.3, #1–8).
    fn cv_base(&mut self, node: &mut Node) -> Result<(), CvjError> {
        self.r.schema()?;
        node.id_at = self.r.pos();
        node.object_id = self.r.u32()?;
        node.name_at = self.r.pos();
        node.name = self.r.string()?;
        node.name_len = self.r.pos() - node.name_at;
        let desc_at = self.r.pos();
        node.description = self.r.string()?;
        node.desc_len = self.r.pos() - desc_at;
        self.r.u32()?;
        self.r.schema()?;
        node.asm_type = self.r.u16()?;
        self.r.u32()?;
        Ok(())
    }

    /// `CVAsmBase` (§2.3, #1–21) vrátane zoznamu parametrov.
    fn asm_base(&mut self, node: &mut Node, depth: usize) -> Result<(), CvjError> {
        self.cv_base(node)?;
        // CVParBase
        self.r.schema()?;
        node.params_count_at = self.r.pos();
        let n = self.r.u16()?;
        let n = self.r.check_count("parametre", n.into(), 2)?;
        for _ in 0..n {
            if let Obj::Param(p) = self.object(depth)? {
                node.params.push(p);
            }
        }
        // CVAsmBase
        let s = self.r.schema()?;
        node.cv_class = self.r.u16()?;
        node.cv_type = self.r.u16()?;
        node.flags_at = Some(self.r.pos());
        self.r.u32()?;
        self.r.u32()?;
        self.r.u16()?;
        self.r.u32()?;
        self.r.u32()?;
        if s.minor() > 2 {
            self.r.u32()?;
        }
        if s.minor() > 1 {
            self.object(depth)?; // 2D profil alebo null
        }
        Ok(())
    }

    /// `LoadDataAndChildren`: dátové pakety a deti (§2.3).
    fn data_and_children(&mut self, node: &mut Node, depth: usize) -> Result<(), CvjError> {
        let s = self.r.schema()?;
        let p = self.r.u16()?;
        self.objects("dátové pakety", p.into(), depth)?;
        node.children_count_at = Some(self.r.pos());
        let c = self.r.u16()?;
        node.link_at = Some(self.r.pos());
        self.r.u32()?;
        if s.minor() < 2 {
            self.r.u32()?;
            self.r.u32()?;
        }
        node.children = self.objects("deti", c.into(), depth)?;
        Ok(())
    }

    /// Telo `CVJob` za AB po deti (§4.2). Údaje zákazníka sa čítajú, ale nikam neukladajú.
    fn job_body(&mut self) -> Result<(), CvjError> {
        let s = self.r.schema()?;
        let v = s.minor();
        self.r.u16()?;
        self.r.u32()?;
        for _ in 0..3 {
            self.r.u32()?;
        }
        for _ in 0..2 {
            self.r.string()?;
        }
        for _ in 0..3 {
            self.r.u32()?;
        }
        if v >= 3 {
            self.r.i16()?;
        }
        self.r.u32()?;
        self.r.u16()?;
        self.r.u32()?;
        for _ in 0..7 {
            self.r.u32()?;
        }
        for _ in 0..7 {
            self.r.f64()?;
        }
        for _ in 0..10 {
            self.r.string()?;
        }
        let n = self.r.u16()?;
        let n = self.r.check_count("položky ponuky", n.into(), 33)?;
        for _ in 0..n {
            self.r.u32()?;
            self.r.string()?;
            self.r.u32()?;
            self.r.f64()?;
            self.r.f64()?;
            self.r.f64()?;
        }
        self.r.u32()?;
        for _ in 0..10 {
            self.r.string()?;
        }
        if v < 5 {
            self.r.u32()?;
            self.r.u32()?;
            self.r.f64()?;
        }
        for _ in 0..6 {
            self.r.string()?;
        }
        self.r.u32()?;
        self.r.string()?;
        if s.major() > 9 || (s.major() == 9 && v >= 1) {
            self.r.u32()?;
        }
        if v >= 4 {
            self.r.u32()?;
        }
        if v >= 6 {
            self.r.u32()?;
        }
        if v >= 7 {
            self.r.u32()?;
        }
        Ok(())
    }

    /// Telo `CVFloorLevel` za AB po deti (§4.3).
    fn floor_level_body(&mut self, node: &mut Node, depth: usize) -> Result<(), CvjError> {
        let s = self.r.schema()?;
        let r = self.r.u16()?;
        node.children
            .extend(self.objects("miestnosti", r.into(), depth)?);
        let mut shapes = 0usize;
        if s.minor() > 1 {
            let k: i64 = if s.minor() >= 3 {
                self.r.i32()?.into()
            } else {
                1
            };
            let k = self.r.check_count("tvary podlahy", k, 4)?;
            for _ in 0..k {
                let n = self.r.i32()?;
                if n != 0 {
                    let n = self.r.check_count("body obrysu podlahy", n.into(), 24)?;
                    self.r.skip(n * 24)?;
                    let loops = self.r.i32()?;
                    let loops = self
                        .r
                        .check_count("slučky obrysu podlahy", loops.into(), 4)?;
                    for _ in 0..loops {
                        let m = self.r.i32()?;
                        let m = self.r.check_count("indexy slučky", m.into(), 4)?;
                        self.r.skip(m * 4)?;
                    }
                    shapes += 1;
                }
            }
        }
        node.extra = Extra::FloorLevel { shapes };
        Ok(())
    }

    /// Vložené `CVCatOptions` (§4.6).
    fn cat_options(&mut self, depth: usize) -> Result<(), CvjError> {
        self.r.schema()?;
        let n = self.r.i32()?;
        self.objects("voľby katalógu", n.into(), depth)?;
        let m = self.r.i32()?;
        self.objects("účinky volieb", m.into(), depth)?;
        Ok(())
    }

    /// Báza `CVTemplate` (§4.8).
    fn template_base(&mut self, node: &mut Node) -> Result<(), CvjError> {
        self.cv_base(node)?;
        self.r.schema()?;
        self.r.u32()?;
        self.r.u32()?;
        self.r.point3()?;
        self.r.point3()?;
        Ok(())
    }

    /// `CVParameter` (§3.1).
    fn parameter(&mut self, depth: usize) -> Result<Param, CvjError> {
        let mut base = Node::default();
        self.cv_base(&mut base)?;
        let s = self.r.schema()?;
        if s.minor() < 2 {
            return Err(CvjError::Unsupported {
                what: "zastaraný formát parametra (schéma < 2)",
                pos: self.r.pos(),
            });
        }
        let flags = self.r.u32()?;
        let equations = flags & 0xFFFF;
        if s.minor() >= 3 {
            self.r.u32()?;
        } else {
            self.r.u16()?;
        }
        let (value, value_at, value_type) = self.result_at()?;
        self.objects("rovnice", equations.into(), depth)?;
        self.r.string()?;
        if s.minor() > 3 {
            self.r.u16()?;
        }
        Ok(Param {
            name: base.name,
            value,
            at: 0,
            end: 0,
            value_at,
            value_type,
            object_id: base.object_id,
            id_at: base.id_at,
        })
    }

    /// `CXResult` s polohou hodnoty a typom (pre zápis): schéma u16, typ u32, hodnota.
    fn result_at(&mut self) -> Result<(Value, usize, u32), CvjError> {
        let start = self.r.pos();
        let value = self.result()?;
        let value_type = self.r.u32_at(start + 2).unwrap_or(0);
        Ok((value, start + 6, value_type))
    }

    /// `CXResult` / `CXValue` (§3.3).
    fn result(&mut self) -> Result<Value, CvjError> {
        let s = self.r.schema()?;
        let t = self.r.u32()?;
        if s.minor() <= 1 {
            return Ok(if t == 8 {
                Value::Text(self.r.string()?)
            } else {
                number(t, self.r.f64()?)
            });
        }
        Ok(match t {
            4 => Value::Int(self.r.i32()?),
            5 => Value::Bool(self.r.i32()? != 0),
            8 => Value::Text(self.r.string()?),
            10 => Value::UInt(self.r.u32()?),
            _ => number(t, self.r.f64()?),
        })
    }

    /// Báza `CXGeometry2d` (§4.10).
    fn geometry2d(&mut self, depth: usize) -> Result<(), CvjError> {
        self.r.schema()?;
        self.r.i32()?;
        self.object(depth)?; // CXIdLst alebo null
        let n = self.r.i32()?;
        self.objects("parametre geometrie", n.into(), depth)?;
        Ok(())
    }

    fn curve2d(&mut self, depth: usize) -> Result<(), CvjError> {
        self.geometry2d(depth)?;
        self.r.schema()?;
        self.r.point2()?;
        self.r.point2()?;
        Ok(())
    }

    /// `CXRel2d` (§4.10).
    fn rel2d(&mut self, depth: usize) -> Result<(), CvjError> {
        self.geometry2d(depth)?;
        let s = self.r.schema()?;
        self.r.u8()?;
        // CXGRefLst
        self.r.schema()?;
        let n = self.r.i32()?;
        let n = self.r.check_count("odkazy väzby", n.into(), 22)?;
        for _ in 0..n {
            // CXGRef
            self.r.schema()?;
            self.r.i32()?;
            self.r.u32()?;
            self.r.u32()?;
            self.rel_help_data()?;
        }
        if s.minor() > 1 {
            for _ in 0..4 {
                self.r.u32()?;
            }
            self.r.f64()?;
            self.r.u32()?;
        }
        if s.minor() >= 3 {
            for _ in 0..4 {
                self.r.point2()?;
            }
        }
        if s.minor() >= 4 {
            self.r.f64()?;
            self.r.f64()?;
        }
        Ok(())
    }

    /// `CXRelHelpData` (§4.10).
    fn rel_help_data(&mut self) -> Result<(), CvjError> {
        self.r.schema()?;
        match self.r.u32()? {
            1 => self.r.skip(24)?,
            2 => self.r.skip(8)?,
            3 | 6 | 8 => self.r.skip(4)?,
            7 => self.r.skip(16)?,
            9 => self.r.skip(12)?,
            _ => {}
        }
        self.r.f64()?;
        Ok(())
    }
}

fn other() -> Obj {
    Obj::Other
}

fn number(t: u32, v: f64) -> Value {
    match t {
        1 => Value::Length(v),
        2 => Value::Angle(v),
        _ => Value::Number(v),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    //! Syntetické bajty jednotlivých tried. Zapisovač nižšie skladá prúd rovnako,
    //! ako ho číta importér, a sleduje indexy tried MFC.

    use super::*;

    /// Jednoduchý zapisovač archívu MFC pre testy.
    #[derive(Default)]
    pub struct W {
        pub buf: Vec<u8>,
        classes: Vec<(String, u32)>,
        next_index: u32,
    }

    impl W {
        pub fn new() -> W {
            W {
                next_index: 1,
                ..W::default()
            }
        }
        pub fn u8(&mut self, v: u8) -> &mut Self {
            self.buf.push(v);
            self
        }
        pub fn u16(&mut self, v: u16) -> &mut Self {
            self.buf.extend(v.to_le_bytes());
            self
        }
        pub fn u32(&mut self, v: u32) -> &mut Self {
            self.buf.extend(v.to_le_bytes());
            self
        }
        pub fn i32(&mut self, v: i32) -> &mut Self {
            self.buf.extend(v.to_le_bytes());
            self
        }
        pub fn f64(&mut self, v: f64) -> &mut Self {
            self.buf.extend(v.to_le_bytes());
            self
        }
        pub fn p3(&mut self) -> &mut Self {
            self.f64(0.0).f64(0.0).f64(0.0)
        }
        pub fn p2(&mut self) -> &mut Self {
            self.f64(0.0).f64(0.0)
        }
        /// Unicode `CString`.
        pub fn s(&mut self, v: &str) -> &mut Self {
            let units: Vec<u16> = v.encode_utf16().collect();
            self.buf.extend([0xFF, 0xFE, 0xFF]);
            if units.len() < 0xFF {
                self.buf.push(units.len() as u8);
            } else {
                self.buf.push(0xFF);
                self.u16(units.len() as u16);
            }
            for u in units {
                self.u16(u);
            }
            self
        }
        pub fn null(&mut self) -> &mut Self {
            self.u16(0)
        }
        /// Značka nového objektu triedy `class`.
        pub fn tag(&mut self, class: &str) -> &mut Self {
            if let Some((_, idx)) = self.classes.iter().find(|(c, _)| c == class) {
                let idx = *idx;
                self.u16(0x8000 | idx as u16);
            } else {
                self.u16(0xFFFF).u16(0x07DC).u16(class.len() as u16);
                self.buf.extend(class.as_bytes());
                self.classes.push((class.to_owned(), self.next_index));
                self.next_index += 1;
            }
            self.next_index += 1;
            self
        }
        /// `CVParameter` s číselnou hodnotou typu `t` a jednou rovnicou.
        pub fn param_f64(&mut self, name: &str, t: u32, v: f64) -> &mut Self {
            self.tag("CVParameter");
            self.cv_base(0, name, "", 0x17);
            self.u16(0xEA04).u32(1).u32(t);
            self.u16(0xD002).u32(t).f64(v);
            self.tag("CVEquation").u16(0xEA04).s("1").s("x").u8(0);
            self.s("").u16(0)
        }
        pub fn param_int(&mut self, name: &str, v: i32) -> &mut Self {
            self.tag("CVParameter");
            self.cv_base(0, name, "", 0x17);
            self.u16(0xEA04).u32(0).u32(4);
            self.u16(0xD002).u32(4).i32(v);
            self.s("").u16(0)
        }
        pub fn param_text(&mut self, name: &str, v: &str) -> &mut Self {
            self.tag("CVParameter");
            self.cv_base(0, name, "", 0x17);
            self.u16(0xEA04).u32(0).u32(8);
            self.u16(0xD002).u32(8).s(v);
            self.s("").u16(0)
        }
        pub fn cv_base(&mut self, id: u32, name: &str, desc: &str, asm: u16) -> &mut Self {
            self.u16(0xD001).u32(id).s(name).s(desc).u32(0);
            self.u16(0xEA01).u16(asm).u32(0)
        }
        /// AB s parametrami zapísanými funkciou `params` (vráti ich počet).
        pub fn asm_base(
            &mut self,
            id: u32,
            name: &str,
            asm: u16,
            class: u16,
            ty: u16,
            params: &[(&str, u32, f64)],
        ) -> &mut Self {
            self.asm_head(id, name, asm, params.len() as u16);
            for (n, t, v) in params {
                self.param_f64(n, *t, *v);
            }
            self.asm_tail(class, ty)
        }
        /// Začiatok AB po počet parametrov (parametre zapíše volajúci).
        pub fn asm_head(&mut self, id: u32, name: &str, asm: u16, params: u16) -> &mut Self {
            self.cv_base(id, name, name, asm);
            self.u16(0xEA01).u16(params)
        }
        /// Koniec AB za parametrami.
        pub fn asm_tail(&mut self, class: u16, ty: u16) -> &mut Self {
            self.u16(0xEA03).u16(class).u16(ty);
            self.u32(0).u32(0).u16(0).u32(0).u32(0).u32(0);
            self.null()
        }
        /// Začiatok `LoadDataAndChildren` s `children` deťmi (deti zapíše volajúci).
        pub fn children(&mut self, children: u16) -> &mut Self {
            self.u16(0xEA02).u16(0).u16(children).u32(0)
        }
    }

    pub const IN: f64 = 25.4;

    /// Hlavička úlohy: `CVJob` s telom a jednou miestnosťou s `room_children` deťmi
    /// (deti zapíše volajúci). `shape`: miestnosť má jeden tvar obrysu podlahy.
    pub fn job_head(w: &mut W, shape: bool, room_children: u16) {
        w.tag("CVJob").asm_base(1, "JOB", 0, 0, 0, &[]);
        // telo jobu
        w.u16(0xEA07).u16(0).u32(0).u32(0).u32(0).u32(0).s("").s("");
        w.u32(0).u32(0).u32(0).u16(0x0001).u32(0).u16(0).u32(0);
        for _ in 0..7 {
            w.u32(0);
        }
        for _ in 0..7 {
            w.f64(0.0);
        }
        for _ in 0..10 {
            w.s("tajné");
        }
        w.u16(1)
            .u32(0)
            .s("ponuka")
            .u32(0)
            .f64(0.0)
            .f64(0.0)
            .f64(0.0);
        w.u32(0);
        for _ in 0..10 {
            w.s("");
        }
        for _ in 0..6 {
            w.s("");
        }
        w.u32(0).s("").u32(0).u32(0).u32(0).u32(0);
        w.children(1);
        // miestnosť
        w.tag("CVFloorLevel").asm_base(2, "Room 1", 1, 0, 0, &[]);
        if shape {
            // jeden tvar s 2 bodmi a 1 slučkou s 2 indexmi
            w.u16(0xEA03).u16(0).i32(1);
            w.i32(2).p3().p3().i32(1).i32(2).u32(0).u32(1);
        } else {
            // jeden prázdny tvar
            w.u16(0xEA03).u16(0).i32(1).i32(0);
        }
        w.children(room_children);
    }

    /// Stena č. `number` so začiatkom (x, z) mm, dĺžkou `len` a uhlom `ay`; za ňou
    /// nasleduje `LoadDataAndChildren` s 2 plochami (zapíše volajúci).
    pub fn wall(w: &mut W, number: u16, x: f64, z: f64, len: f64, ay: f64) {
        w.tag("CVWall").asm_base(
            10 + number as u32,
            "WALL",
            3,
            10,
            0,
            &[
                ("X", 1, x / IN),
                ("Y", 1, 0.0),
                ("Z", 1, z / IN),
                ("DX", 1, len / IN),
                ("DY", 1, 2400.0 / IN),
                ("DZ", 1, 100.0 / IN),
                ("AY", 2, ay),
            ],
        );
        w.u16(0xEA01)
            .u16(number)
            .u16(0)
            .u16(0)
            .u16(0)
            .u16(0)
            .u16(0)
            .u16(2);
        w.u32(0).u32(0).u16(0);
        w.children(2);
    }

    /// Plocha steny (`back` = zadná) s `children` deťmi.
    pub fn face(w: &mut W, wall: u16, back: bool, children: u16, params: &[(&str, u32, f64)]) {
        let (name, code) = if back {
            ("Back Face", 2)
        } else {
            ("Front Face", 1)
        };
        w.tag("CVWallFace").asm_base(0, name, 4, 11, 0, params);
        w.u16(0xEA01)
            .u32(((wall as u32) << 16) | code)
            .u32(0)
            .u32(0);
        w.children(children);
    }

    /// Diel (ASM 16) bez detí.
    pub fn part(w: &mut W, code: &str, pos: [f64; 3], size: [f64; 3], ay: f64) {
        w.tag("CVAssembly").asm_base(
            0,
            code,
            16,
            1,
            0,
            &[
                ("X", 1, pos[0] / IN),
                ("Y", 1, pos[1] / IN),
                ("Z", 1, pos[2] / IN),
                ("DX", 1, size[0] / IN),
                ("DY", 1, size[1] / IN),
                ("DZ", 1, size[2] / IN),
                ("AY", 2, ay),
            ],
        );
        w.u16(0xEA01).children(0);
    }

    /// Hlavička skrinky (`CVCabinet`) s parametrami a `children` deťmi.
    pub fn cabinet(
        w: &mut W,
        no: i32,
        class: u16,
        ty: u16,
        pos: [f64; 3],
        size: [f64; 3],
        ay: f64,
        children: u16,
    ) {
        w.tag("CVCabinet").asm_head(100 + no as u32, "S_TEST", 9, 8);
        for (n, v) in [
            ("X", pos[0]),
            ("Y", pos[1]),
            ("Z", pos[2]),
            ("DX", size[0]),
            ("DY", size[1]),
            ("DZ", size[2]),
        ] {
            w.param_f64(n, 1, v / IN);
        }
        w.param_f64("AY", 2, ay).param_int("CABNO", no);
        w.asm_tail(class, ty);
        w.u16(0xEA01).u16(0).u32(0).u32(0).p3();
        w.u16(0xEA01).i32(0).i32(0);
        w.u32(0).u32(0).u16(0).p3().p3().u32(0);
        w.children(children);
    }

    /// Korpus 600 × 720 × 500 nad soklom 100 (4 diely v skupine „Case“).
    pub fn case_group(w: &mut W) {
        w.tag("CVAssembly").asm_base(0, "Case", 14, 1, 0, &[]);
        w.u16(0xEA01).children(4);
        part(w, "LU", [18.0, 100.0, 0.0], [500.0, 720.0, 18.0], -90.0);
        part(w, "RU", [600.0, 100.0, 0.0], [500.0, 720.0, 18.0], -90.0);
        part(w, "DE", [18.0, 100.0, 0.0], [564.0, 18.0, 500.0], 0.0);
        part(w, "TS", [18.0, 802.0, 0.0], [564.0, 18.0, 500.0], 0.0);
    }

    /// Malá úloha: 1 miestnosť, 1 stena (dĺžka 2000, výška 2400, hrúbka 100 mm),
    /// na prednej ploche 1 spodná skrinka so 4 dielmi korpusu, dvierkami a soklom.
    pub fn sample_job() -> Vec<u8> {
        let mut w = W::new();
        job_head(&mut w, true, 1);
        // stena: začiatok (0, 0, -1000) mm, dĺžka 2000, AY = 0
        w.tag("CVWall").asm_base(
            3,
            "WALL",
            3,
            10,
            0,
            &[
                ("X", 1, 0.0),
                ("Y", 1, 0.0),
                ("Z", 1, -1000.0 / IN),
                ("DX", 1, 2000.0 / IN),
                ("DY", 1, 2400.0 / IN),
                ("DZ", 1, 100.0 / IN),
                ("AY", 2, 0.0),
            ],
        );
        w.u16(0xEA01)
            .u16(1)
            .u16(0)
            .u16(0)
            .u16(0)
            .u16(0)
            .u16(0)
            .u16(2);
        w.u32(0).u32(0).u16(0);
        w.children(2);
        // predná plocha s jednou skrinkou
        w.tag("CVWallFace").asm_base(4, "Front Face", 4, 11, 0, &[]);
        w.u16(0xEA01).u32((1 << 16) | 1).u32(0).u32(0);
        w.children(1);
        w.tag("CVCabinet").asm_head(5, "S_TEST", 9, 8);
        for (n, v) in [
            ("X", 100.0),
            ("Y", 0.0),
            ("Z", 20.0),
            ("DX", 600.0),
            ("DY", 820.0),
            ("DZ", 500.0),
        ] {
            w.param_f64(n, 1, v / IN);
        }
        w.param_int("CABNO", 7).param_text("COMMENT", "poznámka");
        w.asm_tail(1, 1);
        w.u16(0xEA01).u16(0).u32(0).u32(0).p3();
        w.u16(0xEA01).i32(0).i32(0);
        w.u32(0).u32(0).u16(0).p3().p3().u32(0);
        w.children(3);
        // skupina Case so 4 dielmi korpusu (výška korpusu 720 nad soklom 100)
        w.tag("CVAssembly").asm_base(6, "Case", 14, 1, 0, &[]);
        w.u16(0xEA01).children(4);
        for (code, x, y, z, dx, dy, dz, ay) in [
            // ľavý bok: X = 18, AY = −90 → hrúbka do −X
            ("LU", 18.0, 100.0, 0.0, 500.0, 720.0, 18.0, -90.0),
            ("RU", 600.0, 100.0, 0.0, 500.0, 720.0, 18.0, -90.0),
            ("DE", 18.0, 100.0, 0.0, 564.0, 18.0, 500.0, 0.0),
            ("TS", 18.0, 802.0, 0.0, 564.0, 18.0, 500.0, 0.0),
        ] {
            w.tag("CVAssembly").asm_base(
                0,
                code,
                16,
                1,
                0,
                &[
                    ("X", 1, x / IN),
                    ("Y", 1, y / IN),
                    ("Z", 1, z / IN),
                    ("DX", 1, dx / IN),
                    ("DY", 1, dy / IN),
                    ("DZ", 1, dz / IN),
                    ("AY", 2, ay),
                ],
            );
            w.u16(0xEA01).children(0);
        }
        // dvierka s panelom (nepatria do korpusu)
        w.tag("CVDoor").asm_base(7, "DOR", 11, 1, 17, &[]);
        w.u16(0xEA01).i32(0).u32(0).i32(0).children(1);
        w.tag("CVAssembly").asm_base(
            0,
            "S_DSLAB",
            16,
            1,
            17,
            &[
                ("X", 1, 0.0),
                ("Y", 1, 100.0 / IN),
                ("Z", 1, 500.0 / IN),
                ("DX", 1, 596.0 / IN),
                ("DY", 1, 716.0 / IN),
                ("DZ", 1, 18.0 / IN),
            ],
        );
        w.u16(0xEA01).children(0);
        // sokel priamo pod skrinkou
        w.tag("CVAssembly").asm_base(
            0,
            "BO",
            16,
            1,
            0,
            &[
                ("Y", 1, 0.0),
                ("DX", 1, 600.0 / IN),
                ("DY", 1, 100.0 / IN),
                ("DZ", 1, 18.0 / IN),
            ],
        );
        w.u16(0xEA01).children(0);
        // zadná plocha bez detí
        w.tag("CVWallFace").asm_base(8, "Back Face", 4, 11, 0, &[]);
        w.u16(0xEA01).u32((1 << 16) | 2).u32(0).u32(0);
        w.children(0);
        // zástupné objekty za koreňom
        for c in PLACEHOLDERS {
            w.tag(c);
        }
        w.buf
    }

    #[test]
    fn sample_job_walks_completely() {
        let data = sample_job();
        let walk = walk(&data).unwrap();
        assert!(walk.consumed_all, "{:?}", walk.trailing_error);
        assert_eq!(walk.trailing.len(), 7);
        let room = &walk.root.children[0];
        assert_eq!(room.class, "CVFloorLevel");
        assert_eq!(room.extra, Extra::FloorLevel { shapes: 1 });
        let wall = &room.children[0];
        assert_eq!(
            wall.extra,
            Extra::Wall {
                number: 1,
                prev: 0,
                next: 0
            }
        );
        assert_eq!(wall.children.len(), 2);
        let cab = &wall.children[0].children[0];
        assert_eq!(cab.class, "CVCabinet");
        assert_eq!(cab.param("CABNO"), Some(&Value::Int(7)));
        assert_eq!(cab.children.len(), 3);
        assert_eq!(cab.children[0].children.len(), 4);
    }

    /// Prečíta jeden objekt z `bytes` a overí, že sa spotrebovali všetky bajty.
    fn read_one(bytes: &[u8]) -> Result<Obj, CvjError> {
        let mut walker = Walker {
            r: Reader::new(bytes),
        };
        let obj = walker.object(0)?;
        assert_eq!(walker.r.remaining(), 0, "nespotrebované bajty");
        Ok(obj)
    }

    fn node_of(bytes: &[u8]) -> Node {
        match read_one(bytes).unwrap() {
            Obj::Node(n) => *n,
            _ => panic!("očakávaný uzol"),
        }
    }

    /// Báza `CXGeometry2d` s voliteľným `CXIdLst` a jedným `CXParameter`.
    fn geo2d(w: &mut W, idlst: bool) {
        w.u16(0xD001).i32(5);
        if idlst {
            w.tag("CXIdLst")
                .u16(0xD001)
                .u32(0)
                .u16(2)
                .buf
                .extend([0u8; 16]);
        } else {
            w.null();
        }
        w.i32(1);
        w.tag("CXParameter").u16(0xD001).s("R").u32(0);
        w.u16(0xD002).u32(1).f64(1.0);
    }

    fn rel2d(w: &mut W, class: &str) {
        w.tag(class);
        geo2d(w, false);
        // CXRel2d so schémou 4: u8, zoznam odkazov so všetkými druhmi pomocných dát
        w.u16(0xD004).u8(0);
        let kinds = [0u32, 1, 2, 3, 6, 7, 8, 9];
        w.u16(0xD001).i32(kinds.len() as i32);
        for k in kinds {
            w.u16(0xD001).i32(1).u32(0).u32(0);
            w.u16(0xD001).u32(k);
            match k {
                1 => {
                    w.f64(0.0).f64(0.0).f64(0.0);
                }
                2 => {
                    w.f64(0.0);
                }
                3 | 6 | 8 => {
                    w.u32(0);
                }
                7 => {
                    w.f64(0.0).f64(0.0);
                }
                9 => {
                    w.u32(0).f64(0.0);
                }
                _ => {}
            }
            w.f64(0.0);
        }
        w.u32(0).u32(0).u32(0).u32(0).f64(0.0).u32(0);
        for _ in 0..4 {
            w.p2();
        }
        w.f64(0.0).f64(0.0);
        if class == "CXFixedRel2d" || class == "CXHorzVertRel2d" {
            w.u16(0xD001).u8(1);
        }
    }

    /// `CVTemplate2d` s profilom (čiara, oblúk, 4 druhy väzieb).
    fn template2d_with_profile(w: &mut W) {
        w.tag("CVTemplate2d").cv_base(0, "T", "", 26);
        w.u16(0xEA01).u32(0).u32(0).p3().p3();
        w.u16(0xEA01).f64(0.0);
        w.tag("CXVTemplate").u16(0xD002).u32(0).i32(2);
        w.tag("CXLine2d");
        geo2d(w, true);
        w.u16(0xD001).p2().p2().u16(0xD001);
        w.tag("CXArc2d");
        geo2d(w, false);
        w.u16(0xD001).p2().p2().u16(0xD001).p2();
        w.i32(4);
        for c in [
            "CXCoincidentRel2d",
            "CXTangentRel2d",
            "CXFixedRel2d",
            "CXHorzVertRel2d",
        ] {
            rel2d(w, c);
        }
        w.u16(0xD001).u32(0).u32(0);
        w.u32(0).u32(0).p3().p3().f64(0.0).u32(0);
    }

    #[test]
    fn template2d_profile_and_nodes() {
        let mut w = W::new();
        template2d_with_profile(&mut w);
        assert!(matches!(read_one(&w.buf).unwrap(), Obj::Other));

        // bez profilu: uzly šablóny
        let mut w = W::new();
        w.tag("CVTemplate2d").cv_base(0, "T", "", 26);
        w.u16(0xEA01).u32(0).u32(0).p3().p3();
        w.u16(0xEA01).f64(0.0).null().i32(2);
        for _ in 0..2 {
            w.tag("CVTemplateNode")
                .p3()
                .u32(0)
                .u16(0xEA01)
                .f64(0.0)
                .f64(0.0);
        }
        assert!(matches!(read_one(&w.buf).unwrap(), Obj::Other));
    }

    #[test]
    fn template3d_vertices_and_unsupported_faces() {
        let head = |w: &mut W, faces: i32| {
            w.tag("CVTemplate3d").cv_base(0, "T3", "", 27);
            w.u16(0xEA01).u32(0).u32(0).p3().p3();
            w.u16(0xEA01).i32(1);
            w.tag("CVVertexNode").p3().u32(0);
            w.i32(faces);
        };
        let mut w = W::new();
        head(&mut w, 0);
        assert!(read_one(&w.buf).is_ok());
        let mut w = W::new();
        head(&mut w, 1);
        let mut walker = Walker {
            r: Reader::new(&w.buf),
        };
        assert!(matches!(
            walker.object(0),
            Err(CvjError::Unsupported { .. })
        ));
    }

    #[test]
    fn assembly_with_packets_and_profile_template() {
        // diel s 2D profilom v AB a s tromi dátovými paketmi
        let mut w = W::new();
        w.tag("CVAssembly").asm_head(0, "TVAR", 16, 0);
        w.u16(0xEA03)
            .u16(1)
            .u16(0)
            .u32(0)
            .u32(0)
            .u16(0)
            .u32(0)
            .u32(0)
            .u32(0);
        template2d_with_profile(&mut w);
        w.u16(0xEA01);
        w.u16(0xEA01).u16(3);
        w.tag("CVBandPacket").u16(0xEA01).u32(0).u16(0xEA01).u16(1);
        for _ in 0..4 {
            w.f64(0.0);
        }
        template2d_with_profile(&mut w);
        w.tag("CVShapePacket").u16(0xEA01).u32(0);
        w.tag("CVDataPacket").u16(0xEA01).u32(0);
        // zastaraná schéma LoadDataAndChildren (< 2): 2× u32 navyše
        w.u16(0).u32(0).u32(0).u32(0);
        let n = node_of(&w.buf);
        assert_eq!(n.asm_type, 16);
        assert!(n.children.is_empty());
    }

    #[test]
    fn container_and_leaf_classes() {
        let mut w = W::new();
        w.tag("CVAssembly").asm_base(0, "G", 14, 1, 0, &[]);
        w.u16(0xEA01).children(17);
        w.tag("CVWidget").asm_base(0, "W", 14, 1, 0, &[]);
        w.u16(0xEA01).children(0);
        w.tag("CVMolding").asm_base(0, "M", 14, 9, 0, &[]);
        w.u16(0xEA01).children(0);
        w.tag("CVDesignCat").asm_base(0, "D", 14, 1, 0, &[]);
        w.u16(0xEA01).u16(0xEA01).i32(1);
        w.tag("CVOptOption")
            .u16(0xEA01)
            .u32(0)
            .u32(0)
            .u16(0)
            .s("a")
            .s("b");
        w.i32(1);
        w.tag("CVOptEffect")
            .u16(0xEA01)
            .u32(0)
            .u32(0)
            .u16(0)
            .u16(0)
            .f64(0.0)
            .u16(0);
        w.children(0);
        w.tag("CVDoor").asm_base(0, "DWR", 12, 1, 18, &[]);
        w.u16(0xEA01).i32(0).u32(0).i32(0).children(0);
        w.tag("CVSmartFace").asm_base(0, "SF", 17, 1, 0, &[]);
        w.u16(0xEA01).u32(0);
        w.tag("CVRouteOp").asm_base(0, "R1", 20, 1, 0, &[]);
        w.u16(0xEA01).u32(0).u32(0);
        w.tag("CVRouteOp").asm_base(0, "R2", 20, 1, 0, &[]);
        w.u16(0xEA02).u16(0xEA01).u32(0).u32(0);
        for c in LEAF_ASSEMBLIES {
            w.tag(c).asm_base(0, c, 56, 1, 0, &[]);
            w.u16(0xEA01);
        }
        let n = node_of(&w.buf);
        assert_eq!(n.children.len(), 17);
        assert_eq!(n.children[3].asm_type, 12);
        assert_eq!(n.children[7].class, "CVHardware");
    }

    #[test]
    fn counter_with_splits_and_splash() {
        let mut w = W::new();
        w.tag("CVCounter")
            .asm_base(0, "Counter", 7, 8, 0, &[("DX", 1, 10.0)]);
        w.u16(0xEA01).u16(0).u32(0).u16(2);
        for _ in 0..2 {
            w.u16(0xEA01).f64(1.0).u32(0);
        }
        w.children(1);
        w.tag("CVSplash").asm_base(0, "Splash", 7, 16, 0, &[]);
        w.u16(0xEA01).u16(0).children(0);
        let n = node_of(&w.buf);
        assert_eq!(n.asm_type, 7);
        assert_eq!(n.param("DX"), Some(&Value::Length(10.0)));
        assert_eq!(n.children[0].class, "CVSplash");
    }

    #[test]
    fn room_object_and_value_types() {
        let mut w = W::new();
        w.tag("CVRoom").asm_head(0, "R", 2, 6);
        w.param_int("I", -3).param_text("T", "ľ");
        // bool a u32
        for (name, t) in [("B", 5u32), ("U", 10)] {
            w.tag("CVParameter").cv_base(0, name, "", 0x17);
            w.u16(0xEA04)
                .u32(0)
                .u32(t)
                .u16(0xD002)
                .u32(t)
                .u32(1)
                .s("")
                .u16(0);
        }
        w.param_f64("N", 6, 2.5).param_f64("A", 2, 90.0);
        w.asm_tail(0, 0);
        let n = node_of(&w.buf);
        assert_eq!(n.param("I"), Some(&Value::Int(-3)));
        assert_eq!(n.param("T"), Some(&Value::Text("ľ".into())));
        assert_eq!(n.param("B"), Some(&Value::Bool(true)));
        assert_eq!(n.param("U"), Some(&Value::UInt(1)));
        assert_eq!(n.param("N"), Some(&Value::Number(2.5)));
        assert_eq!(n.param("A"), Some(&Value::Angle(90.0)));
    }

    #[test]
    fn parameter_schema_variants() {
        // parameter so schémou 2 (u16 typ), stará hodnota (schéma 1, text) a stará rovnica
        let mut w = W::new();
        w.tag("CVParameter").cv_base(0, "P", "", 0x17);
        w.u16(0xEA02).u32(1).u16(0);
        w.u16(0xD001).u32(8).s("txt");
        w.tag("CVEquation").u16(0xEA01).u8(1).u8(b'1');
        w.u8(2).u8(b'a').u8(b'b');
        w.s("");
        match read_one(&w.buf).unwrap() {
            Obj::Param(p) => assert_eq!(p.value, Value::Text("txt".into())),
            _ => panic!("očakávaný parameter"),
        }
        // zastaraný parameter (schéma 1) sa nečíta
        let mut w = W::new();
        w.tag("CVParameter").cv_base(0, "P", "", 0x17);
        w.u16(0xEA01).u32(0);
        let mut walker = Walker {
            r: Reader::new(&w.buf),
        };
        assert!(matches!(
            walker.object(0),
            Err(CvjError::Unsupported { .. })
        ));
    }

    #[test]
    fn unknown_class_names_class_and_position() {
        let mut w = W::new();
        w.tag("CVAssembly").asm_base(0, "G", 14, 1, 0, &[]);
        w.u16(0xEA01).children(1);
        let pos = w.buf.len();
        w.tag("CVLightSpot").u32(0);
        let mut walker = Walker {
            r: Reader::new(&w.buf),
        };
        match walker.object(0) {
            Err(CvjError::UnknownClass { class, pos: at }) => {
                assert_eq!(class, "CVLightSpot");
                assert_eq!(at, pos);
            }
            Err(e) => panic!("{e}"),
            Ok(_) => panic!("očakávaná chyba"),
        }
    }

    #[test]
    fn nesting_depth_is_limited() {
        let mut w = W::new();
        for _ in 0..(MAX_DEPTH + 5) {
            w.tag("CVAssembly").asm_base(0, "G", 14, 1, 0, &[]);
            w.u16(0xEA01).children(1);
        }
        w.null();
        // na vlákne s rovnakým zásobníkom ako `walk`
        let data = w.buf;
        let r = std::thread::Builder::new()
            .stack_size(WALK_STACK)
            .spawn(move || {
                let mut walker = Walker {
                    r: Reader::new(&data),
                };
                matches!(walker.object(0), Err(CvjError::TooDeep { .. }))
            })
            .unwrap()
            .join()
            .unwrap();
        assert!(r);
    }

    #[test]
    fn absurd_counts_are_rejected() {
        let mut w = W::new();
        w.tag("CVAssembly").asm_base(0, "G", 14, 1, 0, &[]);
        w.u16(0xEA01).children(0xFFF0);
        let mut walker = Walker {
            r: Reader::new(&w.buf),
        };
        assert!(matches!(walker.object(0), Err(CvjError::BadCount { .. })));
    }

    #[test]
    fn known_classes_cover_all_observed() {
        // 39 tried pozorovaných v ukážkových úlohách
        let observed = [
            "CVAssembly",
            "CVBandPacket",
            "CVCabinet",
            "CVCatalogDataList",
            "CVConstDataListX",
            "CVCounter",
            "CVDadoExOp",
            "CVDoor",
            "CVDoorDataList",
            "CVDrawManager",
            "CVEquation",
            "CVFloorLevel",
            "CVGuide",
            "CVHardware",
            "CVHinge",
            "CVHingePlate",
            "CVJob",
            "CVLegLeveler",
            "CVLightSchedList",
            "CVLineboreOp",
            "CVMatDataListX",
            "CVParameter",
            "CVRouteOp",
            "CVShapePacket",
            "CVSmartFace",
            "CVSplash",
            "CVTemplate2d",
            "CVTemplateList",
            "CVTemplateNode",
            "CVWall",
            "CVWallFace",
            "CXArc2d",
            "CXCoincidentRel2d",
            "CXHorzVertRel2d",
            "CXIdLst",
            "CXLine2d",
            "CXParameter",
            "CXTangentRel2d",
            "CXVTemplate",
        ];
        let known = known_classes();
        for c in observed {
            assert!(known.contains(&c), "{c}");
        }
    }
}
