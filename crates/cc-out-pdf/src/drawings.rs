//! Výkresy do PDF: A4 na šírku, rám a popisové pole (firma, číslo a meno zákazky,
//! zákazník, výkres, kreslil, list).
//!
//! - výkres zákazky: 3D so stenami, pôdorys a pohľad spredu,
//! - výkres skrinky: 3D, spredu, zhora, zľava v spoločnej mierke s kótami,
//! - výkres dielu: náčrt plochy F s obrábaním a kótami, 3D náhľad, operácie.
//!
//! Obrazy pohľadov kreslí volajúci (crate `cc-scene` je v rovnakej vrstve, preto sem
//! nepatrí). Tento modul určí, kam a v akej mierke ich treba vykresliť
//! ([`plan_job`], [`plan_cabinet`]), a poskladá strany.

use cc_i18n::tr;
use cc_model::{CabinetType, Edge, Face, OpeningKind, Operation, Part};

use crate::pdf::{self, Align, Canvas, Page, PdfImage, RgbImage};
use crate::{mm, role_name};

pub const PAGE_W: f64 = 297.0;
pub const PAGE_H: f64 = 210.0;
/// Hustota obrazov pohľadov (bodov na mm, ≈ 150 dpi).
pub const PX_PER_MM: f64 = 6.0;

const FRAME: f64 = 10.0;
const TITLE_H: f64 = 24.0;
/// Miesto na popis nad pohľadom.
const CAPTION_H: f64 = 5.0;
/// Okraj pre kóty okolo pravouhlého pohľadu.
const DIM_MARGIN: f64 = 9.0;
/// Vyplnenie obrazu rendererom (`cc-scene`): pravouhlo 0,86, perspektíva 0,78.
const FIT_ORTHO: f64 = 0.86;
const BLUE: (f64, f64, f64) = (0.10, 0.30, 0.65);

/// Obdĺžnik v mm (ľavý dolný roh, šírka, výška).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Area {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Area {
    fn shrink(self, d: f64) -> Area {
        Area {
            x: self.x + d,
            y: self.y + d,
            w: (self.w - 2.0 * d).max(1.0),
            h: (self.h - 2.0 * d).max(1.0),
        }
    }

    /// Veľkosť obrazu v bodoch pre túto plochu.
    pub fn pixels(self) -> (usize, usize) {
        let px = |mm: f64| (mm * PX_PER_MM).round().clamp(16.0, 4000.0) as usize;
        (px(self.w), px(self.h))
    }
}

/// Plocha na kreslenie (nad popisovým poľom).
pub fn drawing_area() -> Area {
    let x = FRAME + 4.0;
    let y = FRAME + TITLE_H + 4.0;
    Area {
        x,
        y,
        w: PAGE_W - FRAME - 4.0 - x,
        h: PAGE_H - FRAME - 4.0 - y,
    }
}

/// Pohľad na výkrese.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ViewKind {
    Iso,
    Front,
    Top,
    Left,
}

impl ViewKind {
    /// Popis pohľadu v zvolenom jazyku.
    pub fn caption(self) -> String {
        match self {
            ViewKind::Iso => tr!("pdf.drawings.view.iso"),
            ViewKind::Front => tr!("pdf.drawings.view.front"),
            ViewKind::Top => tr!("pdf.drawings.view.top"),
            ViewKind::Left => tr!("pdf.drawings.view.left"),
        }
    }

    /// Kamera pre `cc-scene`: (otočenie, náklon v radiánoch, perspektíva).
    pub fn camera(self) -> (f64, f64, bool) {
        use std::f64::consts::FRAC_PI_2;
        match self {
            ViewKind::Iso => (32f64.to_radians(), 20f64.to_radians(), true),
            ViewKind::Front => (0.0, 0.0, false),
            ViewKind::Top => (0.0, FRAC_PI_2, false),
            ViewKind::Left => (FRAC_PI_2, 0.0, false),
        }
    }

    /// Šírka a výška obrysu skrinky `[w, h, d]` v pravouhlom pohľade.
    fn extents(self, [w, h, d]: [f64; 3]) -> Option<[f64; 2]> {
        let e = match self {
            ViewKind::Iso => return None,
            ViewKind::Front => [w, h],
            ViewKind::Top => [w, d],
            ViewKind::Left => [d, h],
        };
        (e[0] > 0.0 && e[1] > 0.0).then_some(e)
    }
}

/// Kam a ako vykresliť jeden pohľad.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewPlan {
    pub kind: ViewKind,
    /// Plocha obrazu (bez popisu).
    pub area: Area,
    /// Priblíženie kamery (spoločná mierka pravouhlých pohľadov).
    pub zoom: f64,
    /// Pravouhlý pohľad skrinky: rozmer obrysu (mm) a mierka (mm papiera na mm).
    pub dims: Option<([f64; 2], f64)>,
}

/// Bunka pohľadu → plocha obrazu pod popisom.
fn image_area(cell: Area) -> Area {
    Area {
        h: (cell.h - CAPTION_H).max(1.0),
        ..cell
    }
}

/// Zákazka: veľké 3D vľavo, vpravo pôdorys a pohľad spredu.
pub fn plan_job() -> Vec<ViewPlan> {
    let a = drawing_area();
    let gap = 5.0;
    let big = a.w * 0.6;
    let side = a.w - big - gap;
    let half = (a.h - gap) / 2.0;
    let cells = [
        (
            ViewKind::Iso,
            Area {
                x: a.x,
                y: a.y,
                w: big,
                h: a.h,
            },
        ),
        (
            ViewKind::Top,
            Area {
                x: a.x + big + gap,
                y: a.y + half + gap,
                w: side,
                h: half,
            },
        ),
        (
            ViewKind::Front,
            Area {
                x: a.x + big + gap,
                y: a.y,
                w: side,
                h: half,
            },
        ),
    ];
    cells
        .into_iter()
        .map(|(kind, cell)| ViewPlan {
            kind,
            area: image_area(cell),
            zoom: 1.0,
            dims: None,
        })
        .collect()
}

