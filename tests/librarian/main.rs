//! Harness for the integration tests under `tests/librarian/`.
//!
//! **Why this file exists.** Cargo auto-discovers integration tests as
//! `tests/*.rs` only — never `tests/*/*.rs`. These files lived at
//! `crates/librarian-mcp/tests/*.rs` and were real, compiled targets until
//! `d48bf992` (2026-05-16) dissolved that crate and moved them one directory
//! deeper. Nothing failed: cargo simply stopped seeing them, and 18 test
//! functions across 5 files went uncompiled for ~3.5 months behind a green
//! suite. `cargo metadata --no-deps` reported 25 test targets and **0** whose
//! `src_path` was under `tests/librarian/`.
//!
//! So a file in this directory is only compiled if it is declared below.
//! Adding a `.rs` here without a `mod` line re-creates the exact defect this
//! harness was written to end.
//!
//! See docs/issues/archive/2026-09-02-a-test-file-in-no-cargo-target-asserts-nothing-and-is-a-tautology-anyway.md

mod companion_hint;
mod goal_archetype;
// The next two compile but do NOT execute — annotated so neither is credited with
// coverage it does not provide. This target is 19 tests, 17 of which run.
//
// `goal_eval` is a tier-3 eval, `#[ignore]`d pending an API key and `synthesize()`
// being wired. `mcp_integration` is `#[ignore]`d because it spawns a `librarian-mcp`
// binary that the 2026-05-16 dissolution deleted, and asserts a 15-tool list from
// before the tool collapse — reviving it is a rewrite, not a re-enable.
//
// Declaring them is still worth doing rather than leaving them undeclared: an ignored
// test prints its reason on every run, whereas an undeclared file is silent in exactly
// the way this harness exists to prevent.
mod goal_eval;
mod mcp_integration;
mod timemachine_smoke;
