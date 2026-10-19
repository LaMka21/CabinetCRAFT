//! Minimálny zapisovač PDF 1.4: čiary, obdĺžniky, kruhy, obrázky a text. Všetky rozmery
//! v mm, počiatok vľavo dole. Výstup je deterministický (bez dátumu a náhodných ID).
//!
//! Text ide vloženým písmom TrueType (podmnožina, `Identity-H`, `ToUnicode`), ak ho
//! [`Fonts`] má; inak písmom Helvetica s WinAnsi (znaky mimo WinAnsi bez diakritiky,
//! lebo štandardné písma ich nemusia mať).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::sync::Arc;

use crate::fonts::Fonts;
use crate::ttf::TtFont;

const PT_PER_MM: f64 = 72.0 / 25.4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

/// Obsah jednej strany.
#[derive(Debug, Clone, Default)]
pub struct Canvas {
    buf: Vec<u8>,
    /// Obrázky strany (`/Im0`, `/Im1`, …), už skomprimované.
    images: Vec<PdfImage>,
    fonts: Fonts,
    /// Použité glyfy vloženého písma a ich znaky (obyčajné, tučné).
    used: [BTreeMap<u16, char>; 2],
}

/// Obrázok RGB (8 bitov na kanál, riadky zhora nadol).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbImage {
    pub width: usize,
    pub height: usize,
    pub rgb: Vec<u8>,
}

/// Obrázok skomprimovaný pre PDF (`/FlateDecode`). Klony zdieľajú údaje a v dokumente
/// je obrázok iba raz (napr. logo na každej strane).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfImage(Arc<ImageData>);

#[derive(Debug, PartialEq, Eq)]
struct ImageData {
    width: usize,
    height: usize,
    data: Vec<u8>,
}

impl PdfImage {
    /// `None`, ak rozmer nesedí s údajmi.
    pub fn new(img: &RgbImage) -> Option<PdfImage> {
        if img.width == 0
            || img.height == 0
            || img.rgb.len() != img.width.saturating_mul(img.height).saturating_mul(3)
        {
            return None;
        }
        Some(PdfImage(Arc::new(ImageData {
            width: img.width,
            height: img.height,
            data: miniz_oxide::deflate::compress_to_vec_zlib(&img.rgb, 6),
        })))
    }

    /// Pomer šírky k výške.
    pub fn aspect(&self) -> f64 {
        self.0.width as f64 / self.0.height.max(1) as f64
    }
}

fn n(v: f64) -> String {
    let v = if v.is_finite() { v } else { 0.0 };
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" || s.is_empty() {
        "0".to_owned()
    } else {
        s.to_owned()
    }
}

fn pt(mm: f64) -> String {
    n(mm * PT_PER_MM)
}

impl Canvas {
    /// Prázdna strana s písmami `fonts`. Všetky strany jedného PDF majú mať rovnaké písma.
    pub fn new(fonts: &Fonts) -> Canvas {
        Canvas {
            fonts: fonts.clone(),
            ..Canvas::default()
        }
    }

    fn op(&mut self, s: &str) {
        self.buf.extend_from_slice(s.as_bytes());
        self.buf.push(b'\n');
    }

    /// Odtieň sivej pre čiary aj výplne (0 = čierna, 1 = biela).
    pub fn gray(&mut self, level: f64) {
        let l = n(level.clamp(0.0, 1.0));
        self.op(&format!("{l} G {l} g"));
    }

    pub fn line_width(&mut self, mm: f64) {
        self.op(&format!("{} w", pt(mm.max(0.01))));
    }

    /// Prerušovaná čiara (`None` = plná).
    pub fn dash(&mut self, pattern: Option<(f64, f64)>) {
        match pattern {
            Some((on, off)) => self.op(&format!("[{} {}] 0 d", pt(on), pt(off))),
            None => self.op("[] 0 d"),
        }
    }

    pub fn line(&mut self, x1: f64, y1: f64, x2: f64, y2: f64) {
        self.op(&format!(
            "{} {} m {} {} l S",
            pt(x1),
            pt(y1),
            pt(x2),
            pt(y2)
        ));
    }

    pub fn rect(&mut self, x: f64, y: f64, w: f64, h: f64, fill: bool) {
        let op = if fill { "f" } else { "S" };
        self.op(&format!("{} {} {} {} re {op}", pt(x), pt(y), pt(w), pt(h)));
    }

