//! Porovnanie s referenčnou zákazkou CABINET VISION (W6, `docs/parita-cv.md`).
//!
//! `cabinetcraft parity úloha.cvj --dataset tests/fixtures/cv-ref` naimportuje `.cvj`
//! do dočasného projektu, vygeneruje diely a obrábanie a porovná ich s datasetom dielov
//! a operácií z CV (`parts.csv`, `ops.csv`, popis v `README.md` datasetu): rozmery dielu,
//! diery v plochách aj hranách. Výsledok je správa v Markdowne; nič sa nezapisuje do CV.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use cc_model::{Face, Model, Operation, Part, Role};

/// Tolerancia polohy diery a rozmeru dielu (mm).
const TOL: f64 = 0.1;
/// Najviac riadkov s rozdielmi dier na jeden diel v správe.
const MAX_DETAIL: usize = 8;

/// Diel z `parts.csv` (rozmery a plochy v našom ráme).
#[derive(Debug, Clone)]
pub struct CvPart {
    pub ncid: u32,
    pub cabno: u32,
    pub cabinet_name: String,
    pub code: String,
    pub name: String,
    pub l: f64,
    pub w: f64,
    pub t: f64,
}

/// Diera z `ops.csv` v našom ráme dielu.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hole {
    pub face: Face,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub diameter: f64,
    pub depth: f64,
}

#[derive(Debug, Default)]
pub struct Dataset {
    pub parts: Vec<CvPart>,
    pub holes: BTreeMap<u32, Vec<Hole>>,
    /// Riadky drážok pílou na diel.
    pub saw: BTreeMap<u32, usize>,
}

/// Načíta `parts.csv` a `ops.csv` z priečinka datasetu.
pub fn load_dataset(dir: &Path) -> Result<Dataset, String> {
    let read = |name: &str| {
        let p = dir.join(name);
        std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))
    };
    let mut ds = Dataset::default();
    for row in rows(&read("parts.csv")?)? {
        ds.parts.push(CvPart {
            ncid: num(&row, "ncid")? as u32,
            cabno: num(&row, "cabno").unwrap_or(0.0) as u32,
            cabinet_name: text(&row, "cabinet_name"),
            code: text(&row, "part_code"),
            name: text(&row, "part_name"),
            l: num(&row, "our_L").or_else(|_| num(&row, "L"))?,
            w: num(&row, "our_W").or_else(|_| num(&row, "W"))?,
            t: num(&row, "T")?,
        });
    }
    for row in rows(&read("ops.csv")?)? {
        let ncid = num(&row, "ncid")? as u32;
        match text(&row, "kind").as_str() {
            "P" | "L" | "E008" => {
                let Some(face) = face(&text(&row, "our_face")) else {
                    continue;
                };
                ds.holes.entry(ncid).or_default().push(Hole {
                    face,
                    x: num(&row, "our_x")?,
                    y: num(&row, "our_y")?,
                    z: num(&row, "our_z").unwrap_or(0.0),
                    diameter: num(&row, "diameter")?,
                    depth: num(&row, "depth").unwrap_or(0.0),
                });
            }
            "saw" => *ds.saw.entry(ncid).or_default() += 1,
            _ => {}
        }
    }
    Ok(ds)
}

type Row = BTreeMap<String, String>;

fn rows(text: &str) -> Result<Vec<Row>, String> {
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let header: Vec<String> = lines
        .next()
        .ok_or("prázdny súbor CSV")?
        .trim_start_matches('\u{feff}')
        .split(';')
        .map(str::to_owned)
        .collect();
    Ok(lines
        .map(|l| {
            header
                .iter()
                .cloned()
                .zip(l.split(';').map(|v| v.trim().to_owned()))
                .collect()
        })
        .collect())
}

fn text(row: &Row, key: &str) -> String {
    row.get(key).cloned().unwrap_or_default()
}

fn num(row: &Row, key: &str) -> Result<f64, String> {
    let v = row.get(key).map(String::as_str).unwrap_or("");
    match v.parse::<f64>() {
        Ok(n) if n.is_finite() => Ok(n),
        _ => Err(format!("stĺpec `{key}`: neplatné číslo `{v}`")),
    }
}

