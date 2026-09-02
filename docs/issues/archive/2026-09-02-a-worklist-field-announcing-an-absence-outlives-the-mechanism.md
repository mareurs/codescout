---
id: e359101007c7b2ed
kind: bug
status: fixed
title: 'BUG: a worklist field announcing an absence outlives the mechanism that filled it, and dispatches the next session to rebuild it'
tags:
- cluster/doc-contradicted-by-code
- trackers
- worklist
- forward-reference
- observer-blindness
- statement-validity
closed: 2026-09-02
opened: 2026-09-02
owner: marius
related:
- docs/issues/2026-09-02-a-doc-comment-announcing-unbuilt-work-outlives-the-work.md
severity: high
unverified: 'No regression test, and § *Tests added* records that as a deliberate finding rather than an omission: a test would have to assert that no `Mechanism status` field in the corpus contradicts a later commit, which is the unbuilt mechanism itself and not a test of this fix. Recurrence of the class is therefore unguarded — the corrected field is pinned, the mechanism that would keep it correct is not.'
---

# BUG: a worklist field announcing an absence outlives the mechanism that filled it, and dispatches the next session to rebuild it

## Summary

`docs/trackers/issue-clusters.md` `IC-17`'s `**Mechanism status:**` field read *"Nothing
owns the working tree, the git index, or `entry_high_water_<PREFIX>`"*. It was written at
`0dea2246` (2026-09-01 00:44:39); `99d5acac` built the git-index owner at 01:48:45 the
same night — **64 minutes later** — and the field sat unfalsified for a day. Because
`CLAUDE.md` § *Observer Blindness* routes `Mechanism status` rows to `H-N` and `I-N` as a
design worklist, a stale *"nothing owns X"* does not merely misinform: it **dispatches**.

## Symptom (Effect)

The field, as read on 2026-09-02:

```
**Mechanism status:** partial — an outbound `unreviewed-content` pre-commit gate for the
commit path, and the gate reorder at `73066479` closing `target/`'s terminal state.
Nothing owns the working tree, the git index, or `entry_high_water_<PREFIX>`. The
**inbound** half is not closeable by any per-session behaviour.
```

What was on disk at the same moment:

```
.git/hooks/post-index-change    -> scripts/post-index-change-stage-log.sh   (installed)
.git/session-stage-log          -> live, mtime 2026-09-02 11:29             (written)
```

`scripts/post-index-change-stage-log.sh` records `(staged blob, path) → session id`;
`scripts/pre-commit-foreign-index.sh` refuses against it. Both were observed firing during
this session's own commit `a55396ec`:

```
refuse a pathspec commit carrying unstaged content........................Passed
refuse an index commit carrying another session's staged paths............Passed
```

**The effect is not the wrong sentence; it is the work it commissions.** This session read
the field, believed it, and proposed *"give the index an owner"* as new design work. It
reached the point of reading `append_entry.rs`'s refusal site to price the build before an
unrelated `ls scripts/` showed the mechanism already existed.

## Reproduction

At `f72a93f6`:

```
git log -1 --format='%ci' 0dea2246     # 2026-09-01 00:44:39 +0300   field written
git log -1 --format='%ci' 99d5acac     # 2026-09-01 01:48:45 +0300   mechanism built
ls .git/hooks/post-index-change        # installed
```

Then read `IC-17`'s `**Mechanism status:**` and observe it names the git index as unowned.

## Environment

Not environment-dependent — a property of the committed text at `f72a93f6`. Arch Linux,
`experiments`, seven-session shared checkout.

## Root cause

`Mechanism status` is a claim about **the state of the world outside its own file**, and
nothing binds the two. `99d5acac` touched `scripts/` and `.pre-commit-config.yaml`; it
produced no diff hunk in `docs/trackers/issue-clusters.md`, no broken link, and no check
naming the section it retired. The retired sentence kept reading as settled prose.

That is `docs/trackers/observer-blindness.md` `OB-12` exactly — *nothing in the act of
appending points backwards* — with the aggravating factor that this particular field is
**consumed as a worklist**, so the blast radius is a rebuild rather than a misreading.