    /// Uzavretý mnohouholník (menej ako 3 body sa vynechajú).
    pub fn polygon(&mut self, points: &[(f64, f64)], fill: bool) {
        let [first, rest @ ..] = points else {
            return;
        };
        if rest.len() < 2 {
            return;
        }
        let mut s = format!("{} {} m", pt(first.0), pt(first.1));
        for p in rest {
            let _ = write!(s, " {} {} l", pt(p.0), pt(p.1));
        }
        s.push_str(if fill { " h f" } else { " h S" });
        self.op(&s);
    }

    /// Kruh zo štyroch Bézierových oblúkov.
    pub fn circle(&mut self, cx: f64, cy: f64, r: f64, fill: bool) {
        const K: f64 = 0.552_284_75;
        let k = r * K;
        let mut s = String::new();
        let _ = write!(s, "{} {} m ", pt(cx + r), pt(cy));
        for (a, b, c) in [
            ((cx + r, cy + k), (cx + k, cy + r), (cx, cy + r)),
            ((cx - k, cy + r), (cx - r, cy + k), (cx - r, cy)),
            ((cx - r, cy - k), (cx - k, cy - r), (cx, cy - r)),
            ((cx + k, cy - r), (cx + r, cy - k), (cx + r, cy)),
        ] {
            let _ = write!(
                s,
                "{} {} {} {} {} {} c ",
                pt(a.0),
                pt(a.1),
                pt(b.0),
                pt(b.1),
                pt(c.0),
                pt(c.1)
            );
        }
        s.push_str(if fill { "f" } else { "S" });
        self.op(&s);
    }

    /// Šírka textu v mm.
    pub fn width(&self, text: &str, size: f64, bold: bool) -> f64 {
        match self.fonts.get(bold) {
            Some(font) => {
                let units: f64 = glyphs(font, text)
                    .map(|(g, _)| f64::from(font.advance(g)))
                    .sum();
                units / f64::from(font.units_per_em) * size / PT_PER_MM
            }
            None => base14_width(text, size, bold),
        }
    }

    /// Skráti text tak, aby sa zmestil do `max_mm` (pridá „..“).
    pub fn fit(&self, text: &str, size: f64, bold: bool, max_mm: f64) -> String {
        if self.width(text, size, bold) <= max_mm {
            return text.to_owned();
        }
        let mut chars: Vec<char> = text.chars().collect();
        while !chars.is_empty() {
            chars.pop();
            let s: String = chars.iter().collect::<String>() + "..";
            if self.width(&s, size, bold) <= max_mm {
                return s;
            }
        }
        String::new()
    }

    /// Reťazec pre operátor `Tj` (`(…)` alebo `<…>`); zapamätá použité glyfy.
    fn show(&mut self, text: &str, bold: bool) -> Vec<u8> {
        let bytes = encoded_text(&self.fonts, text, bold);
        if let Some(font) = self.fonts.get(bold) {
            let slot = &mut self.used[usize::from(bold && self.fonts.get(true).is_some())];
            for (g, c) in glyphs(font, text) {
                slot.entry(g).or_insert(c);
            }
        }
        bytes
    }

    /// Text na základnej čiare `y`; `size` v bodoch. Vráti šírku textu v mm.
    pub fn text(&mut self, x: f64, y: f64, size: f64, bold: bool, align: Align, text: &str) -> f64 {
        let w = self.width(text, size, bold);
        let x = match align {
            Align::Left => x,
            Align::Center => x - w / 2.0,
            Align::Right => x - w,
        };
        let font = if bold { "F2" } else { "F1" };
        let shown = self.show(text, bold);
        self.buf.extend_from_slice(
            format!("BT /{font} {} Tf {} {} Td ", n(size), pt(x), pt(y)).as_bytes(),
        );
        self.buf.extend(shown);
        self.buf.extend_from_slice(b" Tj ET\n");
        w
    }

