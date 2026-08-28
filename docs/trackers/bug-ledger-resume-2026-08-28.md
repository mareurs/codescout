---
id: '291b0f85cd2544a2'
kind: tracker
status: active
title: Bug-ledger resume — 2026-08-28 cross-machine handoff
owners:
- marius
tags:
- handoff
- bug-ledger
- cross-machine
- session-state
topic: bug-ledger handoff
---

# Bug-ledger resume — 2026-08-28

**Read this first if you are picking up the codescout bug ledger on another machine
or in a new session.** It is a handoff, not a log: it records where the work stopped,
what is decided, and what is waiting on you.

## How to use this file

Everything needed is **in this markdown**, deliberately. The librarian catalog is
machine-local and gitignored, so a tracker's augmentation (`prompt`, `params`,
`render_template`) does not travel — on another machine `artifact(get)` would return
`augmentation: null` and say nothing about it. So this tracker has none, owns no
`entry_prefix`, and cites nothing that only resolves on one host.

Every fix below carries a **patch-id** as well as a SHA. `experiments` is rebased after
every ship, so a SHA can be orphaned by the time you read this; `git patch-id --stable`
is a content hash of the diff and survives rebase and cherry-pick:

```
git log --all -p > /tmp/all.patch
git patch-id --stable < /tmp/all.patch > /tmp/patch-ids.txt
grep <first-12-of-patch-id> /tmp/patch-ids.txt
```

**Branch state at handoff:** `experiments` at `2243b477`, fully pushed to
`origin/experiments`, 1766 ahead of `master`. Nothing local-only. A peer session was
working the same checkout and its commits are interleaved — do not assume every commit
in this range is from this work stream.

## Shipped 2026-08-28

| SHA | patch-id | what | verified |
|---|---|---|---|
| `685a6c77` | `e573af2423474037c75b9e65ad4f7a44fa6ed7ff` | route 7 bare required-param failures | live |
| `63029176` | `722d1be5c3f8b91daf227bed6249dac76b865997` | the last 2 (`get`, `timeline`) | live |
| `03618605` | `235b5b833777a2b3a04fe43f515e94b01b58d205` | `link_scan` cross-repo split keyed on git root | live, predicted 5/15, measured 5/15 |
| `74dfbfca` | `e9d6780b06fb9004067a0e573c80195b719b9d11` | sidecar records which process + build wrote it | live, all 4 branches |
| `3ccfefb2` | `5e6cd821540336f76930737b7be91d2c1f2af2a9` | write refusal states its cause and names the project | live |
| `fae6492b` | `246a099f73c586e0a6a7ac0882aac069cb16cba2` | operator-rules: report every profile; error leads with its cause | live (CLI) |
| `fbd7f348` | `3b7974495dc3db1edf6c08a4a8ac3f0cfbb4b919` | `workspace(status)` declares the serving binary | gate only |

Three bugs archived: required-param routing, `link_scan` cross-repo bucket,
operator-rules error path.

## Open bugs and where each actually stands

Query them with
`artifact(action="find", kind="bug", filter={"status": {"in": ["open","investigating","zombie"]}})`.

- **zombie servers on deleted binaries** — 3 of 4 directions shipped (`ca2b0226`,
  `74dfbfca`, `fbd7f348`). Direction 2 **must not be built as drafted** (see Decision 1).
  Left `open` rather than `mitigated` on purpose: the canonical triage query filters on
  `open|investigating|zombie`, so `mitigated` would hide the decision it waits on.
- **workspace read_only flips mid-session** — diagnosis fully closed and live-verified.
  The remaining fix is one sentence, see Decision 2.
- **rendezvous gate latches open** — first frequency number exists: **0 of 5** stamped
  slots showed the failure, joined against `usage.db` call activity. One ~20-minute
  snapshot, so an upper bound, not a closing argument. Re-run the join before closing.
  A peer session was adding a test here at handoff time.
- **rendezvous slot never stamped** — filed then **self-refuted** the same day; now
  informational, no fix warranted.
- **guide topics are atomic nodes** — Phase 1 shipped; what remains is a sampling study.
- **wine-lane flakes under load** — needs a CI reproduction; not reproducible locally.
- **SDD ledger + catalog rows vanished** — historical data loss, cold.