/// Skrinka `w × h × d`: 3D, spredu, zhora, zľava; pravouhlé v spoločnej mierke.
pub fn plan_cabinet(w: f64, h: f64, d: f64) -> Vec<ViewPlan> {
    let a = drawing_area();
    let gap = 5.0;
    let (cw, ch) = ((a.w - gap) / 2.0, (a.h - gap) / 2.0);
    let cell = |col: f64, row: f64| Area {
        x: a.x + col * (cw + gap),
        y: a.y + row * (ch + gap),
        w: cw,
        h: ch,
    };
    let cells = [
        (ViewKind::Iso, cell(0.0, 1.0)),
        (ViewKind::Front, cell(1.0, 1.0)),
        (ViewKind::Top, cell(0.0, 0.0)),
        (ViewKind::Left, cell(1.0, 0.0)),
    ];
    let size = [w, h, d];
    let common = cells
        .iter()
        .filter_map(|(k, c)| {
            let e = k.extents(size)?;
            let r = image_area(*c).shrink(DIM_MARGIN);
            Some((r.w / e[0]).min(r.h / e[1]) * FIT_ORTHO)
        })
        .fold(f64::INFINITY, f64::min);
    cells
        .into_iter()
        .map(|(kind, cell)| {
            let area = image_area(cell);
            match kind.extents(size).filter(|_| common.is_finite()) {
                Some(e) => {
                    let natural = (area.w / e[0]).min(area.h / e[1]) * FIT_ORTHO;
                    ViewPlan {
                        kind,
                        area,
                        zoom: common / natural,
                        dims: Some((e, common)),
                    }
                }
                None => ViewPlan {
                    kind,
                    area,
                    zoom: 1.0,
                    dims: None,
                },
            }
        })
        .collect()
}

/// Plocha 3D náhľadu na výkrese dielu.
pub fn part_thumb_area() -> Area {
    let a = drawing_area();
    let w = a.w * 0.34;
    Area {
        x: a.x + a.w - w,
        y: a.y + a.h - 62.0,
        w,
        h: 62.0,
    }
}

/// Údaje popisového poľa.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TitleInfo {
    pub company: String,
    pub project: String,
    pub number: String,
    pub customer: String,
    pub designer: String,
    /// Názov výkresu.
    pub title: String,
    /// Obsah výkresu (rozmery, počty).
    pub content: String,
    /// Logo firmy v políčku firmy (meno firmy je potom pod ním).
    pub logo: Option<PdfImage>,
}

/// Vykreslený pohľad: plán, obraz a popisy skriniek (poloha 0…1 v obraze, zhora).
#[derive(Debug, Clone)]
pub struct RenderedView {
    pub plan: ViewPlan,
    pub image: Option<RgbImage>,
    pub labels: Vec<(f64, f64, String)>,
}

#[derive(Debug, Clone)]
pub enum SheetBody {
    Views(Vec<RenderedView>),
    Part {
        part: Box<Part>,
        programs: Vec<String>,
        thumb: Option<RgbImage>,
    },
    Wall(Box<WallElevation>),
}

/// Pohľad na stenu z miestnosti (kreslí sa vektorovo). Súradnice v mm: `x` od ľavého
/// konca steny, `z` od podlahy.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WallElevation {
    pub wall: String,
    pub length: f64,
    pub height: f64,
    /// Výška podhľadu pod stropom (0 = bez podhľadu).
    pub soffit: f64,
    pub openings: Vec<WallOpening>,
    pub cabinets: Vec<WallCabinet>,
    /// Kótovacie body pozdĺž steny: spodný rad (pod podlahou) a horný (nad stenou).
    pub base_points: Vec<f64>,
    pub upper_points: Vec<f64>,
    /// Výškové body vpravo (podlaha, spodky a vrchy skriniek, strop).
    pub heights: Vec<f64>,
    /// Pôdorys na orientáciu (mm miestnosti, `y` hore).
    pub plan: PlanKey,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WallOpening {
    pub kind: OpeningKind,
    pub x: f64,
    pub z: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WallCabinet {
    pub id: String,
    pub kind: CabinetType,
    pub x: f64,
    pub z: f64,
    pub w: f64,
    pub h: f64,
    pub d: f64,
    /// Počet políc (kreslia sa čiarkovane, rovnomerne).
    pub shelves: usize,
}

/// Malý pôdorys: obrysy stien, skrinky a šípka pohľadu.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlanKey {
    pub walls: Vec<[[f64; 2]; 4]>,
    /// Index kreslenej steny vo `walls`.
    pub current: usize,
    pub cabinets: Vec<[[f64; 2]; 4]>,
    /// Šípka pohľadu (odkiaľ, kam).
    pub arrow: Option<[[f64; 2]; 2]>,
}

/// Mierky výkresu steny (1 : n), od najväčšej.
const WALL_SCALES: [f64; 7] = [10.0, 15.0, 20.0, 25.0, 30.0, 50.0, 100.0];

/// Najväčšia bežná mierka, v ktorej sa `length × height` zmestí do `w × h` (mm papiera).
pub fn wall_scale(length: f64, height: f64, w: f64, h: f64) -> f64 {
    if !(length > 0.0 && height > 0.0) {
        return 100.0;
    }
    WALL_SCALES
        .into_iter()
        .find(|n| length / n <= w && height / n <= h)
        .unwrap_or_else(|| (length / w).max(height / h).ceil().max(1.0))
}

#[derive(Debug, Clone)]
pub struct Sheet {
    pub title: TitleInfo,
    pub body: SheetBody,
}

/// Poskladá výkresy do jedného PDF (strana = výkres).
pub fn drawings_pdf(sheets: &[Sheet], fonts: &crate::Fonts) -> Vec<u8> {
    let total = sheets.len();
    let pages: Vec<Page> = sheets
        .iter()
        .enumerate()
        .map(|(i, sheet)| {
            let mut c = Canvas::new(fonts);
            frame(&mut c);
            match &sheet.body {
                SheetBody::Views(views) => {
                    for v in views {
                        view(&mut c, v);
                    }
                }
                SheetBody::Part {
                    part,
                    programs,
                    thumb,
                } => part_sheet(&mut c, part, programs, thumb.as_ref()),
                SheetBody::Wall(wall) => wall_sheet(&mut c, wall),
            }
            title_block(&mut c, &sheet.title, i + 1, total);
            Page {
                width: PAGE_W,
                height: PAGE_H,
                canvas: c,
            }
        })
        .collect();
    pdf::document(&pages)
}

fn frame(c: &mut Canvas) {
    c.gray(0.0);
    c.line_width(0.5);
    c.rect(
        FRAME,
        FRAME,
        PAGE_W - 2.0 * FRAME,
        PAGE_H - 2.0 * FRAME,
        false,
    );
}

