//! Opätovný zápis prúdu `Contents` s novou mapou tried MFC (findings 13 §10.3).
//!
//! Bajty medzi značkami objektov sa kopírujú (so záplatami hodnôt), značky sa tvoria
//! nanovo v poradí výstupu: trieda, ktorá sa ešte nevyskytla → `FF FF` + schéma + meno,
//! inak odkaz `0x8000 | index`; každý objekt zvýši počítadlo mapy. Spätné odkazy na
//! objekty úlohy CV 2026 nepoužívajú; ak sa vyskytnú, zápis skončí chybou.

use std::collections::{BTreeMap, HashMap};

use cc_import_cvj::archive::{TagEvent, TagKind};

use crate::CvjWriteError;

/// Úsek prúdu, ktorý sa má vynechať: [začiatok, koniec) celého objektu.
pub type Skip = (usize, usize);

pub struct Emitter<'a> {
    data: &'a [u8],
    events: &'a [TagEvent],
    /// mapa MFC: index 0 = null
    count: u32,
    class_index: HashMap<String, u32>,
    pub out: Vec<u8>,
    /// náhrady inej dĺžky pre nasledujúce `emit`: poloha → (pôvodná dĺžka, nové bajty)
    pub replaces: BTreeMap<usize, (usize, Vec<u8>)>,
}

impl<'a> Emitter<'a> {
    pub fn new(data: &'a [u8], events: &'a [TagEvent]) -> Emitter<'a> {
        Emitter {
            data,
            events,
            count: 1,
            class_index: HashMap::new(),
            out: Vec::with_capacity(data.len()),
            replaces: BTreeMap::new(),
        }
    }

    fn tag(&mut self, e: &TagEvent) -> Result<(), CvjWriteError> {
        match &e.kind {
            TagKind::Null => {
                self.out.extend_from_slice(&[0, 0]);
            }
            TagKind::Reference(i) => {
                return Err(CvjWriteError::Template(format!(
                    "spätný odkaz na objekt {i} na 0x{:x} (zápis ho nepodporuje)",
                    e.at
                )))
            }
            TagKind::Object { class, schema } => {
                match self.class_index.get(class) {
                    Some(&i) if i < 0x7FFF => {
                        self.out
                            .extend_from_slice(&(0x8000u16 | i as u16).to_le_bytes());
                    }
                    Some(&i) => {
                        self.out.extend_from_slice(&0x7FFFu16.to_le_bytes());
                        self.out
                            .extend_from_slice(&(0x8000_0000u32 | i).to_le_bytes());
                    }
                    None => {
                        let name = class.as_bytes();
                        let len = u16::try_from(name.len()).map_err(|_| {
                            CvjWriteError::Template(format!("meno triedy `{class}` je príliš dlhé"))
                        })?;
                        self.out.extend_from_slice(&0xFFFFu16.to_le_bytes());
                        self.out.extend_from_slice(&schema.to_le_bytes());
                        self.out.extend_from_slice(&len.to_le_bytes());
                        self.out.extend_from_slice(name);
                        self.class_index.insert(class.clone(), self.count);
                        self.count += 1;
                    }
                }
                // samotný objekt
                self.count += 1;
            }
        }
        Ok(())
    }

    fn copy(
        &mut self,
        a: usize,
        b: usize,
        patches: &BTreeMap<usize, Vec<u8>>,
    ) -> Result<(), CvjWriteError> {
        // náhrady inej dĺžky (meno objektu): úsek po náhradu, nové bajty, preskočiť starý
        let mut pos = a;
        let replaces: Vec<(usize, usize, Vec<u8>)> = self
            .replaces
            .range(a..b)
            .map(|(&at, (len, bytes))| (at, *len, bytes.clone()))
            .collect();
        for (at, len, bytes) in replaces {
            if at + len > b {
                return Err(CvjWriteError::Template(format!(
                    "náhrada na 0x{at:x} presahuje značku objektu"
                )));
            }
            self.copy_fixed(pos, at, patches)?;
            self.out.extend_from_slice(&bytes);
            pos = at + len;
        }
        self.copy_fixed(pos, b, patches)
    }

    fn copy_fixed(
        &mut self,
        a: usize,
        b: usize,
        patches: &BTreeMap<usize, Vec<u8>>,
    ) -> Result<(), CvjWriteError> {
        let chunk = self
            .data
            .get(a..b)
            .ok_or_else(|| CvjWriteError::Template(format!("úsek 0x{a:x}–0x{b:x} mimo prúdu")))?;
        let start = self.out.len();
        self.out.extend_from_slice(chunk);
        for (&off, val) in patches.range(a..b) {
            if off + val.len() > b {
                return Err(CvjWriteError::Template(format!(
                    "záplata na 0x{off:x} presahuje značku objektu"
                )));
            }
            let at = start + (off - a);
            if let Some(dst) = self.out.get_mut(at..at + val.len()) {
                dst.copy_from_slice(val);
            }
        }
        Ok(())
    }

    /// Zapíše úsek [`start`, `end`) pôvodného prúdu so záplatami a vynechanými objektmi.
    pub fn emit(
        &mut self,
        start: usize,
        end: usize,
        patches: &BTreeMap<usize, Vec<u8>>,
        skips: &[Skip],
    ) -> Result<(), CvjWriteError> {
        let skip_end: HashMap<usize, usize> = skips.iter().copied().collect();
        let first = self.events.partition_point(|e| e.at < start);
        let mut pos = start;
        let events = self.events;
        for e in events.get(first..).unwrap_or_default() {
            if e.at >= end {
                break;
            }
            if e.at < pos {
                // značka vo vnútri vynechaného objektu
                continue;
            }
            self.copy(pos, e.at, patches)?;
            if let Some(&to) = skip_end.get(&e.at) {
                pos = to;
                continue;
            }
            self.tag(e)?;
            pos = e.at + e.len;
        }
        self.copy(pos, end, patches)
    }

    /// Vloží surové bajty (napr. nový objekt zostavený mimo šablóny); značky vnútri
    /// musia byť zaznamenané volajúcim cez [`Emitter::emit_events`].
    pub fn finish(self) -> Vec<u8> {
        self.out
    }
}