    /// Text otočený o 90° proti smeru hodín (číta sa zdola nahor), vycentrovaný na `y`.
    pub fn text_up(&mut self, x: f64, y: f64, size: f64, bold: bool, text: &str) {
        let w = self.width(text, size, bold);
        let font = if bold { "F2" } else { "F1" };
        let shown = self.show(text, bold);
        self.buf.extend_from_slice(
            format!(
                "BT /{font} {} Tf 0 1 -1 0 {} {} Tm ",
                n(size),
                pt(x),
                pt(y - w / 2.0)
            )
            .as_bytes(),
        );
        self.buf.extend(shown);
        self.buf.extend_from_slice(b" Tj ET\n");
    }

    /// Farba RGB (0…1) pre čiary aj výplne.
    pub fn rgb(&mut self, r: f64, g: f64, b: f64) {
        let (r, g, b) = (
            n(r.clamp(0.0, 1.0)),
            n(g.clamp(0.0, 1.0)),
            n(b.clamp(0.0, 1.0)),
        );
        self.op(&format!("{r} {g} {b} RG {r} {g} {b} rg"));
    }

    /// Obrázok do obdĺžnika (x, y = ľavý dolný roh, mm). Nesprávny rozmer údajov
    /// sa vynechá.
    pub fn image(&mut self, x: f64, y: f64, w: f64, h: f64, img: &RgbImage) {
        if let Some(img) = PdfImage::new(img) {
            self.place_image(x, y, w, h, &img);
        }
    }

    /// Už skomprimovaný obrázok do obdĺžnika (rovnaký obrázok na viacerých stranách
    /// je v PDF raz).
    pub fn place_image(&mut self, x: f64, y: f64, w: f64, h: f64, img: &PdfImage) {
        let index = match self.images.iter().position(|i| Arc::ptr_eq(&i.0, &img.0)) {
            Some(i) => i,
            None => {
                self.images.push(img.clone());
                self.images.len() - 1
            }
        };
        self.op(&format!(
            "q {} 0 0 {} {} {} cm /Im{index} Do Q",
            pt(w),
            pt(h),
            pt(x),
            pt(y)
        ));
    }

    /// Orezanie ďalšieho kreslenia na obdĺžnik (platí do `restore`).
    pub fn clip(&mut self, x: f64, y: f64, w: f64, h: f64) {
        self.op(&format!("{} {} {} {} re W n", pt(x), pt(y), pt(w), pt(h)));
    }

    /// Uloží/obnoví stav grafiky (napr. okolo prerušovaných čiar).
    pub fn save(&mut self) {
        self.op("q");
    }

    pub fn restore(&mut self) {
        self.op("Q");
    }
}

/// Glyfy textu vo vloženom písme; chýbajúci znak skúsi bez diakritiky.
fn glyphs<'a>(font: &'a TtFont, text: &'a str) -> impl Iterator<Item = (u16, char)> + 'a {
    text.chars().flat_map(move |c| {
        let g = font.glyph(c);
        if g != 0 || c.is_ascii() {
            vec![(g, c)]
        } else {
            cc_out_util::to_ascii(&c.to_string())
                .into_iter()
                .map(|b| (font.glyph(char::from(b)), char::from(b)))
                .collect()
        }
    })
}

/// Bajty, ktoré [`Canvas::text`] zapíše pred `Tj` pre `text` (na kontrolu v testoch).
pub fn encoded_text(fonts: &Fonts, text: &str, bold: bool) -> Vec<u8> {
    match fonts.get(bold) {
        Some(font) => {
            let mut s = String::from("<");
            for (g, _) in glyphs(font, text) {
                let _ = write!(s, "{g:04X}");
            }
            s.push('>');
            s.into_bytes()
        }
        None => {
            let mut out = vec![b'('];
            out.extend(escape(&encode(text)));
            out.push(b')');
            out
        }
    }
}

/// Šírka textu v mm písmom Helvetica (tučné približne).
fn base14_width(text: &str, size: f64, bold: bool) -> f64 {
    let units: f64 = encode(text)
        .iter()
        .map(|&b| f64::from(glyph_width(b)))
        .sum();
    let k = if bold { 1.07 } else { 1.0 };
    units / 1000.0 * size * k / PT_PER_MM
}

