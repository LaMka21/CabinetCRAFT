//! PDF výstupy. Štítky dielov podľa návrhu štítkov CV dielne „80x80 V2“ (findings 14):
//! štítok 80 × 80 mm (rozmer sa dá nastaviť, polia sa úmerne roztiahnu), strana = štítok
//! pre tlačiareň štítkov a A4 s toľkými štítkami, koľko sa zmestí.
//!
//! Polia: zákazka, „Skr.c.:“ a skrinka, materiál, „n: meno dielu“ (n = poradie prerezania na
//! tabuli), hrany „písmeno: páska“, náčrt dielu (obrys, diery v mierke, kolíky v hranách,
//! krúžky s písmenom na olepených hranách), rozmer rezu L a W, symbol orientácie, `Up`/`Down`
//! (či sa diel pred CX100 otáča), program CX100 a jeho čiarový kód Code 39. Písmo nemá pevnú
//! veľkosť: v každom poli je najväčšie, ktoré sa zmestí (krok 0,75 pt, najmenej 3 pt).

#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::todo,
        clippy::unimplemented,
        clippy::unreachable
    )
)]

pub mod code39;
pub mod drawings;
pub mod fonts;
pub mod pdf;
mod ttf;

pub use fonts::{FontChoice, Fonts};

use cc_i18n::tr;
use cc_model::{Edge, Face, Operation, Part, Role};

use pdf::{Align, Canvas, Page};

/// Predvolený rozmer štítku (mm), ako návrh CV dielne.
pub const LABEL_W: f64 = 80.0;
pub const LABEL_H: f64 = 80.0;

/// Údaje nestingu pre štítok.
#[derive(Debug, Clone, PartialEq)]
pub struct NestInfo {
    /// Program tabule, napr. `0101N`.
    pub sheet: String,
    /// Plocha hore na nestingu (na nej je štítok).
    pub face_up: Face,
}

/// Jeden štítok.
#[derive(Debug, Clone, PartialEq)]
pub struct LabelInfo {
    pub project: String,
    pub part: Part,
    /// Číslo pred menom dielu: poradie prerezania na tabuli (nesting), inak poradie v skrinke.
    pub number: usize,
    /// Mená programov CX100 dielu (súbory `.xcs`).
    pub programs: Vec<String>,
    pub nesting: Option<NestInfo>,
}

/// Plocha, ktorú program obrába zhora (program `…_B.xcs` = B).
fn program_face(name: &str) -> Face {
    if name.to_ascii_uppercase().ends_with("_B.XCS") {
        Face::B
    } else {
        Face::F
    }
}

fn stem(name: &str) -> &str {
    name.rsplit_once('.').map_or(name, |(s, _)| s)
}

impl LabelInfo {
    /// Primárny program (na ploche F, inak B) a sekundárny (druhý), findings 14 §4.1.
    pub fn primary_secondary(&self) -> (Option<&str>, Option<&str>) {
        let f = self.programs.iter().find(|p| program_face(p) == Face::F);
        let b = self.programs.iter().find(|p| program_face(p) == Face::B);
        match (f, b) {
            (Some(f), b) => (Some(f.as_str()), b.map(String::as_str)),
            (None, Some(b)) => (Some(b.as_str()), None),
            (None, None) => (None, None),
        }
    }

    /// `Up`/`Down` pri nestingu: plocha, ktorú program obrába zhora, je hore už na
    /// nestingu (diel sa neotáča) alebo dole (otočiť).
    fn up_down_of(&self, program: Option<&str>) -> Option<&'static str> {
        let nest = self.nesting.as_ref()?;
        let face = program_face(program?);
        Some(if face == nest.face_up { "Up" } else { "Down" })
    }

    /// `Up`/`Down` primárneho programu.
    pub fn up_down(&self) -> Option<&'static str> {
        self.up_down_of(self.primary_secondary().0)
    }
}

