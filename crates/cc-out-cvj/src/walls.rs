//! Steny projektu do stien šablóny (findings 13 §11, §13.5).
//!
//! CV stenu pri otvorení neprepočíta (`CVWall::Build` stavia iba z vlastných parametrov), preto
//! sa zapíšu všetky závislé hodnoty:
//! - stena `X Z DX DY DZ AY`: pri stene zo šablóny pôvodná hodnota CV + zmena našej steny
//!   oproti importu šablóny, pri novej stene hodnota projektu,
//! - číslo steny = poradie v miestnosti (CV `RenumberWalls`), susedia podľa geometrie (koniec
//!   steny = začiatok pravého suseda), kódy plôch `(číslo << 16) | strana` a pripojené plochy,
//! - `_DR` / `_DL` = predĺženie zadnej línie po priesečník so zadnou líniou suseda, `_AI`,
//! - zadná plocha začína na konci steny posunutom o `_DR` po smere a o hrúbku dozadu,
//!   `DX = L + _DL + _DR`, `AY` = opačný smer.
//!
//! Stena šablóny, ktorú projekt nemá, sa vynechá aj s objektmi na nej. Nová stena je kópia
//! rovnej steny šablóny bez objektov na plochách (findings 13 §11.3), vloží sa na koniec
//! miestnosti. Hodnota v stene šablóny sa prepíše iba vtedy, keď sa líši od uloženej
//! (nezmenená zákazka ostane bajtovo rovnaká).

use std::collections::BTreeMap;

use cc_i18n::tr;
use cc_import_cvj::classes::{Extra, Node, Param, Value};
use cc_model::{ProjectFile, WallSpec};

use crate::emit::Skip;

const INCH: f64 = 25.4;
/// rozdiel, pod ktorým sa hodnota neprepisuje (mm, °)
const SAME: f64 = 1e-6;
/// koniec a začiatok susedných stien bližšie ako toto sú spojené (mm)
const JOIN: f64 = 1.0;

type Patches = BTreeMap<usize, Vec<u8>>;

fn param<'a>(n: &'a Node, name: &str) -> Option<&'a Param> {
    n.params.iter().find(|p| p.name == name)
}

/// Hodnota parametra v mm (dĺžka) alebo v stupňoch (uhol).
fn value(p: &Param) -> Option<f64> {
    match p.value {
        Value::Length(v) => Some(v * INCH),
        Value::Angle(v) | Value::Number(v) => Some(v),
        _ => None,
    }
}

fn get(n: &Node, name: &str) -> Option<f64> {
    param(n, name).and_then(value)
}

fn bytes(p: &Param, v: f64) -> Option<Vec<u8>> {
    match p.value {
        Value::Length(_) => Some((v / INCH).to_le_bytes().to_vec()),
        Value::Angle(_) | Value::Number(_) => Some(v.to_le_bytes().to_vec()),
        _ => None,
    }
}

/// Prepíše parameter, ak sa nová hodnota líši od uloženej.
fn set(patches: &mut Patches, n: &Node, name: &str, v: f64) {
    if let Some(p) = param(n, name) {
        if value(p).is_some_and(|old| (old - v).abs() > SAME) {
            if let Some(b) = bytes(p, v) {
                patches.insert(p.value_at, b);
            }
        }
    }
}

fn read_u16(data: &[u8], at: usize) -> Option<u16> {
    data.get(at..at + 2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
}

fn read_u32(data: &[u8], at: usize) -> Option<u32> {
    data.get(at..at + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn set_u16(patches: &mut Patches, data: &[u8], at: usize, v: u16) {
    if read_u16(data, at) != Some(v) {
        patches.insert(at, v.to_le_bytes().to_vec());
    }
}

fn set_u32(patches: &mut Patches, data: &[u8], at: usize, v: u32) {
    if read_u32(data, at) != Some(v) {
        patches.insert(at, v.to_le_bytes().to_vec());
    }
}

fn norm180(a: f64) -> f64 {
    let a = a % 360.0;
    if a <= -180.0 {
        a + 360.0
    } else if a > 180.0 {
        a - 360.0
    } else {
        a
    }
}

fn snap(x: f64) -> f64 {
    [-1.0, 0.0, 1.0]
        .into_iter()
        .find(|t| (x - t).abs() < 1e-12)
        .unwrap_or(x)
}

struct TemplateWall<'a> {
    node: &'a Node,
    number: u16,
    left: u16,
    right: u16,
}

fn collect<'a>(n: &'a Node, out: &mut Vec<TemplateWall<'a>>, floor: &mut Option<&'a Node>) {
    for c in &n.children {
        if let Extra::Wall { number, prev, next } = c.extra {
            if c.class == "CVWall" {
                out.push(TemplateWall {
                    node: c,
                    number,
                    left: prev,
                    right: next,
                });
                if floor.is_none() {
                    *floor = Some(n);
                }
            }
        }
        collect(c, out, floor);
    }
}

fn face<'a>(wall: &'a Node, name: &str) -> Option<&'a Node> {
    wall.children
        .iter()
        .find(|c| c.class == "CVWallFace" && c.name == name)
}

