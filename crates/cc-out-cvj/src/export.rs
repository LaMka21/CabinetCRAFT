//! Zákazka CabinetCRAFT → `.cvj` podľa šablóny (findings 13 §10.5, stratégia „klon kostry“).
//!
//! Šablóna je zákazka uložená v CV dielne. Jej skrinky s korpusom sa nahradia skrinkami
//! projektu: každá naša skrinka je kostra (parametre, skupiny `Case`/`Interior`/`Face`
//! a uzly sekcií, bez dielov) skrinky šablóny rovnakej triedy s najbližšími rozmermi.
//! CV pri otvorení skrinky prestaví z parametrov a sekcií v súbore a z plánov
//! `Construction`/`Materials`/`Doors`, ktoré sú v súbore šablóny – katalóg CV nepotrebuje.
//!
//! Delenie čiel (sekcie) sa tvorí z čiel skrinky projektu (`sections`): každý uzol je kópia
//! vzorového uzla šablóny rovnakého druhu s našimi ID, rozmermi, pántami a korpusom zásuvky.
//! Ak šablóna potrebný druh uzla nemá, ostanú sekcie vzorovej skrinky (úmerne roztiahnuté).
//!
//! Steny projektu `W<n>` sa zapíšu do stien šablóny č. n (`walls`, aj zadné plochy a spoje).
//! Okná, dvere a otvory (CLASS 7) sú kópie okna/dverí šablóny rovnakého druhu s našou polohou
//! a rozmerom. Pridanie a zmazanie celej steny zatiaľ nie.

use std::collections::BTreeMap;

use cc_i18n::tr;
use cc_import_cvj::classes::{Extra, Node, Param};
use cc_model::{CabinetSpec, CabinetType, ProjectFile, Standard};

use crate::emit::{Emitter, Skip};
use crate::sections::{face_sections, NodeKind, SectionNode};
use crate::{CvjWriteError, Template};

const INCH: f64 = 25.4;

/// Výsledok exportu.
pub struct Export {
    pub contents: Vec<u8>,
    /// skrinky zapísané do súboru (ID projektu → číslo CABNO)
    pub cabinets: Vec<(String, i32)>,
    /// poznámky pre používateľa (čo sa nezapísalo a prečo)
    pub notes: Vec<String>,
}

/// Skrinka šablóny s korpusom na prednej ploche steny.
struct TemplateCabinet<'a> {
    node: &'a Node,
    /// číslo steny
    wall: u16,
}

/// Predná plocha steny šablóny.
struct FaceRef<'a> {
    wall: u16,
    node: &'a Node,
}

fn param<'a>(n: &'a Node, name: &str) -> Option<&'a Param> {
    n.params.iter().find(|p| p.name == name)
}

fn f64_param(n: &Node, name: &str) -> Option<f64> {
    match param(n, name).map(|p| &p.value) {
        Some(cc_import_cvj::classes::Value::Length(v))
        | Some(cc_import_cvj::classes::Value::Angle(v))
        | Some(cc_import_cvj::classes::Value::Number(v)) => Some(*v),
        _ => None,
    }
}

fn has_case(n: &Node) -> bool {
    n.children
        .iter()
        .any(|c| c.class == "CVAssembly" && c.name.starts_with("Case"))
}

fn collect<'a>(
    n: &'a Node,
    wall: Option<u16>,
    faces: &mut Vec<FaceRef<'a>>,
    cabs: &mut Vec<TemplateCabinet<'a>>,
) {
    let wall = match n.extra {
        Extra::Wall { number, .. } => Some(number),
        _ => wall,
    };
    if n.class == "CVWallFace" && n.name == "Front Face" {
        if let Some(w) = wall {
            faces.push(FaceRef { wall: w, node: n });
            for c in &n.children {
                if c.class == "CVCabinet"
                    && c.asm_type == 9
                    && matches!(c.cv_class, 1..=3)
                    && param(c, "CABNO").is_some()
                    && has_case(c)
                {
                    cabs.push(TemplateCabinet { node: c, wall: w });
                }
            }
        }
    }
    for c in &n.children {
        collect(c, wall, faces, cabs);
    }
}

fn cv_class(t: CabinetType) -> u16 {
    match t {
        CabinetType::Base => 1,
        CabinetType::Wall => 2,
        CabinetType::Tall => 3,
    }
}

/// Zapíše hodnotu parametra (palce pri dĺžkach) ako záplatu.
fn patch_value(patches: &mut BTreeMap<usize, Vec<u8>>, p: &Param, mm_or_value: f64) {
    match p.value_type {
        4 | 5 => {
            patches.insert(
                p.value_at,
                (mm_or_value.round() as i32).to_le_bytes().to_vec(),
            );
        }
        8 | 10 => {}
        1 => {
            patches.insert(p.value_at, (mm_or_value / INCH).to_le_bytes().to_vec());
        }
        _ => {
            patches.insert(p.value_at, mm_or_value.to_le_bytes().to_vec());
        }
    }
}

/// Je hodnota dĺžkou v palcoch (typ 1)?
fn is_length(p: &Param) -> bool {
    p.value_type == 1
}

