//! Čítanie písma TrueType a jeho podmnožina na vloženie do PDF.
//!
//! Podmnožina zachová čísla glyfov (CID = GID, `CIDToGIDMap /Identity`): nepoužité glyfy
//! sú prázdne, tabuľky `loca`, `glyf` sa poskladajú nanovo, ostatné potrebné tabuľky
//! (`head`, `hhea`, `maxp`, `hmtx`, `cvt `, `fpgm`, `prep`) sa skopírujú.
//! Súbor písma je vstup zvonka: všetky čítania sú kontrolované, chyba = `None`.

use std::collections::{BTreeMap, BTreeSet};

fn u16_at(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*d.get(o)?, *d.get(o + 1)?]))
}

fn i16_at(d: &[u8], o: usize) -> Option<i16> {
    u16_at(d, o).map(|v| v as i16)
}

fn u32_at(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_be_bytes([
        *d.get(o)?,
        *d.get(o + 1)?,
        *d.get(o + 2)?,
        *d.get(o + 3)?,
    ]))
}

/// Najväčší počet glyfov, ktorý ešte spracujeme.
const MAX_GLYPHS: u16 = u16::MAX - 1;

#[derive(Debug, Clone)]
pub struct TtFont {
    data: Vec<u8>,
    tables: BTreeMap<[u8; 4], (usize, usize)>,
    pub units_per_em: u16,
    num_glyphs: u16,
    long_loca: bool,
    num_hmetrics: u16,
    /// Podtabuľka cmap formátu 4 (posun v súbore).
    cmap4: usize,
    pub bbox: [i16; 4],
    pub ascent: i16,
    pub descent: i16,
    pub cap_height: i16,
    /// PostScript meno (len písmená a číslice).
    pub name: String,
}

impl TtFont {
    pub fn parse(data: Vec<u8>) -> Option<TtFont> {
        let version = u32_at(&data, 0)?;
        if version != 0x0001_0000 && version != u32::from_be_bytes(*b"true") {
            return None;
        }
        let count = usize::from(u16_at(&data, 4)?);
        let mut tables = BTreeMap::new();
        for i in 0..count {
            let r = 12 + 16 * i;
            let tag: [u8; 4] = data.get(r..r + 4)?.try_into().ok()?;
            let off = u32_at(&data, r + 8)? as usize;
            let len = u32_at(&data, r + 12)? as usize;
            if off.checked_add(len)? > data.len() {
                return None;
            }
            tables.insert(tag, (off, len));
        }
        let head = tables.get(b"head")?.0;
        let hhea = tables.get(b"hhea")?.0;
        let maxp = tables.get(b"maxp")?.0;
        tables.get(b"loca")?;
        tables.get(b"glyf")?;
        tables.get(b"hmtx")?;
        let units_per_em = u16_at(&data, head + 18)?.max(16);
        let bbox = [
            i16_at(&data, head + 36)?,
            i16_at(&data, head + 38)?,
            i16_at(&data, head + 40)?,
            i16_at(&data, head + 42)?,
        ];
        let long_loca = i16_at(&data, head + 50)? == 1;
        let ascent = i16_at(&data, hhea + 4)?;
        let descent = i16_at(&data, hhea + 6)?;
        let num_hmetrics = u16_at(&data, hhea + 34)?.max(1);
        let num_glyphs = u16_at(&data, maxp + 4)?.min(MAX_GLYPHS);
        let cap_height = tables
            .get(b"OS/2")
            .filter(|(_, len)| *len >= 90)
            .and_then(|(o, _)| i16_at(&data, o + 88))
            .filter(|v| *v > 0)
            .unwrap_or(ascent);
        let cmap4 = find_cmap4(&data, tables.get(b"cmap")?.0)?;
        let name = ps_name(&data, tables.get(b"name").copied()).unwrap_or_else(|| "Font".into());
        Some(TtFont {
            data,
            tables,
            units_per_em,
            num_glyphs,
            long_loca,
            num_hmetrics,
            cmap4,
            bbox,
            ascent,
            descent,
            cap_height,
            name,
        })
    }

