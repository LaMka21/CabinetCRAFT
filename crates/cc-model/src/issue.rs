use std::fmt;

use serde::{Deserialize, Serialize};

/// Kód pravidla validácie (kap. 8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IssueCode {
    PartTooSmall,
    PartTooLarge,
    HoleOutside,
    FaceHoleTooDeep,
    EdgeHoleTooDeep,
    EdgeHoleTooWide,
    HoleCollision,
    HoleInGroove,
    GrooveTooDeep,
    JointTooShort,
    ToolMissing,
    ShelfNoSupport,
    UnsupportedToolAxis,
    /// Diely sa dotýkajú, ale štandard pre ne nemá pravidlo spoja (kap. 4.3).
    JointRuleMissing,
    /// Skrinka presahuje koniec steny (kap. 13.4).
    PlacementOutsideWall,
    /// Skrinky sa v miestnosti prekrývajú (kap. 13.4).
    PlacementOverlap,
    /// Skrinka zasahuje do okna, dverí alebo otvoru (kap. 13.4).
    PlacementOpening,
    /// Horná hrana skrinky je nad stropom (kap. 13.4).
    PlacementAboveCeiling,
    /// Skrinka má police, ale iba zásuvky a slepé čelá: police sa nevložili (CV ich dáva
    /// iba do otvoru s dvierkami alebo otvoreného).
    ShelvesNoOpening,
}

impl fmt::Display for IssueCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

/// Jedno zistenie validácie.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Issue {
    pub code: IssueCode,
    pub severity: Severity,
    /// ID dielu, ak sa zistenie týka konkrétneho dielu.
    pub part: Option<String>,
    pub message: String,
}

impl Issue {
    pub fn error(code: IssueCode, part: Option<&str>, message: impl Into<String>) -> Self {
        Issue {
            code,
            severity: Severity::Error,
            part: part.map(str::to_owned),
            message: message.into(),
        }
    }

    pub fn warning(code: IssueCode, part: Option<&str>, message: impl Into<String>) -> Self {
        Issue {
            code,
            severity: Severity::Warning,
            part: part.map(str::to_owned),
            message: message.into(),
        }
    }
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let level = match self.severity {
            Severity::Error => "chyba",
            Severity::Warning => "varovanie",
        };
        write!(f, "{level} [{}]", self.code)?;
        if let Some(part) = &self.part {
            write!(f, " {part}")?;
        }
        write!(f, ": {}", self.message)
    }
}
