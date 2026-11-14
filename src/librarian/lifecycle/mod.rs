//! Tracker lifecycle: the pure core that places tracker files under
//! `docs/trackers/<lifecycle>/`, parses them and checks their status rules, plus `scan`, the one
//! impure entry point (git to list the files, the read door to read them) that feeds the core.

pub mod checks;
pub mod entry;
pub mod layout;
pub mod naming;
pub mod relations;
pub mod scan;
pub mod schema;
pub mod table;
#[cfg(test)]
pub(crate) mod testkit;
pub mod tree;
pub mod write;