fn face(s: &str) -> Option<Face> {
    Some(match s {
        "F" => Face::F,
        "B" => Face::B,
        "X0" => Face::X0,
        "X1" => Face::X1,
        "Y0" => Face::Y0,
        "Y1" => Face::Y1,
        _ => return None,
    })
}

/// Skupina dielu: kód CV a naša rola, ktoré sa môžu zhodovať.
fn group_cv(code: &str, name: &str) -> &'static str {
    // diely blendy majú v CV kódy bokov a dna (RU/LF „LISTA BLENDY“, BO „BLENDA PRAVA“)
    if name.to_uppercase().contains("BLEND") {
        return "blenda";
    }
    // diely niky majú v CV predponu `NES_` (NES_Dno, NES_LED RAMPA)
    let name = name.trim();
    let bare = name.strip_prefix("NES_").unwrap_or(name);
    match code {
        // dno hornej skrinky má v CV kód BO
        "BO" if bare.eq_ignore_ascii_case("Dno") => "dno",
        "BO" if bare.to_uppercase().contains("LED RAMPA") => "LED rampa",
        "FB" => "chrbát",
        // bočný sokel „SOKEL BOCNY“ má v CV kód boku (RU), nie je to bok korpusu
        "LU" | "RU" if name.to_uppercase().contains("SOKEL") => "sokel / iné",
        "LU" | "LF" => "bok ľavý",
        "RU" | "RF" => "bok pravý",
        "DE" => "dno",
        "TO" => "vrch",
        "TS" => "vlys",
        "UB" => "chrbát",
        "AS" => "polica",
        "S_DSLAB" => "čelo",
        "BBT" => "dno zásuvky",
        "BBK" => "zadok zásuvky",
        "BO" => "sokel / iné",
        _ => "iné",
    }
}

fn group_ours(role: Role) -> &'static str {
    match role {
        Role::SideLeft => "bok ľavý",
        Role::SideRight => "bok pravý",
        Role::Bottom => "dno",
        Role::Top => "vrch",
        Role::StretcherFront | Role::StretcherRear => "vlys",
        Role::Back | Role::FinishedBack => "chrbát",
        Role::AdjustableShelf | Role::FixedShelf => "polica",
        Role::Door | Role::DrawerFront | Role::FalseFront => "čelo",
        Role::DrawerBottom => "dno zásuvky",
        Role::DrawerBack => "zadok zásuvky",
        Role::Plinth | Role::CoverSide => "sokel / iné",
        Role::FillerStrip | Role::FillerFront | Role::FillerBottom => "blenda",
        Role::LedRail => "LED rampa",
    }
}

/// Číslo skrinky CV z nášho ID (`SP4` → 4, `HO9` → 9; import ich tak pomenúva).
fn cabno_of(id: &str) -> Option<u32> {
    let digits: String = id.chars().skip_while(|c| !c.is_ascii_digit()).collect();
    digits.parse().ok()
}

fn dims_diff(cv: &CvPart, p: &Part) -> f64 {
    let mut a = [cv.l, cv.w];
    let mut b = [p.length, p.width];
    a.sort_by(f64::total_cmp);
    b.sort_by(f64::total_cmp);
    (a[0] - b[0]).abs() + (a[1] - b[1]).abs() + (cv.t - p.thickness).abs()
}

/// Naše diery dielu.
fn our_holes(p: &Part) -> Vec<Hole> {
    p.operations
        .iter()
        .filter_map(|op| match *op {
            Operation::Hole {
                face,
                x,
                y,
                z,
                diameter,
                depth,
                ..
            } => Some(Hole {
                face,
                x,
                y,
                z,
                diameter,
                depth,
            }),
            Operation::Groove { .. } => None,
        })
        .collect()
}

/// Jedna z 8 súmerností obdĺžnika dielu (výmena osí, zrkadlenie x, zrkadlenie y).
/// Rám dielu CV a náš sa pri niektorých dieloch (čelá) líšia; berie sa najlepšia zhoda.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Sym {
    swap: bool,
    mx: bool,
    my: bool,
}

