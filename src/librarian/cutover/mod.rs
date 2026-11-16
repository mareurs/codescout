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
//! - `cutover build` builds `.codescout/derived.db` in the checkout from `HEAD`, but only when a
//!   plan of `HEAD` writes nothing.
//!
//! # Inputs (D-R10)
//!
//! The input is `HEAD`, not the working tree: a plan is a function of (`HEAD`, old catalog).
//! Uncommitted edits are invisible to it, so the operator commits or stashes first.
//!
//! # Parts
//!
//! - [`old_rows`] loads the rows of the old catalog that lie under the repository.
//! - [`apply`] writes a plan to the working tree for `cutover export`, one file at a time.
//! - [`plan`] holds the plan types and [`plan::plan_export`].
//! - [`code_refs`] finds the markdown files that code, a dot directory or an mdBook reads by
//!   path: they are frozen (D-R26).
//! - [`history`] recovers the path an old artifact id hashes from git history (D-R28).
//! - [`rename`] renames the files whose names give no valid key, and reports each it cannot
//!   (D-R14, D-R21).
//! - [`rewrite`] rewrites the old citation forms of one document into the grammar (D-R15).
//! - [`entries`] recovers the params entries a body does not define, and reports the other params.
//! - [`events`] writes events and observations into the `## Events` section of a single-writer
//!   document (D-R6, D-R12), and [`links`] writes manual links as relation lines (D-R19).
//! - [`sidecars`] plans the sidecar and the declaration of an augmentation's shape (D-R24).
//! - [`report`] renders a plan as markdown or JSON. Neither form names an absolute path.

pub mod apply;
pub mod code_refs;
pub mod entries;
pub mod events;
pub mod history;
pub mod links;
pub mod old_rows;
pub mod plan;
pub mod rename;
pub mod report;
pub mod rewrite;
pub mod sidecars;

#[cfg(test)]
pub(crate) mod testkit;
