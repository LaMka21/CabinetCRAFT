//! Základné prvky binárnej serializácie MFC (`CArchive`): čísla, reťazce, počty
//! a značky tried/objektov. Rozloženie polí jednotlivých tried je v `classes`.
//!
//! Všetky čítania kontrolujú hranice dát; nič tu nespadne ani pri poškodenom súbore.

use crate::CvjError;

/// Značka novej triedy (`FF FF`).
pub const NEW_CLASS_TAG: u16 = 0xFFFF;
/// Značka odkazu na triedu (horný bit).
pub const CLASS_TAG: u16 = 0x8000;
/// Značka veľkého indexu (nasleduje u32).
pub const BIG_OBJECT_TAG: u16 = 0x7FFF;

/// Najdlhšie meno triedy, ktoré pripustíme.
const MAX_CLASS_NAME: usize = 128;
/// Najdlhší reťazec (znaky), ktorý pripustíme.
const MAX_STRING_CHARS: usize = 4 * 1024 * 1024;

/// Čo nasleduje v prúde na mieste objektu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectTag {
    /// `null`
    Null,
    /// Nový objekt triedy s daným menom (trieda je nová alebo už známa).
    New { class: String, schema: u16 },
    /// Odkaz na už načítaný objekt (index v tabuľke načítaných položiek).
    Reference(u32),
}

/// Značka objektu v prúde, ako sa zapísala (pre bezstratový zápis, `cc-out-cvj`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagEvent {
    /// začiatok značky v prúde
    pub at: usize,
    /// dĺžka značky v bajtoch
    pub len: usize,
    pub kind: TagKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagKind {
    Null,
    /// nový objekt triedy (značka novej triedy alebo odkaz na triedu)
    Object {
        class: String,
        schema: u16,
    },
    /// spätný odkaz na objekt (v úlohách CV 2026 sa nevyskytuje)
    Reference(u32),
}