impl Sym {
    fn all() -> impl Iterator<Item = Sym> {
        (0..8).map(|k| Sym {
            swap: k & 4 != 0,
            mx: k & 1 != 0,
            my: k & 2 != 0,
        })
    }

    fn is_identity(self) -> bool {
        !(self.swap || self.mx || self.my)
    }

    /// Naša diera v ráme dielu CV (rozmery nášho dielu `l × w`).
    fn apply(self, h: Hole, l: f64, w: f64) -> Hole {
        let (mut x, mut y, mut f, mut l2, mut w2) = (h.x, h.y, h.face, l, w);
        if self.swap {
            (x, y, l2, w2) = (y, x, w, l);
            f = match f {
                Face::X0 => Face::Y0,
                Face::X1 => Face::Y1,
                Face::Y0 => Face::X0,
                Face::Y1 => Face::X1,
                other => other,
            };
        }
        if self.mx {
            x = l2 - x;
            f = match f {
                Face::X0 => Face::X1,
                Face::X1 => Face::X0,
                other => other,
            };
        }
        if self.my {
            y = w2 - y;
            f = match f {
                Face::Y0 => Face::Y1,
                Face::Y1 => Face::Y0,
                other => other,
            };
        }
        Hole { face: f, x, y, ..h }
    }

    fn describe(self) -> String {
        let mut s = Vec::new();
        if self.swap {
            s.push("osi x↔y");
        }
        if self.mx {
            s.push("zrkadlo x");
        }
        if self.my {
            s.push("zrkadlo y");
        }
        s.join(", ")
    }
}

fn same_place(a: &Hole, b: &Hole) -> bool {
    let edge = !matches!(a.face, Face::F | Face::B);
    a.face == b.face
        && (a.x - b.x).abs() <= TOL
        && (a.y - b.y).abs() <= TOL
        && (!edge || (a.z - b.z).abs() <= 0.5)
        && (a.diameter - b.diameter).abs() <= 0.05
}

#[derive(Debug, Default)]
struct HoleMatch {
    matched: usize,
    depth_diff: Vec<(Hole, f64)>,
    cv_only: Vec<Hole>,
    ours_only: Vec<Hole>,
}

fn match_holes(cv: &[Hole], ours: &[Hole]) -> HoleMatch {
    let mut used = vec![false; ours.len()];
    let mut m = HoleMatch::default();
    for c in cv {
        let found = ours
            .iter()
            .enumerate()
            .find(|(i, o)| !used.get(*i).copied().unwrap_or(true) && same_place(c, o));
        match found {
            Some((i, o)) => {
                if let Some(u) = used.get_mut(i) {
                    *u = true;
                }
                m.matched += 1;
                if (c.depth - o.depth).abs() > TOL {
                    m.depth_diff.push((*c, o.depth));
                }
            }
            None => m.cv_only.push(*c),
        }
    }
    m.ours_only = ours
        .iter()
        .zip(&used)
        .filter(|(_, u)| !**u)
        .map(|(o, _)| *o)
        .collect();
    m
}

/// Súhrn porovnania (počty pre test a pre záhlavie správy).
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Totals {
    pub cv_parts: usize,
    pub matched_parts: usize,
    pub same_size: usize,
    pub cv_holes: usize,
    pub matched_holes: usize,
    pub our_extra_holes: usize,
}

