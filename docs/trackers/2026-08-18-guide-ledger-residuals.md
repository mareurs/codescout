---
kind: tracker
status: active
title: Guide Ledger — Session Residuals 2026-08-18
tags:
- guide-ledger
- session-log
entry_prefix: S
entry_high_water_S: 15
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
- `docs/issues/archive/2026-08-18-spawned-binary-test-points-guide-gc-at-real-state-dir.md` —
  fixed but deliberately unarchived; its `## Resume` owns the missing regression test.
- `docs/issues/archive/2026-08-18-clear-leaves-mcp-session-id-stale.md` — fixed and
  archived by Phase B Task 7 (`experiments` `5bdb7f45..feb845aa`; companion
  `codescout-companion:b8ffa8b`).
- `docs/issues/archive/2026-08-18-global-config-dir-accepts-relative-xdg-config-home.md`
  — was out of scope for this stream; fixed and archived 2026-08-18 as `b17987b8`
  (`experiments`), independently of the guide-ledger work.
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

### S-03 — `re_arm` returns `()` where `expire_idle` returns `usize` ✅ DECIDED 2026-08-19

Phase C's `ActivateProject::call` was the first real caller this entry was waiting on: it
calls `led.re_arm(PROJECT_SCOPED)` only when `led.rendezvous_active()` and the project
actually switched, and discards the return value — no logging. There are now **two**
production callers, not one: `CodeScoutServer::from_parts_with_env` (`src/server.rs`)
also calls `led.re_arm(PROJECT_SCOPED)`, on any non-empty reloaded ledger at server
construction — ungated by rendezvous state — and likewise discards the return value.
Both callers already know exactly which topic they named, so a removal count would
carry no information at either call site; `expire_idle`'s caller (the idle-TTL sweep)
doesn't know in advance what it will find, which is why that one needs a count. Two
callers, neither wanting the value, *strengthens* the "leave it `()`" ruling rather than
undermining it. The design spec's own `GuideLedger` API sketch (§3
`GuideLedger` API, `docs/superpowers/specs/2026-08-18-guide-ledger-session-identity-design.md`)
never gave `re_arm` a return value either, so the asymmetry was deliberate from the design,
not an oversight waiting on a caller to decide it. Left as `()`; no harmonization needed.

Original note follows.

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

### S-14 — Phase C measured on the wire after `cargo rb` + `/mcp` — all three lifecycles confirmed

**Measured 2026-08-19 ~23:16–23:27 UTC**, release binary, server pid `1235465`
(parent `claude --resume 55515bc5-…`). Each prediction was pre-registered *before* its
observation, which is the only reason the two misses below are legible as misses rather
than as things narrated after the fact as expected.

**Trace A — reconnect (Task 4 + Task 2), ungated by design.** Server respawned 23:15:59
and loaded the persisted ledger for this conversation, which held five topics. The
startup re-arm removed exactly one:

```
librarian                     23:10:05   ← survived
progressive-disclosure        22:58:53   ← survived
symbol-navigation             22:58:09   ← survived
tracker-conventions           23:02:15   ← survived
project-activation-bootstrap  23:16:48   ← removed at construction, re-delivered
```

Four topics kept their **pre-restart timestamps**, so they were never re-sent. The first
tool call after the restart re-delivered the bootstrap and nothing else.

**This trace also measures Task 2, which is the point.** The ledger held **four** topics
when the opener fired. Under the pre-Phase-C predicate `emitted.is_empty()` that is
`false` → the opener is **suppressed**. So this is the original ~900K-token waste bug
reproducing on live traffic and now behaving correctly. Mutations prove the tests would
notice a regression; this proves the thing the tests are a proxy for.

**Trace B — activate with the gate closed (Task 3, hookless path).** `hook_at: null` on
the slot, measured four minutes after publish, so `rendezvous_active()` was false and
`ActivateProject::call` took the `else` branch. `clear()` wiped all five topics and
deleted the file; the opener on that same response re-inserted the bootstrap and
recreated it:

```
{"project-activation-bootstrap":"…23:26:48"}                       ← immediately after activate
{"project-activation-bootstrap":"…", "tracker-conventions":"…"}    ← after one artifact call
```

The A/B is clean: the **identical** `artifact(find, kind="bug")` call emitted **no guide**
before the clear and a full guide body after it, with nothing changing but ledger state.
Re-injection cost of the wipe, measured: `tracker-conventions` + `librarian` ≈ 15–20K
tokens across the following three calls.

**Two predictions missed, both worth keeping.**

1. Predicted the ledger file would be *deleted*. It is deleted — and recreated in the
   same call by the opener's insert. An intermediate state was described as the end
   state. The observable end state of a blunt clear is a **one-topic file**, not a
   missing one; anything asserting absence would read as a failure.
