//! The derived index: a rebuildable view of one commit's markdown, built from git objects.

pub mod build;
pub mod scan;
pub mod schema;

#[cfg(test)]
mod testkit;