/// Názov dielu podľa roly (štítok, výkres dielu) v zvolenom jazyku.
pub fn role_name(role: Role) -> String {
    match role {
        Role::SideLeft => tr!("pdf.role.side_left"),
        Role::SideRight => tr!("pdf.role.side_right"),
        Role::Bottom => tr!("pdf.role.bottom"),
        Role::Top => tr!("pdf.role.top"),
        Role::StretcherFront => tr!("pdf.role.stretcher_front"),
        Role::StretcherRear => tr!("pdf.role.stretcher_rear"),
        Role::Back => tr!("pdf.role.back"),
        Role::AdjustableShelf => tr!("pdf.role.adjustable_shelf"),
        Role::FixedShelf => tr!("pdf.role.fixed_shelf"),
        Role::Door => tr!("pdf.role.door"),
        Role::DrawerFront => tr!("pdf.role.drawer_front"),
        Role::FalseFront => tr!("pdf.role.false_front"),
        Role::DrawerBottom => tr!("pdf.role.drawer_bottom"),
        Role::DrawerBack => tr!("pdf.role.drawer_back"),
        Role::Plinth => tr!("pdf.role.plinth"),
        Role::FillerStrip => tr!("pdf.role.filler_strip"),
        Role::FillerFront => tr!("pdf.role.filler_front"),
        Role::FillerBottom => tr!("pdf.role.filler_bottom"),
        Role::FinishedBack => tr!("pdf.role.finished_back"),
        Role::LedRail => tr!("pdf.role.led_rail"),
        Role::CoverSide => tr!("pdf.role.cover_side"),
    }
}

