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
**Members:** `filter={"tags": {"contains": "cluster/record-asserts-an-unchecked-completion"}}` **New member 2026-09-02: `sendmessage-returns-success-for-a-message-held-for-approval`** — `SendMessage` returns `{"success": true, "… → <name>"}` for a message **held for the recipient user's approval and never delivered**. The return is a record asserting a completed action; the `[Cross-session delivery notice]` contradicting it arrives **asynchronously, one or more turns later**. **The variant worth naming, because it is not the class's usual shape:** here a re-check *does* eventually arrive — it is simply later than the window in which the assertion is acted on, which is functionally identical to none. Measured cost: a claim that a mutation window had been announced to a peer was committed at `8dee33a8` on the strength of the success return, and was false when written. The distinguishing test still holds — at the moment of use, nothing re-checks, and the tool offers no field distinguishing *accepted for delivery* from *delivered*. — `n=5`, 2026-08-31, by query after archive backfill.
**Blind party:** the reader of the record, who has no signal distinguishing a verified closure from an asserted one. The author is not blind — they simply wrote what they intended to do.
**Promotes to:** `DC` — `docs/trackers/claim-decay.md`, whose `undecidable-green` and `premature-assertion` types already name this shape. This class is the bug-corpus entry point to that ledger, not a competitor.
**Mechanism status:** none yet — **not checked against the code as of 2026-09-02**, so read this as an open question rather than an established absence.
**Valid:** dated 2026-08-31

One member, kept because the instance is unusually instructive. `bench-worktree-deletion-recorded-as-done-never-happened`: an archived bug file is `status: fixed`, `closed: 2026-08-16`, and its `## Fix` section states the worktree *"was removed with `git worktree remove --force .worktrees/bench`"*, closing with *"174 MB reclaimed, 163 MB of it regenerable `.codescout` index state."* The directory is still on disk. It is 174 MB, of which 163 MB is `.codescout`.

**The numbers match the live directory exactly, and that is the finding.** They were measured — correctly — *before* the deletion, then written in the past tense. So the record is not fabricated and not sloppy; it is a true measurement placed under a false verb. No plausibility check catches it, because every figure in it is right.

This is filed as a class of one deliberately rather than folded into `DC`. The threshold is not met and it will not promote on its own, but the *bug corpus* is where instances arrive, and a class with a defined slug is what lets the second instance find the first. If it stays at one for a long while, the correct disposition is to retire it here and keep the analysis in `DC`.

**Falsified by** the closure note turning out to have been written after a deletion that was later undone, which would make it a lost-work bug rather than an unchecked assertion.