/// Čítač nad bajtmi prúdu.
pub struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
    /// Tabuľka načítaných položiek (triedy aj objekty zdieľajú číslovanie od 1).
    /// `Some(meno)` = trieda, `None` = objekt.
    loaded: Vec<Option<(String, u16)>>,
    /// Záznam značiek (iba ak je zapnutý, pre zápis).
    events: Option<Vec<TagEvent>>,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Reader {
            data,
            pos: 0,
            // index 0 je vyhradený pre null
            loaded: vec![None],
            events: None,
        }
    }

    /// Čítač, ktorý si zapisuje všetky značky objektov ([`TagEvent`]).
    pub fn recording(data: &'a [u8]) -> Self {
        Reader {
            events: Some(Vec::new()),
            ..Reader::new(data)
        }
    }

    /// Zaznamenané značky (prázdne, ak záznam nebol zapnutý).
    pub fn take_events(&mut self) -> Vec<TagEvent> {
        self.events.take().unwrap_or_default()
    }

    /// u32 na danom mieste bez posunu čítania.
    pub fn u32_at(&self, at: usize) -> Option<u32> {
        let b = self.data.get(at..at.checked_add(4)?)?;
        Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn record(&mut self, at: usize, kind: TagKind) {
        let len = self.pos.saturating_sub(at);
        if let Some(ev) = &mut self.events {
            ev.push(TagEvent { at, len, kind });
        }
    }

    pub fn pos(&self) -> usize {
        self.pos
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    /// Počet doteraz načítaných objektov (bez tried).
    pub fn object_count(&self) -> usize {
        self.loaded.iter().skip(1).filter(|e| e.is_none()).count()
    }

    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8], CvjError> {
        let end = self.pos.checked_add(n).ok_or(CvjError::Eof {
            pos: self.pos,
            need: n,
        })?;
        let s = self.data.get(self.pos..end).ok_or(CvjError::Eof {
            pos: self.pos,
            need: n,
        })?;
        self.pos = end;
        Ok(s)
    }

    /// Preskočí `n` bajtov.
    pub fn skip(&mut self, n: usize) -> Result<(), CvjError> {
        self.bytes(n).map(|_| ())
    }

    /// Pevný počet bajtov ako pole.
    pub fn array<const N: usize>(&mut self) -> Result<[u8; N], CvjError> {
        let mut out = [0u8; N];
        out.copy_from_slice(self.bytes(N)?);
        Ok(out)
    }

    pub fn u8(&mut self) -> Result<u8, CvjError> {
        Ok(u8::from_le_bytes(self.array::<1>()?))
    }

    pub fn u16(&mut self) -> Result<u16, CvjError> {
        Ok(u16::from_le_bytes(self.array::<2>()?))
    }

    pub fn i16(&mut self) -> Result<i16, CvjError> {
        Ok(i16::from_le_bytes(self.array::<2>()?))
    }

    pub fn u32(&mut self) -> Result<u32, CvjError> {
        Ok(u32::from_le_bytes(self.array::<4>()?))
    }

    pub fn i32(&mut self) -> Result<i32, CvjError> {
        Ok(i32::from_le_bytes(self.array::<4>()?))
    }

    pub fn f32(&mut self) -> Result<f32, CvjError> {
        Ok(f32::from_le_bytes(self.array::<4>()?))
    }

    pub fn f64(&mut self) -> Result<f64, CvjError> {
        Ok(f64::from_le_bytes(self.array::<8>()?))
    }

    /// Vložený bod 3D (3× f64, bez schémy).
    pub fn point3(&mut self) -> Result<[f64; 3], CvjError> {
        Ok([self.f64()?, self.f64()?, self.f64()?])
    }

    /// Vložený bod 2D (2× f64, bez schémy).
    pub fn point2(&mut self) -> Result<[f64; 2], CvjError> {
        Ok([self.f64()?, self.f64()?])
    }

    /// Schéma triedy (u16); vetvenie sa riadi nízkym bajtom (číslo verzie triedy).
    pub fn schema(&mut self) -> Result<Schema, CvjError> {
        Ok(Schema(self.u16()?))
    }

    /// Počet prvkov kolekcie: u16, pri `0xFFFF` nasleduje u32.
    pub fn count(&mut self) -> Result<u32, CvjError> {
        let n = self.u16()?;
        if n == 0xFFFF {
            self.u32()
        } else {
            Ok(n as u32)
        }
    }

    /// Overí, že `n` prvkov po aspoň `min_size` bajtoch sa ešte zmestí do zvyšku dát.
    /// Chráni pred nezmyselnými počtami z poškodeného súboru.
    pub fn check_count(
        &self,
        what: &'static str,
        n: i64,
        min_size: usize,
    ) -> Result<usize, CvjError> {
        let fits = usize::try_from(n)
            .ok()
            .filter(|&n| n.saturating_mul(min_size.max(1)) <= self.remaining());
        fits.ok_or(CvjError::BadCount {
            what,
            count: n,
            pos: self.pos,
        })
    }

    /// Reťazec `CString`: dĺžka u8 / u16 / u32 s rozšíreniami; `FF FE FF` označuje UTF-16LE.
    pub fn string(&mut self) -> Result<String, CvjError> {
        let start = self.pos;
        let mut unicode = false;
        let len = loop {
            let b = self.u8()? as u32;
            if b < 0xFF {
                break b;
            }
            let w = self.u16()? as u32;
            if w == 0xFFFE {
                if unicode {
                    // dvojitá značka Unicode nedáva zmysel
                    return Err(CvjError::BadString { pos: start });
                }
                unicode = true;
                continue;
            }
            if w < 0xFFFF {
                break w;
            }
            break self.u32()?;
        };
        let len = len as usize;
        if len > MAX_STRING_CHARS {
            return Err(CvjError::BadString { pos: start });
        }
        if unicode {
            let n = len
                .checked_mul(2)
                .ok_or(CvjError::BadString { pos: start })?;
            let raw = self.bytes(n)?;
            let units: Vec<u16> = raw
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| u16::from_le_bytes(*c))
                .collect();
            Ok(String::from_utf16_lossy(&units))
        } else {
            let raw = self.bytes(len)?;
            // jednobajtové reťazce sú v kódovej stránke Windows; ASCII stačí, ostatné nahradíme
            Ok(raw
                .iter()
                .map(|&b| if b.is_ascii() { b as char } else { '\u{FFFD}' })
                .collect())
        }
    }

    /// Starý reťazec ANSI s dĺžkou u8 (zastarané schémy).
    pub fn short_ansi(&mut self) -> Result<String, CvjError> {
        let len = self.u8()? as usize;
        let raw = self.bytes(len)?;
        Ok(raw
            .iter()
            .map(|&b| if b.is_ascii() { b as char } else { '\u{FFFD}' })
            .collect())
    }

    /// Značka objektu (`ReadObject`): nová trieda, odkaz na triedu alebo na objekt.
    pub fn object_tag(&mut self) -> Result<ObjectTag, CvjError> {
        let at = self.pos;
        let tag = self.u16()?;
        let (is_class, index) = if tag == BIG_OBJECT_TAG {
            let big = self.u32()?;
            (big & 0x8000_0000 != 0, big & 0x7FFF_FFFF)
        } else if tag == NEW_CLASS_TAG {
            let schema = self.u16()?;
            let len = self.u16()? as usize;
            if len == 0 || len > MAX_CLASS_NAME {
                return Err(CvjError::BadClassName { pos: at });
            }
            let raw = self.bytes(len)?;
            if !raw.iter().all(|b| b.is_ascii_alphanumeric() || *b == b'_') {
                return Err(CvjError::BadClassName { pos: at });
            }
            let name = String::from_utf8_lossy(raw).into_owned();
            self.loaded.push(Some((name.clone(), schema)));
            self.loaded.push(None); // nový objekt
            self.record(
                at,
                TagKind::Object {
                    class: name.clone(),
                    schema,
                },
            );
            return Ok(ObjectTag::New {
                class: name,
                schema,
            });
        } else {
            (tag & CLASS_TAG != 0, (tag & !CLASS_TAG) as u32)
        };
        if is_class {
            let (class, schema) = self
                .loaded
                .get(index as usize)
                .cloned()
                .flatten()
                .ok_or(CvjError::BadClassRef { pos: at, index })?;
            self.loaded.push(None);
            self.record(
                at,
                TagKind::Object {
                    class: class.clone(),
                    schema,
                },
            );
            Ok(ObjectTag::New { class, schema })
        } else if index == 0 {
            self.record(at, TagKind::Null);
            Ok(ObjectTag::Null)
        } else if (index as usize) < self.loaded.len() {
            self.record(at, TagKind::Reference(index));
            Ok(ObjectTag::Reference(index))
        } else {
            Err(CvjError::BadObjectRef { pos: at, index })
        }
    }
}

