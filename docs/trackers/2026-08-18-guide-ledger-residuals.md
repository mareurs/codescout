---
kind: tracker
status: active
title: Guide Ledger — Session Residuals 2026-08-18
tags:
  - guide-ledger
  - session-log
---

# Guide Ledger — Session Residuals (2026-08-18)

**Created:** 2026-08-18 · **Status:** open

Open follow-ups from the guide-ledger Phase A work stream (12 commits,
`d2d5686f`..`45918ca8`), which are **not** already captured by:

- `docs/superpowers/specs/2026-08-18-guide-ledger-session-identity-design.md` — the
  programme itself. **Phases B and C live there**, in *Suggested phasing*; § 4 carries
  the Phase C requirement for the cross-project-restart suppression, and § 7 carries the
  Phase B open question about a fully-expired ledger re-firing the opener. Do not
  duplicate those here.
- `docs/issues/2026-08-18-spawned-binary-test-points-guide-gc-at-real-state-dir.md` —
  fixed but deliberately unarchived; its `## Resume` owns the missing regression test.
- `docs/issues/2026-08-18-clear-leaves-mcp-session-id-stale.md` — open; Phase B closes it.
- `docs/issues/2026-08-18-global-config-dir-accepts-relative-xdg-config-home.md` — open,
  deliberately out of scope.
- `docs/trackers/bug-fix-session-log.md` — `F-54` and `W-46` from this stream.

Each item is a single-paragraph note, not a design doc. Several are explicitly
**closed-with-reasoning** rather than open, so a later session does not re-raise them.

## Measurements Phase B should take

### S-01 — Re-measure the uuid-fallback population before trusting the GC's cost

The whole-branch review quantified one growth vector: when neither
`CLAUDE_CODE_SESSION_ID` nor `.codescout/cc_session_id` resolves, `src/server.rs:273`
mints a fresh uuid per process start, so every such start leaves one more ledger file.
Steady state is roughly `starts/day × 35`; at 30 starts/day that is ~1,050 files and tens
of milliseconds of read+parse once at startup. Acceptable today, but note the *shape*
changed from O(1) per start (project-local, one file) to O(N) (per-user, whole directory).
If § 1's key chain does not shrink the uuid-fallback population, re-measure this before
Phase C adds load. The number to watch is the file count in
`$XDG_STATE_HOME/codescout/guide_hints/`.

### S-02 — Measure Phase A against `usage.db` before starting Phase B

The spec's phasing says each phase should be measurable before the next begins, and the
programme was opened against a measured ~900K tokens of re-injection waste. Phase A
changed only *where* the ledger lives, so the expected delta is near zero — which makes
it a good calibration of the measurement itself. If Phase A shows a large delta, the
measurement is picking up something other than what we think.

## Code residuals — small, deferred with reasons

### S-03 — `re_arm` returns `()` where `expire_idle` returns `usize`

`src/tools/guide_ledger.rs:160` vs `:179`. Both are removal APIs and both land in Phase B
/ C call sites that will plausibly want to log what they re-armed. Decide the shape when
the first real caller exists rather than guessing now — but decide it deliberately, since
an asymmetric pair is the kind of thing that gets "harmonised" wrongly later.

### S-04 — A crashed write strands an uncollectable `.tmp`

`atomic_write` stages to `path.with_extension("tmp")` (`src/util/fs.rs:62-77`) and `gc`'s
filter only recognises `.json` (`src/tools/guide_ledger.rs:250`), so a kill between write
and rename leaves `<session>.tmp` forever. Bounded — one per session id, overwritten by
that session's next successful write — and strictly on the safe side of the namespace
guard, since widening the filter to collect `.tmp` would give the GC a second extension to
reason about. Record only unless the directory is ever observed accumulating them.

### S-05 — Empty-file and missing-file inputs to `read_entries` are not directly asserted

Verified during final triage that both reduce exactly to arms already covered: missing →
`read_to_string` Err, walked by every `load` on a fresh tempdir; zero-byte → serde Err,
pinned by `a_malformed_file_yields_an_empty_ledger_rather_than_a_panic`; `{}` →
`Stamped(empty)`, whose consumer has both directions pinned. No uncovered arm, so this is
coverage breadth rather than a gap. Left deliberately.

### S-06 — The atomicity characterization test is happy-path only

`persist_never_leaves_a_partial_file_behind` would pass identically against the old
`std::fs::write`. It is honestly named and what it pins is real (parent creation, no stray
`.tmp`, complete JSON), but it is evidence that normal writes land, **not** that torn
writes are prevented. A genuine torn-write test needs process-kill machinery this suite
has no precedent for. Do not "fix" it by strengthening the name.

### S-07 — `guide_ledger.rs` is 731 lines, 58% test module

