//! Riadiaci kanál CabinetCRAFT (vrstva L5, `docs/control-protocol.md`).
//!
//! Server počúva na `127.0.0.1:PORT`, jedna požiadavka JSON na riadok
//! (`{"id":1,"method":"…","params":{…}}`), jedna odpoveď na riadok. Požiadavky
//! predáva hostiteľovi (UI vláknu okna alebo slučke `cabinetcraft serve`) cez
//! [`ControlServer::try_next`]; hostiteľ ich vybaví a odpovie [`Incoming::respond`].
//! Metódy nad jadrom (`engine.*`, `project.inspect`, …) vybaví [`engine::handle`],
//! metódy okna (`ui.*`) hostiteľ sám.

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

pub mod engine;
mod protocol;
mod server;

pub use protocol::{ControlError, MethodInfo, Request};
pub use server::{ControlServer, Incoming, DEFAULT_TIMEOUT};
