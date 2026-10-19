//! Voľba písma pre PDF: vložené systémové písmo TrueType (podmnožina), alebo
//! štandardná Helvetica bez vloženia.
//!
//! Systémové písmo (Windows Arial, na Linuxe Liberation Sans alebo DejaVu Sans) sa
//! načíta raz za beh programu. Jeho verzia sa medzi počítačmi líši, preto testy so
//! zlatými súbormi používajú [`FontChoice::Base14`].

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use crate::ttf::TtFont;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FontChoice {
    /// Vložiť systémové písmo s diakritikou; keď chýba, ako `Base14`.
    #[default]
    System,
    /// Helvetica bez vloženia (WinAnsi); znaky mimo WinAnsi bez diakritiky.
    Base14,
}

/// Písma strany: obyčajné a tučné (`None` = Helvetica).
#[derive(Debug, Clone, Default)]
pub struct Fonts {
    regular: Option<Arc<TtFont>>,
    bold: Option<Arc<TtFont>>,
}

impl Fonts {
    pub fn new(choice: FontChoice) -> Fonts {
        match choice {
            FontChoice::Base14 => Fonts::default(),
            FontChoice::System => system().clone(),
        }
    }

    /// Je písmo vložené (s diakritikou)?
    pub fn embedded(&self) -> bool {
        self.regular.is_some()
    }

    pub(crate) fn get(&self, bold: bool) -> Option<&TtFont> {
        if bold {
            self.bold.as_deref().or(self.regular.as_deref())
        } else {
            self.regular.as_deref()
        }
    }
}

fn system() -> &'static Fonts {
    static SYSTEM: OnceLock<Fonts> = OnceLock::new();
    SYSTEM.get_or_init(|| {
        for (regular, bold) in system_paths() {
            let load = |p: &PathBuf| std::fs::read(p).ok().and_then(TtFont::parse).map(Arc::new);
            if let Some(r) = load(&regular) {
                return Fonts {
                    bold: load(&bold).or_else(|| Some(r.clone())),
                    regular: Some(r),
                };
            }
        }
        Fonts::default()
    })
}

/// Kandidáti (obyčajné, tučné), ktoré na tomto počítači existujú.
pub(crate) fn system_paths() -> Vec<(PathBuf, PathBuf)> {
    let mut out = Vec::new();
    if let Some(win) = std::env::var_os("WINDIR") {
        let dir = PathBuf::from(win).join("Fonts");
        out.push((dir.join("arial.ttf"), dir.join("arialbd.ttf")));
    }
    for (r, b) in [
        (
            "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf",
        ),
        (
            "/usr/share/fonts/liberation-sans/LiberationSans-Regular.ttf",
            "/usr/share/fonts/liberation-sans/LiberationSans-Bold.ttf",
        ),
        (
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
        ),
        (
            "/System/Library/Fonts/Supplemental/Arial.ttf",
            "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
        ),
    ] {
        out.push((PathBuf::from(r), PathBuf::from(b)));
    }
    out.retain(|(r, _)| r.is_file());
    out
}