/// Porovná model s datasetom CV; vráti súhrn a správu v Markdowne.
pub fn compare(model: &Model, ds: &Dataset) -> (Totals, String) {
    let mut totals = Totals::default();
    let mut body = String::new();
    let mut table = String::from(
        "| Skrinka CV | Naša | Diely CV / spárované / rozmer sedí | Diery CV / sedia / navyše u nás |\n|---|---|---|---|\n",
    );
    let mut by_cab: BTreeMap<u32, Vec<&CvPart>> = BTreeMap::new();
    for p in &ds.parts {
        by_cab.entry(p.cabno).or_default().push(p);
    }
    for (cabno, cv_parts) in &by_cab {
        let name = cv_parts
            .first()
            .map(|p| p.cabinet_name.clone())
            .unwrap_or_default();
        totals.cv_parts += cv_parts.len();
        let cv_hole_count: usize = cv_parts
            .iter()
            .map(|p| ds.holes.get(&p.ncid).map_or(0, Vec::len))
            .sum();
        totals.cv_holes += cv_hole_count;
        let Some(cab) = model
            .cabinets
            .iter()
            .find(|c| cabno_of(&c.id) == Some(*cabno))
        else {
            let _ = writeln!(
                table,
                "| {cabno} {name} | – | {} / 0 / 0 | {cv_hole_count} / 0 / 0 |",
                cv_parts.len()
            );
            continue;
        };
        let _ = writeln!(body, "\n## Skrinka {cabno} `{name}` = `{}`\n", cab.id);
        // páry v rámci skupiny podľa najmenšieho rozdielu rozmerov
        let mut free: Vec<&Part> = cab.parts.iter().collect();
        let (mut matched, mut same, mut c_holes, mut m_holes, mut extra) = (0, 0, 0, 0, 0);
        let mut ordered = cv_parts.clone();
        ordered.sort_by_key(|p| p.ncid);
        for cv in ordered {
            let group = group_cv(&cv.code, &cv.name);
            let best = free
                .iter()
                .enumerate()
                .filter(|(_, p)| group_ours(p.role) == group)
                .min_by(|a, b| dims_diff(cv, a.1).total_cmp(&dims_diff(cv, b.1)))
                .map(|(i, _)| i);
            // polica AS: dataset má F = horná plocha ako náš generátor (README datasetu)
            let cv_holes = ds.holes.get(&cv.ncid).cloned().unwrap_or_default();
            c_holes += cv_holes.len();
            let Some(i) = best else {
                let _ = writeln!(
                    body,
                    "- {:04} {} `{}` {} × {} × {}: **u nás chýba** (diery CV {})",
                    cv.ncid,
                    cv.code,
                    cv.name,
                    fmt(cv.l),
                    fmt(cv.w),
                    fmt(cv.t),
                    cv_holes.len()
                );
                continue;
            };
            let p = free.remove(i);
            matched += 1;
            let size_ok = dims_diff(cv, p) <= TOL;
            if size_ok {
                same += 1;
            }
            // najlepšia súmernosť rámca
            let ours = our_holes(p);
            let (sym, hm) = Sym::all()
                .map(|s| {
                    let mapped: Vec<Hole> = ours
                        .iter()
                        .map(|h| s.apply(*h, p.length, p.width))
                        .collect();
                    (s, match_holes(&cv_holes, &mapped))
                })
                .max_by(|a, b| {
                    (a.1.matched, a.0.is_identity()).cmp(&(b.1.matched, b.0.is_identity()))
                })
                .unwrap_or((
                    Sym {
                        swap: false,
                        mx: false,
                        my: false,
                    },
                    HoleMatch::default(),
                ));
            m_holes += hm.matched;
            extra += hm.ours_only.len();
            let ok = size_ok
                && hm.cv_only.is_empty()
                && hm.ours_only.is_empty()
                && hm.depth_diff.is_empty();
            let mark = if ok { "✅" } else { "❌" };
            let _ = write!(
                body,
                "- {mark} {:04} {} `{}` ↔ `{}`: CV {} × {} × {}, u nás {} × {} × {}; diery {}/{}",
                cv.ncid,
                cv.code,
                cv.name,
                p.id,
                fmt(cv.l),
                fmt(cv.w),
                fmt(cv.t),
                fmt(p.length),
                fmt(p.width),
                fmt(p.thickness),
                hm.matched,
                cv_holes.len()
            );
            if !hm.ours_only.is_empty() {
                let _ = write!(body, ", navyše u nás {}", hm.ours_only.len());
            }
            if !sym.is_identity() && hm.matched > 0 {
                let _ = write!(body, " (rám: {})", sym.describe());
            }
            if let Some(n) = ds.saw.get(&cv.ncid) {
                let ours_g = p
                    .operations
                    .iter()
                    .filter(|o| matches!(o, Operation::Groove { .. }))
                    .count();
                let _ = write!(body, "; drážky CV {n} rezov, u nás {ours_g}");
            }
            body.push('\n');
            for h in hm.cv_only.iter().take(MAX_DETAIL) {
                let _ = writeln!(body, "  - iba CV: {}", hole_text(h));
            }
            for h in hm.ours_only.iter().take(MAX_DETAIL) {
                let _ = writeln!(body, "  - iba u nás: {}", hole_text(h));
            }
            for (h, d) in hm.depth_diff.iter().take(MAX_DETAIL) {
                let _ = writeln!(body, "  - hĺbka: {} (u nás {})", hole_text(h), fmt(*d));
            }
            let more = hm.cv_only.len().saturating_sub(MAX_DETAIL)
                + hm.ours_only.len().saturating_sub(MAX_DETAIL);
            if more > 0 {
                let _ = writeln!(body, "  - … a ďalších {more}");
            }
        }
        for p in &free {
            let _ = writeln!(
                body,
                "- ➕ `{}` {} × {} × {}: u nás navyše (v CV nie je)",
                p.id,
                fmt(p.length),
                fmt(p.width),
                fmt(p.thickness)
            );
            extra += our_holes(p).len();
        }
        let _ = writeln!(
            table,
            "| {cabno} {name} | {} | {} / {matched} / {same} | {c_holes} / {m_holes} / {extra} |",
            cab.id,
            cv_parts.len()
        );
        totals.matched_parts += matched;
        totals.same_size += same;
        totals.matched_holes += m_holes;
        totals.our_extra_holes += extra;
    }
    let mut out = String::from("# Parita s CABINET VISION – diel po diele\n\n");
    let _ = writeln!(
        out,
        "Diely: CV {}, spárované {}, rozmer sedí {}. Diery: CV {}, sedia {}, navyše u nás {}.\n\
         Tolerancia {TOL} mm; rám dielu podľa README datasetu.\n",
        totals.cv_parts,
        totals.matched_parts,
        totals.same_size,
        totals.cv_holes,
        totals.matched_holes,
        totals.our_extra_holes
    );
    out.push_str(&table);
    out.push_str(&body);
    (totals, out)
}

