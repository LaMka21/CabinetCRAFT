//! Jadro príkazov CabinetCRAFT (vrstva L4, `plan/architecture.md` §4).
//!
//! [`Session`] drží otvorený projekt a postavený model. Každá zmena ide cez príkaz
//! z registra ([`commands`]): okno, skripty CLI aj riadiaci kanál volajú
//! [`Session::run`] s menom príkazu a parametrami v JSON. Meniace príkazy ukladajú
//! snímku projektu pre späť/znova (ADR 0002).

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

mod cmd;
mod document;
mod error;
pub mod export_cvj;
mod history;
pub mod import_cvj;
mod logo;
mod room_diff;
mod session;
mod tomlmerge;

pub use cmd::{commands, find, CommandSpec};
pub use document::{new_cabinet, Document};
pub use error::EngineError;
pub use room_diff::room_edit_commands;
pub use session::{Selection, Session};
pub use tomlmerge::merge as merge_toml;

/// Najviac skriniek v projekte (ochrana pred nezmyselným vstupom).
pub const MAX_CABINETS: usize = 1000;