/// Kostra skrinky: čo sa vynechá a nové počty detí (findings 13 §10.4).
fn skeleton<'a>(
    cab: &'a Node,
    skips: &mut Vec<Skip>,
    patches: &mut BTreeMap<usize, Vec<u8>>,
) -> Vec<&'a Node> {
    let mut kept: Vec<&Node> = vec![cab];
    let mut groups = 0u16;
    for g in &cab.children {
        let is_group = g.class == "CVAssembly"
            && ["Case", "Interior", "Face"]
                .iter()
                .any(|n| g.name.starts_with(n));
        if !is_group {
            // objekt bez PID je doplnok používateľa alebo UCS dielne (sokel, LED rampa,
            // krycí bok…): CV ho pri prestavbe zachová, ale sám ho nevytvorí → ponechať
            // celý aj s deťmi (findings 13 §10.2 bod 5, skúška vlastníka: sokel chýbal)
            if param(g, "PID").is_none() {
                groups += 1;
                add_subtree(g, &mut kept);
            } else {
                skips.push((g.pos, g.end));
            }
            continue;
        }
        groups += 1;
        kept.push(g);
        let mut keep = 0u16;
        for c in &g.children {
            let section = !g.name.starts_with("Case") && param(c, "ID").is_some();
            if section {
                keep += 1;
                kept.push(c);
                for k in &c.children {
                    skips.push((k.pos, k.end));
                }
                if let Some(at) = c.children_count_at {
                    patches.insert(at, 0u16.to_le_bytes().to_vec());
                }
            } else {
                skips.push((c.pos, c.end));
            }
        }
        if let Some(at) = g.children_count_at {
            patches.insert(at, keep.to_le_bytes().to_vec());
        }
    }
    // počet ponechaných priamych detí skrinky (skupiny + doplnky bez PID)
    if let Some(at) = cab.children_count_at {
        patches.insert(at, groups.to_le_bytes().to_vec());
    }
    kept
}

fn add_subtree<'a>(n: &'a Node, kept: &mut Vec<&'a Node>) {
    kept.push(n);
    for c in &n.children {
        add_subtree(c, kept);
    }
}

/// Úsek šablóny na zápis so záplatami a vynechanými objektmi.
struct Segment {
    start: usize,
    end: usize,
    patches: BTreeMap<usize, Vec<u8>>,
    skips: Vec<Skip>,
}

/// Klon kostry na zápis: úseky šablóny (skrinka, vložené uzly sekcií) a náhrady inej dĺžky.
struct CloneJob {
    segments: Vec<Segment>,
    replaces: BTreeMap<usize, (usize, Vec<u8>)>,
}

fn int_param(n: &Node, name: &str) -> Option<i32> {
    match param(n, name).map(|p| &p.value) {
        Some(cc_import_cvj::classes::Value::Int(v)) => Some(*v),
        _ => None,
    }
}

fn is_face_group(g: &Node) -> bool {
    g.class == "CVAssembly" && g.name.starts_with("Face") && param(g, "ID").is_none()
}

/// Uzly sekcií čiel všetkých skriniek šablóny (vzory) a ich skrinky.
fn collect_sections<'a>(n: &'a Node, out: &mut Vec<(&'a Node, &'a Node)>) {
    if n.class == "CVCabinet" {
        for g in n.children.iter().filter(|g| is_face_group(g)) {
            for c in g.children.iter().filter(|c| param(c, "ID").is_some()) {
                out.push((n, c));
            }
        }
    }
    for c in &n.children {
        collect_sections(c, out);
    }
}

/// Vzorový uzol pre druh sekcie a STYPE na zápis: najprv uzol vzorovej skrinky `src` s tým
/// istým ID (`wing` = poradie krídla pri dvojkrídlových), ak sa importuje ako ten istý druh
/// (zachová všetky jeho parametre), potom rovnaký druh zo skrinky `src`, z celej šablóny,
/// nakoniec príbuzný druh (STYPE sa prepíše). Deliče `MR`/`MS` náhradu nemajú.
fn prototype<'a>(
    kind: NodeKind,
    id: i32,
    wing: usize,
    src: &Node,
    sections: &[(&'a Node, &'a Node)],
) -> Option<(&'a Node, i32)> {
    let (name, stype) = kind.cv();
    let same = sections
        .iter()
        .filter(|(cab, s)| std::ptr::eq(*cab, src) && int_param(s, "ID") == Some(id))
        .nth(wing)
        .map(|(_, s)| *s)
        .and_then(|s| {
            let st = int_param(s, "STYPE")?;
            // umývačka: dvierka bez závesov (HNG 4) importujeme ako pevné čelo
            let fixed_door = kind == NodeKind::FalseFront
                && s.name == "DOR_OPEN"
                && st == 1
                && int_param(s, "HNG") == Some(4);
            ((s.name == name && st == stype) || fixed_door).then_some((s, st))
        });
    if same.is_some() {
        return same;
    }
    let find = |st: i32, own: bool| {
        sections
            .iter()
            .find(|(cab, s)| {
                (!own || std::ptr::eq(*cab, src))
                    && s.name == name
                    && int_param(s, "STYPE") == Some(st)
            })
            .map(|(_, s)| *s)
    };
    let related: &[i32] = match kind {
        NodeKind::Door => &[4, 8],
        NodeKind::Pair => &[1, 8],
        NodeKind::Panel => &[1, 4],
        NodeKind::Drawer => &[5],
        NodeKind::FalseFront => &[2],
        NodeKind::Open => &[0],
        NodeKind::Split => &[3],
        NodeKind::Rail | NodeKind::Stile => &[],
    };
    find(stype, true)
        .or_else(|| find(stype, false))
        .or_else(|| {
            related
                .iter()
                .find_map(|&st| find(st, true).or_else(|| find(st, false)))
        })
        .map(|s| (s, stype))
}

