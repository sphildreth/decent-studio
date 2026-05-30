//! DecentDB Studio — a cross-platform DecentDB client and administration tool.
//!
//! The crate is split into a reusable library (database access, conversion,
//! theming, export) and the iced-based `decentdb-studio` binary that drives the
//! user interface.

pub mod convert;
pub mod db;
pub mod export;
pub mod settings;
pub mod theme;
