---
kind: bug
status: fixed
title: 'BUG: the worktree-ambiguity guard refuses writes and lets reads through, so an unpinned read silently answers from the wrong tree'
tags:
- cluster/guard-narrower-than-its-name
- worktree
- workspace
- symbols
- grep
opened: 2026-09-02
closed:
severity: high
owner: marius
---

# BUG: the worktree-ambiguity guard refuses writes and lets reads through, so an unpinned read silently answers from the wrong tree

## Summary

When a checkout has git worktrees, codescout's **write** path refuses an unpinned call with an
explicit error naming every worktree. The **read** path does not: `symbols` and `grep` without an
explicit `workspace` resolve against whatever project the activation slot holds and return a
result with **no indication of which tree answered**.

The guard's name — worktree ambiguity — describes the ambiguity, not the write path. It covers one
half of it.

**The read failure is not always a zero, and the populated case is worse.** A zero at least
prompts doubt. A *populated* result from the wrong tree reads as an answer.

## Symptom (Effect)

Three tiers of evidence, kept separate because they establish different things and merging them
would claim a mechanism nobody tested.

### Tier 1 — a silent zero (`codescout-0a`, sessionId `2cb44cd3`)

Reported at the strength its reporter insisted on: *"unpinned `symbols`/`grep` from within
`.worktrees/tool-collapse` returned `0 matches` against a symbol present in that tree; pinning
`workspace=<abs path>` returned it. Observed by my round-4 implementer, not reproduced by me
directly."* A reviewer's `cargo clippy` in the same round silently resolved against the main
checkout and the run was discarded.

### Tier 2 — unpinned reads follow the active slot (`codescout-17`, sessionId `9716a130`)

The discriminating probe, and the one that rules out "pattern simply absent". Its worktree base
predates `1559daa5`, so `src/prompts/guides/workspace-state.md` carries the pre-archive citation
there and the fixed one in main. Same pattern, same glob, same minute:

```
pinned   workspace=<main>       -> 0 matches
unpinned (worktree active)      -> 1 match, workspace-state.md:147
pinned   workspace=<worktree>   -> 1 match
```

Because the two trees genuinely differ on that string, the result *identifies which tree
answered*. Tier 1's zero and a true absence are indistinguishable; this is not.

### Tier 3 — a populated near-miss from the wrong tree (this session)

Sharper than the zero, and the reason severity is `high`. `duplicate_definitions` exists 7× in
`.worktrees/entry-id-collision` (`git grep -c` at `entry-id-collision`) and **0×** at `experiments`
HEAD. With main active:

```
symbols(name="duplicate_definitions")                        # unpinned
  -> src/librarian/tools/link_scan/extract.rs (1)
       Function 829 duplicate_definitions_and_citations_dedupe

symbols(name="duplicate_definitions", workspace=<worktree>)  # pinned
  -> 8 matches in 2 files
       Function 2737-2757  duplicate_definitions          <- the actual symbol
       + 6 of its tests
```

The unpinned answer is **correct for the tree it searched** — `name` is documented as a substring
match, and that test function is main's only substring hit. It is wrong for the question, it names
no scope, and it does not look like absence. A reader gets a plausible, related-looking identifier
and no signal to doubt it.

## Reproduction

Deterministic. Branch a worktree from a commit that predates any single-line change, then query the
changed string with and without `workspace=`. Or, as in Tier 3, query a symbol that exists in one
tree only.

## Root cause

Not diagnosed to a line; the shape is established and the trigger is not.

The write path has a guard that refuses and enumerates:

```
Write blocked: git worktrees detected but workspace(action='activate') has not been called.
Worktrees: [.../tool-collapse, /home/marius/work/claude/cs-wt-p50, .../entry-id-collision]
```

No equivalent exists on the read path, which silently uses the active project. Per
`docs/adrs/2026-08-27-negative-results-name-their-scope.md`, a suspicious zero should name the
scope examined — an unpinned worktree read is exactly that case and does not.

**Two different slot states produce it, and they must not be merged.** Tier 2's slot held a *wrong*
project (a subagent had activated the worktree) — it resolves silently against a real tree. This
session's `/mcp` case left the slot **empty** — there is no tree to resolve against, and only that
state can explain a write refusing while a read stays quiet. They share a remedy and not a
diagnosis.

## Hypotheses tried

1. **Hypothesis:** the `/mcp` reconnect is what clears the activation.
   **Test:** observed directly — my user ran `/mcp`, and the next `edit_markdown` was refused with
   the worktree-enumeration error; `workspace(action="activate")` cleared it.
   **Verdict:** confirmed for *reconnect clears the slot on the write path*. Observed twice, in two
   sessions, on two different tools — and the instruments genuinely differ (a read returning a
   zero, a write returning a refusal), which is the only condition under which two agreeing
   observations beat one.

2. **Hypothesis:** Tier 1 and Tier 2 are the same state.
   **Verdict:** rejected by `codescout-17`, which corrected its own contribution rather than let it
   stand in: wrong-slot and empty-slot are different states with a shared remedy.

## Fix

