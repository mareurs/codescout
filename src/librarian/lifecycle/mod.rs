//! Tracker lifecycle: the pure core that places tracker files under
//! `docs/trackers/<lifecycle>/` and (in later modules) checks their status rules.

pub mod entry;
pub mod layout;
pub mod schema;
#[cfg(test)]
pub(crate) mod testkit;
