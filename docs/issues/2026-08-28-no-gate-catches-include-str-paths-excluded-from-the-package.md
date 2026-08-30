---
status: open
opened: 2026-08-28
closed:
severity: high
owner: marius
related: ["F-14"]
tags: [release-pipeline, packaging, include_str]
kind: bug
---

# BUG: No CI gate or pre-commit hook catches an `include_str!("../...")` path excluded from the published package

## Summary

Nothing in the repo — no CI job, no pre-commit hook, no test — verifies that every
`src/`-escaping `include_str!` path survives `cargo package`'s `exclude` filtering. This
already caused one high-severity publish-blocker (F-14, `docs/trackers/bug-fix-session-log.md:66,1165`)
and, during Operator Rules Phase 2 Task 2, a second `include_str!` addition
(`src/operator_rules/corpus.rs:16`) reintroduced the identical class of defect — caught only
by a task reviewer running `cargo package --list` by hand, not by any of the four standard
gate commands.

## Symptom (Effect)

None yet manifested at `cargo publish` for the second occurrence — it was caught in review,
before shipping, by manually running `cargo package --list --allow-dirty --offline` and
noting that `docs/trackers/operator-rules.md` was absent from the listing (only
`docs/PROGRESSIVE_DISCOVERABILITY.md` appeared under `docs/`). Had it shipped, the symptom
would reproduce F-14's exact failure mode: `cargo build`/`cargo test`/`cargo clippy` all pass
against the working tree, and only the verification compile inside the packaged tarball (run
during `cargo publish`) fails with `error: couldn't read '<path>': No such file or directory`.

## Reproduction

Before the fix applied alongside this bug file (`Cargo.toml:34` gaining
`"!docs/trackers/operator-rules.md"`):

```
cd /home/marius/work/claude/codescout/.claude/worktrees/operator-rules-phase-2
cargo package --list --allow-dirty --offline | grep '^docs/'
```
would show only:
```
docs/PROGRESSIVE_DISCOVERABILITY.md
```
— `docs/trackers/operator-rules.md` absent, despite `src/operator_rules/corpus.rs:16` reading
`include_str!("../../docs/trackers/operator-rules.md")`.

Git commit at time of discovery: `9b385498` (Task 2's commit, on branch
`sdd/operator-rules-phase-2`).

## Environment

Rust/Cargo toolchain per `Cargo.toml:8` (`rust-version = "1.88"`). Reproducible with a plain
`cargo package --list` — no MCP transport, no runtime environment involved. Verified in the
`operator-rules-phase-2` worktree at `/home/marius/work/claude/codescout/.claude/worktrees/operator-rules-phase-2`.

## Root cause

`Cargo.toml:34`'s `exclude` list strips `docs/` wholesale from the published tarball, with
individual files re-included via gitignore-style negation (`"!docs/PROGRESSIVE_DISCOVERABILITY.md"`).
Any new `include_str!("../docs/...")` or `include_str!("../../docs/...")` site added to `src/`
compiles and tests cleanly against the *working tree*, where `docs/` is fully present — the
`exclude` list only takes effect inside the packaged tarball that `cargo package`/`cargo publish`
build and verify-compile. There are exactly two such `src/`-escaping sites today:

- `src/server.rs:1499` — `include_str!("../docs/PROGRESSIVE_DISCOVERABILITY.md")` (the original
  F-14 site, already covered by its own `"!docs/PROGRESSIVE_DISCOVERABILITY.md"` negation entry).
- `src/operator_rules/corpus.rs:16` — `include_str!("../../docs/trackers/operator-rules.md")`
  (new in Operator Rules Phase 2 Task 2; now covered by the
  `"!docs/trackers/operator-rules.md"` negation entry added alongside this bug file).

All other `include_str!` call sites in `src/` (verified via
`grep(pattern="include_str!", path="src")`, 50 matches across 12 files) stay within `src/`
(e.g. `src/prompts/guides/*.md`, `src/dashboard/static/*`, `src/librarian/catalog/schema.sql`)
and are unaffected — `src/` itself is never excluded.

**No gate catches this class.** `cargo fmt`, `cargo clippy --workspace --all-targets
--features local-embed -- -D warnings`, `cargo test`, and `cargo check --no-default-features`
— the four commands required before every task completion in this repo — all build from the
working tree, where `Cargo.toml`'s `exclude` list has no effect. They are structurally blind
to this defect by construction, not by omission in any one run.