/// Zapíše nezmenený prúd nanovo; musí vyjsť bajtovo zhodný (kontrola zapisovača).
pub fn rewrite(data: &[u8], events: &[TagEvent]) -> Result<Vec<u8>, CvjWriteError> {
    let mut e = Emitter::new(data, events);
    e.emit(0, data.len(), &BTreeMap::new(), &[])?;
    Ok(e.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Dva objekty triedy `A`: prvý so značkou novej triedy, druhý s odkazom na triedu.
    fn sample() -> (Vec<u8>, Vec<TagEvent>) {
        let mut d = vec![0xFF, 0xFF, 0x01, 0x00, 0x01, 0x00, b'A', b'x'];
        d.extend([0x01, 0x80, b'y']);
        let obj = || TagKind::Object {
            class: "A".into(),
            schema: 1,
        };
        let ev = vec![
            TagEvent {
                at: 0,
                len: 7,
                kind: obj(),
            },
            TagEvent {
                at: 8,
                len: 2,
                kind: obj(),
            },
        ];
        (d, ev)
    }

    #[test]
    fn rewrite_is_identical() {
        let (d, ev) = sample();
        assert_eq!(rewrite(&d, &ev).unwrap(), d);
    }

    #[test]
    fn skipping_first_object_redeclares_class() {
        let (d, ev) = sample();
        let mut e = Emitter::new(&d, &ev);
        e.emit(0, d.len(), &BTreeMap::new(), &[(0, 8)]).unwrap();
        // druhý objekt teraz nesie deklaráciu triedy
        assert_eq!(
            e.finish(),
            vec![0xFF, 0xFF, 0x01, 0x00, 0x01, 0x00, b'A', b'y']
        );
    }

    #[test]
    fn patch_changes_body_bytes() {
        let (d, ev) = sample();
        let mut e = Emitter::new(&d, &ev);
        let patches = BTreeMap::from([(10usize, vec![b'z'])]);
        e.emit(0, d.len(), &patches, &[]).unwrap();
        assert_eq!(*e.finish().last().unwrap(), b'z');
    }
}