fn view(c: &mut Canvas, v: &RenderedView) {
    let a = v.plan.area;
    c.gray(0.0);
    c.text(
        a.x,
        a.y + a.h + 1.4,
        8.0,
        true,
        Align::Left,
        &v.plan.kind.caption(),
    );
    c.gray(0.82);
    c.line_width(0.15);
    c.rect(a.x, a.y, a.w, a.h + CAPTION_H, false);
    if let Some(img) = &v.image {
        c.image(a.x, a.y, a.w, a.h, img);
    }
    match v.plan.dims {
        Some((ext, scale)) => {
            let (bw, bh) = (ext[0] * scale, ext[1] * scale);
            let x0 = a.x + (a.w - bw) / 2.0;
            let y0 = a.y + (a.h - bh) / 2.0;
            dim_h(c, x0, x0 + bw, y0, -5.0, &mm(ext[0]));
            dim_v(c, y0, y0 + bh, x0, -5.0, &mm(ext[1]));
        }
        None => {
            c.save();
            c.clip(a.x, a.y, a.w, a.h);
            c.gray(0.1);
            for (u, t, label) in &v.labels {
                let x = a.x + u * a.w;
                let y = a.y + a.h * (1.0 - t);
                c.text(x, y + 0.6, 7.0, true, Align::Center, label);
            }
            c.restore();
        }
    }
}

/// Veľkosť písma kót.
const DIM_TEXT: f64 = 7.0;

/// Vodorovná kóta úsečky (x0…x1, y) vo vzdialenosti `off` (záporná = dole, kladná =
/// hore). Text je nad kótovacou čiarou; keď sa medzi značky nezmestí, ide pod ňu.
fn dim_h(c: &mut Canvas, x0: f64, x1: f64, y: f64, off: f64, text: &str) {
    let yl = y + off;
    let s = if off < 0.0 { -1.0 } else { 1.0 };
    c.rgb(BLUE.0, BLUE.1, BLUE.2);
    c.line_width(0.15);
    c.line(x0, y + s * 0.5, x0, yl + s * 1.2);
    c.line(x1, y + s * 0.5, x1, yl + s * 1.2);
    c.line(x0, yl, x1, yl);
    for x in [x0, x1] {
        c.line(x - 0.9, yl - 0.9, x + 0.9, yl + 0.9);
    }
    let fits = c.width(text, DIM_TEXT, false) + 1.0 <= (x1 - x0).abs();
    let ty = if fits { yl + 0.8 } else { yl - 3.2 };
    c.text((x0 + x1) / 2.0, ty, DIM_TEXT, false, Align::Center, text);
    c.gray(0.0);
}

/// Zvislá kóta úsečky (y0…y1, x) vo vzdialenosti `off` (záporná = vľavo, kladná =
/// vpravo). Text je vľavo od kótovacej čiary; keď sa nezmestí, ide vpravo.
fn dim_v(c: &mut Canvas, y0: f64, y1: f64, x: f64, off: f64, text: &str) {
    let xl = x + off;
    let s = if off < 0.0 { -1.0 } else { 1.0 };
    c.rgb(BLUE.0, BLUE.1, BLUE.2);
    c.line_width(0.15);
    c.line(x + s * 0.5, y0, xl + s * 1.2, y0);
    c.line(x + s * 0.5, y1, xl + s * 1.2, y1);
    c.line(xl, y0, xl, y1);
    for y in [y0, y1] {
        c.line(xl - 0.9, y - 0.9, xl + 0.9, y + 0.9);
    }
    let fits = c.width(text, DIM_TEXT, false) + 1.0 <= (y1 - y0).abs();
    let tx = if fits { xl - 0.8 } else { xl + 3.2 };
    c.text_up(tx, (y0 + y1) / 2.0, DIM_TEXT, false, text);
    c.gray(0.0);
}

/// Kótovací reťazec: kóta medzi každou dvojicou susedných bodov (už v mm papiera).
fn chain_h(c: &mut Canvas, xs: &[f64], values: &[f64], y: f64, off: f64) {
    for (p, v) in xs.windows(2).zip(values.windows(2)) {
        if let ([a, b], [va, vb]) = (p, v) {
            if vb - va > 0.5 {
                dim_h(c, *a, *b, y, off, &mm(vb - va));
            }
        }
    }
}

fn chain_v(c: &mut Canvas, ys: &[f64], values: &[f64], x: f64, off: f64) {
    for (p, v) in ys.windows(2).zip(values.windows(2)) {
        if let ([a, b], [va, vb]) = (p, v) {
            if vb - va > 0.5 {
                dim_v(c, *a, *b, x, off, &mm(vb - va));
            }
        }
    }
}

/// Šírka pravého stĺpca výkresu steny (pôdorys a zoznam skriniek).
const WALL_SIDE_W: f64 = 66.0;

/// Typ skrinky v zvolenom jazyku (zoznam skriniek na výkrese, obsah výkresu skrinky).
pub fn cabinet_type_name(t: CabinetType) -> String {
    match t {
        CabinetType::Base => tr!("pdf.drawings.cabinet_type.base"),
        CabinetType::Wall => tr!("pdf.drawings.cabinet_type.wall"),
        CabinetType::Tall => tr!("pdf.drawings.cabinet_type.tall"),
    }
}

fn opening_name(k: OpeningKind) -> String {
    match k {
        OpeningKind::Window => tr!("pdf.drawings.opening.window"),
        OpeningKind::Door => tr!("pdf.drawings.opening.door"),
        OpeningKind::Opening => tr!("pdf.drawings.opening.opening"),
    }
}

/// Plocha pohľadu na stenu (bez pravého stĺpca), pôdorys a zoznam vpravo.
fn wall_areas() -> (Area, Area) {
    let a = drawing_area();
    let gap = 5.0;
    let side = Area {
        x: a.x + a.w - WALL_SIDE_W,
        y: a.y,
        w: WALL_SIDE_W,
        h: a.h,
    };
    let view = Area {
        w: a.w - WALL_SIDE_W - gap,
        ..a
    };
    (view, side)
}

/// Okraje okolo steny na kóty: vľavo, vpravo, dole, hore (mm papiera).
const WALL_MARGINS: [f64; 4] = [6.0, 22.0, 20.0, 16.0];

/// Mierka (1 : n) pohľadu na stenu `length × height` na výkrese.
pub fn wall_sheet_scale(length: f64, height: f64) -> f64 {
    let (v, _) = wall_areas();
    let [l, r, b, t] = WALL_MARGINS;
    wall_scale(length, height, v.w - l - r, v.h - b - t)
}

