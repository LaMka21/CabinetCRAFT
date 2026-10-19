//! Zápis zákazky do súboru CABINET VISION (`.cvj`) podľa šablóny (findings 13 §10).
//!
//! Šablóna = `.cvj` uložený v CV dielne. Prúd `Contents` (MFC `CArchive`) sa prečíta
//! s polohami všetkých značiek objektov a zapíše nanovo: bajty medzi značkami sa kopírujú
//! (s prípadnými záplatami hodnôt), každá značka sa vytvorí znova z novej mapy tried, takže
//! objekty sa dajú vkladať aj vynechávať. Ostatné prúdy sa kopírujú. `Header` (licenčná
//! vyrovnávacia pamäť, findings 13 §3) sa nečíta, nedešifruje ani nevytvára.
//!
//! Šablónové súbory CV patria dielni: do repa sa necommitujú, cesta ide z nastavení.

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

pub mod emit;
pub mod export;
pub mod sections;
pub mod walls;

use std::io::{Read, Seek, Write};
use std::path::Path;

use cc_import_cvj::archive::TagEvent;
use cc_import_cvj::classes::Walk;

#[derive(Debug, thiserror::Error)]
pub enum CvjWriteError {
    #[error("súbor CV: {0}")]
    Io(#[from] std::io::Error),
    #[error("čítanie šablóny .cvj: {0}")]
    Read(#[from] cc_import_cvj::CvjError),
    #[error("šablóna .cvj: {0}")]
    Template(String),
}

/// Najväčší prúd šablóny, ktorý načítame (ochrana pred poškodeným súborom).
const MAX_STREAM: u64 = 512 * 1024 * 1024;

/// Šablónová zákazka: všetky prúdy v koreni a rozobratý `Contents`.
pub struct Template {
    /// CLSID koreňa kontajnera (zachová sa)
    pub root_clsid: [u8; 16],
    /// prúdy v koreni v pôvodnom poradí (meno, dáta)
    pub streams: Vec<(String, Vec<u8>)>,
    /// prúd `Contents`
    pub contents: Vec<u8>,
    /// rozobratý `Contents` s polohami uzlov a parametrov
    pub walk: Walk,
    /// značky objektov `Contents` v poradí čítania
    pub events: Vec<TagEvent>,
}

impl Template {
    pub fn open(path: &Path) -> Result<Template, CvjWriteError> {
        Template::read_from(std::fs::File::open(path)?)
    }

    pub fn read_from<F: Read + Seek>(inner: F) -> Result<Template, CvjWriteError> {
        let mut comp = cfb::CompoundFile::open(inner)?;
        let root_clsid = *comp.root_entry().clsid().as_bytes();
        let names: Vec<(String, u64)> = comp
            .read_root_storage()
            .filter(|e| e.is_stream())
            .map(|e| (e.name().to_owned(), e.len()))
            .collect();
        let mut streams = Vec::with_capacity(names.len());
        for (name, size) in names {
            if size > MAX_STREAM {
                return Err(CvjWriteError::Template(format!(
                    "prúd `{name}` je príliš veľký ({size} B)"
                )));
            }
            let mut buf = Vec::new();
            comp.open_stream(format!("/{name}"))?
                .take(MAX_STREAM)
                .read_to_end(&mut buf)?;
            streams.push((name, buf));
        }
        let contents = streams
            .iter()
            .find(|(n, _)| n == "Contents")
            .map(|(_, d)| d.clone())
            .ok_or_else(|| CvjWriteError::Template("chýba prúd `Contents`".into()))?;
        let (walk, events) = cc_import_cvj::classes::walk_recorded(&contents)?;
        if !walk.consumed_all {
            return Err(CvjWriteError::Template(format!(
                "`Contents` sa nepodarilo prečítať celý: {}",
                walk.trailing_error.clone().unwrap_or_default()
            )));
        }
        Ok(Template {
            root_clsid,
            streams,
            contents,
            walk,
            events,
        })
    }

    /// Najväčšie použité ObjectID: v `Contents` a v menách prúdov `Scene<N>`/`Sheet<N>`
    /// (findings 13 §10.5). Nové ID musia byť väčšie.
    pub fn max_object_id(&self) -> u32 {
        fn walk_max(n: &cc_import_cvj::classes::Node) -> u32 {
            n.children.iter().map(walk_max).fold(n.object_id, u32::max)
        }
        let streams = self
            .streams
            .iter()
            .filter_map(|(name, _)| {
                name.strip_prefix("Scene")
                    .or_else(|| name.strip_prefix("Sheet"))
                    .and_then(|n| n.parse::<u32>().ok())
            })
            .fold(0, u32::max);
        walk_max(&self.walk.root).max(streams)
    }
}

/// Zapíše kontajner `.cvj`: prúdy v poradí, `Contents` nahradený; `skip` = prúdy, ktoré
/// sa vynechajú (napr. `Header`).
pub fn write_cvj<W: Read + Write + Seek>(
    out: W,
    template: &Template,
    contents: &[u8],
    skip: &[&str],
) -> Result<(), CvjWriteError> {
    let mut comp = cfb::CompoundFile::create_with_version(cfb::Version::V3, out)?;
    comp.set_storage_clsid("/", uuid_from(template.root_clsid))?;
    for (name, data) in &template.streams {
        if skip.contains(&name.as_str()) {
            continue;
        }
        let bytes = if name == "Contents" {
            contents
        } else {
            data.as_slice()
        };
        let mut s = comp.create_stream(format!("/{name}"))?;
        s.write_all(bytes)?;
    }
    comp.flush()?;
    Ok(())
}

fn uuid_from(b: [u8; 16]) -> uuid::Uuid {
    uuid::Uuid::from_bytes(b)
}
