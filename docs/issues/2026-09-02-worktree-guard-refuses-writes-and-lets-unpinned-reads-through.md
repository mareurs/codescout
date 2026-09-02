---
kind: bug
status: open
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

Not implemented. The primary claim is the asymmetry, which is nameable and checkable **independently
of the still-open causal question** — that framing is `codescout-0a`'s and is adopted deliberately:

1. **Give the read path the write path's guard**, or at minimum make an unpinned read in a
   worktree-bearing checkout name the tree it answered from. The ADR already requires a suspicious
   zero to name its scope; this extends it to a suspicious *non*-zero, which Tier 3 shows is the
   more dangerous case.
2. Do **not** silence it by auto-pinning to the active project — that is the current behaviour and
   is what produces the wrong answer.

## Tests added

None yet. A regression test needs a two-tree fixture where the trees differ on one string, which is
Tier 2's recipe.

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