    /// Glyf znaku (0 = chýba).
    pub fn glyph(&self, c: char) -> u16 {
        let code = c as u32;
        if code > 0xFFFF {
            return 0;
        }
        self.cmap4_lookup(code as u16).unwrap_or(0)
    }

    fn cmap4_lookup(&self, c: u16) -> Option<u16> {
        let d = &self.data;
        let t = self.cmap4;
        let seg2 = usize::from(u16_at(d, t + 6)?);
        let ends = t + 14;
        let starts = ends + seg2 + 2;
        let deltas = starts + seg2;
        let ranges = deltas + seg2;
        for i in (0..seg2).step_by(2) {
            let end = u16_at(d, ends + i)?;
            if c > end {
                continue;
            }
            let start = u16_at(d, starts + i)?;
            if c < start {
                return Some(0);
            }
            let delta = u16_at(d, deltas + i)?;
            let range = usize::from(u16_at(d, ranges + i)?);
            if range == 0 {
                return Some(c.wrapping_add(delta));
            }
            let at = ranges + i + range + 2 * usize::from(c - start);
            let g = u16_at(d, at)?;
            return Some(if g == 0 { 0 } else { g.wrapping_add(delta) });
        }
        Some(0)
    }

    /// Šírka glyfu v jednotkách písma.
    pub fn advance(&self, gid: u16) -> u16 {
        let Some(&(hmtx, _)) = self.tables.get(b"hmtx") else {
            return 0;
        };
        let i = usize::from(gid.min(self.num_hmetrics - 1));
        u16_at(&self.data, hmtx + 4 * i).unwrap_or(0)
    }

    /// Rozsah dát glyfu v tabuľke `glyf` (posun od začiatku súboru, dĺžka).
    fn glyph_range(&self, gid: u16) -> Option<(usize, usize)> {
        if gid >= self.num_glyphs {
            return None;
        }
        let (loca, _) = *self.tables.get(b"loca")?;
        let (glyf, glyf_len) = *self.tables.get(b"glyf")?;
        let i = usize::from(gid);
        let (a, b) = if self.long_loca {
            (
                u32_at(&self.data, loca + 4 * i)? as usize,
                u32_at(&self.data, loca + 4 * i + 4)? as usize,
            )
        } else {
            (
                2 * usize::from(u16_at(&self.data, loca + 2 * i)?),
                2 * usize::from(u16_at(&self.data, loca + 2 * i + 2)?),
            )
        };
        if b < a || b > glyf_len {
            return None;
        }
        Some((glyf + a, b - a))
    }

    /// Glyfy, z ktorých sa skladá zložený glyf.
    fn components(&self, gid: u16) -> Vec<u16> {
        let mut out = Vec::new();
        let Some((o, len)) = self.glyph_range(gid) else {
            return out;
        };
        let d = &self.data;
        if len < 10 || i16_at(d, o).unwrap_or(0) >= 0 {
            return out;
        }
        let end = o + len;
        let mut p = o + 10;
        // najviac 64 súčastí (ochrana pred zacyklením pri poškodenom súbore)
        for _ in 0..64 {
            let (Some(flags), Some(g)) = (u16_at(d, p), u16_at(d, p + 2)) else {
                break;
            };
            out.push(g);
            p += 4;
            p += if flags & 0x0001 != 0 { 4 } else { 2 };
            if flags & 0x0008 != 0 {
                p += 2;
            } else if flags & 0x0040 != 0 {
                p += 4;
            } else if flags & 0x0080 != 0 {
                p += 8;
            }
            if flags & 0x0020 == 0 || p >= end {
                break;
            }
        }
        out
    }

