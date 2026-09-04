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

// NOT YET DECLARED, and deliberately so — `goal_archetype`, `goal_eval`,
// `mcp_integration`, `timemachine_smoke`: 4 files, 15 test functions, orphaned
// by the same move and still uncompiled. Declaring them is one line each; what
// is unknown is what 15 tests that have not run since 2026-05-16 do when they
// do run, and two of them are evals rather than unit tests. Measured
// 2026-09-04: with all four declared, the tree is **one** compile error from
// building — `tests/librarian/timemachine_smoke.rs:23` is missing
// `artifact_store`, `lsp` and `temp_guard` on its `ToolContext` literal.
// Tracked separately so turning them on is its own change with its own gate
// run: docs/issues/2026-09-04-four-more-test-files-orphaned-by-the-same-move.md