pub(crate) fn mm(v: f64) -> String {
    let s = format!("{v:.1}");
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// Rozmer na štítku: celé číslo bez desatinných miest, inak dve (findings 14 §2).
pub fn label_dim(v: f64) -> String {
    let r = (v * 100.0).round() / 100.0;
    if (r - r.round()).abs() < 1e-9 {
        format!("{}", r.round() as i64)
    } else {
        format!("{r:.2}")
    }
}

/// Štítky po jednom na strane `size` (mm) pre tlačiareň štítkov.
pub fn labels_pdf(labels: &[LabelInfo], fonts: &Fonts, size: (f64, f64)) -> Vec<u8> {
    let (w, h) = size;
    let pages: Vec<Page> = labels
        .iter()
        .map(|l| {
            let mut c = Canvas::new(fonts);
            draw_label(&mut c, 0.0, 0.0, w, h, l);
            Page {
                width: w,
                height: h,
                canvas: c,
            }
        })
        .collect();
    pdf::document(&pages)
}

/// Štítky na A4 (koľko sa zmestí, okraj 5 mm) s tenkým rámikom na strihanie.
pub fn labels_a4_pdf(labels: &[LabelInfo], fonts: &Fonts, size: (f64, f64)) -> Vec<u8> {
    const A4: (f64, f64) = (210.0, 297.0);
    let (w, h) = size;
    let cols = (((A4.0 - 10.0) / w).floor() as usize).max(1);
    let rows = (((A4.1 - 10.0) / h).floor() as usize).max(1);
    let x0 = ((A4.0 - cols as f64 * w) / 2.0).max(0.0);
    let y0 = ((A4.1 - rows as f64 * h) / 2.0).max(0.0);
    let pages: Vec<Page> = labels
        .chunks(cols * rows)
        .map(|chunk| {
            let mut c = Canvas::new(fonts);
            for (i, l) in chunk.iter().enumerate() {
                let (col, row) = (i % cols, i / cols);
                let ox = x0 + col as f64 * w;
                let oy = y0 + (rows - 1 - row) as f64 * h;
                c.gray(0.75);
                c.line_width(0.1);
                c.rect(ox, oy, w, h, false);
                draw_label(&mut c, ox, oy, w, h, l);
            }
            Page {
                width: A4.0,
                height: A4.1,
                canvas: c,
            }
        })
        .collect();
    pdf::document(&pages)
}

// ----- návrh štítku (80 × 80 mm, x a y od ľavého horného rohu: x, y, šírka, výška) -----

/// Rozmer, pre ktorý sú polia návrhu.
const DESIGN: f64 = 80.0;
type Field = (f64, f64, f64, f64);
const JOB: Field = (1.18, 0.85, 56.13, 5.40);
const CAB_TEXT: Field = (59.85, 1.07, 9.81, 4.63);
const CAB: Field = (69.88, 0.96, 9.26, 8.27);
const MATERIAL: Field = (1.20, 7.18, 67.87, 6.71);
const NAME: Field = (1.14, 14.20, 76.05, 4.00);
const EDGES: Field = (1.09, 18.70, 77.15, 10.00);
const SKETCH: Field = (20.32, 30.22, 50.32, 25.80);
const WIDTH: Field = (72.86, 37.68, 6.07, 17.42);
const SYMBOL: Field = (1.09, 40.17, 10.06, 9.73);
const LENGTH: Field = (42.35, 58.39, 20.24, 5.03);
const UP_DOWN: Field = (2.73, 61.06, 9.15, 4.08);
const PROGRAM: Field = (12.43, 61.17, 22.83, 3.97);
const BARCODE: Field = (1.73, 65.80, 37.05, 7.28);
const UP_DOWN_2: Field = (40.99, 67.46, 12.13, 3.64);
const PROGRAM_2: Field = (53.78, 67.24, 24.70, 4.08);
const BARCODE_2: Field = (41.21, 72.20, 37.05, 6.18);

/// mm na bod
const PT: f64 = 25.4 / 72.0;
/// výška riadku a dĺžka dolného dotahu písma (násobok veľkosti)
const LINE: f64 = 1.117;
const DESCENT: f64 = 0.212;

/// Štítok s ľavým dolným rohom (ox, oy) a rozmerom w × h (mm).
struct Frame {
    ox: f64,
    oy: f64,
    h: f64,
    sx: f64,
    sy: f64,
}

impl Frame {
    /// Pole návrhu → obdĺžnik PDF (ľavý dolný roh, šírka, výška).
    fn rect(&self, f: Field) -> (f64, f64, f64, f64) {
        (
            self.ox + f.0 * self.sx,
            self.oy + self.h - (f.1 + f.3) * self.sy,
            f.2 * self.sx,
            f.3 * self.sy,
        )
    }
}

/// Najväčšia veľkosť písma (pt), pri ktorej sa text zmestí do w × h (mm).
fn fit_size(c: &Canvas, text: &str, bold: bool, w: f64, h: f64) -> f64 {
    let by_height = h / (LINE * PT);
    let w6 = c.width(text, 6.0, bold);
    let mut s = if w6 > 0.0 {
        by_height.min(6.0 * w / w6)
    } else {
        by_height
    };
    s = (s / 0.75).floor() * 0.75 + 0.75;
    while s > 3.0 && c.width(text, s, bold) > w {
        s -= 0.75;
    }
    s.clamp(3.0, by_height.max(3.0))
}

/// Text v poli (zvislo na stred).
fn field_text(c: &mut Canvas, r: (f64, f64, f64, f64), text: &str, bold: bool, align: Align) {
    if text.is_empty() {
        return;
    }
    let (x, y, w, h) = r;
    let s = fit_size(c, text, bold, w, h);
    let shown = c.fit(text, s, bold, w);
    let base = y + (h - LINE * s * PT) / 2.0 + DESCENT * s * PT;
    let tx = match align {
        Align::Left => x,
        Align::Center => x + w / 2.0,
        Align::Right => x + w,
    };
    c.text(tx, base, s, bold, align, &shown);
}

/// Viac riadkov rovnomerne po výške poľa (miesto aspoň na `min_lines`), spoločná veľkosť písma.
fn field_lines(
    c: &mut Canvas,
    r: (f64, f64, f64, f64),
    lines: &[String],
    bold: bool,
    min_lines: usize,
) {
    if lines.is_empty() {
        return;
    }
    let (x, y, w, h) = r;
    let lh = h / lines.len().max(min_lines) as f64;
    let s = lines
        .iter()
        .map(|l| fit_size(c, l, bold, w, lh))
        .fold(f64::INFINITY, f64::min);
    for (i, l) in lines.iter().enumerate() {
        let top = y + h - i as f64 * lh;
        let base = top - lh + (lh - LINE * s * PT) / 2.0 + DESCENT * s * PT;
        let shown = c.fit(l, s, bold, w);
        c.text(x, base, s, bold, Align::Left, &shown);
    }
}

/// Text otočený o 90° (číta sa zdola nahor), začína pri spodnej hrane poľa.
fn field_text_up(c: &mut Canvas, r: (f64, f64, f64, f64), text: &str, bold: bool) {
    let (x, y, w, h) = r;
    // otočené pole: dĺžka textu po výške, veľkosť písma po šírke
    let s = fit_size(c, text, bold, h, w);
    let len = c.width(text, s, bold);
    let base = x + w - (w - LINE * s * PT) / 2.0 - DESCENT * s * PT;
    c.text_up(base, y + len / 2.0, s, bold, text);
}

/// Písmená hrán: (hrana, písmeno, páska) pre olepené hrany; rovnaká páska = rovnaké písmeno.
pub fn edge_letters(p: &Part) -> Vec<(Edge, char, String)> {
    let mut bands: Vec<(String, char)> = Vec::new();
    let mut out = Vec::new();
    for e in Edge::ALL {
        let Some(band) = p.edges.get(e) else {
            continue;
        };
        let letter = match bands.iter().find(|(b, _)| b == band) {
            Some((_, l)) => *l,
            None => {
                let first = band
                    .chars()
                    .find(|c| c.is_ascii_alphabetic())
                    .map(|c| c.to_ascii_uppercase());
                let used = |c: char| bands.iter().any(|(_, l)| *l == c);
                let l = first
                    .filter(|c| !used(*c))
                    .or_else(|| ('A'..='Z').find(|c| !used(*c)))
                    .unwrap_or('?');
                bands.push((band.to_owned(), l));
                l
            }
        };
        out.push((e, letter, band.to_owned()));
    }
    out
}

fn display(id: &str) -> String {
    id.replace('_', " ")
}

/// Nakreslí štítok w × h s ľavým dolným rohom v (ox, oy).
pub fn draw_label(c: &mut Canvas, ox: f64, oy: f64, w: f64, h: f64, l: &LabelInfo) {
    let p = &l.part;
    let f = Frame {
        ox,
        oy,
        h,
        sx: w / DESIGN,
        sy: h / DESIGN,
    };
    c.gray(0.0);
    field_text(c, f.rect(JOB), &l.project, false, Align::Left);
    field_text(
        c,
        f.rect(CAB_TEXT),
        &tr!("pdf.labels.cabinet"),
        false,
        Align::Left,
    );
    field_text(c, f.rect(CAB), &p.cabinet, true, Align::Left);
    field_text(
        c,
        f.rect(MATERIAL),
        &display(&p.material),
        false,
        Align::Left,
    );
    field_text(
        c,
        f.rect(NAME),
        &format!("{}: {}", l.number, role_name(p.role)),
        true,
        Align::Left,
    );
    let letters = edge_letters(p);
    let mut legend: Vec<String> = Vec::new();
    for (_, letter, band) in &letters {
        let line = format!("{letter}: {}", display(band));
        if !legend.contains(&line) {
            legend.push(line);
        }
    }
    // pole hrán má v návrhu miesto na dva riadky
    field_lines(c, f.rect(EDGES), &legend, false, 2);

    let shown = l.nesting.as_ref().map_or(Face::F, |n| n.face_up);
    sketch(c, f.rect(SKETCH), p, shown, &letters);
    field_text_up(c, f.rect(WIDTH), &label_dim(p.width), true);
    field_text(c, f.rect(LENGTH), &label_dim(p.length), true, Align::Left);

    // programy CX100: primárny s Up/Down, symbolom a kódom, sekundárny iba ak existuje
    let (primary, secondary) = l.primary_secondary();
    if let Some(prog) = primary {
        let ud = l.up_down_of(Some(prog));
        if let Some(ud) = ud {
            field_text(c, f.rect(UP_DOWN), ud, false, Align::Left);
        }
        field_text(c, f.rect(PROGRAM), stem(prog), false, Align::Left);
        orientation(c, f.rect(SYMBOL), ud == Some("Down"));
        barcode(c, f.rect(BARCODE), stem(prog), f.sy);
    }
    if let Some(prog) = secondary {
        if let Some(ud) = l.up_down_of(Some(prog)) {
            field_text(c, f.rect(UP_DOWN_2), ud, false, Align::Left);
        }
        field_text(c, f.rect(PROGRAM_2), stem(prog), false, Align::Left);
        barcode(c, f.rect(BARCODE_2), stem(prog), f.sy);
    }
}

/// Symbol orientácie: krúžok so šípkou nahor; pri `Down` slučka na spodku šípky.
fn orientation(c: &mut Canvas, r: (f64, f64, f64, f64), turn: bool) {
    let (x, y, w, h) = r;
    let rad = w.min(h) / 2.0 * 0.92;
    let (cx, cy) = (x + w / 2.0, y + h / 2.0);
    c.gray(0.0);
    c.line_width(0.25);
    c.circle(cx, cy, rad, false);
    let top = cy + rad * 0.72;
    let bottom = cy - rad * 0.55;
    c.line(cx, bottom, cx, top - rad * 0.2);
    let head = rad * 0.28;
    c.polygon(
        &[
            (cx, top),
            (cx - head * 0.8, top - head * 1.2),
            (cx + head * 0.8, top - head * 1.2),
        ],
        true,
    );
    if turn {
        c.circle(cx, bottom - rad * 0.14, rad * 0.14, false);
    }
}

/// Code 39 zarovnaný vľavo v poli: úzky prvok 0,25 mm (užší, ak sa nezmestí), výška ≈ 4,8 mm
/// pri štítku 80 mm.
fn barcode(c: &mut Canvas, r: (f64, f64, f64, f64), text: &str, sy: f64) {
    let (x, y, w, h) = r;
    let Some((bars, total)) = code39::bars(text) else {
        field_text(
            c,
            r,
            &tr!("pdf.labels.barcode_unsupported"),
            false,
            Align::Left,
        );
        return;
    };
    let module = (w / total).min(0.25);
    let bh = (4.8 * sy).min(h);
    let by = y + h - bh;
    c.gray(0.0);
    for (bx, bw) in bars {
        c.rect(x + bx * module, by, bw * module, bh, true);
    }
}

/// Náčrt dielu v poli (findings 14 §3): dĺžka L vodorovne, predná hrana Y0 dole, pohľad na
/// plochu `shown` (pri B zrkadlovo). Mierka min(šírka / (L + r), výška / (W + r)), r = 50,8
/// pri olepenom diele (miesto na krúžky hrán).
pub(crate) fn sketch(
    c: &mut Canvas,
    r: (f64, f64, f64, f64),
    p: &Part,
    shown: Face,
    letters: &[(Edge, char, String)],
) {
    let (bx, by, bw, bh) = r;
    let (l, wd) = (p.length.max(1.0), p.width.max(1.0));
    let reserve = if letters.is_empty() { 0.0 } else { 50.8 };
    let s = (bw / (l + reserve)).min(bh / (wd + reserve));
    let (sw, sh) = (l * s, wd * s);
    let ox = bx + (bw - sw) / 2.0;
    let oy = by + (bh - sh) / 2.0;
    let mirror = shown == Face::B;
    let px = |x: f64| ox + if mirror { (l - x) * s } else { x * s };
    let py = |y: f64| oy + y * s;

    c.gray(0.0);
    c.line_width(0.15);
    c.rect(ox, oy, sw, sh, false);
    c.line_width(0.12);
    for op in &p.operations {
        match *op {
            Operation::Hole {
                face,
                x,
                y,
                diameter,
                depth,
                through,
                ..
            } => {
                let rad = (diameter / 2.0 * s).max(0.12);
                match face {
                    Face::F | Face::B => {
                        let (cx, cy) = (px(x), py(y));
                        c.save();
                        if face != shown && !through {
                            c.dash(Some((0.3, 0.3)));
                        }
                        c.circle(cx, cy, rad, false);
                        c.restore();
                        if through {
                            c.line(cx - rad, cy, cx + rad, cy);
                            c.line(cx, cy - rad, cx, cy + rad);
                        }
                    }
                    Face::X0 | Face::X1 | Face::Y0 | Face::Y1 => {
                        // značka „⊏“ z hrany dovnútra v hĺbke diery
                        let d = (depth * s).max(0.6);
                        let half = rad.max(0.3);
                        let (ex, ey) = match face {
                            Face::X0 => (px(0.0), py(y)),
                            Face::X1 => (px(l), py(y)),
                            Face::Y0 => (px(x), py(0.0)),
                            _ => (px(x), py(wd)),
                        };
                        // smer dovnútra dielu na náčrte
                        let inward: (f64, f64) = match face {
                            Face::X0 => (if mirror { -1.0 } else { 1.0 }, 0.0),
                            Face::X1 => (if mirror { 1.0 } else { -1.0 }, 0.0),
                            Face::Y0 => (0.0, 1.0),
                            _ => (0.0, -1.0),
                        };
                        let side = (inward.1.abs(), inward.0.abs());
                        let a = (ex + side.0 * half, ey + side.1 * half);
                        let b = (ex - side.0 * half, ey - side.1 * half);
                        let a2 = (a.0 + inward.0 * d, a.1 + inward.1 * d);
                        let b2 = (b.0 + inward.0 * d, b.1 + inward.1 * d);
                        c.line(a.0, a.1, a2.0, a2.1);
                        c.line(a2.0, a2.1, b2.0, b2.1);
                        c.line(b2.0, b2.1, b.0, b.1);
                    }
                }
            }
            Operation::Groove {
                face,
                start,
                end,
                width,
                ..
            } => {
                c.save();
                c.line_width((width * s).max(0.15));
                if face != shown {
                    c.dash(Some((0.6, 0.4)));
                }
                c.line(
                    px(start.x.clamp(0.0, l)),
                    py(start.y.clamp(0.0, wd)),
                    px(end.x.clamp(0.0, l)),
                    py(end.y.clamp(0.0, wd)),
                );
                c.restore();
            }
        }
    }
    // krúžky s písmenom na olepených hranách (stred na hrane)
    let rad = 1.4_f64.min(bh / 8.0);
    for (e, letter, _) in letters {
        let (cx, cy) = match e {
            Edge::X0 => (px(0.0), py(wd / 2.0)),
            Edge::X1 => (px(l), py(wd / 2.0)),
            Edge::Y0 => (px(l / 2.0), py(0.0)),
            Edge::Y1 => (px(l / 2.0), py(wd)),
        };
        c.gray(1.0);
        c.circle(cx, cy, rad, true);
        c.gray(0.0);
        c.line_width(0.15);
        c.circle(cx, cy, rad, false);
        let t = letter.to_string();
        let size = fit_size(c, &t, false, rad * 1.3, rad * 1.5);
        c.text(
            cx,
            cy - (LINE * size * PT) / 2.0 + DESCENT * size * PT,
            size,
            false,
            Align::Center,
            &t,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cc_model::{Edges, Grain, Transform, P2};
    use glam::DVec3;

    fn part() -> Part {
        Part {
            id: "SP1-BOTTOM".into(),
            cabinet: "SP1".into(),
            role: Role::Bottom,
            material: "DTDL_18_W960_biela".into(),
            length: 564.0,
            width: 510.0,
            thickness: 18.0,
            grain: Grain::None,
            edges: Edges {
                y0: Some("ABS_biela".into()),
                ..Edges::default()
            },
            transform: Transform::from_axes(DVec3::X, DVec3::Y, DVec3::ZERO),
            operations: vec![
                Operation::Hole {
                    face: Face::X0,
                    x: 0.0,
                    y: 32.0,
                    z: 9.0,
                    diameter: 8.0,
                    depth: 26.0,
                    through: false,
                    dowel: true,
                },
                Operation::Hole {
                    face: Face::B,
                    x: 50.0,
                    y: 50.0,
                    z: 0.0,
                    diameter: 10.0,
                    depth: 12.0,
                    through: false,
                    dowel: false,
                },
                Operation::Groove {
                    face: Face::F,
                    start: P2 { x: 0.0, y: 491.5 },
                    end: P2 { x: 564.0, y: 491.5 },
                    width: 3.5,
                    depth: 9.0,
                },
            ],
        }
    }

    fn info(nesting: Option<NestInfo>, programs: &[&str]) -> LabelInfo {
        LabelInfo {
            project: "Dielna – kuchyňa".into(),
            part: part(),
            number: 3,
            programs: programs.iter().map(|s| s.to_string()).collect(),
            nesting,
        }
    }

    #[test]
    fn up_down_rule() {
        let n = |f| {
            Some(NestInfo {
                sheet: "0101N".into(),
                face_up: f,
            })
        };
        assert_eq!(info(n(Face::F), &["SP1-BOTTOM.xcs"]).up_down(), Some("Up"));
        assert_eq!(
            info(n(Face::B), &["SP1-BOTTOM.xcs"]).up_down(),
            Some("Down")
        );
        assert_eq!(
            info(n(Face::B), &["SP1-BOTTOM_B.xcs"]).up_down(),
            Some("Up")
        );
        assert_eq!(info(n(Face::F), &[]).up_down(), None);
        assert_eq!(info(None, &["SP1-BOTTOM.xcs"]).up_down(), None);
    }

    #[test]
    fn pdf_has_one_page_per_label_and_a4_six_per_page_80() {
        let labels: Vec<LabelInfo> = (0..7).map(|_| info(None, &["SP1-BOTTOM.xcs"])).collect();
        let one = labels_pdf(&labels, &Fonts::new(FontChoice::Base14), (LABEL_W, LABEL_H));
        let s = String::from_utf8_lossy(&one);
        assert_eq!(s.matches("/Type /Page ").count(), 7);
        let a4 = labels_a4_pdf(&labels, &Fonts::new(FontChoice::Base14), (LABEL_W, LABEL_H));
        let s = String::from_utf8_lossy(&a4);
        assert_eq!(s.matches("/Type /Page ").count(), 2);
        assert!(s.contains("/MediaBox [0 0 595.276 841.89]"));
        // deterministické
        assert_eq!(
            labels_pdf(&labels, &Fonts::new(FontChoice::Base14), (LABEL_W, LABEL_H)),
            one
        );
    }

    /// Uloží ukážku na kontrolu: `CABINETCRAFT_LABEL_PDF=cesta.pdf cargo test -p cc-out-pdf`.
    #[test]
    fn optional_sample_dump() {
        let Ok(path) = std::env::var("CABINETCRAFT_LABEL_PDF") else {
            return;
        };
        let labels = vec![
            info(
                Some(NestInfo {
                    sheet: "0101N".into(),
                    face_up: Face::B,
                }),
                &["SP1-BOTTOM.xcs"],
            ),
            info(None, &[]),
        ];
        std::fs::write(
            path,
            labels_pdf(&labels, &Fonts::new(FontChoice::Base14), (LABEL_W, LABEL_H)),
        )
        .unwrap();
    }
}