/// Úseky jedného uzla sekcie: kópia vzoru `p` bez detí, s našimi hodnotami; parametre, ktoré
/// vzor nemá a my ich potrebujeme, sa doplnia kópiou parametra iného uzla šablóny (`donors`).
#[allow(clippy::too_many_arguments)]
fn section_segments(
    s: &SectionNode,
    p: &Node,
    stype: i32,
    donors: &BTreeMap<&str, &Param>,
    pid_base: Option<f64>,
    unlinked: bool,
    next_id: &mut u32,
    out: &mut Vec<Segment>,
) -> Result<(), CvjWriteError> {
    let Some(last) = p.params.last() else {
        return Err(CvjWriteError::Template(format!(
            "vzorový uzol `{}` nemá parametre",
            p.name
        )));
    };
    let mut patches = BTreeMap::new();
    let mut extra: Vec<&Param> = Vec::new();
    let mut values: Vec<(&str, f64, bool)> = vec![
        ("ID", f64::from(s.id), false),
        ("PARID", f64::from(s.parent), false),
        ("TYPE", f64::from(s.kind.cv_type()), false),
        ("STYPE", f64::from(stype), false),
        ("X", s.x, false),
        ("Y", s.y, false),
        ("DX", s.dx, false),
        ("DY", s.dy, false),
    ];
    if let Some(base) = pid_base {
        values.push(("PID", base + f64::from(s.id) * 100.0, false));
    }
    if let Some(h) = s.hinge {
        values.push(("HNG", f64::from(h), true));
    }
    if let Some(v) = s.box_height {
        values.push(("BOXH", v, true));
    }
    if let Some(v) = s.box_depth {
        values.push(("BOXD", v, true));
    }
    if s.kind == NodeKind::Drawer {
        // šírka korpusu zásuvky: rovnaký odstup od šírky otvoru ako vo vzore
        if let (Some(bw), Some(pdx)) = (f64_param(p, "BOXW"), f64_param(p, "DX")) {
            values.push(("BOXW", s.dx - (pdx - bw) * INCH, false));
        }
    }
    if param(p, "EQ").is_some() || s.equal {
        values.push(("EQ", if s.equal { 2.0 } else { 0.0 }, true));
    }
    let with_front = !matches!(
        s.kind,
        NodeKind::Split | NodeKind::Rail | NodeKind::Stile | NodeKind::Open
    );
    if with_front {
        for (name, v) in [
            ("OLL", s.adjust.left),
            ("OLR", s.adjust.right),
            ("OLT", s.adjust.top),
            ("OLB", s.adjust.bottom),
        ] {
            if param(p, name).is_some() || v != 0.0 {
                values.push((name, v, true));
            }
        }
    }
    for (name, v, insert) in values {
        if let Some(pp) = param(p, name) {
            patch_value(&mut patches, pp, v);
        } else if insert {
            if let Some(d) = donors.get(name) {
                patch_value(&mut patches, d, v);
                extra.push(d);
            }
        }
    }
    // nové ObjectID, žiadne deti (CV ich prestaví), väzba na katalóg ako skrinka
    let mut new_id = |at: usize, patches: &mut BTreeMap<usize, Vec<u8>>| {
        patches.insert(at, next_id.to_le_bytes().to_vec());
        *next_id = next_id.saturating_add(1);
    };
    if p.object_id != 0 {
        new_id(p.id_at, &mut patches);
    }
    for pp in p.params.iter().chain(extra.iter().copied()) {
        if pp.object_id != 0 {
            new_id(pp.id_at, &mut patches);
        }
    }
    if let Some(at) = p.children_count_at {
        patches.insert(at, 0u16.to_le_bytes().to_vec());
    }
    if unlinked {
        if let Some(at) = p.link_at {
            patches.insert(at, 0u32.to_le_bytes().to_vec());
        }
    }
    let count = u16::try_from(p.params.len() + extra.len())
        .map_err(|_| CvjWriteError::Template("príliš veľa parametrov uzla sekcie".into()))?;
    patches.insert(p.params_count_at, count.to_le_bytes().to_vec());
    let skips: Vec<Skip> = p.children.iter().map(|k| (k.pos, k.end)).collect();

    out.push(Segment {
        start: p.pos,
        end: last.end,
        patches: patches.clone(),
        skips: Vec::new(),
    });
    for d in extra {
        out.push(Segment {
            start: d.at,
            end: d.end,
            patches: patches.clone(),
            skips: Vec::new(),
        });
    }
    out.push(Segment {
        start: last.end,
        end: p.end,
        patches,
        skips,
    });
    Ok(())
}

/// `CString` MFC pre ASCII meno (dĺžka u8, pri dlhšom u16).
fn cstring(name: &str) -> Vec<u8> {
    let ascii: Vec<u8> = name
        .chars()
        .map(|c| {
            if c.is_ascii() && !c.is_ascii_control() {
                c as u8
            } else {
                b'_'
            }
        })
        .take(200)
        .collect();
    let mut out = Vec::with_capacity(ascii.len() + 1);
    out.push(ascii.len() as u8);
    out.extend_from_slice(&ascii);
    out
}

/// Rozmery a poloha skrinky v súradniciach plochy steny CV (mm).
struct Target {
    x: f64,
    y: f64,
    z: f64,
    dx: f64,
    dy: f64,
    dz: f64,
}

fn target(c: &CabinetSpec, elevation: f64, toe: f64) -> Target {
    match c.cabinet_type {
        // spodné a vysoké: v CV výška vrátane sokla, spodok na podlahe (findings 13, RavaCAB)
        CabinetType::Base | CabinetType::Tall => Target {
            x: c.at,
            y: (elevation - toe).max(0.0),
            z: c.outset,
            dx: c.w,
            dy: c.h + toe,
            dz: c.d,
        },
        CabinetType::Wall => Target {
            x: c.at,
            y: elevation,
            z: c.outset,
            dx: c.w,
            dy: c.h,
            dz: c.d,
        },
    }
}