2. Predicted the re-fired topic would be `librarian`. The next call re-fired
   `tracker-conventions` instead — same tool, same arguments, because the topic is routed
   from the **paths in the result** and bug-file paths route to the tracker guide.
   `librarian` did re-fire, two calls later, on an `artifact(action="update")`. A
   guide-delivery prediction has to name the topic the *result* selects, not the tool.

**Standing gap, safe direction: the gate is closed for a window after every `/mcp`
reconnect.** The new process publishes a fresh slot and Claude Code fires no
`SessionStart` on reconnect, so nothing stamps `hook_at` until the next real session
event. Task 4's startup re-arm is ungated so the reconnect itself is covered, but an
`activate` inside that window blunt-clears instead of re-arming surgically. It degrades
toward re-sending, so the governing invariant holds. This is the exact complement of the
latch-open hazard filed at
`docs/issues/2026-08-19-rendezvous-gate-latches-open-when-the-hook-goes-quiet.md`: that
one is the gate never closing, this one is the gate not opening.

**Method note that cost three wrong readings before it was fixed.** "My" server was twice
identified as the newest file by mtime under `servers/` and `guide_hints/`. There are
**23 codescout processes** on this machine; both times that read another session's state,
and the second time it was two steps from a phantom bug report about a session-id
mismatch that did not exist. The reliable handle is the process tree — `$PPID` of any
`run_command` resolves to the owning server by construction, and its parent names the
conversation. Identity by timestamp is a heuristic that degrades silently as the machine
gets busier; identity by parentage cannot pick the wrong one. Same failure as the
rendezvous-slot miscount earlier the same session: a *plausible* selector standing in for
an *identifying* one.

**Status:** validated — measured end to end on the release binary.

### S-15 — The gated surgical path measured — completes S-14's matrix, and settles what opens the gate

**Measured 2026-08-19 09:59–10:02 UTC**, release binary, server pid `2773111`. S-14 left two
cells unmeasured because a `/mcp` reconnect never stamps the rendezvous slot. A **full Claude Code
restart** does, which made them reachable.

**What actually opens the gate — settled, not inferred.** The slot was stamped
`hook_at: 09:59:11.744Z` against `started_at: 09:59:11.574Z` — **170 ms** after the server
published it. So the companion `SessionStart` hook fires on a CC restart and not on a `/mcp`
reconnect, where S-14 measured four minutes of `hook_at: null`. That is the exposure window for
S-14's standing gap, now bounded: **reconnect-only sessions run gate-closed until the next real
session event.**

**Cell 3 — gate open, same project → total no-op.** The exact A/B against S-14's Trace B: same
call, same project, opposite gate state.

| | S-14 (gate closed) | here (gate open) |
|---|---|---|
| `hook_at` | `null` | stamped +170 ms |
| ledger effect | all 5 topics wiped | **nothing touched** |
| guide emitted | `project-activation-bootstrap` | `workspace-state` |

The differing guide is mechanism, not noise. Closed-gate wiped the ledger, so the opener's
`!contains(SESSION_OPENING_GUIDE)` fired and pre-empted the tool's own topic. Open-gate left the
bootstrap in place, the opener was satisfied, and the tool's own *novel* topic surfaced instead.

**Cell 4 — gate open, genuine switch → surgical re-arm.** Activated `/home/marius/work/mirela`
(read-only), then restored home. Both directions are genuine switches, so `re_arm(PROJECT_SCOPED)`
ran twice. Survivor timestamps were **byte-identical** across both:

```
librarian                     2026-08-18T23:28:46.979521412Z   ← unchanged through both switches
tracker-conventions           2026-08-18T23:27:17.886261079Z   ← unchanged
workspace-state               2026-08-19T10:00:33.077869167Z   ← unchanged (52s old at first switch)
project-activation-bootstrap  10:01:25 → 10:01:49              ← re-armed and re-delivered, twice
```

Three topics survived two project switches; exactly one was re-armed each time. **This is the
behaviour Phase C was built for, and it had never been observed outside a test until now.**

**Full matrix, all four cells now measured on live traffic:**

| | gate closed | gate open |
|---|---|---|
| **reconnect** | bootstrap only re-armed (ungated by design) | same — Task 4 is deliberately ungated |
| **activate, same project** | **wipes everything** (S-14 Trace B) | **no-op** |
| **activate, switched** | wipes everything | **bootstrap only** |

**Prediction miss — third instance of one error class, now named.** Predicted "no guide fires at
all" on the same-project activate. `workspace-state` fired: a topic never delivered this session,
surfacing on first touch, entirely independent of the re-arm logic. The two S-14 misses were the
same shape inverted — naming a topic the call did not select, then assuming no topic would be
selected. **A guide-delivery prediction requires two facts, and each miss supplied one:** which
topic *this call* selects (routed from the tool AND its result paths), and whether that topic is
already in the ledger. Neither alone predicts anything. Worth pinning because the ledger's whole
observable surface is guide deliveries, so every future claim about it needs both halves.

**Status:** validated — all four cells measured on the release binary; supersedes nothing in S-14,
completes it.

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