/// Šírky znakov Helvetica (AFM) pre ASCII 32–126, ostatné 556.
fn glyph_width(b: u8) -> u16 {
    const W: [u16; 95] = [
        278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556,
        556, 556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556, 1015, 667, 667, 722,
        722, 667, 611, 778, 722, 278, 500, 667, 556, 833, 722, 778, 667, 778, 722, 667, 611, 722,
        667, 944, 667, 667, 611, 278, 278, 278, 469, 556, 333, 556, 556, 500, 556, 556, 278, 556,
        556, 222, 222, 500, 222, 833, 556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500,
        500, 334, 260, 334, 584,
    ];
    if (32..=126).contains(&b) {
        W.get(usize::from(b - 32)).copied().unwrap_or(556)
    } else {
        556
    }
}

/// Text → bajty WinAnsi (cp1252) pre Helveticu. Znaky mimo kódovej stránky sa prepíšu
/// bez diakritiky (štandardné písma nemusia mať ďalšie glyfy).
pub fn encode(text: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len());
    for c in text.chars() {
        let code = c as u32;
        let b = match c {
            '€' => Some(0x80),
            '‚' => Some(0x82),
            '„' => Some(0x84),
            '…' => Some(0x85),
            'Š' => Some(0x8A),
            '‹' => Some(0x8B),
            'Ž' => Some(0x8E),
            '‘' => Some(0x91),
            '’' => Some(0x92),
            '“' => Some(0x93),
            '”' => Some(0x94),
            '•' => Some(0x95),
            '–' => Some(0x96),
            '—' => Some(0x97),
            'š' => Some(0x9A),
            '›' => Some(0x9B),
            'ž' => Some(0x9E),
            _ if (0x20..0x7F).contains(&code) || (0xA0..=0xFF).contains(&code) => {
                u8::try_from(code).ok()
            }
            _ => None,
        };
        match b {
            Some(b) => out.push(b),
            None => out.extend(cc_out_util::to_ascii(&c.to_string())),
        }
    }
    out
}

fn escape(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    for &b in bytes {
        if matches!(b, b'(' | b')' | b'\\') {
            out.push(b'\\');
        }
        out.push(b);
    }
    out
}

/// Strana: rozmer v mm a obsah.
pub struct Page {
    pub width: f64,
    pub height: f64,
    pub canvas: Canvas,
}

/// Objekty jedného vloženého písma (okrem slovníka `Type0`).
const EMBEDDED_OBJECTS: usize = 4;