Production surface is ~306 lines across one struct and four free functions, all under 25
lines, so no split is warranted today. Worth re-checking before Phase B adds to it — the
plan's own expectation was ~400 lines total.

## Doc drift found in passing

### S-08 — The plan doc names a renamed test in a runnable command ✅ DONE 2026-08-18 (`98fd36aa`)

Resolved by adding a note at the step carrying the current name
(`guide_ledger_lives_in_the_injected_dir_not_under_the_project_root`,
`src/server.rs:4443`), **not** by rewriting the step — the plan is a record of what was
planned, not of what shipped. Same call as
`docs/trackers/archive/2026-05-07-retrieval-session-residuals.md` § S-08 made for the
identical situation. Original note follows.

`docs/superpowers/plans/2026-08-18-guide-ledger-phase-a-storage.md:812` and `:831`. Line
831 is a copy-pasteable `cargo test --lib guide_hint_tests::guide_ledger_does_not_live_under_the_project_root`,
and the test was renamed during the final fix wave to
`guide_ledger_lives_in_the_injected_dir_not_under_the_project_root`. The filter now matches
zero tests and **exits 0** — a false green for anyone replaying the plan. Either re-point
both lines or add a note at the top that the plan is historical record.

### S-09 — `src/util/librarian_guard.rs:44` line numbers drifted

Its comment cites five `src/server.rs` line numbers that this work stream's insertions
moved: `:1686`→1742, `:3713`→3859, `:4084`→4383, `:1510`→1562, `:1571`→1622. The comment's
*claim* is still accurate — `from_parts_with_env` really does have three test-helper
callers — only the coordinates are stale. Outside this plan's five files, so left alone.

### S-10 — `get_guide("tracker-conventions")` prescribes the master-SHA Resume line unconditionally ✅ DONE 2026-08-18 (`98fd36aa`)

Fixed directly rather than routed to the peer session: the change is in a different
section of the guide (the archive trigger, not the id-stamping advice they corrected), so
there was no overlap to collide with. `src/prompts/guides/tracker-conventions.md` now
carries the same two-path table as `CLAUDE.md` and `docs/issues/_TEMPLATE.md`, plus the
`git rev-list --left-right --count master...experiments` check that decides which path
applies, and a note that the 24 existing archived files carrying the old form are stale
instructions rather than open debt. Verified against the three invariants that iterate
every guide body, since the file is `include_str!`'d at `src/prompts/mod.rs:442`; no size
cap exists and no prompt surface changed, so no `ONBOARDING_VERSION` bump. Original note
follows.

It says an experiments-only archived bug MUST carry a `## Resume` line stating the
master-side SHA is still owed. `CLAUDE.md` and `docs/issues/_TEMPLATE.md` both correct this
to **cherry-pick only** — under a fast-forward promotion the experiments SHA already *is*
the master SHA, and the line sends a later session hunting for one that will never exist.
Not filed as a bug because a concurrent session archived a *different* staleness in that
same guide the same day (`docs/issues/archive/2026-08-18-tracker-conventions-guide-recommends-reverted-id-stamping.md`),
so that surface has an active owner; route this to them rather than opening a competing file.

## Declined — recorded so they are not re-raised

### S-11 — mtime, not `MetadataExt::ino()`, as the no-write detector ✅ DECIDED 2026-08-18

A reviewer correctly observed that `ino()` is granularity-free and strictly stronger than
mtime for detecting that `persist` rewrote a file, since `atomic_write` renames a fresh
inode into place every time. Declined: `ino()` is unix-only and this repo's CI matrix
includes `windows-latest` with `mod tests` ungated, so it would need a `#[cfg(unix)]` gate
— and gating a test away deletes its coverage on the gated platform, which is exactly how
this plan's only Critical finding arose (a POSIX path literal whose `is_absolute()` is
false on Windows). mtime only ever flakes *green* against a mutant, never red, so the suite
stays stable. Accepted cost: an unconditional-persist mutant could hide inside one
filesystem tick on a coarse-timestamp platform.

### S-12 — TTL boundary `>` vs `>=` left unpinned ✅ DECIDED 2026-08-18

`src/tools/guide_ledger.rs:185`. Exact nanosecond equality with `Utc::now() - ttl` is
measure-zero, so the two operators are indistinguishable in practice and both satisfy the
invariant. The inversion mutant (`>` → `<`) *is* killed and was demonstrated.

### S-13 — Windows keeps the POSIX-shaped state path ✅ DECIDED 2026-08-18 (user)

`%USERPROFILE%\.local\state\codescout\`, not `%LOCALAPPDATA%`. Full reasoning and the one
accepted cost (a roamed profile carries a ledger whose session ids never match, which the
GC collects) are in spec § 2, *"Windows uses the same POSIX-shaped path, deliberately"*.
Recorded here only so the question is not re-opened from scratch.