fn wall_sheet(c: &mut Canvas, w: &WallElevation) {
    let (v, side) = wall_areas();
    let n = wall_sheet_scale(w.length, w.height);
    let s = 1.0 / n;
    let [ml, mr, mb, mt] = WALL_MARGINS;
    let (aw, ah) = (v.w - ml - mr, v.h - mb - mt);
    let x0 = v.x + ml + (aw - w.length * s).max(0.0) / 2.0;
    let y0 = v.y + mb + (ah - w.height * s).max(0.0) / 2.0;
    let px = |x: f64| x0 + x * s;
    let pz = |z: f64| y0 + z * s;

    c.gray(0.0);
    c.text(
        v.x,
        v.y + v.h - 3.0,
        8.0,
        true,
        Align::Left,
        &tr!("pdf.drawings.wall.title", wall = &w.wall, scale = mm(n)),
    );

    // stena, podhľad, podlaha
    let (wl, wh) = (w.length * s, w.height * s);
    c.gray(0.965);
    c.rect(x0, y0, wl, wh, true);
    if w.soffit > 0.0 {
        c.gray(0.88);
        let zs = (w.height - w.soffit).max(0.0);
        c.rect(x0, pz(zs), wl, wh - zs * s, true);
    }
    c.gray(0.35);
    c.line_width(0.3);
    c.rect(x0, y0, wl, wh, false);
    c.gray(0.0);
    c.line_width(0.6);
    c.line(x0 - 4.0, y0, x0 + wl + 4.0, y0);

    // otvory
    for o in &w.openings {
        let (ox, oz, ow, oh) = (px(o.x), pz(o.z), o.w * s, o.h * s);
        match o.kind {
            OpeningKind::Window => c.rgb(0.84, 0.91, 0.97),
            OpeningKind::Door => c.rgb(0.91, 0.89, 0.85),
            OpeningKind::Opening => c.gray(0.99),
        }
        c.rect(ox, oz, ow, oh, true);
        c.rgb(0.35, 0.47, 0.63);
        c.line_width(0.25);
        c.rect(ox, oz, ow, oh, false);
        if o.kind == OpeningKind::Window {
            c.line(ox + ow / 2.0, oz, ox + ow / 2.0, oz + oh);
        }
        c.rgb(0.24, 0.35, 0.51);
        let label = format!("{} {} × {}", opening_name(o.kind), mm(o.w), mm(o.h));
        let label = c.fit(&label, 6.0, false, ow - 1.0);
        c.text(
            ox + ow / 2.0,
            oz + oh / 2.0 - 1.0,
            6.0,
            false,
            Align::Center,
            &label,
        );
    }

    // skrinky: korpus, sokel, police čiarkovane, označenie a rozmer
    for cab in &w.cabinets {
        let (cx, cz, cw, ch) = (px(cab.x), pz(cab.z), cab.w * s, cab.h * s);
        if cab.z > 1.0 && cab.kind != CabinetType::Wall {
            let inset = (50.0 * s).min(cw / 4.0);
            c.gray(0.78);
            c.rect(cx + inset, y0, cw - 2.0 * inset, cz - y0, true);
        }
        match cab.kind {
            CabinetType::Base => c.rgb(0.93, 0.89, 0.84),
            CabinetType::Tall => c.rgb(0.87, 0.83, 0.77),
            CabinetType::Wall => c.rgb(0.87, 0.90, 0.95),
        }
        c.rect(cx, cz, cw, ch, true);
        c.gray(0.55);
        c.line_width(0.15);
        c.save();
        c.dash(Some((1.2, 0.8)));
        let shelves = cab.shelves.min(50);
        for k in 1..=shelves {
            let z = cz + ch * k as f64 / (shelves + 1) as f64;
            c.line(cx + 0.8, z, cx + cw - 0.8, z);
        }
        c.restore();
        c.gray(0.0);
        c.line_width(0.35);
        c.rect(cx, cz, cw, ch, false);
        let id = c.fit(&cab.id, 8.0, true, cw - 1.0);
        c.text(cx + cw / 2.0, cz + ch / 2.0, 8.0, true, Align::Center, &id);
        c.gray(0.3);
        let size = c.fit(
            &format!("{} × {}", mm(cab.w), mm(cab.h)),
            6.0,
            false,
            cw - 1.0,
        );
        c.text(
            cx + cw / 2.0,
            cz + ch / 2.0 - 3.2,
            6.0,
            false,
            Align::Center,
            &size,
        );
    }

    // kóty: rad spodných pod podlahou, celá dĺžka pod ním, horný rad nad stenou,
    // výšky vpravo a celá výška za nimi
    let xs = |pts: &[f64]| pts.iter().map(|x| px(*x)).collect::<Vec<f64>>();
    let mut below = -7.0;
    if w.base_points.len() > 2 {
        chain_h(c, &xs(&w.base_points), &w.base_points, y0, below);
        below -= 7.0;
    }
    dim_h(c, x0, x0 + wl, y0, below, &mm(w.length));
    if w.upper_points.len() > 2 {
        chain_h(c, &xs(&w.upper_points), &w.upper_points, y0 + wh, 7.0);
    }
    let zs: Vec<f64> = w.heights.iter().map(|z| pz(*z)).collect();
    let mut right = 7.0;
    if w.heights.len() > 2 {
        chain_v(c, &zs, &w.heights, x0 + wl, right);
        right += 8.0;
    }
    dim_v(c, y0, y0 + wh, x0 + wl, right, &mm(w.height));

    wall_side(c, side, w);
}