/// Nová stena: kópia steny šablóny `src` bez objektov na plochách.
pub struct AddedWall<'a> {
    /// ID steny v projekte
    pub id: String,
    pub src: &'a Node,
    /// predná plocha vzoru (objekty projektu sa vložia na jej koniec)
    pub front: &'a Node,
    pub patches: Patches,
    pub replaces: BTreeMap<usize, (usize, Vec<u8>)>,
    pub skips: Vec<Skip>,
}

/// Čo sa so stenami zapíše.
#[derive(Default)]
pub struct WallPlan<'a> {
    /// záplaty v hlavnom prúde (steny šablóny, počet stien miestnosti)
    pub global: Patches,
    /// náhrady popisov stien šablóny („Stena N“ po prečíslovaní)
    pub replaces: BTreeMap<usize, (usize, Vec<u8>)>,
    pub notes: Vec<String>,
    /// steny šablóny, ktoré projekt nemá (celé, aj s objektmi)
    pub removed: Vec<Skip>,
    /// číslo steny šablóny → ID steny projektu (ponechané steny)
    pub kept: BTreeMap<u16, String>,
    /// nové steny (vložia sa na koniec miestnosti = `floor_end`)
    pub added: Vec<AddedWall<'a>>,
    pub floor_end: Option<usize>,
}

/// Stena na zápis.
struct Entry<'a> {
    node: &'a Node,
    /// nová stena: index v `added`
    added: Option<usize>,
    number: u16,
    left: u16,
    right: u16,
    a: f64,
    s: (f64, f64),
    len: f64,
    t: f64,
    h: Option<f64>,
}

impl Entry<'_> {
    fn dir(&self) -> (f64, f64) {
        let r = self.a.to_radians();
        (snap(r.cos()), snap(-r.sin()))
    }
    fn end(&self) -> (f64, f64) {
        let d = self.dir();
        (self.s.0 + self.len * d.0, self.s.1 + self.len * d.1)
    }
}

/// `CString` MFC: ASCII s dĺžkou u8, inak UTF-16LE so značkou `FF FE FF`.
fn cstring(text: &str) -> Vec<u8> {
    let text: String = text.chars().take(200).collect();
    if text.is_ascii() {
        let mut out = vec![text.len() as u8];
        out.extend(text.bytes());
        out
    } else {
        let units: Vec<u16> = text.encode_utf16().collect();
        let mut out = vec![0xFF, 0xFE, 0xFF, units.len().min(0xFE) as u8];
        for u in units.iter().take(0xFE) {
            out.extend(u.to_le_bytes());
        }
        out
    }
}

/// Popis steny podľa vzoru šablóny: text pred číslom na konci („Wall 1“ → „Wall N“).
fn wall_description(sample: Option<&str>, number: u16) -> String {
    let prefix = sample
        .map(|s| s.trim_end_matches(|c: char| c.is_ascii_digit()))
        .filter(|p| !p.trim().is_empty())
        .unwrap_or("Wall ");
    format!("{prefix}{number}")
}

