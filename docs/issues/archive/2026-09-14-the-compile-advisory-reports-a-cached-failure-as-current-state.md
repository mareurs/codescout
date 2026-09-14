---
kind: bug
status: fixed
title: 'BUG: the compile advisory replays a refused write''s stale verdict as current state'
tags:
- cluster/unclassified
closed: 2026-09-14
opened: 2026-09-14
owner: marius
related: []
severity: medium
---

# BUG: the compile advisory replays a refused write's stale verdict as current state

## Summary

A `doc(action="update")` response carried an advisory asserting *"your uncommitted edit does
not compile"* with 33 errors, roughly four minutes after the full four-command gate had run
green on that same tree. The errors were real — at an earlier instant. They were presented,
with no timestamp and no hedge, as a fact about the tree right now, and the advisory extends
the claim to other sessions: *"peers' gates see this break too."*

## Symptom (Effect)

Appended to an unrelated `doc(action="update")` result:

```
[codescout] your uncommitted edit does not compile, and this checkout is shared with other live sessions.
  src/librarian/tools/audit_doc_refs/mod.rs:549  missing field `live_artifact_ids` in initializer of `ResolveCtx<'_>`
  src/librarian/tools/audit_doc_refs/resolver.rs:104  non-exhaustive patterns: `RefKind::ArtifactId` not covered
  src/librarian/tools/audit_doc_refs/mod.rs:1085  non-exhaustive patterns: `Verdict::ArtifactMissing` not covered
  … showing 3 of 33