/// Pravý stĺpec výkresu steny: pôdorys so šípkou pohľadu, skrinky a otvory.
fn wall_side(c: &mut Canvas, a: Area, w: &WallElevation) {
    let key_h = 62.0;
    let key = Area {
        x: a.x,
        y: a.y + a.h - key_h,
        w: a.w,
        h: key_h,
    };
    c.gray(0.0);
    c.text(
        key.x,
        key.y + key.h - 3.0,
        8.0,
        true,
        Align::Left,
        &tr!("pdf.drawings.wall.plan"),
    );
    c.gray(0.82);
    c.line_width(0.15);
    c.rect(key.x, key.y, key.w, key.h - CAPTION_H, false);
    plan_key(
        c,
        Area {
            h: key.h - CAPTION_H,
            ..key
        }
        .shrink(3.0),
        &w.plan,
    );

    // zoznam skriniek a otvorov
    let mut y = key.y - 7.0;
    let bottom = a.y + 2.0;
    c.gray(0.0);
    c.text(
        a.x,
        y,
        8.0,
        true,
        Align::Left,
        &tr!("pdf.drawings.wall.cabinets"),
    );
    y -= 4.5;
    let cols = [
        (tr!("pdf.drawings.wall.col_cabinet"), 0.0),
        (tr!("pdf.drawings.wall.col_type"), 15.0),
        (tr!("pdf.drawings.wall.col_size"), 30.0),
    ];
    for (t, dx) in &cols {
        c.text(a.x + dx, y, 6.5, true, Align::Left, t);
    }
    c.line_width(0.15);
    c.line(a.x, y - 1.2, a.x + a.w, y - 1.2);
    y -= 4.0;
    let mut cabinets: Vec<&WallCabinet> = w.cabinets.iter().collect();
    cabinets.sort_by(|p, q| p.x.total_cmp(&q.x).then(p.z.total_cmp(&q.z)));
    let rows_left = |y: f64| ((y - bottom) / 3.4).floor().max(0.0) as usize;
    let n_open = w.openings.len().min(6);
    let reserve = if n_open > 0 { n_open + 2 } else { 0 };
    let room = rows_left(y).saturating_sub(reserve);
    let shown = if cabinets.len() > room {
        room.saturating_sub(1)
    } else {
        cabinets.len()
    };
    for cab in cabinets.iter().take(shown) {
        c.text(
            a.x,
            y,
            6.5,
            true,
            Align::Left,
            &c.fit(&cab.id, 6.5, true, 14.0),
        );
        c.text(
            a.x + 15.0,
            y,
            6.5,
            false,
            Align::Left,
            &cabinet_type_name(cab.kind),
        );
        let size = format!("{} × {} × {}", mm(cab.w), mm(cab.h), mm(cab.d));
        c.text(a.x + 30.0, y, 6.5, false, Align::Left, &size);
        y -= 3.4;
    }
    if shown < cabinets.len() {
        c.gray(0.35);
        c.text(
            a.x,
            y,
            6.5,
            false,
            Align::Left,
            &tr!("pdf.drawings.wall.more", count = cabinets.len() - shown),
        );
        c.gray(0.0);
        y -= 3.4;
    }
    if cabinets.is_empty() {
        c.gray(0.35);
        c.text(
            a.x,
            y,
            6.5,
            false,
            Align::Left,
            &tr!("pdf.drawings.wall.none"),
        );
        c.gray(0.0);
        y -= 3.4;
    }
    if n_open > 0 && rows_left(y) >= 2 {
        y -= 2.0;
        c.text(
            a.x,
            y,
            8.0,
            true,
            Align::Left,
            &tr!("pdf.drawings.wall.openings"),
        );
        y -= 4.2;
        for o in w.openings.iter().take(n_open.min(rows_left(y))) {
            let mut text = tr!(
                "pdf.drawings.wall.opening_row",
                name = opening_name(o.kind),
                w = mm(o.w),
                h = mm(o.h),
                x = mm(o.x)
            );
            if o.z > 0.5 {
                text.push_str(&tr!("pdf.drawings.wall.sill", sill = mm(o.z)));
            }
            c.text(
                a.x,
                y,
                6.5,
                false,
                Align::Left,
                &c.fit(&text, 6.5, false, a.w),
            );
            y -= 3.4;
        }
    }
}

/// Pôdorys miestnosti do plochy `a`: steny (kreslená zvýraznená), skrinky, šípka.
fn plan_key(c: &mut Canvas, a: Area, p: &PlanKey) {
    let pts = p
        .walls
        .iter()
        .chain(&p.cabinets)
        .flat_map(|q| q.iter())
        .filter(|q| q[0].is_finite() && q[1].is_finite());
    let Some((lo, hi)) = pts.fold(None, |acc: Option<([f64; 2], [f64; 2])>, q| {
        Some(match acc {
            None => (*q, *q),
            Some((lo, hi)) => (
                [lo[0].min(q[0]), lo[1].min(q[1])],
                [hi[0].max(q[0]), hi[1].max(q[1])],
            ),
        })
    }) else {
        return;
    };
    let (bw, bh) = ((hi[0] - lo[0]).max(1.0), (hi[1] - lo[1]).max(1.0));
    let k = (a.w / bw).min(a.h / bh);
    let ox = a.x + (a.w - bw * k) / 2.0;
    let oy = a.y + (a.h - bh * k) / 2.0;
    let map = |q: [f64; 2]| (ox + (q[0] - lo[0]) * k, oy + (q[1] - lo[1]) * k);
    let poly = |q: &[[f64; 2]; 4]| q.map(map);

    for q in &p.cabinets {
        c.gray(0.92);
        c.polygon(&poly(q), true);
        c.gray(0.45);
        c.line_width(0.12);
        c.polygon(&poly(q), false);
    }
    for (i, q) in p.walls.iter().enumerate() {
        if i == p.current {
            c.rgb(BLUE.0, BLUE.1, BLUE.2);
        } else {
            c.gray(0.6);
        }
        c.polygon(&poly(q), true);
    }
    if let Some([from, to]) = p.arrow {
        let (fx, fy) = map(from);
        let (tx, ty) = map(to);
        let (dx, dy) = (tx - fx, ty - fy);
        let len = (dx * dx + dy * dy).sqrt();
        if len > 0.5 {
            let (ux, uy) = (dx / len, dy / len);
            c.rgb(0.85, 0.35, 0.05);
            c.line_width(0.4);
            c.line(fx, fy, tx, ty);
            let head = 2.2_f64.min(len / 2.0);
            c.polygon(
                &[
                    (tx, ty),
                    (
                        tx - ux * head - uy * head * 0.5,
                        ty - uy * head + ux * head * 0.5,
                    ),
                    (
                        tx - ux * head + uy * head * 0.5,
                        ty - uy * head - ux * head * 0.5,
                    ),
                ],
                true,
            );
        }
    }
    c.gray(0.0);
}

