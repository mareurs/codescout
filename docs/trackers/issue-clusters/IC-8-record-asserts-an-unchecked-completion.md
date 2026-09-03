---
kind: tracker
status: active
title: 'a record asserts a completed action that nothing re-checked'
owners:
- marius
tags:
- defect-classes
- clusters
- record-asserts-an-unchecked-completion
topic: issue clusters and rule promotion
---

## IC-8 — a record asserts a completed action that nothing re-checked

**Slug:** `cluster/record-asserts-an-unchecked-completion`
**Claim:** A record states that an action completed, and nothing is scheduled to re-check it. The assertion is written at the moment of *intent* and read forever after as *outcome*, so a step that silently did not happen leaves a closure note that reads exactly like a successful one.
**Members:** `filter={"tags": {"contains": "cluster/record-asserts-an-unchecked-completion"}}` **New member 2026-09-02: `sendmessage-returns-success-for-a-message-held-for-approval`** — `SendMessage` returns `{"success": true, "… → <name>"}` for a message **held for the recipient user's approval and never delivered**. The return is a record asserting a completed action; the `[Cross-session delivery notice]` contradicting it arrives **asynchronously, one or more turns later**. **The variant worth naming, because it is not the class's usual shape:** here a re-check *does* eventually arrive — it is simply later than the window in which the assertion is acted on, which is functionally identical to none. Measured cost: a claim that a mutation window had been announced to a peer was committed at `8dee33a8` on the strength of the success return, and was false when written. The distinguishing test still holds — at the moment of use, nothing re-checks, and the tool offers no field distinguishing *accepted for delivery* from *delivered*. — `n=5`, 2026-08-31, by query after archive backfill. **+1, 2026-09-03: `artifact-move-writes-a-stale-snapshot-and-leaves-the-source`, and it is a new SUBSYSTEM** — librarian artifact moves, where the existing members are bug-file frontmatter and `status: taken` claims. `doc(action="move")` returned `"moved": true`, a `history_grafted` count, and a `stage_hint` instructing the caller to *"confirm that `git status --short` shows a single `R` rename line"* — while writing a **stale** body to the destination (predating two `update` calls that had each returned `updated: true`) and **not deleting the source**. Three assertions of a completed action, none re-checked, all false of what landed on disk. It satisfies this class's distinguishing test sharply: the destination is a *plausible, complete, well-formed* bug file, so no plausibility check catches it and the only party who could notice is one who diffs two paths they have been told are a rename. **Note the direction of the luck** — the un-performed deletion is the sole reason the newer content survived; a fix that repairs the deletion without repairing the staleness converts this from a recoverable inconsistency into silent content loss.
**Blind party:** the reader of the record, who has no signal distinguishing a verified closure from an asserted one. The author is not blind — they simply wrote what they intended to do.
**Promotes to:** `DC` — `docs/trackers/claim-decay.md`, whose `undecidable-green` and `premature-assertion` types already name this shape. This class is the bug-corpus entry point to that ledger, not a competitor.
**Mechanism status:** `shipped (partial)` — **checked against the code 2026-09-03, and the previous `none yet` was wrong.** `librarian(action="doctor")` now ships four checks that are exactly this class's remedy — something that re-reads an asserted closure:

| check | what it re-checks | live count 2026-09-03 |
|---|---|---|
| `terminal_status_with_caveat` | a terminal `status` whose `unverified:` field is non-empty | **92** |
| `terminal_status_without_fix_anchor` | a terminal status with no fix anchor to resolve | 5 |
| `archived_fix_sha_unresolvable` | a declared fix SHA that no longer names an object | 0 (54 of 350 archived files in scope) |
| `claim_liveness` (`claim_held_by_dead_session`, `claim_unresolvable_here`, `claim_without_claimant`, `claim_held_by_live_session`) | `status: taken` backed by a sessionId that still resolves | 0 |

Two of them cite this class by name as the reason they exist: `src/librarian/tools/doctor.rs:5597` (*"A record claiming a live owner with nothing re-checking it is `issue-clusters:IC-8`"*) and `src/librarian/session_registry.rs:5`. `terminal_status_with_caveat`'s own doc comment states the class's structure precisely — *"authors write the caveat, the canonical triage query filters on `status`"* — i.e. without the reader half, `unverified:` only moved an unchecked assertion from prose into frontmatter.

**What is still owed, and why this is `partial` rather than `shipped`:** every check above re-reads a **declared field**. None re-checks a closure stated only in **prose**, which is the shape of the one member below — a `## Fix` section asserting a `git worktree remove` in the past tense. No check opens that sentence, and `archived_fix_sha_unresolvable` deliberately skips freeform prose rather than sweeping it.
**Valid:** dated 2026-08-31

One member, kept because the instance is unusually instructive. `bench-worktree-deletion-recorded-as-done-never-happened`: an archived bug file is `status: fixed`, `closed: 2026-08-16`, and its `## Fix` section states the worktree *"was removed with `git worktree remove --force .worktrees/bench`"*, closing with *"174 MB reclaimed, 163 MB of it regenerable `.codescout` index state."* At filing the directory was **still on disk**, at 174 MB, of which 163 MB was `.codescout`.

**The directory has since been deleted — re-checked 2026-09-03** (`ls -d .worktrees/bench` → `No such file or directory`; `git worktree list` shows only `result-cap-marker-gate` and `tool-collapse`). **This paragraph read "The directory is still on disk. It is 174 MB" in the present tense until that re-check**, which is this very class holding about its own evidence: a true measurement, correctly taken, left under a verb tense nothing re-read. The instance stands — it was real on 2026-08-16 and the record was false when written — but the *evidence* is now historical, and a reader who re-runs the `ls` to confirm the class will find it refuted. Whoever verifies this class next: the artifact to check is the archived bug file's prose, not the filesystem.

**The numbers match the live directory exactly, and that is the finding.** They were measured — correctly — *before* the deletion, then written in the past tense. So the record is not fabricated and not sloppy; it is a true measurement placed under a false verb. No plausibility check catches it, because every figure in it is right.

This is filed as a class of one deliberately rather than folded into `DC`. The threshold is not met and it will not promote on its own, but the *bug corpus* is where instances arrive, and a class with a defined slug is what lets the second instance find the first. If it stays at one for a long while, the correct disposition is to retire it here and keep the analysis in `DC`.

**Falsified by** the closure note turning out to have been written after a deletion that was later undone, which would make it a lost-work bug rather than an unchecked assertion.