This is a statement about your own working tree, not a request — peers' gates see this break too.
```

Checked at the bytes in the same minute:

```
$ cargo check --workspace --all-targets ; echo $?
0
```

All three named errors were from a window earlier in the session, between adding two enum
variants and adding their match arms. That window closed before the gate ran.

## Reproduction

```
git rev-parse HEAD          # d311762a at time of filing
```

1. Add a variant to an enum with exhaustive `match` sites elsewhere in the crate. The tree
   does not compile.
2. Add the arms. Run `cargo check --workspace --all-targets` — exits 0.
3. Run the four-command gate — all four exit 0.
4. Call any `doc(action=...)` write. The advisory arrives naming step 1's errors.

## Environment

codescout `experiments`, MCP stdio, shared checkout with four live sessions, `target/`
contended by three concurrent `cargo test --workspace` runs.

## Root cause

**ESTABLISHED 2026-09-14 at `27f3627a`, by reading the three functions that own the state
machine.** It is the third of the three candidates this section originally listed — a result
computed before the edit and delivered after — and not an LSP or `cargo` cache. There are **two**
independent defects, and the second is the one the § Fix directions do not reach.

### 1. The trigger that would clear the notice is DROPPED, and dropped silently

`on_source_write` (`src/agent/build_check.rs:469-512`) gates on `may_start`:

```rust
if !st.may_start(Instant::now(), env.debounce) {
    return;                       // no state change, no pending flag, no record
}
```

and `may_start` (`:196-199`) is `!matches!(self.state, Running) && debounce_elapsed(...)`.

So **any write landing while a check is in flight is discarded entirely.** `SessionBuildState`
(`:172-179`) is `{ edits, state, last_started }` — there is no `pending`/`dirty` field, so a
suppressed trigger leaves nothing to replay. The window is not the 1500 ms debounce
(`DEBOUNCE_DEFAULT`, `:78`) but the **check's own duration**, documented in this module's header
as a measured ~7 s `--all-targets` incremental — roughly 4.7x the debounce.

**The write most likely to be discarded is the one that FIXES the tree.** Break at T0 and the
check starts at T0; the repair lands somewhere inside T0+1.5s..T0+7s, which is exactly the
interval `may_start` refuses. Nothing after that burst writes `.rs`, so nothing re-checks.

### 2. The in-flight result then overwrites state AFTER the repair

The spawned task closes over `edits` cloned at spawn time and ends:

```rust
let outcome = run_once(&root, &edits, &env).await;
if let Ok(mut st) = slot.lock() { st.state = outcome; }   // unconditional
```

So the failure is not merely *retained* across the repair — it is **written down several seconds
after the repair landed**, by a check that began before it, over a file list that predates it.
That is why the gate could run green minutes later and change nothing.

### 3. Delivery cannot detect any of this, by signature

`take_notice(state: &mut BuildCheckState)` (`:528`) receives no instant, no tree state, no edit
generation — and `BuildCheckState::Done { my_break, delivered }` (`:149-153`) stores none. The
reader is structurally incapable of telling a 2-second-old result from a 40-minute-old one. This
is `OB-20`'s shape one layer down: the notice ARRIVES correctly and says nothing answerable,
because the party rendering it holds no input that could falsify it.

### Why that changes § Fix

"Carry the result's instant" addresses **3** and neither **1** nor **2**. A correctly-stamped
notice about a tree repaired forty minutes ago is still a notice about a tree repaired forty
minutes ago; the reader now knows it is old, which is not the same as knowing it is false. The
defect that has to be closed is the dropped trigger — a `pending` flag re-armed on suppression
and drained when a check completes, so the repair's own write re-checks.

**Not yet reproduced by observation.** The chain above is three unconditional code paths plus
one branch; the branch (`may_start` false while `Running`) is the only part a test needs, and
that test is owed with the fix. A live reproduction requires reddening a checkout shared with
five live sessions, which is `9ca234d453d61233` and was declined.
## Evidence

Three independent readings within the same minute, two of them authoritative:

| instrument | verdict |
|---|---|
| the advisory | 33 errors, tree does not compile |
| `cargo check --workspace --all-targets` | exit 0 |
| `grep -c ArtifactId` over the three named files | 6 / 4 / 3 — every symbol present |

The four-command gate had reported `FMT 0 / CLIPPY 0 / LEAN 0 / DEFAULT 0` minutes earlier,
with 15 named tests from the change listed `ok` in the default lane.


### Three reader-side instances, contributed 2026-09-14

### Reader-side instances, contributed 2026-09-14 — **two, after one was refused by its owner**

### Three candidate instances were offered and ALL THREE withdrawn — the withdrawal is the record

sessionId `9403d62d-116b-46ea-ac9b-004acff2b1cb` contributed three reader-side reds, each of
which went green afterwards with no action by them. All three were withdrawn within the hour,
two by the contributor themself. **The population of this defect is 1 — the symptom above — and
a later reader adding a second should re-derive rather than trust any table here.**

**Why they do not belong, which is the useful part.** Each was a *correct* measurement of a tree
that changed afterwards:

| offered | what it actually was |
|---|---|
| a hook-rule test failing 7-vs-6 | the script genuinely declared 6 at that instant; a +98-line edit then finished landing |
| a lean lane with 15 `E0061` errors | `check_tool_access` had just gone from 2 params to 3 (`a13b31c6`); ~16 call sites were then fixed |
| lean red / default green in ONE gate invocation | the tree really was inconsistent at lean-time and consistent two minutes later |

That is **the corpus moved** — a class this repo already has a law for (*a count needs its INSTANT
and its TREE*). This defect is the opposite: the advisory is **wrong at the moment you read it
and stays wrong**, because the repairing write was discarded and a pre-repair outcome was
written down after the fact. Nothing but another `.rs` write can make it right. A re-run fixes
theirs; a re-run cannot reach this one.

**The inference that failed, stated so it transfers.** *Red, then green with no action by me,
therefore the red was stale.* Repair and staleness produce the **same observable sequence**, and
the fact that separates them — *"I broke that and I fixed it"* — is held only by the owner of
the mid-write state. Three careful observations by a careful session, none of which could have
been resolved from what that session could see. It is this repo's authorship law arriving in a
new place: **identify positively; a careful third-party inference is still an inference.**

**One detail corrected because the lesson depends on it.** The contributor never read an
advisory — their observation was their own `cargo` output (`LEAN_EXIT=101`, *"due to 15 previous
errors"*). The *"showing 3 of 14"* advisory text belongs to
`6be73414-6293-4a4e-95a4-4bada8327f08`, reporting their own instance. Had this section recorded
*"someone misread an advisory as a lane result"*, a later reader would learn to check which
instrument they are holding — which would have helped nobody here. The error was never about the
instrument.

**And the advisory was observed working, correctly, the same day.** `6be73414`'s arity change
fired it on their very next tool call, naming three real `E0061` sites, and it stopped when they
fixed the call sites. A correct current warning and a stale one are the same sentence — which is
the whole reason this record exists.
## Hypotheses tried

1. **Hypothesis** — a peer reverted or clobbered the edit on the shared checkout.
   **Test** — grep the three named symbols in the three named files.
   **Verdict** — rejected. All present; `cargo check` exits 0.

## Fix

- **SHA** — `61d92a7f` (branch `experiments`; positional, dies on the next rebase)
- **patch-id** — `a280d131420a7cb73454e23e4341bcbbace42873`
  (`git show 61d92a7f > f && git patch-id --stable < f`; content hash of the diff, survives
  rebase **and** cherry-pick). Single parent, 21794 bytes — neither of the two merge-commit
  traps applies.

**SHIPPED on `experiments` 2026-09-14.** A deferred trigger, not a timestamp.

The two directions this section originally proposed were written before the mechanism was
known, and only one survives. **"Carry the result's instant" reaches defect 3 alone** — a
correctly-stamped notice about a tree repaired forty minutes ago is still a notice about a tree
repaired forty minutes ago; the reader learns it is OLD, which is not that it is FALSE. **The
hedge is kept and remains right for its own reason** (the sentence escalates to a claim about
other sessions), but it treats the symptom.

What shipped closes defects 1 and 2 at their source:

| piece | role |
|---|---|
| `SessionBuildState::pending` | records a write that `may_start` refused |
| `defer()` | sets it — idempotent, so a burst inside one window defers once |
| `drain_pending(now)` | consumes it, re-arms `Running`, returns the **current** `edits` |
| `on_source_write` | loops: `run_once` → publish → drain → run again if a write was deferred |

The re-check reads `st.edits` fresh rather than reusing the spawn-time clone — the set growing
after that clone is exactly what the first check missed.

### `decide_start` was extracted so the defect's own site could be tested

Every gate in `on_source_write` was unreachable from a test: it read `BuildCheckEnv::from_env()`
internally, and this module's header forbids `EnvGuard`
(`docs/conventions/test-env-isolation.md` marks it NOT VIABLE). So the bare `return` shipped
into a function with **zero** direct test coverage and one production caller. `decide_start` now
takes `env` and `now` as arguments — the shape `run_once` already documents for this exact
reason — and the deferral branch returns before `tokio::spawn`, so a plain `#[test]` drives it.