/// Schéma triedy `CXSchema`: nízky bajt = číslo verzie triedy, vysoký bajt = „version“.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Schema(pub u16);

impl Schema {
    /// Číslo verzie triedy (nízky bajt), podľa neho sa vetví rozloženie.
    pub fn minor(self) -> u8 {
        (self.0 & 0xFF) as u8
    }

    /// Vysoký bajt.
    pub fn major(self) -> u8 {
        (self.0 >> 8) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_ascii_and_unicode() {
        let mut data = vec![3, b'a', b'b', b'c'];
        // FF FE FF + dĺžka 2 + "ľa" v UTF-16LE
        data.extend([0xFF, 0xFE, 0xFF, 2, 0x3E, 0x01, b'a', 0]);
        // prázdny Unicode reťazec
        data.extend([0xFF, 0xFE, 0xFF, 0]);
        // dlhá dĺžka cez u16
        data.extend([0xFF, 0x03, 0x00, b'x', b'y', b'z']);
        let mut r = Reader::new(&data);
        assert_eq!(r.string().unwrap(), "abc");
        assert_eq!(r.string().unwrap(), "ľa");
        assert_eq!(r.string().unwrap(), "");
        assert_eq!(r.string().unwrap(), "xyz");
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn string_too_long_is_error_not_panic() {
        // u32 dĺžka 0xFFFFFFFF bez dát
        let data = [0xFF, 0xFE, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
        let mut r = Reader::new(&data);
        assert!(r.string().is_err());
        let data = [0xFF, 0xFF, 0xFF, 0x10, 0x00, 0x00, 0x00];
        let mut r = Reader::new(&data);
        assert!(matches!(r.string(), Err(CvjError::Eof { .. })));
    }

    #[test]
    fn count_and_check_count() {
        let data = [0x02, 0x00, 0xFF, 0xFF, 0x05, 0x00, 0x00, 0x00, 1, 2, 3, 4];
        let mut r = Reader::new(&data);
        assert_eq!(r.count().unwrap(), 2);
        assert_eq!(r.count().unwrap(), 5);
        assert_eq!(r.check_count("x", 4, 1).unwrap(), 4);
        assert!(r.check_count("x", 5, 1).is_err());
        assert!(r.check_count("x", -1, 1).is_err());
        assert!(r.check_count("x", 3, 2).is_err());
    }

    #[test]
    fn schema_bytes() {
        let s = Schema(0xEA03);
        assert_eq!(s.minor(), 3);
        assert_eq!(s.major(), 0xEA);
    }

    #[test]
    fn class_tags_and_references() {
        let mut data = vec![0xFF, 0xFF, 0xDC, 0x07, 5, 0];
        data.extend(b"CVJob");
        // druhý objekt tej istej triedy: odkaz na triedu s indexom 1
        data.extend([0x01, 0x80]);
        // odkaz na objekt 2, potom null
        data.extend([0x02, 0x00, 0x00, 0x00]);
        // big tag: nový objekt triedy 1
        data.extend([0xFF, 0x7F, 0x01, 0x00, 0x00, 0x80]);
        // odkaz na neexistujúci objekt
        data.extend([0x50, 0x00]);
        let mut r = Reader::new(&data);
        assert_eq!(
            r.object_tag().unwrap(),
            ObjectTag::New {
                class: "CVJob".into(),
                schema: 0x07DC
            }
        );
        assert_eq!(
            r.object_tag().unwrap(),
            ObjectTag::New {
                class: "CVJob".into(),
                schema: 0x07DC
            }
        );
        assert_eq!(r.object_tag().unwrap(), ObjectTag::Reference(2));
        assert_eq!(r.object_tag().unwrap(), ObjectTag::Null);
        assert_eq!(
            r.object_tag().unwrap(),
            ObjectTag::New {
                class: "CVJob".into(),
                schema: 0x07DC
            }
        );
        assert_eq!(r.object_count(), 3);
        assert!(matches!(
            r.object_tag(),
            Err(CvjError::BadObjectRef { index: 0x50, .. })
        ));
    }

    #[test]
    fn class_ref_to_object_is_error() {
        let mut data = vec![0xFF, 0xFF, 0xDC, 0x07, 1, 0, b'A'];
        // index 2 je objekt, nie trieda
        data.extend([0x02, 0x80]);
        let mut r = Reader::new(&data);
        r.object_tag().unwrap();
        assert!(matches!(
            r.object_tag(),
            Err(CvjError::BadClassRef { index: 2, .. })
        ));
    }

    #[test]
    fn bad_class_name() {
        let data = [0xFF, 0xFF, 0xDC, 0x07, 2, 0, b'A', b' '];
        let mut r = Reader::new(&data);
        assert!(matches!(r.object_tag(), Err(CvjError::BadClassName { .. })));
        let data = [0xFF, 0xFF, 0xDC, 0x07, 0, 0];
        let mut r = Reader::new(&data);
        assert!(matches!(r.object_tag(), Err(CvjError::BadClassName { .. })));
    }
}