Fixed 2026-09-02. **The mechanism already existed and was silent.** This file said *"No
equivalent exists on the read path"*; that was wrong, and the correction is the fix.

`worktree_read_notice` (`src/tools/core/types.rs`) has shipped since
`docs/issues/archive/2026-08-15-worktree-guard-covers-writes-but-not-reads.md`, wired into
`Tool::call_content` so it covers every read tool. Its last gate was
`notice_once(WORKTREE_READ_NOTICE)` — one-shot per conversation, on the stated grounds that
*"a notice on every call is noise"*, pinned by a test asserting the second read carries
nothing.

**The one-shot was structurally guaranteed to be spent on the wrong episode.** The notice
fires on the first qualifying read — early, while the agent is orienting and has no worktree
work to do. A `/mcp` reconnect then clears `project_chosen_this_session`, *recreating* the
exact ambiguity the notice describes, and re-arms nothing: only `GuideLedger::clear` (an
`activate`, or a post-compact) and `rekey` restore the key — and `activate` is precisely the
act that makes the notice unnecessary. So the one state that re-arms it is the one state
where it has nothing to say.

### Measured on this session's own transcript

| line | event | notice |
|---|---|---|
| 2516 | `/mcp` reconnect — clears the slot | — |
| 2863–2936 | Tier 3: unpinned read **and** a write refused for worktree ambiguity | **absent** |
| ~3770 | `workspace(post_compact=true)` → `GuideLedger::clear()` | — |
| 3775 | the very next read | **fires** |

Six reconnect markers in that file, zero notices across them; one ledger clear, a notice
immediately. The refused write in the same window is what makes the absence diagnostic
rather than circumstantial — it proves the slot was empty and worktrees present, i.e. every
condition met.

### What changed

1. **The novelty gate is gone.** The notice is gated on the CONDITION — worktrees exist, no
   tree chosen — so it speaks whenever the answer could be from a tree the caller did not
   pick. The condition is self-limiting: a repo without linked worktrees never sees it, and
   either documented remedy silences it permanently, so the correct path ends quiet and
   compliance leaves nothing armed.
2. **A pinned call is now silent.** `workspace_override` is the per-call form of the choice
   `activate` makes for the session, so a caller who named their tree gets nothing. This is
   what makes (1) affordable rather than merely louder: it removes the notice from exactly
   the calls already doing the right thing, which is the shape that otherwise gets a guard
   disabled.

The `WORKTREE_READ_NOTICE` const was removed with its only reader. `GuideLedger::notice_once`
now has **no production caller** — it survives as a `pub` ledger facility with test coverage
and nothing reaching it, which is the shape CLAUDE.md names under *loudness is a property of
a PATH*. Left in place deliberately rather than silently: named here so the next reader finds
it as a fact rather than discovering it.

### Two peers converged on this independently, without seeing the code

`codescout-17` predicted the defect from first principles while the fix was being scoped —
*"do not novelty-gate the banner … a once-per-window banner is suppressed exactly when it is
needed, because it already fired for an unrelated reason. Gate it on the condition, not on
novelty."* `codescout-0a` arrived at the same place from the other side — *"the discriminator
is not in the result, it is in the CALL: was this a worktree-bearing checkout, and was the
call unpinned"* — and supplied the argument this file adopts for why
`docs/adrs/2026-08-27-negative-results-name-their-scope.md` does not transfer: a zero is
already a prompt to doubt and the scope line finishes the thought, while a populated answer
supplies no such prompt, so there is nothing for the reader's judgement to act on. Both were
asked about the design, neither was shown `types.rs`.

### One consequence, filed rather than absorbed

Removing the one-shot makes the notice repeat for peer-serve clients, who can act on neither
remedy: `workspace` is absent from `PEER_EXPOSED_TOOLS` and `handle_tool_call_inner` strips
the `workspace` argument before dispatch. That strip is the fix for
`docs/issues/archive/2026-06-01-peer-workspace-arg-pin-escape.md` and must stay. No
discriminator exists at this seam — `home_root` cannot separate peer-serve from the ordinary
startup fallback, which is the case the notice must fire on. Filed as
`docs/issues/2026-09-02-the-worktree-notice-prescribes-two-calls-a-served-peer-cannot-make.md`.

## Fix provenance

Fixed on `experiments`.

- **SHA:** `7a3aee93`
- **patch-id:** `ea0e75497a182f07c1fbd0f753d86841f874fc1d`

Gate green in the load-bearing order: `fmt` clean; `clippy --workspace --all-targets --features
local-embed -D warnings` exit 0; lean lane **3487 passed**, exit 0; default lane last, **4996
passed / 1 failed** on `peer::server::tests::run_exits_after_idle_timeout_with_no_connections`.

That failure is **not this change**, and it was established here rather than inherited: the
test passes in isolation in **1.13s** on this machine, and the diff touches nothing under
`src/peer/`. It is the load-sensitive test of
`docs/issues/2026-09-01-peer-idle-timeout-test-is-the-third-load-sensitive-step.md`, measured
with **nine sessions live in this checkout** and a peer mid-build against the shared `target/`.
Prior sessions reached the same verdict; citing theirs instead of running it would have been
one blind spot counted twice, since a stale binary and a loaded machine produce identical
symptoms.