### The mutation run changed what shipped

Removing `st.defer();` from `decide_start`: **32 passed, 1 failed.** Only
`a_write_arriving_during_a_running_check_is_deferred_not_discarded` died. All three
`drain_pending` tests stayed green, because none of them reaches the branch that calls `defer()`
— the same blindness that let the original bare `return` ship, reproduced against its own fix.
Without the extraction this would have landed a guard incapable of detecting its own removal.

Applied and reverted inside one shell invocation with a restoring `trap`, so the shared tree was
never left holding an armed red (`df0c18734b20fddd`).

### Known ceiling, stated rather than left to be found

`pending` is also set when `may_start` refuses for the **debounce** alone, and nothing drains
that until the next write. It cannot strand a harmful state: `take_notice` fires only on
`Done { my_break: Some }`, which requires a completed full check, after which `last_started` is
older than the 1500 ms window — so the next write starts a check rather than deferring. Reaching
the hole needs a sub-debounce *full* check, which this workspace does not have.
## Tests added

Six, in `src/agent/build_check.rs`. Two are **must-survive** rather than restatements — they
exist because `drain_pending` returning `Some` unconditionally, or `defer()` called
unconditionally, would satisfy every under-firing test above and re-check forever on a quiet
tree.

| test | what it buys |
|---|---|
| `a_write_refused_by_a_running_check_is_replayed_when_that_check_finishes` | RED before the fix |
| `a_deferred_write_is_consumed_by_the_drain_that_replays_it` | RED before the fix |
| `a_write_arriving_during_a_running_check_is_deferred_not_discarded` | kills the production mutation — the only one that does |
| `a_check_that_refused_nothing_replays_nothing` | MUST-SURVIVE: over-firing |
| `a_write_that_starts_a_check_defers_nothing` | MUST-SURVIVE: over-firing |
| `a_non_rust_write_is_neither_checked_nor_deferred` | MUST-SURVIVE: the cheapest gate stays first |

The RED was **observed**, not asserted: the two marked above failed against a `drain_pending`
stubbed to `None` (the pre-fix behaviour) while the rest of the module stayed green, and the
must-survives passed against that same stub — which is what makes them over-firing detectors
rather than a second way of saying the first two.
## Workarounds

**Run `cargo check --workspace --all-targets` before believing it.** That is the whole
workaround and it costs seconds against a warm `target/`. The advisory is an advisory; it is
not an instrument, and on this evidence it is not current.

## Resume

Fixed and archived. Nothing outstanding on this record.

Two things a later reader should not have to re-derive:

- **The title and slug said "cached" and the root cause says otherwise.** The frontmatter title
  is corrected; the dated slug is kept as-is deliberately, because renaming it re-keys the
  artifact (`id = sha256(abs_path)`) a second time on top of the archive move and orphans every
  inbound citation twice for a word. Raised by sessionId
  `9403d62d-116b-46ea-ac9b-004acff2b1cb`, who was right that a stale title costs a reader a
  wrong first hypothesis.
- **The interesting half is not the bug, it is why nothing caught it.** `on_source_write` had
  one production caller and zero direct tests, because it read its own configuration and this
  module forbids `EnvGuard`. A function that reads the environment is a function whose branches
  no test can drive — and it had four of them. The fix that mattered was making the decision
  take its inputs; the `pending` flag is the easy part.
## References

- `docs/issues/archive/2026-09-14-audit-doc-refs-omits-the-one-ref-kind-every-archive-breaks.md` — the
  change being edited when this fired
- `CLAUDE.md` § *Bug Tracking* — misleading errors from codescout's own MCP tools are in scope