/// Zapíše projekt do `Contents` podľa šablóny.
pub fn export_project(
    project: &ProjectFile,
    standard: &Standard,
    t: &Template,
) -> Result<Export, CvjWriteError> {
    let mut faces = Vec::new();
    let mut cabs = Vec::new();
    collect(&t.walk.root, None, &mut faces, &mut cabs);
    let mut protos = Vec::new();
    collect_sections(&t.walk.root, &mut protos);
    // parametre na doplnenie do vzoru, ktorý ich nemá (prvý výskyt v šablóne)
    let mut donors: BTreeMap<&str, &Param> = BTreeMap::new();
    for (_, s) in &protos {
        for p in &s.params {
            donors.entry(p.name.as_str()).or_insert(p);
        }
    }
    let mut notes = Vec::new();
    if cabs.is_empty() {
        return Err(CvjWriteError::Template(
            "šablóna nemá žiadnu skrinku s korpusom na prednej ploche steny".into(),
        ));
    }
    let room = cc_room::Room::new(project);

    // Čo CabinetCRAFT zo šablóny modeluje: skrinky, ktoré importér prevezme. Tie sa nahradia
    // skrinkami projektu (aj zmazané v projekte zmiznú); ostatné objekty CV (výplne, dosky,
    // spotrebiče) ostanú. Skrinka projektu bez steny sa nezapíše – jej pôvodná v CV ostane.
    let job = cc_import_cvj::parse_contents(&t.contents)?;
    let defaults = cc_model::Defaults {
        panel_material: String::new(),
        back_material: String::new(),
        construction: String::new(),
    };
    // šablóna tak, ako ju vidí import: zápis mení rozmery CV o rozdiel našej skrinky oproti
    // importu (import berie napr. šikmé skrinky z obrysu dielov, CV má vlastný rozmer)
    let imported_project = cc_import_cvj::to_project(&job, &defaults).0;
    let imported_room = cc_room::Room::new(&imported_project);
    let imported: BTreeMap<u32, &CabinetSpec> = imported_project
        .cabinets
        .iter()
        .filter_map(|c| c.cv_id.map(|id| (id, c)))
        .collect();
    let importable: std::collections::BTreeSet<u32> = imported.keys().copied().collect();
    // ostanú v CV: skrinky projektu bez steny a skrinky, ktoré import vynechal
    let kept_in_cv: std::collections::BTreeSet<u32> = project
        .cabinets
        .iter()
        .filter(|c| c.wall.is_none())
        .filter_map(|c| c.cv_id)
        .chain(project.project.cv_skipped.iter().copied())
        .collect();
    cabs.retain(|c| importable.contains(&c.node.object_id));
    let removed_cabs: Vec<&TemplateCabinet> = cabs
        .iter()
        .filter(|c| !kept_in_cv.contains(&c.node.object_id))
        .collect();

    // čísla CABNO, ktoré ostávajú v šablóne (výplne, dosky…)
    let removed: Vec<usize> = removed_cabs.iter().map(|c| c.node.pos).collect();
    let mut used_cabno: Vec<i32> = Vec::new();
    fn cabnos(n: &Node, removed: &[usize], out: &mut Vec<i32>) {
        if n.class == "CVCabinet" && !removed.contains(&n.pos) {
            if let Some(cc_import_cvj::classes::Value::Int(v)) = param(n, "CABNO").map(|p| &p.value)
            {
                out.push(*v);
            }
        }
        for c in &n.children {
            cabnos(c, removed, out);
        }
    }
    cabnos(&t.walk.root, &removed, &mut used_cabno);

    let mut next_id = t.max_object_id().saturating_add(1);
    let mut next_cabno = 1i32;
    // steny: ponechané, vynechané, nové (findings 13 §11)
    let wall_plan = crate::walls::plan(
        &t.walk.root,
        &t.contents,
        Some((project, &imported_project)),
        &mut next_id,
    );
    let wall_known = |id: &str| {
        wall_plan.kept.values().any(|k| k == id) || wall_plan.added.iter().any(|a| a.id == id)
    };
    // klony podľa steny projektu (ID)
    let mut per_face: BTreeMap<String, Vec<CloneJob>> = BTreeMap::new();
    let mut written = Vec::new();

    for c in &project.cabinets {
        let Some(wall_id) = c.wall.as_deref() else {
            notes.push(tr!("export_cvj.cabinet_not_on_wall", id = &c.id));
            continue;
        };
        if !wall_known(wall_id) {
            notes.push(tr!(
                "export_cvj.cabinet_wall_skipped",
                id = &c.id,
                wall = wall_id
            ));
            continue;
        }
        let class = cv_class(c.cabinet_type);
        // vzor: tá istá skrinka (import z tejto zákazky), inak rovnaká položka katalógu,
        // inak skrinka rovnakej triedy s najbližšími rozmermi
        let by_id = c
            .cv_id
            .and_then(|id| cabs.iter().find(|s| s.node.object_id == id));
        let by_name = || {
            c.catalog.as_deref().and_then(|name| {
                cabs.iter()
                    .find(|s| s.node.name.trim() == name && s.node.cv_class == class)
            })
        };
        let by_class = || {
            cabs.iter()
                .filter(|s| s.node.cv_class == class)
                .min_by(|a, b| {
                    let d = |s: &TemplateCabinet| {
                        let dx = f64_param(s.node, "DX").map_or(0.0, |v| v * INCH);
                        let dz = f64_param(s.node, "DZ").map_or(0.0, |v| v * INCH);
                        (dx - c.w).abs() + (dz - c.d).abs()
                    };
                    d(a).total_cmp(&d(b))
                })
        };
        let src = by_id.or_else(by_name).or_else(by_class);
        // vzor iba podľa druhu: skrinka bez väzby na katalóg (findings 13 §12.5)
        let unlinked = by_id.is_none() && by_name().is_none();
        if by_id.is_none() && by_name().is_none() {
            if let Some(s) = src {
                notes.push(tr!(
                    "export_cvj.cabinet_pattern",
                    id = &c.id,
                    pattern = s.node.name.trim()
                ));
            }
        }
        let Some(src) = src else {
            notes.push(tr!("export_cvj.cabinet_no_pattern", id = &c.id));
            continue;
        };
        let n = src.node;
        let mut patches = BTreeMap::new();
        let mut skips = Vec::new();
        let mut kept = skeleton(n, &mut skips, &mut patches);

        // poloha a rozmery: vzor + zmena našej skrinky oproti importu vzoru
        let toe = f64_param(n, "TOEH").map_or(0.0, |v| v * INCH);
        let ours = target(c, room.elevation(c), toe);
        let len = |name: &str, or: f64| f64_param(n, name).map_or(or, |v| v * INCH);
        let orig = Target {
            x: len("X", ours.x),
            y: len("Y", ours.y),
            z: len("Z", ours.z),
            dx: len("DX", ours.dx),
            dy: len("DY", ours.dy),
            dz: len("DZ", ours.dz),
        };
        let tg = match imported.get(&n.object_id) {
            Some(imp) => {
                let was = target(imp, imported_room.elevation(imp), toe);
                Target {
                    x: orig.x + ours.x - was.x,
                    y: orig.y + ours.y - was.y,
                    z: orig.z + ours.z - was.z,
                    dx: orig.dx + ours.dx - was.dx,
                    dy: orig.dy + ours.dy - was.dy,
                    dz: orig.dz + ours.dz - was.dz,
                }
            }
            None => ours,
        };

        // delenie čiel z našej skrinky: pôvodné sekcie vzoru vynechať, nové vložiť na ich miesto
        let face = n.children.iter().find(|g| is_face_group(g));
        let face_len = |name: &str| face.and_then(|g| f64_param(g, name)).map(|v| v * INCH);
        let face_w = face_len("DX").map_or(tg.dx, |v| v + tg.dx - orig.dx);
        let face_h = face_len("DY").map_or(tg.dy - toe, |v| v + tg.dy - orig.dy);
        let generated = face_sections(c, standard, face_w, face_h, 18.0);
        let mut wings: BTreeMap<i32, usize> = BTreeMap::new();
        let resolved: Option<Vec<(&SectionNode, &Node, i32)>> = generated
            .iter()
            .map(|s| {
                let w = wings.entry(s.id).or_insert(0);
                let wing = *w;
                *w += 1;
                prototype(s.kind, s.id, wing, n, &protos).map(|(p, st)| (s, p, st))
            })
            .collect();
        // čelá sa od importu nezmenili: delenie zo zákazky CV (úmerne rozmeru), nie z našich
        // čiel, ktoré import mohol zjednodušiť
        let fronts_from_cv =
            by_id.is_some() && c.cv_fronts.as_deref() == Some(c.front_fingerprint().as_str());
        let resolved = resolved.filter(|_| !fronts_from_cv);
        let regen = match (face, resolved) {
            (Some(g), Some(r)) if !g.children.is_empty() && !r.is_empty() => Some((g, r)),
            (Some(_), None) if !fronts_from_cv => {
                notes.push(tr!("export_cvj.cabinet_fronts_from_pattern", id = &c.id));
                None
            }
            _ => None,
        };
        if let Some((g, _)) = &regen {
            for sec in &g.children {
                skips.push((sec.pos, sec.end));
            }
            kept.retain(|k| !g.children.iter().any(|sec| std::ptr::eq(*k, sec)));
            if let Some(at) = g.children_count_at {
                let count = u16::try_from(generated.len()).map_err(|_| {
                    CvjWriteError::Template(format!("skrinka `{}`: príliš veľa otvorov", c.id))
                })?;
                patches.insert(at, count.to_le_bytes().to_vec());
            }
        }

        // nové ObjectID pre všetko, čo ostane (uzly aj parametre s nenulovým ID)
        for k in &kept {
            if k.object_id != 0 {
                patches.insert(k.id_at, next_id.to_le_bytes().to_vec());
                next_id = next_id.saturating_add(1);
            }
            for p in &k.params {
                if p.object_id != 0 {
                    patches.insert(p.id_at, next_id.to_le_bytes().to_vec());
                    next_id = next_id.saturating_add(1);
                }
            }
        }

        let (old_dx, old_dy, old_dz) = (orig.dx, orig.dy, orig.dz);
        for (name, v) in [
            ("X", tg.x),
            ("Y", tg.y),
            ("Z", tg.z),
            ("DX", tg.dx),
            ("DY", tg.dy),
            ("DZ", tg.dz),
        ] {
            if let Some(p) = param(n, name).filter(|p| is_length(p)) {
                patch_value(&mut patches, p, v);
            }
        }
        // tá istá skrinka zo zákazky si nechá svoje číslo, nové dostanú prvé voľné
        let original = by_id.and_then(|s| match param(s.node, "CABNO").map(|p| &p.value) {
            Some(cc_import_cvj::classes::Value::Int(v)) if !used_cabno.contains(v) => Some(*v),
            _ => None,
        });
        let cabno = match original {
            Some(v) => v,
            None => {
                while used_cabno.contains(&next_cabno) {
                    next_cabno += 1;
                }
                next_cabno
            }
        };
        used_cabno.push(cabno);
        if let Some(p) = param(n, "CABNO") {
            patch_value(&mut patches, p, f64::from(cabno));
        }
        // natiahnutie W/H/D povolené len v tejto inštancii (findings 13 §10.6)
        if let (Some(at), Some(flags)) = (
            n.flags_at,
            n.flags_at.and_then(|at| {
                t.contents
                    .get(at..at + 4)
                    .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            }),
        ) {
            patches.insert(at, (flags | 7).to_le_bytes().to_vec());
        }
        // skupiny a sekcie: rozmery úmerne novej skrinke
        let (sx, sy) = (
            if old_dx > 0.0 { tg.dx / old_dx } else { 1.0 },
            if old_dy - toe > 0.0 {
                (tg.dy - toe) / (old_dy - toe)
            } else {
                1.0
            },
        );
        for k in kept.iter().skip(1) {
            let group = k.class == "CVAssembly"
                && ["Case", "Interior", "Face"]
                    .iter()
                    .any(|g| k.name.starts_with(g))
                && param(k, "ID").is_none();
            let section = param(k, "ID").is_some();
            // doplnky bez PID (sokel…) sa nemenia, CV ich prepočíta podľa ich rovníc
            if !group && !section {
                continue;
            }
            for p in k.params.iter().filter(|p| is_length(p)) {
                let v = match p.value {
                    cc_import_cvj::classes::Value::Length(v) => v * INCH,
                    _ => continue,
                };
                // skupiny: o zmenu skrinky; plocha čiel (`Face`) je rovina v hĺbke skrinky
                let face_group = k.name.starts_with("Face");
                let nv = match (group, p.name.as_str()) {
                    (true, "DX") => v + tg.dx - old_dx,
                    (true, "DY") => v + tg.dy - old_dy,
                    (true, "DZ") if v.abs() > 1e-6 => v + tg.dz - old_dz,
                    (true, "Z") if face_group && (v - old_dz).abs() < 0.5 => v + tg.dz - old_dz,
                    (false, "X" | "DX") => v * sx,
                    (false, "Y" | "DY") => v * sy,
                    _ => continue,
                };
                patch_value(&mut patches, p, nv);
            }
        }
        let mut replaces = BTreeMap::new();
        if unlinked {
            for k in &kept {
                if let Some(at) = k.link_at {
                    patches.insert(at, 0u32.to_le_bytes().to_vec());
                }
            }
            if let Some(at) = n.nominal_at {
                let mut b = Vec::with_capacity(24);
                for v in [tg.dx, tg.dy, tg.dz] {
                    b.extend_from_slice(&(v / INCH).to_le_bytes());
                }
                patches.insert(at, b);
            }
            let name = c.catalog.clone().unwrap_or_else(|| {
                match c.cabinet_type {
                    CabinetType::Base => "CC_SPODNA",
                    CabinetType::Wall => "CC_HORNA",
                    CabinetType::Tall => "CC_VYSOKA",
                }
                .to_owned()
            });
            replaces.insert(n.name_at, (n.name_len, cstring(&name)));
        }
        let mut segments = Vec::new();
        match regen {
            Some((g, resolved)) => {
                // PID otvorov ako v CV: základ skrinky + ID × 100 (napr. 20100, 20200…)
                let pid_base = g
                    .children
                    .first()
                    .and_then(|sec| f64_param(sec, "PID"))
                    .map(|v| (v / 10_000.0).floor() * 10_000.0);
                let insert_at = g.children.first().map_or(g.end, |sec| sec.pos);
                segments.push(Segment {
                    start: n.pos,
                    end: insert_at,
                    patches: patches.clone(),
                    skips: skips.clone(),
                });
                for (s, p, st) in resolved {
                    section_segments(
                        s,
                        p,
                        st,
                        &donors,
                        pid_base,
                        unlinked,
                        &mut next_id,
                        &mut segments,
                    )?;
                }
                segments.push(Segment {
                    start: insert_at,
                    end: n.end,
                    patches,
                    skips,
                });
            }
            None => segments.push(Segment {
                start: n.pos,
                end: n.end,
                patches,
                skips,
            }),
        }
        per_face
            .entry(wall_id.to_owned())
            .or_default()
            .push(CloneJob { segments, replaces });
        written.push((c.id.clone(), cabno));
    }

    // okná, dvere, otvory: tie, ktoré import prevzal, sa nahradia otvormi projektu
    let opening_kind = |n: &Node| match (n.cv_class, n.cv_type) {
        (7, 19) => Some(cc_model::OpeningKind::Window),
        (7, 20) => Some(cc_model::OpeningKind::Door),
        (7, 26) => Some(cc_model::OpeningKind::Opening),
        _ => None,
    };
    let mut template_openings: Vec<(u16, &Node, cc_model::OpeningKind)> = Vec::new();
    for f in &faces {
        for c in f.node.children.iter().filter(|c| c.class == "CVCabinet") {
            if let Some(k) = opening_kind(c) {
                template_openings.push((f.wall, c, k));
            }
        }
    }
    let imported_as = |wall: u16, n: &Node, k: cc_model::OpeningKind| {
        let id = format!("W{wall}");
        let (x, dx) = (
            f64_param(n, "X").map_or(f64::NAN, |v| v * INCH),
            f64_param(n, "DX").map_or(f64::NAN, |v| v * INCH),
        );
        imported_project.openings.iter().any(|o| {
            o.wall == id
                && o.kind == k
                && (o.at - x.max(0.0)).abs() < 1.0
                && (o.width - dx).abs() < 1.0
        })
    };
    let removed_openings: Vec<(u16, &Node)> = template_openings
        .iter()
        .filter(|(w, n, k)| imported_as(*w, n, *k))
        .map(|(w, n, _)| (*w, *n))
        .collect();
    // okno projektu a jeho vzor zo šablóny (pre doplnky CV, ktoré idú s oknom)
    let mut moves: Vec<(u16, &Node, &cc_model::OpeningSpec)> = Vec::new();
    for o in &project.openings {
        if !wall_known(&o.wall) {
            notes.push(tr!(
                "export_cvj.opening_wall_skipped",
                kind = opening_kind_text(o.kind),
                wall = &o.wall
            ));
            continue;
        }
        // číslo steny šablóny, ak je to stena zo šablóny
        let number = wall_plan
            .kept
            .iter()
            .find(|(_, id)| **id == o.wall)
            .map_or(0, |(n, _)| *n);
        // vzor rovnakého druhu: na tej istej stene najbližší, inak prvý v šablóne; bez neho iný
        // druh otvoru so zmeneným TYPE a bez detí (tvarov rámu), CV ukáže obálku
        let nearest = |kind: Option<cc_model::OpeningKind>| {
            template_openings
                .iter()
                .filter(|(_, _, k)| kind.is_none_or(|kind| *k == kind))
                .min_by(|a, b| {
                    let d = |(w, n, _): &(u16, &Node, cc_model::OpeningKind)| {
                        let x = f64_param(n, "X").map_or(0.0, |v| v * INCH);
                        (if *w == number { 0.0 } else { 1e9 }) + (x - o.at).abs()
                    };
                    d(a).total_cmp(&d(b))
                })
        };
        let (src, other_kind) = match nearest(Some(o.kind)) {
            Some(s) => (Some(s), false),
            None => (nearest(None), true),
        };
        let Some((src_wall, n, _)) = src else {
            notes.push(tr!(
                "export_cvj.opening_no_pattern",
                kind = opening_kind_text(o.kind),
                wall = &o.wall
            ));
            continue;
        };
        if *src_wall == number
            && removed_openings.iter().any(|(_, r)| std::ptr::eq(*r, *n))
            && !moves.iter().any(|(_, m, _)| std::ptr::eq(*m, *n))
        {
            moves.push((number, n, o));
        }
        let mut patches = BTreeMap::new();
        let mut subtree = Vec::new();
        add_subtree(n, &mut subtree);
        for k in &subtree {
            if k.object_id != 0 {
                patches.insert(k.id_at, next_id.to_le_bytes().to_vec());
                next_id = next_id.saturating_add(1);
            }
            for p in &k.params {
                if p.object_id != 0 {
                    patches.insert(p.id_at, next_id.to_le_bytes().to_vec());
                    next_id = next_id.saturating_add(1);
                }
            }
        }
        let mut skips = Vec::new();
        let mut opening_replaces = BTreeMap::new();
        if other_kind {
            // TYPE je u16 pred príznakmi +0x78 (findings 13 §11.4): 19 okno, 20 dvere, 26 otvor
            let ty: u16 = match o.kind {
                cc_model::OpeningKind::Window => 19,
                cc_model::OpeningKind::Door => 20,
                cc_model::OpeningKind::Opening => 26,
            };
            if let Some(at) = n.flags_at.and_then(|f| f.checked_sub(2)) {
                patches.insert(at, ty.to_le_bytes().to_vec());
            }
            for k in &n.children {
                skips.push((k.pos, k.end));
            }
            if let Some(at) = n.children_count_at {
                patches.insert(at, 0u16.to_le_bytes().to_vec());
            }
            let name = match o.kind {
                cc_model::OpeningKind::Window => "OKNO",
                cc_model::OpeningKind::Door => "DVERE",
                cc_model::OpeningKind::Opening => "OTVOR",
            };
            opening_replaces.insert(n.name_at, (n.name_len, cstring(name)));
            // popis nasleduje hneď za menom
            opening_replaces.insert(n.name_at + n.name_len, (n.desc_len, cstring(name)));
            notes.push(tr!(
                "export_cvj.opening_envelope",
                kind = opening_kind_text(o.kind),
                wall = &o.wall
            ));
        }
        // poloha od začiatku plochy, parapet, šírka a výška; hĺbka ostáva zo vzoru
        for (name, v) in [
            ("X", o.at),
            ("Y", o.elevation),
            ("DX", o.width),
            ("DY", o.height),
        ] {
            if let Some(p) = param(n, name).filter(|p| is_length(p)) {
                patch_value(&mut patches, p, v);
            }
        }
        per_face.entry(o.wall.clone()).or_default().push(CloneJob {
            segments: vec![Segment {
                start: n.pos,
                end: n.end,
                patches,
                skips,
            }],
            replaces: opening_replaces,
        });
    }

    // steny a ich zadné plochy (v hlavnom prúde)
    let mut global = wall_plan.global.clone();
    notes.extend(wall_plan.notes.iter().cloned());

    // doplnky CV pri okne (parapet…): objekt, ktorý import neprevzal, s rovnakou polohou a
    // šírkou ako pôvodné okno na tej istej stene, ide s oknom (posun X/Y, zmena šírky)
    let mm = |n: &Node, k: &str| f64_param(n, k).map(|v| v * INCH);
    for (wall, src, o) in &moves {
        let (Some(ox), Some(oy), Some(odx)) = (mm(src, "X"), mm(src, "Y"), mm(src, "DX")) else {
            continue;
        };
        let (mx, my, mdx) = (o.at - ox, o.elevation - oy, o.width - odx);
        if mx.abs() < 1e-6 && my.abs() < 1e-6 && mdx.abs() < 1e-6 {
            continue;
        }
        for f in faces.iter().filter(|f| f.wall == *wall) {
            for c in f.node.children.iter().filter(|c| {
                c.class == "CVCabinet"
                    && opening_kind(c).is_none()
                    && !importable.contains(&c.object_id)
            }) {
                let (Some(cx), Some(cdx)) = (mm(c, "X"), mm(c, "DX")) else {
                    continue;
                };
                if (cx - ox).abs() > 1.0 || (cdx - odx).abs() > 1.0 || cdx + mdx <= 0.0 {
                    continue;
                }
                for (k, d) in [("X", mx), ("Y", my), ("DX", mdx)] {
                    if let (Some(p), Some(v)) = (param(c, k).filter(|p| is_length(p)), mm(c, k)) {
                        patch_value(&mut global, p, v + d);
                    }
                }
                let ratio = (cdx + mdx) / cdx;
                for g in c.children.iter().filter(|g| {
                    g.class == "CVAssembly"
                        && ["Case", "Interior", "Face"]
                            .iter()
                            .any(|n| g.name.starts_with(n))
                        && param(g, "ID").is_none()
                }) {
                    if let (Some(p), Some(v)) =
                        (param(g, "DX").filter(|p| is_length(p)), mm(g, "DX"))
                    {
                        patch_value(&mut global, p, v + mdx);
                    }
                    for sec in g.children.iter().filter(|s| param(s, "ID").is_some()) {
                        for k in ["X", "DX"] {
                            if let (Some(p), Some(v)) =
                                (param(sec, k).filter(|p| is_length(p)), mm(sec, k))
                            {
                                patch_value(&mut global, p, v * ratio);
                            }
                        }
                    }
                }
                // natiahnutie povolené v tejto inštancii ako pri skrinkách (findings 13 §10.6)
                if let Some(flags) = c.flags_at.and_then(|at| {
                    t.contents
                        .get(at..at + 4)
                        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                }) {
                    if let Some(at) = c.flags_at {
                        global.insert(at, (flags | 7).to_le_bytes().to_vec());
                    }
                }
                notes.push(tr!(
                    "export_cvj.moved_with_window",
                    name = c.name.trim(),
                    wall = *wall,
                    x = format!("{mx:+.1}"),
                    y = format!("{my:+.1}"),
                    width = format!("{mdx:+.1}")
                ));
            }
        }
    }

    // počty detí plôch: pôvodné − odstránené + klony (iba steny, ktoré ostali)
    for f in &faces {
        let Some(at) = f.node.children_count_at else {
            continue;
        };
        let Some(wall_id) = wall_plan.kept.get(&f.wall) else {
            continue;
        };
        let removed_here = removed_cabs.iter().filter(|c| c.wall == f.wall).count()
            + removed_openings
                .iter()
                .filter(|(w, _)| *w == f.wall)
                .count();
        let added = per_face.get(wall_id).map_or(0, Vec::len);
        let count = f.node.children.len() - removed_here + added;
        let count = u16::try_from(count).map_err(|_| {
            CvjWriteError::Template(format!("príliš veľa objektov na stene č. {}", f.wall))
        })?;
        global.insert(at, count.to_le_bytes().to_vec());
    }
    let removed_skips: Vec<Skip> = removed_cabs
        .iter()
        .map(|c| (c.node.pos, c.node.end))
        .chain(removed_openings.iter().map(|(_, n)| (n.pos, n.end)))
        .chain(wall_plan.removed.iter().copied())
        .collect();

    // miesta vloženia: koniec prednej plochy ponechanej steny (CVWallFace::Load končí zoznamom
    // detí) a koniec miestnosti pre nové steny
    enum Insert<'s> {
        Face(&'s str),
        Walls,
    }
    let mut points: Vec<(usize, Insert)> = faces
        .iter()
        .filter_map(|f| {
            wall_plan
                .kept
                .get(&f.wall)
                .map(|id| (f.node.end, Insert::Face(id.as_str())))
        })
        .collect();
    if let (Some(end), false) = (wall_plan.floor_end, wall_plan.added.is_empty()) {
        points.push((end, Insert::Walls));
    }
    points.sort_by_key(|(at, _)| *at);

    fn emit_clones(em: &mut Emitter, clones: Option<&Vec<CloneJob>>) -> Result<(), CvjWriteError> {
        for cl in clones.into_iter().flatten() {
            em.replaces = cl.replaces.clone();
            for sg in &cl.segments {
                em.emit(sg.start, sg.end, &sg.patches, &sg.skips)?;
            }
            em.replaces.clear();
        }
        Ok(())
    }

    let mut em = Emitter::new(&t.contents, &t.events);
    let mut pos = 0usize;
    for (at, what) in points {
        em.replaces = wall_plan.replaces.clone();
        em.emit(pos, at, &global, &removed_skips)?;
        em.replaces.clear();
        match what {
            Insert::Face(id) => emit_clones(&mut em, per_face.get(id))?,
            Insert::Walls => {
                for w in &wall_plan.added {
                    let clones = per_face.get(&w.id);
                    let mut patches = w.patches.clone();
                    if let Some(c) = w.front.children_count_at {
                        let n = u16::try_from(clones.map_or(0, Vec::len)).map_err(|_| {
                            CvjWriteError::Template(format!(
                                "príliš veľa objektov na stene {}",
                                w.id
                            ))
                        })?;
                        patches.insert(c, n.to_le_bytes().to_vec());
                    }
                    em.replaces = w.replaces.clone();
                    em.emit(w.src.pos, w.front.end, &patches, &w.skips)?;
                    em.replaces.clear();
                    emit_clones(&mut em, clones)?;
                    em.replaces = w.replaces.clone();
                    em.emit(w.front.end, w.src.end, &patches, &w.skips)?;
                    em.replaces.clear();
                }
            }
        }
        pos = at;
    }
    em.replaces = wall_plan.replaces.clone();
    em.emit(pos, t.contents.len(), &global, &removed_skips)?;
    Ok(Export {
        contents: em.finish(),
        cabinets: written,
        notes,
    })
}

/// Druh otvoru v poznámke (v zvolenom jazyku).
fn opening_kind_text(kind: cc_model::OpeningKind) -> String {
    match kind {
        cc_model::OpeningKind::Window => tr!("export_cvj.opening.window"),
        cc_model::OpeningKind::Door => tr!("export_cvj.opening.door"),
        cc_model::OpeningKind::Opening => tr!("export_cvj.opening.opening"),
    }
}
