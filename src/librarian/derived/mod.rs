//! The derived index: a rebuildable view of one commit's markdown, built from git objects (the
//! commit's tree and its first-parent history walk), with the working tree's changed markdown laid
//! over it on request (`worktree`).

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
