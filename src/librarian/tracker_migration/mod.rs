//! Tracker migration (Plan L-E1): the library behind the `codescout trackers`
//! commands that move a flat tracker into a lifecycle collection.
//!
//! Spec: `docs/superpowers/specs/2026-11-15-tracker-migration-design.md`.

pub mod check;
pub mod dates;
pub mod index_rows;
pub mod inventory;
pub mod mapping;
pub mod plan;
pub mod records;
pub mod rewrite;