fn part_sheet(c: &mut Canvas, p: &Part, programs: &[String], thumb: Option<&RgbImage>) {
    let a = drawing_area();
    let left_w = a.w * 0.62;
    // náčrt plochy F
    c.gray(0.0);
    c.text(
        a.x,
        a.y + a.h - 3.0,
        8.0,
        true,
        Align::Left,
        &if p.role.is_front() {
            tr!("pdf.drawings.part.face_f_front")
        } else {
            tr!("pdf.drawings.part.face_f")
        },
    );
    sketch(c, a.x, a.y + 4.0, left_w, a.h - 12.0, p, Face::F);

    // 3D náhľad a údaje vpravo
    let t = part_thumb_area();
    c.gray(0.82);
    c.line_width(0.15);
    c.rect(t.x, t.y, t.w, t.h, false);
    if let Some(img) = thumb {
        c.image(t.x, t.y, t.w, t.h, img);
    }
    c.gray(0.0);
    c.text(
        t.x + 1.5,
        t.y + t.h - 4.0,
        7.0,
        true,
        Align::Left,
        &tr!("pdf.drawings.view.iso"),
    );

    let x = t.x;
    let mut y = t.y - 6.0;
    let mut row = |c: &mut Canvas, label: &str, value: &str| {
        c.gray(0.35);
        c.text(x, y, 7.0, false, Align::Left, label);
        c.gray(0.0);
        c.text(
            x + 22.0,
            y,
            7.5,
            true,
            Align::Left,
            &c.fit(value, 7.5, true, t.w - 22.0),
        );
        y -= 4.2;
    };
    row(c, &tr!("pdf.drawings.part.part"), &p.id);
    row(c, &tr!("pdf.drawings.part.description"), &role_name(p.role));
    row(
        c,
        &tr!("pdf.drawings.part.size"),
        &format!(
            "{} × {} × {} mm",
            mm(p.length),
            mm(p.width),
            mm(p.thickness)
        ),
    );
    row(c, &tr!("pdf.drawings.part.material"), &p.material);
    let edges: Vec<&str> = Edge::ALL
        .into_iter()
        .filter(|e| p.edges.get(*e).is_some())
        .map(|e| match e {
            Edge::X0 => "X0",
            Edge::X1 => "X1",
            Edge::Y0 => "Y0",
            Edge::Y1 => "Y1",
        })
        .collect();
    row(
        c,
        &tr!("pdf.drawings.part.edges"),
        &if edges.is_empty() {
            "–".to_owned()
        } else {
            edges.join(" ")
        },
    );
    row(
        c,
        &tr!("pdf.drawings.part.program"),
        &if programs.is_empty() {
            tr!("pdf.drawings.part.no_machining")
        } else {
            programs.join(", ")
        },
    );

    // operácie
    y -= 2.0;
    let cols = [
        ("#".to_owned(), 6.0),
        (tr!("pdf.drawings.part.col_operation"), 15.0),
        (tr!("pdf.drawings.part.col_face"), 9.0),
        ("X".to_owned(), 14.0),
        ("Y".to_owned(), 14.0),
        (tr!("pdf.drawings.part.col_diameter"), 11.0),
        (tr!("pdf.drawings.part.col_depth"), 12.0),
    ];
    let mut cx = x;
    c.gray(0.0);
    for (title, w) in &cols {
        c.text(cx, y, 6.5, true, Align::Left, title);
        cx += w;
    }
    c.line_width(0.15);
    c.line(x, y - 1.2, x + t.w, y - 1.2);
    y -= 4.0;
    let bottom = a.y + 2.0;
    let rows = ((y - bottom) / 3.4).floor().max(0.0) as usize;
    let shown = if p.operations.len() > rows {
        rows.saturating_sub(1)
    } else {
        rows
    };
    for (i, op) in p.operations.iter().enumerate().take(shown) {
        let cells: [String; 7] = match *op {
            Operation::Hole {
                face,
                x,
                y,
                diameter,
                depth,
                dowel,
                ..
            } => [
                (i + 1).to_string(),
                if dowel {
                    tr!("pdf.drawings.part.op_dowel")
                } else {
                    tr!("pdf.drawings.part.op_hole")
                },
                face_code(face).to_owned(),
                mm(x),
                mm(y),
                format!("Ø{}", mm(diameter)),
                mm(depth),
            ],
            Operation::Groove {
                face,
                start,
                width,
                depth,
                ..
            } => [
                (i + 1).to_string(),
                tr!("pdf.drawings.part.op_groove"),
                face_code(face).to_owned(),
                mm(start.x),
                mm(start.y),
                mm(width),
                mm(depth),
            ],
        };
        let mut cx = x;
        for ((_, w), text) in cols.iter().zip(&cells) {
            c.text(cx, y, 6.5, false, Align::Left, text);
            cx += w;
        }
        y -= 3.4;
    }
    if p.operations.len() > shown {
        c.text(
            x,
            y,
            6.5,
            false,
            Align::Left,
            &tr!(
                "pdf.drawings.part.more_operations",
                count = p.operations.len() - shown
            ),
        );
    }
}

fn face_code(f: Face) -> &'static str {
    match f {
        Face::F => "F",
        Face::B => "B",
        Face::X0 => "X0",
        Face::X1 => "X1",
        Face::Y0 => "Y0",
        Face::Y1 => "Y1",
    }
}