/// Zloží PDF zo strán.
pub fn document(pages: &[Page]) -> Vec<u8> {
    let mut out: Vec<u8> = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets: Vec<usize> = Vec::new();
    let obj = |out: &mut Vec<u8>, offsets: &mut Vec<usize>, body: &[u8]| {
        offsets.push(out.len());
        let id = offsets.len();
        out.extend_from_slice(format!("{id} 0 obj\n").as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    };

    // písma: všetky strany majú rovnaké (prvá strana rozhoduje)
    let fonts = pages
        .first()
        .map(|p| p.canvas.fonts.clone())
        .unwrap_or_default();
    let embedded: Vec<(&TtFont, BTreeMap<u16, char>)> = [false, true]
        .iter()
        .filter_map(|&bold| {
            let font = fonts.get(bold)?;
            let slot = usize::from(bold && fonts.get(true).is_some());
            let mut used = BTreeMap::new();
            for p in pages {
                if let Some(u) = p.canvas.used.get(slot) {
                    for (g, c) in u {
                        used.entry(*g).or_insert(*c);
                    }
                }
            }
            Some((font, used))
        })
        .collect();
    let embed = embedded.len() == 2;

    // 1 katalóg, 2 strany, 3–4 písma, (vložené písma), strany (strana, obsah),
    // nakoniec obrázky (každý raz, aj keď je na viacerých stranách)
    let first = 5 + if embed { 2 * EMBEDDED_OBJECTS } else { 0 };
    let first_ids: Vec<usize> = (0..pages.len()).map(|i| first + 2 * i).collect();
    let mut images: Vec<&PdfImage> = Vec::new();
    let image_ids: Vec<Vec<usize>> = pages
        .iter()
        .map(|p| {
            p.canvas
                .images
                .iter()
                .map(|img| {
                    let k = match images.iter().position(|i| Arc::ptr_eq(&i.0, &img.0)) {
                        Some(k) => k,
                        None => {
                            images.push(img);
                            images.len() - 1
                        }
                    };
                    first + 2 * pages.len() + k
                })
                .collect()
        })
        .collect();
    let kids: Vec<String> = first_ids.iter().map(|id| format!("{id} 0 R")).collect();
    obj(&mut out, &mut offsets, b"<< /Type /Catalog /Pages 2 0 R >>");
    obj(
        &mut out,
        &mut offsets,
        format!(
            "<< /Type /Pages /Kids [{}] /Count {} >>",
            kids.join(" "),
            pages.len()
        )
        .as_bytes(),
    );
    if embed {
        let mut subsets = Vec::new();
        for (k, (font, used)) in embedded.iter().enumerate() {
            let base = 5 + k * EMBEDDED_OBJECTS;
            let name = format!("{}+{}", subset_tag(font, used), font.name);
            obj(
                &mut out,
                &mut offsets,
                format!(
                    "<< /Type /Font /Subtype /Type0 /BaseFont /{name} /Encoding /Identity-H /DescendantFonts [{} 0 R] /ToUnicode {} 0 R >>",
                    base,
                    base + 3
                )
                .as_bytes(),
            );
            subsets.push((base, name, font, used));
        }
        for (base, name, font, used) in subsets {
            write_embedded(&mut out, &mut offsets, &obj, base, &name, font, used);
        }
    } else {
        for name in ["Helvetica", "Helvetica-Bold"] {
            obj(
                &mut out,
                &mut offsets,
                format!(
                    "<< /Type /Font /Subtype /Type1 /BaseFont /{name} /Encoding /WinAnsiEncoding >>"
                )
                .as_bytes(),
            );
        }
    }
    for ((page, first), ids) in pages.iter().zip(&first_ids).zip(&image_ids) {
        let content_id = first + 1;
        let xobjects: Vec<String> = ids
            .iter()
            .enumerate()
            .map(|(k, id)| format!("/Im{k} {id} 0 R"))
            .collect();
        let xobject = if xobjects.is_empty() {
            String::new()
        } else {
            format!(" /XObject << {} >>", xobjects.join(" "))
        };
        obj(
            &mut out,
            &mut offsets,
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Resources << /Font << /F1 3 0 R /F2 4 0 R >>{xobject} >> /Contents {content_id} 0 R >>",
                pt(page.width),
                pt(page.height)
            )
            .as_bytes(),
        );
        let mut body = format!("<< /Length {} >>\nstream\n", page.canvas.buf.len()).into_bytes();
        body.extend_from_slice(&page.canvas.buf);
        body.extend_from_slice(b"endstream");
        obj(&mut out, &mut offsets, &body);
    }
    for img in images {
        let img = &img.0;
        let mut body = format!(
                "<< /Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode /Length {} >>\nstream\n",
                img.width,
                img.height,
                img.data.len()
            )
            .into_bytes();
        body.extend_from_slice(&img.data);
        body.extend_from_slice(b"\nendstream");
        obj(&mut out, &mut offsets, &body);
    }
    let xref = out.len();
    out.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len() + 1).as_bytes(),
    );
    for o in &offsets {
        out.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            offsets.len() + 1
        )
        .as_bytes(),
    );
    out
}

/// CIDFont, popis písma, podmnožina a ToUnicode (objekty `base` … `base + 3`).
fn write_embedded(
    out: &mut Vec<u8>,
    offsets: &mut Vec<usize>,
    obj: &impl Fn(&mut Vec<u8>, &mut Vec<usize>, &[u8]),
    base: usize,
    name: &str,
    font: &TtFont,
    used: &BTreeMap<u16, char>,
) {
    let scale = |v: f64| (v * 1000.0 / f64::from(font.units_per_em)).round() as i64;
    let mut widths = String::new();
    for g in used.keys() {
        let _ = write!(widths, "{g} [{}] ", scale(f64::from(font.advance(*g))));
    }
    obj(
        out,
        offsets,
        format!(
            "<< /Type /Font /Subtype /CIDFontType2 /BaseFont /{name} /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> /FontDescriptor {} 0 R /CIDToGIDMap /Identity /DW 1000 /W [{}] >>",
            base + 1,
            widths.trim_end()
        )
        .as_bytes(),
    );
    let [x0, y0, x1, y1] = font.bbox.map(|v| scale(f64::from(v)));
    obj(
        out,
        offsets,
        format!(
            "<< /Type /FontDescriptor /FontName /{name} /Flags 32 /FontBBox [{x0} {y0} {x1} {y1}] /ItalicAngle 0 /Ascent {} /Descent {} /CapHeight {} /StemV 80 /FontFile2 {} 0 R >>",
            scale(f64::from(font.ascent)),
            scale(f64::from(font.descent)),
            scale(f64::from(font.cap_height)),
            base + 2
        )
        .as_bytes(),
    );
    let gids: BTreeSet<u16> = used.keys().copied().collect();
    let raw = font.subset(&gids).unwrap_or_default();
    let packed = miniz_oxide::deflate::compress_to_vec_zlib(&raw, 6);
    let mut body = format!(
        "<< /Length {} /Length1 {} /Filter /FlateDecode >>\nstream\n",
        packed.len(),
        raw.len()
    )
    .into_bytes();
    body.extend_from_slice(&packed);
    body.extend_from_slice(b"\nendstream");
    obj(out, offsets, &body);
    let cmap = to_unicode(used);
    let mut body = format!("<< /Length {} >>\nstream\n", cmap.len()).into_bytes();
    body.extend_from_slice(cmap.as_bytes());
    body.extend_from_slice(b"endstream");
    obj(out, offsets, &body);
}

