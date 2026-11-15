//! Halo's vehicles (specs/halo_ce/vehicles.md): the data (`data`), the
//! mass-point physics on the engine's collision (`physics`; its doc gives
//! the design) and the driving rules around it (`drive`).

pub mod data;
pub mod drive;
pub mod physics;
