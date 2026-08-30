---
kind: bug
status: fixed
tags:
- release-pipeline
- packaging
- include_str
closed: 2026-08-30
opened: 2026-08-28
owner: marius
related:
- F-14
severity: high
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

**Built: `tests/packaged_includes.rs`.** `188cf9f0` on `experiments`, patch-id
`c7604ff8088c8f32`.

The gate scans every `.rs` under each package's `src/` for `include_str!` string literals,
keeps the ones whose target resolves outside `src/`, and asserts each appears in
`cargo package --list` for its package.

**A test, not a CI job — and the choice was measured, not assumed.** F-14's fix idea offered
either. The four documented gate commands all build from the working tree, so they are blind
to `exclude` by construction; a CI job would see it, but only on push, and this repo had sat
**119 commits ahead of origin for two days** when the gate was written. A CI-only gate would
not have run once in that window. `cargo package --list` does not build — measured **0.21s**
per package — so it costs nothing to put it where the gate commands already look. CI gets it
free, since CI runs `cargo test`.

**The oracle is cargo, deliberately.** The tempting alternative is to reimplement `exclude`'s
gitignore-style negation in the test. That is two implementations of one operation — the
exact defect class removed from `reindex_cli` in `9f743091` the same afternoon — and it would
agree with itself while disagreeing with the tool that actually builds the tarball. Shelling
out to `cargo package --list` asks cargo what cargo will do.
## Tests added

Three, in `tests/packaged_includes.rs`, and the second and third are not ceremony:

| test | what it holds |
|---|---|
| `every_escaping_include_str_survives_cargo_package` | the invariant |
| `the_scan_actually_finds_the_known_escaping_sites` | the population is non-empty |
| `the_escape_detector_discriminates` | the predicate separates escaping from internal, and the parser reads both macro forms |

**Mutation matrix — the result, rather than the green tick:**

| mutation | gate | control |
|---|---|---|
| drop `"!docs/trackers/operator-rules.md"` from `exclude` (the defect that shipped) | **FAILS**, naming site, target and the fix | passes |
| break `MARKER` so the scan finds nothing | **passes — vacuously** | **FAILS** |

The second row is the argument for the control. With an empty population the gate is green
over nothing, which is the same false-green shape as the original bug: a check that cannot
see the thing it is named after. `tests/committed_paths.rs` already encodes this convention
(`the_scan_actually_reads_files`), and its doc comment names the identical hazard — so this
file follows it rather than inventing a shape.

Runs in **both** lanes, verified by name in each run's output (4865/0 full, 3385/0 lean).
Those counts are the working **tree**, not HEAD: two other sessions had uncommitted files in
this shared checkout, and cargo does not consult git.

**Stated blind spot, in the source rather than only here:** the scan reads string *literals*,
so an argument built with `concat!` or via a `const` is invisible to it.
## Workarounds

Before any `cargo publish`, manually run `cargo package --list --allow-dirty --offline` and
grep for every `docs/`-relative (or otherwise `exclude`d-directory-relative) path referenced
by an `include_str!` in `src/`. `docs/RELEASE.md`'s standard ship sequence should be checked
for whether this manual step is already prescribed there; if not, that is itself a smaller,
separate documentation gap.

## Resume

Done — nothing outstanding. `188cf9f0`, patch-id `c7604ff8088c8f32`; fmt clean, clippy
`--workspace --all-targets --features local-embed` clean, 4865/0 full, 3385/0 lean.

The reproduction was run before the fix, and it reported the *healthy* state — both escaping
paths present in a 493-file package — which is the point: the symptom had already been
patched by hand and only the gate was missing. What the run established is the oracle's
shape and cost, not the presence of a defect.
## References

- `docs/trackers/bug-fix-session-log.md:66` (F-14 index row) and the full write-up at
  `docs/trackers/bug-fix-session-log.md:1165` region.
- `Cargo.toml:34` — the `exclude` list, now carrying two negation entries.
- `src/server.rs:1499` — the original F-14 `include_str!` site.
- `src/operator_rules/corpus.rs:16` — the new site that reintroduced this class.
- `.superpowers/sdd/2026-08-28-operator-rules-phase-2/task-2-report.md` — Task 2's report,
  where the reviewer's finding that prompted this bug file is recorded.