    /// Písmo iba s glyfmi `used` (a ich súčasťami a glyfom 0). Čísla glyfov ostanú.
    pub fn subset(&self, used: &BTreeSet<u16>) -> Option<Vec<u8>> {
        let mut keep: BTreeSet<u16> = BTreeSet::new();
        let mut stack: Vec<u16> = used.iter().copied().chain([0]).collect();
        while let Some(g) = stack.pop() {
            if g < self.num_glyphs && keep.insert(g) && keep.len() < 20_000 {
                stack.extend(self.components(g));
            }
        }
        let mut glyf = Vec::new();
        let mut loca = Vec::with_capacity(4 * (usize::from(self.num_glyphs) + 1));
        for g in 0..self.num_glyphs {
            loca.extend_from_slice(&(glyf.len() as u32).to_be_bytes());
            if keep.contains(&g) {
                if let Some((o, len)) = self.glyph_range(g) {
                    glyf.extend_from_slice(self.data.get(o..o + len)?);
                    while glyf.len() % 4 != 0 {
                        glyf.push(0);
                    }
                }
            }
        }
        loca.extend_from_slice(&(glyf.len() as u32).to_be_bytes());

        let copy = |tag: &[u8; 4]| -> Option<Vec<u8>> {
            let (o, len) = *self.tables.get(tag)?;
            self.data.get(o..o + len).map(<[u8]>::to_vec)
        };
        let mut head = copy(b"head")?;
        // checkSumAdjustment = 0, indexToLocFormat = 1 (dlhá loca)
        head.get_mut(8..12)?.copy_from_slice(&[0, 0, 0, 0]);
        head.get_mut(50..52)?.copy_from_slice(&1i16.to_be_bytes());
        let mut out_tables: Vec<([u8; 4], Vec<u8>)> = vec![
            (*b"head", head),
            (*b"hhea", copy(b"hhea")?),
            (*b"maxp", copy(b"maxp")?),
            (*b"hmtx", copy(b"hmtx")?),
            (*b"loca", loca),
            (*b"glyf", glyf),
        ];
        for tag in [b"cvt ", b"fpgm", b"prep", b"OS/2"] {
            if let Some(t) = copy(tag) {
                out_tables.push((*tag, t));
            }
        }
        // prázdna cmap (znaky mapuje PDF cez Identity-H) a post 3.0 bez mien glyfov:
        // niektoré tlačiarne a prehliadače ich v písme vyžadujú
        out_tables.push((*b"cmap", empty_cmap()));
        if let Some(mut post) = copy(b"post").and_then(|p| p.get(..32).map(<[u8]>::to_vec)) {
            post.get_mut(0..4)?
                .copy_from_slice(&0x0003_0000u32.to_be_bytes());
            out_tables.push((*b"post", post));
        }
        out_tables.sort_by_key(|t| t.0);
        Some(write_sfnt(&out_tables))
    }
}

/// cmap (3,1) formátu 4 iba s koncovým úsekom `0xFFFF`.
fn empty_cmap() -> Vec<u8> {
    let words: [u16; 16] = [
        0, 1, 3, 1, 0, 12, // hlavička, záznam (3,1) s posunom 12
        4, 24, 0, 2, 2, 0, 0, // formát 4, dĺžka, jazyk, segCountX2, search…
        0xFFFF, 0, 0xFFFF, // endCode, reservedPad, startCode
    ];
    let mut out: Vec<u8> = words.iter().flat_map(|w| w.to_be_bytes()).collect();
    // idDelta 1, idRangeOffset 0
    out.extend_from_slice(&[0, 1, 0, 0]);
    out
}

/// Posun podtabuľky cmap formátu 4 (Windows Unicode BMP, inak Unicode).
fn find_cmap4(d: &[u8], cmap: usize) -> Option<usize> {
    let n = usize::from(u16_at(d, cmap + 2)?);
    let mut best = None;
    for i in 0..n.min(64) {
        let r = cmap + 4 + 8 * i;
        let platform = u16_at(d, r)?;
        let encoding = u16_at(d, r + 2)?;
        let off = cmap + u32_at(d, r + 4)? as usize;
        if u16_at(d, off) != Some(4) {
            continue;
        }
        if platform == 3 && encoding == 1 {
            return Some(off);
        }
        if platform == 0 {
            best = Some(off);
        }
    }
    best
}

