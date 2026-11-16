//! The derived index: a rebuildable view of one commit's markdown, built from git objects.

pub mod build;
pub mod diff;
mod history;
pub mod incremental;
pub mod old_catalog;
pub mod scan;
pub mod schema;
pub mod worktree;

#[cfg(test)]
pub(crate) mod testkit;