## Two decisions waiting on a human

Both rest on measurements already taken. Neither blocks other work.

### Decision 1 — may a process on an unlinked binary re-index?

Direction 2 of the zombie bug says a process whose `/proc/self/exe` is gone should decline
to overwrite shared state. **As drafted it is inverted**, measured in `src/retrieval/sync.rs`:
`flush_pending` puts vectors in the store (~:904), `stream_index` completes (~:1017), and the
sidecar write is *after* both (~:1065). A zombie that declines only the sidecar write has
already shipped its vectors; the sidecar then keeps the previous writer's
`indexed_with_model`, so `index(status)` reports a model that did not produce what is in the
store. **Refusing the write destroys the evidence and keeps the damage.** The two call sites
also disagree on ordering — the worktree-delta write is *before* its embed pass — so no single
rule at the writer is correct for both.

The corrected shape is to refuse **before the embed pass** — decline to re-index, not decline
to record. On the machine measured, that would have stopped 6 of 8 running servers from
indexing. That trade is the decision.

### Decision 2 — the write-root split

Answered 2026-08-28: a read-only activation should mean *exploring only, no writing*.
One implementation was tried and **reverted**, with the blast radius measured: making a
read-only activation resident-but-never-default broke **14 tests**
(`activate_replaces_previous_project`, `is_home_false_after_switching`,
`activate_hint_shows_switched_when_away_from_home`, …). Those tests are the specification
of browse-by-activate, and the change would have forced a `workspace=` pin on every call to
explore the repo you just activated — which is not "exploring only", it is "not exploring".

Current behaviour already matches the intent *for the browsed project*. The real gap is
narrower, and is the whole remaining change:

> **Unpinned WRITES should resolve to the last writable root, while unpinned READS keep
> resolving to the activated one.**

Today both go through a single `with_project_at`, so this needs a second field
(`last_writable_root`, set only on a writable activation) plus write-awareness at
resolution. `call_content` already computes `is_write(&input)` at the choke point, so the
signal exists; the work is threading it through a core primitive that ~4,600 tests sit on.
Not started.

## Host-specific — re-measure, do not inherit

These were true of one machine on one afternoon and are the numbers most likely to mislead
elsewhere:

- "6 of 8 servers on unlinked binaries", the pid census, and the nine-minutes-to-zombie
  datapoint. The *mechanism* generalises (any rebuild unlinks every running server's
  binary); the *ratio* is a function of that host's rebuild cadence and session count.
- Plugin-cache versions and per-profile config (`~/.claude`, `~/.claude-sdd`,
  `~/.claude-kat`). One profile was several versions behind at handoff.
- The `CODESCOUT_EMBEDDER_PROTOCOL` split by spawning profile. The correlation is 1.0 **by
  construction** on any host where those config files pair the keys — read the config, do
  not re-derive it from a process census.
- Umbrella names, member lists and registry paths: per-machine by policy, never committed.

## Method notes worth carrying

- **A test that constructs the state production derives cannot tell you the derivation
  runs.** Three instances found in one day: `link_scan`'s cross-repo bucket (0 findings in
  every real repo, under a passing unit test), this repo's own `indexed_with_model` reader
  (outlived its producer by 3½ months), and a `write_block` feature that passed 4,598 tests
  with its wiring deleted. Each time the fix was a test driving the real derivation.
- **A mutation is only evidence if it applied.** One "passing" mutation here had silently
  not matched its target — character-identical to a test that cannot fail. Apply mutations
  under an asserted occurrence count.
- **Run the reproduction before reading the fix plan.** Twice in one day a bug file's own
  prescribed fix was wrong in *direction*: `link_scan`'s diagnosed cause would have produced
  a no-op, and zombie direction 2 would have made things worse.
- **Re-cost a deferral rationale before accepting it.** Three parked blockers were re-costed;
  two dissolved. The one that held (`RequestContext` carries no per-caller identity) is now
  measured rather than presumed — and a second escape route for it was closed by a one-call
  probe rather than by argument.
- **Reasoning from one code path to a claim about the whole system.** A bug filed here
  claimed a state was "permanent"; the invariant cited proved only that *one writer* could
  not repair it. Another writer did, 90 minutes later.

