//! The cutover: move every catalog-only fact of the old catalog into committed files, then build
//! the new derived catalog.
//!
//! The old catalog (`catalog.db`) holds facts that no committed file holds: events, observations,
//! manual links, augmentation shapes and params. The derived catalog reads committed text only, so
//! each such fact either moves into a committed file or the report names it. The tool never writes
//! a row into the old catalog and never runs `git commit`, `git add` or `git mv` (D-R2).
//!
//! # The three subcommands
//!
//! - `cutover plan` is the dry run. It reads the old catalog (immutable open, side files refused)
//!   and the committed files of `HEAD`, composes every change of every file in memory (an
//!   [`plan::ExportPlan`]), prints the report and writes nothing.
//! - `cutover export` writes the plan, one atomic write per file. The operator commits the result.
//! - `cutover build` builds `derived.db` beside the old catalog from `HEAD`, but only when a plan
//!   of `HEAD` writes nothing.
//!
//! # Inputs (D-R10)
//!
//! The input is `HEAD`, not the working tree: a plan is a function of (`HEAD`, old catalog).
//! Uncommitted edits are invisible to it, so the operator commits or stashes first.
//!
//! # Parts
//!
//! - [`old_rows`] loads the rows of the old catalog that lie under the repository.
//! - [`plan`] holds the plan types and [`plan::plan_export`].
//! - [`code_refs`] finds the markdown files that code, a dot directory or an mdBook reads by
//!   path: they are frozen (D-R26).
//! - [`history`] recovers the path an old artifact id hashes from git history (D-R28).
//! - [`rewrite`] rewrites the old citation forms of one document into the grammar (D-R15).
//! - [`report`] renders a plan as markdown or JSON. Neither form names an absolute path.

pub mod code_refs;
pub mod history;
pub mod old_rows;
pub mod plan;
pub mod report;
pub mod rewrite;

#[cfg(test)]
pub(crate) mod testkit;
