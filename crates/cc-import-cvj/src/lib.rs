//! Import úloh CABINET VISION (`.cvj`), iba čítanie.
//!
//! `.cvj` je kontajner OLE Compound File (MS-CFB). Údaje úlohy sú v prúde `Contents`
//! ako binárna serializácia MFC (`CArchive`) so stromom tried
//! `CVJob → CVFloorLevel (miestnosť) → CVWall → CVWallFace → CVCabinet → … diely`.
//! Šifrovaný prúd `Header` sa nečíta.
//!
//! Implementácia vychádza iba z popisu formátu
//! vlastného popisu formátu `findings/07-cvj-format.md` (clean-room).
//!
//! - [`CvjFile`]: kontajner a jeho prúdy,
//! - [`parse_contents`] / [`read_job`]: prechod `Contents` → neutrálny [`CvjJob`] (mm, °),
//! - [`to_project`]: prevod do projektu CabinetCRAFT.

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

pub mod archive;
mod blind;
pub mod classes;
mod convert;
mod features;
mod filler;
mod job;
mod niche;
mod plinth;
mod slant;

use std::io::{Read, Seek};
use std::path::Path;

pub use convert::{to_plan, to_project, to_project_detailed, to_project_with, Imported};
pub use job::{
    cabinet_type_name, frame, Bbox, CarcassSource, CvClass, CvjAttributes, CvjCabinet, CvjCounter,
    CvjFront, CvjJob, CvjLink, CvjPart, CvjRoom, CvjSection, CvjWall, Face, FaceStyle,
    OpeningParams, PartRole, SectionGroup, SectionKind, MM_PER_INCH,
};

/// Najväčší prúd, ktorý načítame (ochrana pamäte pred poškodeným súborom).
const MAX_STREAM: u64 = 512 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum CvjError {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("súbor nie je úloha CABINET VISION (chýba prúd `{0}`)")]
    MissingStream(&'static str),
    #[error("prúd `{name}` je príliš veľký ({size} B)")]
    StreamTooLarge { name: &'static str, size: u64 },
    #[error("neočakávaný koniec dát na pozícii {pos} (treba {need} B)")]
    Eof { pos: usize, need: usize },
    #[error("neplatný reťazec na pozícii {pos}")]
    BadString { pos: usize },
    #[error("neplatné meno triedy na pozícii {pos}")]
    BadClassName { pos: usize },
    #[error("neplatný odkaz na triedu {index} na pozícii {pos}")]
    BadClassRef { pos: usize, index: u32 },
    #[error("neplatný odkaz na objekt {index} na pozícii {pos}")]
    BadObjectRef { pos: usize, index: u32 },
    #[error("nezmyselný počet ({what}: {count}) na pozícii {pos}")]
    BadCount {
        what: &'static str,
        count: i64,
        pos: usize,
    },
    #[error("príliš hlboké vnorenie objektov na pozícii {pos}")]
    TooDeep { pos: usize },
    #[error("neznáma trieda `{class}` na pozícii {pos}: jej rozloženie nepoznáme, úlohu nemožno dočítať")]
    UnknownClass { class: String, pos: usize },
    #[error("nepodporované: {what} (pozícia {pos})")]
    Unsupported { what: &'static str, pos: usize },
    #[error("nečakaná trieda `{found}` na pozícii {pos} (očakávaná {expected})")]
    UnexpectedClass {
        pos: usize,
        expected: &'static str,
        found: String,
    },
}

/// Prúdy kontajnera `.cvj`.
pub struct CvjFile {
    /// (meno, veľkosť) všetkých prúdov v koreni
    pub streams: Vec<(String, u64)>,
    /// prúd `Version`
    pub version: Vec<u8>,
    /// prúd `Contents` (serializovaná úloha)
    pub contents: Vec<u8>,
}

impl CvjFile {
    pub fn open(path: &Path) -> Result<CvjFile, CvjError> {
        CvjFile::read_from(std::fs::File::open(path)?)
    }

    /// Načíta kontajner z ľubovoľného zdroja (súbor, pamäť).
    pub fn read_from<F: Read + Seek>(inner: F) -> Result<CvjFile, CvjError> {
        let mut comp = cfb::CompoundFile::open(inner)?;
        let streams: Vec<(String, u64)> = comp
            .read_root_storage()
            .filter(|e| e.is_stream())
            .map(|e| (e.name().to_owned(), e.len()))
            .collect();
        let mut read = |name: &'static str| -> Result<Vec<u8>, CvjError> {
            let Some((_, size)) = streams.iter().find(|(n, _)| n == name) else {
                return Err(CvjError::MissingStream(name));
            };
            if *size > MAX_STREAM {
                return Err(CvjError::StreamTooLarge { name, size: *size });
            }
            let mut buf = Vec::new();
            comp.open_stream(format!("/{name}"))?
                .take(MAX_STREAM)
                .read_to_end(&mut buf)?;
            Ok(buf)
        };
        let version = read("Version").unwrap_or_default();
        let contents = read("Contents")?;
        Ok(CvjFile {
            streams,
            version,
            contents,
        })
    }

    /// Verzia z prúdu `Version` (rok, ?, build).
    pub fn version(&self) -> Option<[u32; 3]> {
        let w = |i: usize| -> Option<u32> {
            let b = self.version.get(i * 4..i * 4 + 4)?;
            Some(u32::from_le_bytes([
                *b.first()?,
                *b.get(1)?,
                *b.get(2)?,
                *b.get(3)?,
            ]))
        };
        Some([w(0)?, w(1)?, w(2)?])
    }

    /// Prečíta úlohu z prúdu `Contents`.
    pub fn job(&self) -> Result<CvjJob, CvjError> {
        let mut job = parse_contents(&self.contents)?;
        job.version = self.version();
        Ok(job)
    }
}

/// Otvorí `.cvj` a prečíta úlohu.
pub fn read_job(path: &Path) -> Result<CvjJob, CvjError> {
    CvjFile::open(path)?.job()
}

/// Prečíta úlohu z bajtov prúdu `Contents`.
pub fn parse_contents(data: &[u8]) -> Result<CvjJob, CvjError> {
    let walk = classes::walk(data)?;
    let (rooms, notes) = job::build(&walk.root);
    Ok(CvjJob {
        name: walk.root.name.clone(),
        description: walk.root.description.clone(),
        version: None,
        rooms,
        objects: walk.objects,
        consumed_all: walk.consumed_all,
        trailing_error: walk.trailing_error,
        notes,
    })
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_filler;
#[cfg(test)]
mod tests_fronts;
#[cfg(test)]
mod tests_links;
#[cfg(test)]
mod tests_plinth;
#[cfg(test)]
mod tests_slant;