/// ToUnicode CMap: glyf → znak (aby sa dal text z PDF kopírovať a hľadať).
fn to_unicode(used: &BTreeMap<u16, char>) -> String {
    let mut s = String::from(
        "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n/CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
    );
    let entries: Vec<(&u16, &char)> = used.iter().collect();
    for chunk in entries.chunks(100) {
        let _ = writeln!(s, "{} beginbfchar", chunk.len());
        for (g, c) in chunk {
            let mut units = [0u16; 2];
            let hex: String = c
                .encode_utf16(&mut units)
                .iter()
                .map(|u| format!("{u:04X}"))
                .collect();
            let _ = writeln!(s, "<{g:04X}> <{hex}>");
        }
        s.push_str("endbfchar\n");
    }
    s.push_str("endcmap\nCMapName currentdict /CIDResource defineresource pop\nend\nend\n");
    s
}

/// Šesť veľkých písmen podľa písma a použitých glyfov (rovnaký vstup = rovnaká značka).
fn subset_tag(font: &TtFont, used: &BTreeMap<u16, char>) -> String {
    // FNV-1a
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in font
        .name
        .bytes()
        .chain(used.keys().flat_map(|g| g.to_be_bytes()))
    {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    (0..6)
        .map(|i| char::from(b'A' + ((h >> (i * 8)) % 26) as u8))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fonts::FontChoice;

    #[test]
    fn base14_encoding_is_winansi_only() {
        assert_eq!(encode("šž×á"), vec![0x9A, 0x9E, 0xD7, 0xE1]);
        // č, ľ nie sú vo WinAnsi → bez diakritiky (štandardné písmo ich nemusí mať)
        assert_eq!(encode("čľ"), b"cl".to_vec());
        assert_eq!(escape(b"a(b)\\"), b"a\\(b\\)\\\\".to_vec());
    }

    fn base14() -> Canvas {
        Canvas::new(&Fonts::new(FontChoice::Base14))
    }

    #[test]
    fn document_has_valid_xref() {
        let mut c = base14();
        c.text(10.0, 10.0, 8.0, false, Align::Left, "Skúška (1)");
        c.circle(20.0, 20.0, 3.0, true);
        let pdf = document(&[Page {
            width: 100.0,
            height: 80.0,
            canvas: c,
        }]);
        let s = String::from_utf8_lossy(&pdf);
        assert!(s.starts_with("%PDF-1.4"));
        assert!(s.ends_with("%%EOF\n"));
        // posun objektu 1 v xref ukazuje na "1 0 obj" (v bajtoch, hlavička nie je UTF-8)
        let xref = pdf.windows(5).position(|w| w == b"xref\n").unwrap();
        let tail = String::from_utf8(pdf[xref..].to_vec()).unwrap();
        let off: usize = tail.lines().nth(3).unwrap()[..10].parse().unwrap();
        assert!(pdf[off..].starts_with(b"1 0 obj"));
        assert!(s.contains("/MediaBox [0 0 283.465 226.772]"));
    }

    fn xref_ok(pdf: &[u8]) {
        let xref = pdf.windows(5).position(|w| w == b"xref\n").unwrap();
        let tail = String::from_utf8(pdf[xref..].to_vec()).unwrap();
        let count: usize = tail.lines().nth(1).unwrap()[2..].parse().unwrap();
        for (i, line) in tail.lines().skip(3).take(count - 1).enumerate() {
            let off: usize = line[..10].parse().unwrap();
            assert!(pdf[off..].starts_with(format!("{} 0 obj", i + 1).as_bytes()));
        }
    }

    #[test]
    fn images_get_their_own_objects() {
        let img = RgbImage {
            width: 2,
            height: 1,
            rgb: vec![255, 0, 0, 0, 0, 255],
        };
        let mut a = base14();
        a.image(0.0, 0.0, 10.0, 5.0, &img);
        a.image(20.0, 0.0, 10.0, 5.0, &img);
        let mut b = base14();
        b.text(1.0, 1.0, 8.0, false, Align::Left, "bez obrázka");
        // zlý rozmer sa vynechá
        b.image(
            0.0,
            0.0,
            1.0,
            1.0,
            &RgbImage {
                width: 3,
                height: 3,
                rgb: vec![0; 5],
            },
        );
        let pdf = document(&[
            Page {
                width: 297.0,
                height: 210.0,
                canvas: a,
            },
            Page {
                width: 297.0,
                height: 210.0,
                canvas: b,
            },
        ]);
        let s = String::from_utf8_lossy(&pdf);
        assert_eq!(s.matches("/Subtype /Image").count(), 2);
        assert!(s.contains("/Kids [5 0 R 7 0 R]"), "{s}");
        assert!(s.contains("/XObject << /Im0 9 0 R /Im1 10 0 R >>"));
        assert!(s.contains("/Im1 Do"));
        xref_ok(&pdf);
    }

    #[test]
    fn shared_image_is_stored_once() {
        let logo = PdfImage::new(&RgbImage {
            width: 2,
            height: 1,
            rgb: vec![0, 0, 0, 255, 255, 255],
        })
        .unwrap();
        assert!((logo.aspect() - 2.0).abs() < 1e-9);
        let pages: Vec<Page> = (0..3)
            .map(|_| {
                let mut c = base14();
                c.place_image(0.0, 0.0, 20.0, 10.0, &logo);
                c.place_image(30.0, 0.0, 20.0, 10.0, &logo);
                Page {
                    width: 100.0,
                    height: 50.0,
                    canvas: c,
                }
            })
            .collect();
        let pdf = document(&pages);
        let s = String::from_utf8_lossy(&pdf);
        assert_eq!(s.matches("/Subtype /Image").count(), 1);
        assert_eq!(s.matches("/XObject << /Im0 11 0 R >>").count(), 3);
        assert_eq!(s.matches("/Im0 Do").count(), 6);
        xref_ok(&pdf);
    }

    /// So systémovým písmom: vložená podmnožina, ToUnicode so slovenskými znakmi,
    /// deterministický výstup.
    #[test]
    fn system_font_is_embedded_with_to_unicode() {
        let fonts = Fonts::new(FontChoice::System);
        if !fonts.embedded() {
            return;
        }
        let make = || {
            let mut c = Canvas::new(&fonts);
            c.text(10.0, 10.0, 9.0, false, Align::Left, "bok ľavý, tabuľa");
            c.text(10.0, 20.0, 9.0, true, Align::Left, "Hĺbka č ť ň");
            document(&[Page {
                width: 100.0,
                height: 80.0,
                canvas: c,
            }])
        };
        let pdf = make();
        let s = String::from_utf8_lossy(&pdf);
        assert_eq!(s.matches("/Subtype /Type0").count(), 2);
        assert_eq!(s.matches("/FontFile2").count(), 2);
        assert!(s.contains("/Identity-H"));
        assert!(s.contains("<013E>"), "ľ (U+013E) v ToUnicode");
        assert!(s.contains("<013A>"), "ĺ (U+013A) v ToUnicode");
        let shown = encoded_text(&fonts, "bok ľavý, tabuľa", false);
        assert!(pdf.windows(shown.len()).any(|w| w == shown.as_slice()));
        assert!(
            !shown.windows(4).any(|w| w == b"0000"),
            "žiadny chýbajúci glyf"
        );
        assert_eq!(make(), pdf, "deterministické");
        xref_ok(&pdf);
    }
}