The author is the structurally blind party in the `OB-12` sense: the session that built
the index owner had no reason to look at a cluster field in another file, and the session
that wrote the field could not know what the next hour would build.

*measured 2026-09-02: the two `git log -1 --format='%ci'` reads above, `ls .git/hooks/`,
and the two hook lines in `a55396ec`'s pre-commit output.*

## Evidence

### The 64-minute gap

```
0dea2246  2026-09-01 00:44:39 +0300  docs(trackers): take IC-1's split — mint IC-17 and re-tag the 15 ownership members
99d5acac  2026-09-01 01:48:45 +0300  feat(hooks): session-aware index guard, and stamp attribution at commit time
```

### The near-miss it caused

This session's `AskUserQuestion` offered *"design an owner field for the shared git index"*
as item 3 of a triage plan, and the user selected it. The exploration step that
would have priced it — reading `append_entry.rs`'s worktree refusal — had already begun.
What stopped it was not suspicion of the field but an unrelated directory listing.

### The sibling record, and why this is the third instance

`docs/issues/2026-09-02-a-doc-comment-announcing-unbuilt-work-outlives-the-work.md` holds
this sub-shape. Its `unverified:` states the INSTANCE is corrected at `4aba4c3d` and **the
CLASS is not**, naming `src/agent/mod.rs:687` as a second live case and recording that the
candidate mechanism is *deliberately not built: a new gate over a 3-item population, below
this repo's bar*.

This is the third, and **the first outside a source doc comment** — the population is no
longer confined to `///` text, which is the premise the "3-item population" costing rested
on.

## Hypotheses tried

1. **Hypothesis:** the field is current and the stage log is something narrower than an
   owner for the index. **Test:** read `scripts/post-index-change-stage-log.sh`'s header and
   confirm it records `(staged blob, path) → session id`, then confirm `pre-commit-foreign-index.sh`
   refuses on it and both are installed. **Verdict:** rejected — the log keys ownership on
   the pair, prunes rows for pairs no longer staged, and the guard fired on this session's
   own commit. **Evidence:** *Symptom (Effect)*.

## Fix

The INSTANCE is corrected. `IC-17`'s `Mechanism status` now enumerates six resources with a
verdict each, cites the script or SHA backing every one, and dates itself.

**The CLASS is not fixed**, and the sibling record is where it is tracked. What would close
it is a check relating a claim-of-absence to the thing it names — for `Mechanism status`
specifically, the field already names its resources in prose, so a machine-checkable form
would need the resources declared rather than described.

## Fix provenance

- **SHA:** `3151201a` (`experiments`) — the corrected field.
- **patch-id:** `6de1f659eabfd098dcfc52140b92f7de1448f41f` — content hash; survives rebase and cherry-pick.

## Tests added

**None, and the absence is the finding rather than an omission.** A regression test here
would have to assert that no `Mechanism status` in the corpus contradicts a later commit —
which is the unbuilt mechanism itself, not a test of this fix. Pinning the corrected
sentence would be monotone under exactly the failure that produced this bug: the next
mechanism to land falsifies the new text as silently as it falsified the old, and a
string-pin passes throughout.

## Workarounds

Before reading any `Mechanism status` as a worklist, check its `**Valid:**` date against
`git log` on the paths it names. `IC-17` now carries `dated 2026-09-02`.

## Resume

Decide whether `Mechanism status` should carry **declared resource rows** (a resource name
plus a verdict plus the script or SHA backing it) instead of prose, which is what would
make the third instance's class mechanically checkable. The corrected `IC-17` field is
already written in that shape by hand — six bulleted resources, each with a verdict and a
citation — so the question is whether to formalise it, not whether it is expressible.

## References

- `docs/issues/2026-09-02-a-doc-comment-announcing-unbuilt-work-outlives-the-work.md` — the
  sibling holding this sub-shape; its `unverified:` is where the class status lives
- `docs/trackers/observer-blindness.md` `OB-12` — nothing points backwards from the
  falsifying edit
- `docs/trackers/issue-clusters.md` `IC-17` — the corrected field
- `scripts/post-index-change-stage-log.sh`, `scripts/pre-commit-foreign-index.sh` — the
  mechanism the field said did not exist