/// PostScript meno (name ID 6), iba písmená, číslice a pomlčka.
fn ps_name(d: &[u8], table: Option<(usize, usize)>) -> Option<String> {
    let (t, _) = table?;
    let count = usize::from(u16_at(d, t + 2)?);
    let strings = t + usize::from(u16_at(d, t + 4)?);
    for i in 0..count.min(512) {
        let r = t + 6 + 12 * i;
        let platform = u16_at(d, r)?;
        let id = u16_at(d, r + 6)?;
        if id != 6 {
            continue;
        }
        let len = usize::from(u16_at(d, r + 8)?);
        let off = strings + usize::from(u16_at(d, r + 10)?);
        let raw = d.get(off..off + len)?;
        let s: String = if platform == 3 || platform == 0 {
            raw.chunks(2)
                .filter_map(|c| c.get(1).copied())
                .map(char::from)
                .collect()
        } else {
            raw.iter().copied().map(char::from).collect()
        };
        let s: String = s
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
            .take(60)
            .collect();
        if !s.is_empty() {
            return Some(s);
        }
    }
    None
}

fn checksum(data: &[u8]) -> u32 {
    data.chunks(4).fold(0u32, |acc, c| {
        let mut w = [0u8; 4];
        for (dst, src) in w.iter_mut().zip(c) {
            *dst = *src;
        }
        acc.wrapping_add(u32::from_be_bytes(w))
    })
}

fn write_sfnt(tables: &[([u8; 4], Vec<u8>)]) -> Vec<u8> {
    let n = tables.len() as u16;
    let mut pow = 1u16;
    let mut log = 0u16;
    while pow * 2 <= n {
        pow *= 2;
        log += 1;
    }
    let mut out = Vec::new();
    out.extend_from_slice(&0x0001_0000u32.to_be_bytes());
    out.extend_from_slice(&n.to_be_bytes());
    out.extend_from_slice(&(pow * 16).to_be_bytes());
    out.extend_from_slice(&log.to_be_bytes());
    out.extend_from_slice(&(n * 16 - pow * 16).to_be_bytes());
    let mut offset = 12 + 16 * tables.len();
    for (tag, data) in tables {
        out.extend_from_slice(tag);
        out.extend_from_slice(&checksum(data).to_be_bytes());
        out.extend_from_slice(&(offset as u32).to_be_bytes());
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        offset += data.len().div_ceil(4) * 4;
    }
    for (_, data) in tables {
        out.extend_from_slice(data);
        while out.len() % 4 != 0 {
            out.push(0);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn garbage_is_rejected_without_panic() {
        assert!(TtFont::parse(Vec::new()).is_none());
        assert!(TtFont::parse(vec![0, 1, 0, 0, 0, 200, 0, 0]).is_none());
        let mut bogus = vec![0u8; 400];
        bogus[0..4].copy_from_slice(&0x0001_0000u32.to_be_bytes());
        bogus[4..6].copy_from_slice(&3u16.to_be_bytes());
        assert!(TtFont::parse(bogus).is_none());
    }

    /// Ak je v systéme písmo (Windows Arial), podmnožina sa znova načíta a má glyfy.
    #[test]
    fn system_font_subset_round_trip() {
        let Some(path) = crate::fonts::system_paths().into_iter().next() else {
            return;
        };
        let Ok(data) = std::fs::read(&path.0) else {
            return;
        };
        let Some(font) = TtFont::parse(data) else {
            panic!("systémové písmo {} sa nedá prečítať", path.0.display());
        };
        let used: BTreeSet<u16> = "Bok ľavý č ť ĺ Ž".chars().map(|c| font.glyph(c)).collect();
        assert!(!used.contains(&0), "všetky znaky majú glyf");
        let sub = font.subset(&used).unwrap();
        let again = TtFont::parse(sub).unwrap();
        for c in "ľčťĺ".chars() {
            let g = font.glyph(c);
            assert!(again.glyph_range(g).is_some_and(|(_, len)| len > 0));
            assert_eq!(again.advance(g), font.advance(g));
        }
    }
}