The commit landed five files in one go because the ledger gate requires it: a class gaining a
member and the `**Members:**` field naming it must be atomic, or the tree is red in one
direction. That gate refused this commit once, correctly — it is the forcing function added
earlier the same day, and its first live catch was its own author.
## Tests added

Two, in `src/tools/core/tests.rs`, and **the first is an inverted assertion rather than a new
one** — it is the same test that previously pinned the defect.

- `a_read_says_which_tree_it_answered_from_when_worktrees_are_unchosen` — its repeat
  assertion flipped from `!second.contains("_workspace_notice")` to `second.contains(...)`.
  Its doc comment records what it used to assert and why, so flipping it back reads as a
  regression rather than a tidy-up.
- `a_pinned_read_gets_no_worktree_notice_even_though_the_tree_is_unchosen` — new, and it
  opens with a **fixture check** asserting the same context DOES emit while unpinned. Without
  that, the silence it asserts would be satisfied by a context that emits nothing for any
  reason, which is an absence assertion monotone under the mechanism simply not running.

**Both mutations were run against the production path, not asserted about.** One kill per
guarded site, because a kill at one site says nothing about the other:

| mutation (in `types.rs`) | killed | survived |
|---|---|---|
| re-add `notice_once` gate | `a_read_says_which_tree_…` at its repeat assertion | the pinned test — **vacuously**: with the one-shot back, its first call consumes the key and the pinned call is silent for the wrong reason |
| `if false && ctx.workspace_override.is_some()` | `a_pinned_read_gets_no_worktree_notice_…` | the other |

The second row of that table is the point. Under mutation 1 the pinned test stays green
while proving nothing — so a single mutation run would have reported the pair as covered.
Each message named its own mutation; the file was restored between runs and the suite
re-verified green.
## Resume

**CLOSED 2026-09-02 — the experiment this file named as unowned has been run, and the answer is a
third behaviour neither hypothesis predicted.**

Run from the state that produces it: immediately after a `/mcp` reconnect, before any `activate`.

| step | call | result |
|---|---|---|
| 1 | unpinned `symbols(name="duplicate_definitions")` — 7× in `.worktrees/entry-id-collision`, **0×** at `experiments` HEAD | returned **home's answer**, no error, no zero |
| 2 | a write (`edit_markdown` on this file) | **refused** — *"worktrees detected but `workspace(action='activate')` has not been called"* |

Step 2 is what makes step 1 mean something: it proves the slot was **still cleared after the read
ran**. So the read resolved against the home project **and did not activate it**.

**The answer: the read path has a silent home default that does not set the slot.** Not a fallback
to a previous activation, not a zero, not an error — an implicit default, invisible in the
response, leaving the very state whose absence the write path refuses over. The asymmetry is now
fully characterised: **the write path requires an explicit slot; the read path has a default that
never creates one.** That is why a worktree read is silently wrong rather than loudly blocked, and
it is a stronger statement than *"reads lack the guard"* — there is nothing for a guard to check,
because the read is behaving as designed.

**Two wrong conclusions were drafted before this one, and both are worth recording because they
were confident and cheap to hold.**

1. *"A cleared slot silently auto-activates home."* Drafted off the `project-activation-bootstrap`
   guide arriving with the read, which reads exactly like *"you just activated a project"*.
   Falsified by `get_guide("workspace-state")`: **server construction re-arms the session-opening
   topic alone on any non-empty reloaded ledger, so that guide is re-sent on every `/mcp` reconnect
   regardless of project.** The tell is not a tell.
2. *"The two probes interfere, so the state is one-shot per reconnect."* Falsified by step 2
   succeeding as a probe after step 1 had run. They do **not** interfere — precisely because the
   read does not touch the slot, which is the finding itself. The obstacle assumed to be blocking
   the experiment was the answer to it.

**Remaining, and it is a design question rather than an unknown:** whether the read path *should*
have a silent default at all in a worktree-bearing checkout, or should refuse like the write path.
Per `docs/adrs/2026-08-27-negative-results-name-their-scope.md` the minimum is that it name the
tree it answered from — and Tier 3 above shows the dangerous case is a **populated** answer, which
that ADR does not currently cover.

Practical form, unchanged: **pass `workspace="<abs worktree path>"` on every codescout call from a
worktree, and treat any zero *or plausible near-miss* from an unpinned call as unverified rather
than as evidence.** Pin `cargo` to the worktree manifest too.
## References

- `codescout-0a` (sessionId `2cb44cd3`) — Tier 1, reported at subagent-observed strength at its own
  insistence.
- `codescout-17` (sessionId `9716a130`) — Tier 2, and the wrong-slot/empty-slot correction.
- `docs/adrs/2026-08-27-negative-results-name-their-scope.md` — the standing rule this violates.
- Adjacent, filed separately by `codescout-17`: `git merge-base --is-ancestor` is refused by the
  worktree-ambiguity hook as a "git mutation" though it writes nothing; `git -C <path>` passes.
