//! Dátový model CabinetCRAFT (špecifikácia MVP, kap. 3–6).
//!
//! Všetky dĺžky sú v milimetroch (`f64`).

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

mod catalog;
mod filler;
mod geometry;
mod issue;
mod machine;
mod material;
mod model;
mod nesting;
mod niche;
mod operation;
mod options;
mod project;
mod slant;
mod standard;
mod template_problem;

pub use catalog::{find_catalog_item, valid_catalog_name, Catalog, CatalogItem, NamedCatalog};
pub use filler::{FillerDowels, FillerSide, FillerSpec, FillerStandard};
pub use geometry::{Edge, Transform};
pub use issue::{Issue, IssueCode, Severity};
pub use machine::{
    Drills, EdgeCodes, LabelOrder, LabelSettings, LineEnding, MachineInfo, MachineProfile, MinPart,
    MprBohrHoriz, MprField, MprMacro, MprNuten, MprProfile, Production, ProductionMode, WorkArea,
    XcsDrill, XcsProfile, XcsRunBat, XcsSideNames, XcsSlot, XcsWorkplanes,
};
pub use material::{EdgeBand, Grain, Material, MaterialLibrary};
pub use model::{CabinetModel, Edges, FrontOpening, Hardware, Model, Part, Role};
pub use nesting::{
    NestCncRun, NestDrill, NestDrillVariant, NestLayout, NestRemnant, NestRoute, NestSheetSize,
    NestingProfile,
};
pub use niche::{EndGroups, LedGroove, LedRailStandard, NicheHole, NicheStandard};
pub use operation::{Face, Operation, P2};
pub use options::{
    CabinetOptions, JointType, LinkHole, PlinthSpan, ResolvedOptions, ShelfSupport, TipOn,
    TipOnColor, TipOnKind,
};
pub use project::{
    CabinetSpec, CabinetType, Defaults, FrontAdjust, FrontKind, FrontMount, FrontReveals,
    FrontSpec, HingeSide, LedRail, OpeningKind, OpeningSpec, Placement, PlacementLevels,
    ProjectFile, ProjectInfo, RoomSpec, WallSpec, COUNTER_THICKNESS,
};
pub use slant::{Slant, SlantSide, SlantStandard};
pub use standard::{
    BackMount, BackStandard, BlindCornerStandard, BumperRows, BumperStandard, CabinetLinkStandard,
    CarcassStandard, ConnectionKind, CoverSideStandard, DowelLayout, DowelStandard, DrawerHole,
    DrawerSpec, FittingNames, FrontRows, FrontStandard, HingeHole, HingeSpec, JointRule,
    LegStandard, LineBoreLayout, LineBoreStandard, OneOrMany, PlinthDowel, PlinthStandard,
    RunnerVariant, ShelfSpacing, Standard, TopStyle,
};
pub use template_problem::{template_placeholders, TemplateProblem, UnmatchedBrace};

/// Tolerancia pre porovnávanie dĺžok (mm).
pub const EPS: f64 = 1e-9;