fn hole_text(h: &Hole) -> String {
    format!(
        "{:?} x {} y {} z {} Ø{} × {}",
        h.face,
        fmt(h.x),
        fmt(h.y),
        fmt(h.z),
        fmt(h.diameter),
        fmt(h.depth)
    )
}

fn fmt(v: f64) -> String {
    let r = (v * 10.0).round() / 10.0;
    if r.fract() == 0.0 {
        format!("{r:.0}")
    } else {
        format!("{r:.1}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symmetry_maps_edges() {
        let h = Hole {
            face: Face::X0,
            x: 0.0,
            y: 10.0,
            z: 9.0,
            diameter: 8.0,
            depth: 26.0,
        };
        let s = Sym {
            swap: true,
            mx: false,
            my: true,
        };
        let m = s.apply(h, 100.0, 50.0);
        assert_eq!(m.face, Face::Y1);
        assert_eq!((m.x, m.y), (10.0, 100.0));
    }

    #[test]
    fn filler_parts_group_together() {
        // diely blendy majú v CV kódy bokov a dna, párujú sa s našimi dielmi blendy
        for (code, name) in [
            ("RU", "LISTA BLENDY"),
            ("LF", "LISTA BLENDY"),
            ("BO", "BLENDA LED DNO"),
            ("BO", "BLENDA PRAVA"),
        ] {
            assert_eq!(group_cv(code, name), group_ours(Role::FillerStrip));
        }
        assert_eq!(group_cv("RU", "Pravy BOK A"), "bok pravý");
        assert_eq!(group_cv("BO", "Dno"), "dno");
    }

    #[test]
    fn cabno_from_id() {
        assert_eq!(cabno_of("SP4"), Some(4));
        assert_eq!(cabno_of("HO13"), Some(13));
        assert_eq!(cabno_of("W"), None);
    }

    #[test]
    fn csv_rows() {
        let r = rows("a;b\n1;x\n\n2;y\n").unwrap();
        assert_eq!(r.len(), 2);
        assert_eq!(num(&r[1], "a").unwrap(), 2.0);
        assert!(num(&r[0], "b").is_err());
    }
}