/// Popisové pole dole: firma | zákazka, zákazník | výkres, obsah | kreslil, list.
fn title_block(c: &mut Canvas, t: &TitleInfo, page: usize, total: usize) {
    let x0 = FRAME + 4.0;
    let y0 = FRAME + 2.0;
    let w = PAGE_W - 2.0 * FRAME - 8.0;
    let h = TITLE_H - 2.0;
    let widths = [62.0, 92.0, 80.0, w - 62.0 - 92.0 - 80.0];
    c.gray(0.0);
    c.line_width(0.35);
    c.rect(x0, y0, w, h, false);
    let mut xs = vec![x0];
    for cw in widths {
        let next = xs.last().copied().unwrap_or(x0) + cw;
        xs.push(next);
    }
    c.line_width(0.2);
    for x in xs.iter().skip(1).take(3) {
        c.line(*x, y0, *x, y0 + h);
    }
    for k in 1..4 {
        if let (Some(a), Some(b)) = (xs.get(k), xs.get(k + 1)) {
            c.line(*a, y0 + h / 2.0, *b, y0 + h / 2.0);
        }
    }
    // firma: logo (meno pod ním), alebo iba meno
    let company = t.company.trim();
    let cw = widths[0] - 6.0;
    let cx = x0 + widths[0] / 2.0;
    if let Some(logo) = &t.logo {
        let name_h = if company.is_empty() { 0.0 } else { 4.5 };
        let (bw, bh) = (cw, h - 4.0 - name_h);
        let aspect = logo.aspect().clamp(0.05, 50.0);
        let (lw, lh) = if bw / bh > aspect {
            (bh * aspect, bh)
        } else {
            (bw, bw / aspect)
        };
        let ly = y0 + 2.0 + name_h + (bh - lh) / 2.0;
        c.place_image(cx - lw / 2.0, ly, lw, lh, logo);
        if !company.is_empty() {
            c.gray(0.0);
            let name = c.fit(company, 7.5, true, cw);
            c.text(cx, y0 + 2.2, 7.5, true, Align::Center, &name);
        }
    } else {
        let company = if company.is_empty() {
            "CabinetCRAFT"
        } else {
            company
        };
        let size = if c.width(company, 16.0, true) <= cw {
            16.0
        } else {
            11.0
        };
        c.text(
            cx,
            y0 + h / 2.0 - 1.0,
            size,
            true,
            Align::Center,
            &c.fit(company, size, true, cw),
        );
    }
    let job = match (t.number.trim(), t.project.trim()) {
        ("", name) => name.to_owned(),
        (number, "") => number.to_owned(),
        (number, name) => format!("{number} – {name}"),
    };
    let page_text = format!("{page} / {total}");
    let job_l = tr!("pdf.drawings.title_block.job");
    let customer_l = tr!("pdf.drawings.title_block.customer");
    let drawing_l = tr!("pdf.drawings.title_block.drawing");
    let content_l = tr!("pdf.drawings.title_block.content");
    let designer_l = tr!("pdf.drawings.title_block.designer");
    let sheet_l = tr!("pdf.drawings.title_block.sheet");
    let cells = [
        (
            1,
            job_l.as_str(),
            job.as_str(),
            customer_l.as_str(),
            t.customer.as_str(),
        ),
        (
            2,
            drawing_l.as_str(),
            t.title.as_str(),
            content_l.as_str(),
            t.content.as_str(),
        ),
        (
            3,
            designer_l.as_str(),
            t.designer.as_str(),
            sheet_l.as_str(),
            page_text.as_str(),
        ),
    ];
    for (k, l1, v1, l2, v2) in cells {
        let (Some(x), Some(cw)) = (xs.get(k), widths.get(k)) else {
            continue;
        };
        for (row, label, value) in [(1.0, l1, v1), (0.0, l2, v2)] {
            let yy = y0 + row * h / 2.0 + 1.8;
            c.gray(0.35);
            c.text(x + 1.5, yy + 5.0, 5.5, false, Align::Left, label);
            c.gray(0.0);
            c.text(
                x + 1.5,
                yy,
                8.0,
                true,
                Align::Left,
                &c.fit(value, 8.0, true, cw - 3.0),
            );
        }
    }
}