*measured 2026-08-28:* `grep(pattern="cargo package|cargo publish", ...)` across the repo
found matches only in `docs/trackers/bug-fix-session-log.md` (F-14's own write-up),
`docs/RELEASE.md`, and two spec/plan documents — nothing in `.github/workflows/`. F-14's own
fix-idea section (`docs/trackers/bug-fix-session-log.md:1165` region) predicted exactly this
recurrence: *"Future `include_str!(\"../docs/...\")` additions will silently regress this
until a CI gate or pre-commit hook covers it."* That gate was never built, and the prediction
held on the very next `src/`-escaping `include_str!` addition to the codebase.

## Evidence

### `cargo package --list` before the fix

```
$ cargo package --list --allow-dirty --offline | grep '^docs/'
docs/PROGRESSIVE_DISCOVERABILITY.md
```
(`docs/trackers/operator-rules.md` — the file `corpus.rs:16` `include_str!`s — is absent.)

### F-14 (prior occurrence), `docs/trackers/bug-fix-session-log.md:1165-1193`

> `error: couldn't read 'src/../docs/PROGRESSIVE_DISCOVERABILITY.md': No such file or
> directory (os error 2)` — `cargo build`/`cargo test`/`cargo clippy` all green beforehand;
> only the packaged-tarball verification compile during `cargo publish` caught it.
> **Severity: high — publish-blocker.**

## Hypotheses tried

N/A — root cause is established directly from the `Cargo.toml` `exclude` mechanics and the
`cargo package --list` reproduction; no dead ends to record.

## Fix

Not implemented as part of this bug file — this is a *tracking* issue for the missing gate,
opened per instruction as branch-level follow-up, out of scope for Operator Rules Phase 2
Task 2. The *symptom* for Task 2's specific instance (`corpus.rs:16`) was fixed in the same
change that opens this bug file: `Cargo.toml:34` gained `"!docs/trackers/operator-rules.md"`
immediately after `"docs/"`, mirroring the proven `"!docs/PROGRESSIVE_DISCOVERABILITY.md"`
pattern from F-14's own fix.

**What remains open (the actual subject of this bug):** no gate exists to catch the *next*
occurrence. F-14's fix idea, still unimplemented:

> Add a CI step that runs `cargo publish --dry-run` (or at minimum `cargo package --list` with
> an assertion that every `include_str!("../...")` path appears in the listing). Alternatively,
> a pre-commit hook grepping `src/` for `include_str!.*"\.\./"` and cross-checking against
> `cargo package --list`.

Per this task's instructions, that test/gate is **not** implemented here — it is deliberately
left as follow-up work for whoever picks up this bug file.

## Tests added

None — intentionally. This bug file tracks the *absence* of a gate; adding the gate itself
(a test asserting every `include_str!("../…")` path appears in `cargo package --list`) is the
fix, and is explicitly out of scope for the task that opened this file.

## Workarounds

Before any `cargo publish`, manually run `cargo package --list --allow-dirty --offline` and
grep for every `docs/`-relative (or otherwise `exclude`d-directory-relative) path referenced
by an `include_str!` in `src/`. `docs/RELEASE.md`'s standard ship sequence should be checked
for whether this manual step is already prescribed there; if not, that is itself a smaller,
separate documentation gap.

## Resume

Implement F-14's fix idea: a test (likely in `src/` near the crate root, or a
`tests/packaging.rs` integration test) that runs `cargo package --list` (or parses
`Cargo.toml`'s `exclude`/`include` directly) and asserts every `include_str!("\.\./[^"]+")`
match resolves to a path present in the package listing. Consider whether this belongs in the
four standard gate commands (likely not — it needs `cargo package`, which is slow and network/
registry-adjacent) or as a dedicated CI job / pre-commit hook, per F-14's original suggestion.

## References

- `docs/trackers/bug-fix-session-log.md:66` (F-14 index row) and the full write-up at
  `docs/trackers/bug-fix-session-log.md:1165` region.
- `Cargo.toml:34` — the `exclude` list, now carrying two negation entries.
- `src/server.rs:1499` — the original F-14 `include_str!` site.
- `src/operator_rules/corpus.rs:16` — the new site that reintroduced this class.
- `.superpowers/sdd/2026-08-28-operator-rules-phase-2/task-2-report.md` — Task 2's report,
  where the reviewer's finding that prompted this bug file is recorded.
