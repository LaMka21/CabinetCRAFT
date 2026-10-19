//! Nesting: rozloženie dielov na tabule a delenie obrábania medzi SCM nesting a CX100.
//!
//! - [`pack`]: balenie obdĺžnikov na tabuľu (MaxRects, okraj, medzera, otáčanie podľa vlákna),
//! - [`plan`]: diely modelu po materiáloch na tabule, plocha hore a delenie operácií.
//!
//! Programy `.xcs` z plánu píše `cc-out-xcs` (modul `nest`).

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

pub mod pack;
pub mod plan;

pub use pack::{pack, Item, PackedSheet, Packing, Placed, Rotation, SheetSpec};
pub use plan::{
    choose_face_up, is_small, plan, NestError, NestPlan, PartPlacement, PartPlan, PartRoute,
    SheetGroup, SheetLayout,
};