/// Náčrt dielu v obdĺžniku (x, y, w, h): plocha `shown` pri pohľade zhora,
/// L vodorovne, predná hrana Y0 dole; pri ploche B zrkadlovo.
fn sketch(c: &mut Canvas, bx: f64, by: f64, bw: f64, bh: f64, p: &Part, shown: Face) {
    let (l, wd) = (p.length.max(1.0), p.width.max(1.0));
    let s = ((bw - 8.0) / l).min((bh - 6.0) / wd);
    let (sw, sh) = (l * s, wd * s);
    let ox = bx + (bw - sw) / 2.0;
    let oy = by + (bh - sh) / 2.0 + 1.5;
    let mirror = shown == Face::B;
    let px = |x: f64| ox + if mirror { (l - x) * s } else { x * s };
    let py = |y: f64| oy + y * s;

    // doska
    c.gray(0.0);
    c.line_width(0.25);
    c.rect(ox, oy, sw, sh, false);
    // olepené hrany
    c.line_width(1.0);
    for e in Edge::ALL {
        if p.edges.get(e).is_none() {
            continue;
        }
        let (a, b) = match e {
            Edge::X0 => ((0.0, 0.0), (0.0, wd)),
            Edge::X1 => ((l, 0.0), (l, wd)),
            Edge::Y0 => ((0.0, 0.0), (l, 0.0)),
            Edge::Y1 => ((0.0, wd), (l, wd)),
        };
        c.line(px(a.0), py(a.1), px(b.0), py(b.1));
    }
    c.line_width(0.2);
    let min_r = 0.35;
    for op in &p.operations {
        match *op {
            Operation::Hole {
                face,
                x,
                y,
                diameter,
                depth,
                ..
            } => {
                let r = (diameter / 2.0 * s).max(min_r);
                match face {
                    Face::F | Face::B => {
                        // diera na zobrazenej ploche plná, na opačnej prázdny krúžok
                        c.circle(px(x), py(y), r, face == shown);
                    }
                    Face::X0 | Face::X1 | Face::Y0 | Face::Y1 => {
                        let d = (depth * s).max(0.8);
                        let half = r;
                        let (cx, cy, ww, hh) = match face {
                            Face::X0 => (0.0, y, d, 2.0 * half),
                            Face::X1 => (l, y, d, 2.0 * half),
                            Face::Y0 => (x, 0.0, 2.0 * half, d),
                            _ => (x, wd, 2.0 * half, d),
                        };
                        // značka kolíka v hrane: obdĺžnik od hrany dovnútra
                        let sx = px(cx);
                        let sy = py(cy);
                        let (rx, ry) = match face {
                            Face::X0 => (if mirror { sx - ww } else { sx }, sy - hh / 2.0),
                            Face::X1 => (if mirror { sx } else { sx - ww }, sy - hh / 2.0),
                            Face::Y0 => (sx - ww / 2.0, sy),
                            _ => (sx - ww / 2.0, sy - hh),
                        };
                        c.rect(rx, ry, ww, hh, true);
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
                c.line_width((width * s).max(0.3));
                if face != shown {
                    c.dash(Some((1.0, 0.8)));
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
    // kóty a orientácia
    c.gray(0.0);
    c.text(
        ox + sw / 2.0,
        oy - 3.3,
        7.0,
        true,
        Align::Center,
        &mm(p.length),
    );
    c.text_up(ox + sw + 3.3, oy + sh / 2.0, 7.0, true, &mm(p.width));
    // predná hrana: značka vľavo pri hrane Y0 (neprekrýva kótu dĺžky)
    c.text(ox - 1.2, oy - 0.7, 5.5, false, Align::Right, "Y0");
    if mirror {
        c.text(
            ox + sw,
            oy + sh + 1.0,
            5.5,
            false,
            Align::Right,
            &tr!("pdf.drawings.part.view_face_b"),
        );
    }
}

#[cfg(test)]
mod i18n_keys {
    /// Kľúče z volaní makra prekladu v zdroji (aj keď rustfmt dá kľúč na ďalší riadok,
    /// ktorý `cargo xtask i18n` nevidí).
    fn keys(src: &str) -> Vec<&str> {
        src.match_indices(concat!("tr", "!("))
            // nie `include_str!(`
            .filter(|(i, _)| {
                !src[..*i]
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_alphanumeric() || c == '_')
            })
            .filter_map(|(i, m)| {
                src[i + m.len()..]
                    .trim_start()
                    .strip_prefix('"')?
                    .split('"')
                    .next()
            })
            .filter(|k| {
                k.contains('.')
                    && k.chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
            })
            .collect()
    }

    #[test]
    fn every_key_is_in_builtin_languages() {
        let sources = [include_str!("lib.rs"), include_str!("drawings.rs")];
        let keys: Vec<&str> = sources.iter().flat_map(|s| keys(s)).collect();
        assert!(keys.len() >= 60, "{keys:?}");
        for (code, text) in cc_i18n::BUILTIN {
            let catalog = cc_i18n::Catalog::parse(code, text).unwrap();
            let missing: Vec<&&str> = keys.iter().filter(|k| !catalog.contains(k)).collect();
            assert!(missing.is_empty(), "{code}.toml: chýbajú {missing:?}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cabinet_views_share_one_scale() {
        let plans = plan_cabinet(900.0, 762.0, 510.0);
        assert_eq!(plans.len(), 4);
        let scales: Vec<f64> = plans.iter().filter_map(|p| p.dims.map(|d| d.1)).collect();
        assert_eq!(scales.len(), 3);
        assert!(scales.windows(2).all(|w| (w[0] - w[1]).abs() < 1e-9));
        for p in &plans {
            if let Some((ext, s)) = p.dims {
                // obrys sa zmestí do plochy aj s kótami
                assert!(ext[0] * s <= p.area.w - 2.0 * DIM_MARGIN + 1e-6);
                assert!(ext[1] * s <= p.area.h - 2.0 * DIM_MARGIN + 1e-6);
                assert!(p.zoom > 0.0 && p.zoom <= 1.0 + 1e-9);
            }
        }
    }

    #[test]
    fn sheets_make_one_page_each() {
        let title = TitleInfo {
            company: "CASTEL".into(),
            project: "Kuchyňa".into(),
            number: "REF2026".into(),
            ..TitleInfo::default()
        };
        let img = RgbImage {
            width: 4,
            height: 4,
            rgb: vec![200; 48],
        };
        let views: Vec<RenderedView> = plan_job()
            .into_iter()
            .map(|plan| RenderedView {
                plan,
                image: Some(img.clone()),
                labels: vec![(0.5, 0.5, "SP1".into())],
            })
            .collect();
        let sheets = vec![
            Sheet {
                title: title.clone(),
                body: SheetBody::Views(views),
            },
            Sheet {
                title,
                body: SheetBody::Views(Vec::new()),
            },
        ];
        let pdf = drawings_pdf(&sheets, &crate::Fonts::new(crate::FontChoice::Base14));
        let s = String::from_utf8_lossy(&pdf);
        assert_eq!(s.matches("/Type /Page ").count(), 2);
        assert_eq!(s.matches("/Subtype /Image").count(), 3);
        let needle: &[u8] = b"(REF2026 \x96 Kuchy";
        assert!(
            pdf.windows(needle.len()).any(|w| w == needle),
            "číslo a meno zákazky v popisovom poli"
        );
        assert!(s.contains("(CASTEL)"));
        assert!(s.contains("(1 / 2)"));
        assert_eq!(
            drawings_pdf(&sheets, &crate::Fonts::new(crate::FontChoice::Base14)),
            pdf,
            "deterministické"
        );
    }

    #[test]
    fn wall_scale_picks_largest_standard_scale_that_fits() {
        assert_eq!(wall_scale(3600.0, 2600.0, 160.0, 110.0), 25.0);
        assert_eq!(wall_scale(1000.0, 800.0, 160.0, 110.0), 10.0);
        // dlhá stena mimo zoznamu mierok
        assert_eq!(wall_scale(20_000.0, 2600.0, 160.0, 110.0), 125.0);
        // nezmyselný vstup nespadne
        assert_eq!(wall_scale(f64::NAN, 2600.0, 160.0, 110.0), 100.0);
        assert_eq!(wall_scale(0.0, 0.0, 160.0, 110.0), 100.0);
    }

    #[test]
    fn wall_sheet_has_chains_and_plan_key() {
        let cab = |id: &str, kind, x, z, w, h| WallCabinet {
            id: id.into(),
            kind,
            x,
            z,
            w,
            h,
            d: 510.0,
            shelves: 1,
        };
        let wall = WallElevation {
            wall: "W1".into(),
            length: 3600.0,
            height: 2600.0,
            soffit: 300.0,
            openings: vec![WallOpening {
                kind: OpeningKind::Window,
                x: 1250.0,
                z: 1000.0,
                w: 900.0,
                h: 1200.0,
            }],
            cabinets: vec![
                cab("SP1", CabinetType::Base, 0.0, 100.0, 600.0, 762.0),
                cab("HO1", CabinetType::Wall, 0.0, 1537.0, 600.0, 663.0),
            ],
            base_points: vec![0.0, 600.0, 3600.0],
            upper_points: vec![0.0, 600.0, 1250.0, 2150.0, 3600.0],
            heights: vec![0.0, 100.0, 862.0, 1537.0, 2200.0, 2600.0],
            plan: PlanKey {
                walls: vec![[[0.0, 0.0], [3600.0, 0.0], [3600.0, 100.0], [0.0, 100.0]]],
                current: 0,
                cabinets: vec![[[0.0, -510.0], [600.0, -510.0], [600.0, 0.0], [0.0, 0.0]]],
                arrow: Some([[1800.0, -1000.0], [1800.0, -200.0]]),
            },
        };
        let sheets = vec![Sheet {
            title: TitleInfo::default(),
            body: SheetBody::Wall(Box::new(wall)),
        }];
        let pdf = drawings_pdf(&sheets, &crate::Fonts::new(crate::FontChoice::Base14));
        let s = String::from_utf8_lossy(&pdf);
        assert_eq!(s.matches("/Type /Page ").count(), 1);
        for text in [
            "(SP1)", "(HO1)", "(762)", "(650)", "(2600)", "(3600)", "(M 1:25)",
        ] {
            assert!(
                s.contains(text) || s.contains(&text[1..text.len() - 1]),
                "{text}"
            );
        }
        // reťazec spodného radu: 600 a 3000
        assert!(s.contains("(3000)"));
        // pôdorys: mnohouholníky stien a skriniek (h f)
        assert!(s.matches(" h f").count() >= 3);
    }
}