/// Plán stien: `project` = (náš projekt, import šablóny); bez neho sa iba prepočítajú závislé
/// hodnoty z uložených stien (kontrola). `data` = prúd `Contents` šablóny, `next_id` = ďalšie
/// voľné ObjectID.
pub fn plan<'a>(
    template: &'a Node,
    data: &[u8],
    project: Option<(&ProjectFile, &ProjectFile)>,
    next_id: &mut u32,
) -> WallPlan<'a> {
    let mut walls = Vec::new();
    let mut floor = None;
    collect(template, &mut walls, &mut floor);
    let mut out = WallPlan {
        floor_end: floor.map(|f| f.end),
        ..WallPlan::default()
    };

    // steny na zápis: ponechané v poradí šablóny, potom nové v poradí projektu
    let mut entries: Vec<Entry> = Vec::new();
    for w in &walls {
        let n = w.node;
        let (Some(x), Some(z), Some(len), Some(t), Some(a)) = (
            get(n, "X"),
            get(n, "Z"),
            get(n, "DX"),
            get(n, "DZ"),
            get(n, "AY"),
        ) else {
            out.notes
                .push(tr!("export_cvj.wall_no_position", wall = w.number));
            continue;
        };
        let stored = (x, z, len, t, a, get(n, "DY"));
        let (mut x, mut z, mut len, mut t, mut a, mut h) = stored;
        if let Some((project, imported)) = project {
            let room = project.room.clone().unwrap_or_default();
            let imp_room = imported.room.clone().unwrap_or_default();
            let id = format!("W{}", w.number);
            let ours = project.walls.iter().find(|s| s.id == id);
            let was = imported.walls.iter().find(|s| s.id == id);
            match (ours, was) {
                (Some(o), Some(i)) => {
                    x += o.x - i.x;
                    z -= o.y - i.y;
                    len += o.length - i.length;
                    a += o.angle - i.angle;
                    t += o.thickness.unwrap_or(room.wall_thickness)
                        - i.thickness.unwrap_or(imp_room.wall_thickness);
                    h = h.map(|h| {
                        h + o.height.unwrap_or(room.wall_height)
                            - i.height.unwrap_or(imp_room.wall_height)
                    });
                }
                (Some(o), None) => {
                    x = o.x;
                    z = -o.y;
                    len = o.length;
                    a = o.angle;
                    t = o.thickness.unwrap_or(room.wall_thickness);
                    h = Some(o.height.unwrap_or(room.wall_height));
                }
                (None, _) => {
                    out.notes
                        .push(tr!("export_cvj.wall_removed", wall = w.number));
                    out.removed.push((n.pos, n.end));
                    continue;
                }
            }
            out.kept.insert(w.number, id);
        }
        let checks = [len, t, h.unwrap_or(1.0)];
        if !(x.is_finite() && z.is_finite() && a.is_finite())
            || checks.iter().any(|v| !v.is_finite() || *v <= 0.0)
        {
            out.notes
                .push(tr!("export_cvj.wall_bad_size", wall = w.number));
            (x, z, len, t, a, h) = stored;
        }
        entries.push(Entry {
            node: n,
            added: None,
            number: w.number,
            left: w.left,
            right: w.right,
            a,
            s: (x, z),
            len,
            t,
            h,
        });
    }

    // nové steny: kópia rovnej steny šablóny (najbližšej hrúbky)
    if let Some((project, _)) = project {
        let room = project.room.clone().unwrap_or_default();
        let new: Vec<&WallSpec> = project
            .walls
            .iter()
            .filter(|s| !out.kept.values().any(|k| k == &s.id))
            .collect();
        for s in new {
            let t = s.thickness.unwrap_or(room.wall_thickness);
            let src = walls
                .iter()
                .filter(|w| {
                    face(w.node, "Front Face").is_some() && face(w.node, "Back Face").is_some()
                })
                .min_by(|a, b| {
                    let d = |w: &&TemplateWall| (get(w.node, "DZ").unwrap_or(0.0) - t).abs();
                    d(a).total_cmp(&d(b))
                });
            let (Some(src), true) = (
                src,
                s.length.is_finite() && s.length > 0.0 && t.is_finite() && t > 0.0,
            ) else {
                out.notes
                    .push(tr!("export_cvj.wall_not_written", id = &s.id));
                continue;
            };
            let (Some(front), Some(back)) =
                (face(src.node, "Front Face"), face(src.node, "Back Face"))
            else {
                continue;
            };
            let mut patches = Patches::new();
            let mut skips: Vec<Skip> = Vec::new();
            let mut subtree: Vec<&Node> = vec![src.node, front, back];
            for f in [front, back] {
                for c in &f.children {
                    skips.push((c.pos, c.end));
                }
                if let Some(at) = f.children_count_at {
                    patches.insert(at, 0u16.to_le_bytes().to_vec());
                }
            }
            // ostatné deti steny (ak sú) sa tiež vynechajú
            for c in src.node.children.iter().filter(|c| c.class != "CVWallFace") {
                skips.push((c.pos, c.end));
            }
            for k in subtree.drain(..) {
                if k.object_id != 0 {
                    patches.insert(k.id_at, next_id.to_le_bytes().to_vec());
                    *next_id = next_id.saturating_add(1);
                }
                for p in &k.params {
                    if p.object_id != 0 {
                        patches.insert(p.id_at, next_id.to_le_bytes().to_vec());
                        *next_id = next_id.saturating_add(1);
                    }
                }
            }
            out.added.push(AddedWall {
                id: s.id.clone(),
                src: src.node,
                front,
                patches,
                replaces: BTreeMap::new(),
                skips,
            });
            entries.push(Entry {
                node: src.node,
                added: Some(out.added.len() - 1),
                number: 0,
                left: 0,
                right: 0,
                a: s.angle,
                s: (s.x, -s.y),
                len: s.length,
                t,
                h: Some(s.height.unwrap_or(room.wall_height)),
            });
        }
        // čísla = poradie; susedia podľa geometrie
        for (i, e) in entries.iter_mut().enumerate() {
            e.number = u16::try_from(i + 1).unwrap_or(u16::MAX);
        }
        let ends: Vec<((f64, f64), (f64, f64))> = entries.iter().map(|e| (e.s, e.end())).collect();
        let near = |p: (f64, f64), q: (f64, f64)| (p.0 - q.0).hypot(p.1 - q.1) < JOIN;
        for i in 0..entries.len() {
            let right = (0..entries.len())
                .find(|&j| j != i && near(ends[i].1, ends[j].0))
                .map_or(0, |j| entries[j].number);
            let left = (0..entries.len())
                .find(|&j| j != i && near(ends[j].1, ends[i].0))
                .map_or(0, |j| entries[j].number);
            entries[i].right = right;
            entries[i].left = left;
        }
        // nová stena nadväzuje presne na suseda (import zaokrúhľuje)
        for i in 0..entries.len() {
            if entries[i].added.is_none() {
                continue;
            }
            if let Some(l) = entries
                .iter()
                .find(|e| e.number == entries[i].left && e.number != 0)
            {
                let p = l.end();
                entries[i].s = p;
            }
            if let Some(r) = entries
                .iter()
                .find(|e| e.number == entries[i].right && e.number != 0)
            {
                let (s, d, q) = (entries[i].s, entries[i].dir(), r.s);
                let l = (q.0 - s.0) * d.0 + (q.1 - s.1) * d.1;
                if l > 0.0 {
                    entries[i].len = l;
                }
            }
        }
    }

    // geometria, čísla a kódy
    let by_number: BTreeMap<u16, usize> = entries
        .iter()
        .enumerate()
        .map(|(i, e)| (e.number, i))
        .collect();
    let mut dl: BTreeMap<u16, f64> = BTreeMap::new();
    let mut dr: BTreeMap<u16, f64> = BTreeMap::new();
    for g in &entries {
        if g.right == 0 {
            dr.insert(g.number, 0.0);
            continue;
        }
        let Some(h) = by_number.get(&g.right).and_then(|&i| entries.get(i)) else {
            continue;
        };
        let (ge, hs) = (g.end(), h.s);
        if (ge.0 - hs.0).abs() > 1e-3 || (ge.1 - hs.1).abs() > 1e-3 {
            out.notes.push(tr!(
                "export_cvj.walls_not_closed",
                wall = g.number,
                end = format!("{:.1}; {:.1}", ge.0, ge.1),
                next = h.number,
                start = format!("{:.1}; {:.1}", hs.0, hs.1)
            ));
        }
        let (gd, hd) = (g.dir(), h.dir());
        let (gb, hb) = ((gd.1, -gd.0), (hd.1, -hd.0));
        let p0 = (ge.0 + g.t * gb.0, ge.1 + g.t * gb.1);
        let q0 = (hs.0 + h.t * hb.0, hs.1 + h.t * hb.1);
        let den = gd.0 * (-hd.1) - gd.1 * (-hd.0);
        let (u, v) = if den.abs() < 1e-12 {
            (0.0, 0.0)
        } else {
            let (rx, rz) = (q0.0 - p0.0, q0.1 - p0.1);
            (
                (rx * (-hd.1) - rz * (-hd.0)) / den,
                (gd.0 * rz - gd.1 * rx) / den,
            )
        };
        dr.insert(g.number, u);
        dl.insert(h.number, -v);
    }
    for e in &entries {
        let target: &mut Patches = match e.added {
            Some(i) => match out.added.get_mut(i) {
                Some(a) => &mut a.patches,
                None => continue,
            },
            None => &mut out.global,
        };
        let n = e.node;
        set(target, n, "X", e.s.0);
        set(target, n, "Z", e.s.1);
        set(target, n, "DX", e.len);
        set(target, n, "DZ", e.t);
        set(target, n, "AY", e.a);
        if let Some(h) = e.h {
            set(target, n, "DY", h);
        }
        // číslo, susedia a kódy plôch
        if let Some(at) = n.extra_at {
            set_u16(target, data, at, e.number);
            set_u16(target, data, at + 6, e.left);
            set_u16(target, data, at + 8, e.right);
        }
        let code = |m: u16, side: u32| {
            if m == 0 {
                0
            } else {
                (u32::from(m) << 16) | side
            }
        };
        if let Some(at) = face(n, "Front Face").and_then(|f| f.extra_at) {
            set_u32(target, data, at, code(e.number, 1));
            set_u32(target, data, at + 4, code(e.left, 1));
            set_u32(target, data, at + 8, code(e.right, 1));
        }
        let back = face(n, "Back Face");
        if let Some(at) = back.and_then(|f| f.extra_at) {
            set_u32(target, data, at, code(e.number, 2));
            set_u32(target, data, at + 4, code(e.right, 2));
            set_u32(target, data, at + 8, code(e.left, 2));
        }
        let (Some(&r), l) = (dr.get(&e.number), dl.get(&e.number).copied().unwrap_or(0.0)) else {
            continue;
        };
        set(target, n, "_DL", l);
        set(target, n, "_DR", r);
        if let Some(left) = by_number.get(&e.left).and_then(|&i| entries.get(i)) {
            set(target, n, "_AI", 180.0 + norm180(e.a - left.a));
        }
        if let Some(back) = back {
            let (d, end) = (e.dir(), e.end());
            let b = (d.1, -d.0);
            set(target, back, "X", end.0 + r * d.0 + e.t * b.0);
            set(target, back, "Z", end.1 + r * d.1 + e.t * b.1);
            set(target, back, "DX", e.len + l + r);
            set(target, back, "AY", norm180(e.a + 180.0));
        }
    }
    // popis podľa vzoru šablóny („Wall N“) pri zmene čísla a pri novej stene
    let sample = walls
        .iter()
        .map(|w| w.node.description.as_str())
        .find(|d| d.ends_with(|c: char| c.is_ascii_digit()));
    let template_numbers: BTreeMap<usize, u16> =
        walls.iter().map(|w| (w.node.pos, w.number)).collect();
    for e in &entries {
        let renumbered = e.added.is_some()
            || template_numbers
                .get(&e.node.pos)
                .is_some_and(|&old| old != e.number);
        if !renumbered {
            continue;
        }
        let at = e.node.name_at + e.node.name_len;
        let text = cstring(&wall_description(sample, e.number));
        match e.added.and_then(|i| out.added.get_mut(i)) {
            Some(a) => {
                a.replaces.insert(at, (e.node.desc_len, text));
            }
            None => {
                out.replaces.insert(at, (e.node.desc_len, text));
            }
        }
    }
    // počet stien v miestnosti
    if let Some(f) = floor {
        if !out.removed.is_empty() || !out.added.is_empty() {
            if let Some(at) = f.children_count_at {
                let count = f.children.len() - out.removed.len() + out.added.len();
                if let Ok(c) = u16::try_from(count) {
                    out.global.insert(at, c.to_le_bytes().to_vec());
                }
            }
        }
    }
    out
}

/// Nesúlad uložených stien so vzorcami CV (prázdne = v poriadku): neuzavretá reťaz alebo
/// hodnota spoja / zadnej plochy, ktorá by sa prepočítala inak.
pub fn check(template: &Node, data: &[u8]) -> Vec<String> {
    let mut id = 0;
    let p = plan(template, data, None, &mut id);
    let mut problems = p.notes;
    if !p.global.is_empty() {
        problems.push(tr!(
            "export_cvj.walls_formula_mismatch",
            count = p.global.len(),
            positions = format!("{:x?}", p.global.keys().collect::<Vec<_>>())
        ));
    }
    problems
}
