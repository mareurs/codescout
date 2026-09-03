---
kind: tracker
status: active
title: 'the blast radius of a write is wider than the set of peers you can see'
owners:
- marius
tags:
- defect-classes
- clusters
- blast-radius-exceeds-visibility
topic: issue clusters and rule promotion
---

## IC-1 — the blast radius of a write is wider than the set of peers you can see

**Slug:** `cluster/blast-radius-exceeds-visibility`
**Claim:** A session's writes reach every peer sharing the filesystem; its peer listing reaches only peers sharing its config profile — and the listing reports that short population as a definite count. What follows is that a session cannot know its own blast radius. What does **not** follow is that coordination is impossible: this entry asserted that until 2026-09-01, and `IC-17` falsified it. See *the falsification* below.
**Members:** `filter={"tags": {"contains": "cluster/blast-radius-exceeds-visibility"}}` — `n=3`, 2026-09-01, by query after the `IC-17` split. Was 18; the 15 members whose defect survives complete enumeration moved. All three that remain are *instrument* bugs, which is what makes the remedy uniform — and is the property the pre-split membership did not have. **+1: `editing-an-artifact-removes-it-from-qdrant-backed-semantic-search`** — and it belongs here on the *instrument* test the split established, not on loudness: the reindex reports every refusal, so nothing is quiet, yet no instrument reports the thing that happened. What the error says is *"set the artifact backend to sqlite-vec, or implement chunk-grain Qdrant"* — a sentence about a feature being unavailable. What actually occurred is that the six artifacts in that run left the semantic index and cannot return, because their chunk ids were re-minted and the write of the new ones was refused. The blast radius is *every artifact anyone edits, forever*; the visibility is one line naming a configuration option. **The read side is the sharper half:** `semantic_find` discards 2476 of 5388 points (46%) on every query — artifact-grain ids that can never match an `artifact_chunk` row — and counts them nowhere, while the adjacent `cap_suppressed` counter proves the surface for saying so already exists. **Count not re-derived for the +1.** **+1, 2026-09-03: `the-gates-first-step-reformats-every-peers-uncommitted-rust`** — and it is this class's cleanest instance so far, because the write is **mandated**. `CLAUDE.md` § *Development Commands* requires `cargo fmt` as step 1 of the gate; `cargo fmt` has no file-list form and formats the whole workspace, so a session doing everything right rewrites in-flight `.rs` files belonging to sessions it cannot enumerate. **Measured twice in one evening, in OPPOSITE directions, between two sessions who had each already written about the hazard** — which is the admission test for this class rather than for an ordinary mistake: care was applied, by both parties, and prevented neither. `.pre-commit-config.yaml:144-171` already solves it for the hook (`pass_filenames: true`, with the measurement and an explicit note about the concurrency window), so the knowledge is in the repo and simply absent from the surface an agent reads before committing. Kept apart from `IC-10` on the remedy test: the missing thing is a scoping protocol over the shared resource, not a provenance channel for reconstructing authorship afterwards.
**Blind party:** the session doing the writing. Not carelessness — it *holds* the listing that would reveal the peer, and the listing is scoped narrower than the sharing. `ListAgents` answering *"Peer sessions (2)"* is a confident small number, which survives review in a way a suspicious zero would not.
**Promotes to:** `OB-3` — *a peer listing is arbitrary with respect to the real population*. `OB-8` and `OB-2` moved to `IC-17` with the split: they are the ownership half, and this entry no longer holds their members.
**Mechanism status:** partial — OS enumeration shipped for `OB-3`, which is this class's whole remedy surface now. `scripts/peer-sessions.sh` is that instrument, and the third member above says what it still does not compare. The unowned-resource mechanisms (`target/`, the working tree, the git index, `entry_high_water_<PREFIX>`) moved to `IC-17` with their bugs.
**Valid:** dated 2026-09-01

What the instances share is not concurrency in the ordinary sense — there is no lock to take, because nothing models the resource as shared at all. `target/debug/codescout` is written by feature set and read by path; the working tree is written by whoever runs an editor and read by whoever runs `git commit -a`; `entry_high_water_IC` is read-modify-written by each host from its own committed copy. In every case a second writer is *representable* and simply not *represented*.

The two directions compound. You cannot build an ownership protocol over peers you cannot enumerate, and the enumeration is scoped to the config profile while the sharing is scoped to the filesystem. That compounding is real where both apply, but the sentence this paragraph used to end on is not: it read *"that is why `cross-account-agents-cannot-see-each-other` … is the reason `peer-commit-captures-another-sessions-working-tree` has no remedy available today"*, and the falsification below refutes exactly that causal link. The capture bug is `IC-17`'s now, and it lacks a remedy for a reason of its own — the resource carries no owner field — not because its peer was unlistable.

The two **suspected, not proven** members — `workspace-read-only-flips-mid-session` and `sdd-ledger-and-catalog-rows-vanished` — moved to `IC-17` in the split, and carry their caveat with them. Both are unexplained state changes with no actor found in the owning session's own history, which is what an unseen peer looks like from inside. Neither has a peer identified, so neither class is credited with them as evidence.

**Falsified by** a member whose defect survives a complete and correct listing — that member belongs in `IC-17`. (This entry's original falsifier, stated in these terms, fired on 2026-09-01 and moved 15 of 18 members. The replacement above is the same test applied per-member rather than to the class.)

**This class was demonstrated during its own writing.** The open corpus grew from 30 to 31 files while this entry was being drafted — a peer session in the same checkout filed `peer-sessions-never-compares-start-time-to-build-time`, and the measuring session had no signal that its count had changed. The new file is also a member on its own merits: `scripts/peer-sessions.sh` prints each peer's start time but never compares it to the served binary's build time, so **9 of 13 live processes serving pre-rebuild bytes read as healthy** (measured 2026-08-31T21:47). Same shape as `listagents-omits-cross-profile-sessions` — a peer instrument presenting an incomplete characterisation as a sufficient one.

**This entry's falsification condition FIRED on 2026-09-01, and the class should split.** The
condition was stated here as: *an instance where the writing session could enumerate the peer
and still collided — that would move the defect from visibility to coordination and split this
class in two.*

It fired **four times in 34 minutes**. Two sessions enumerated each other — `ListAgents` named
both, and they exchanged eight messages about this exact mechanism — and collided at
`e0525462` (23:53), `3a5aec7a` (23:55), `1b40dabd` (00:06) and `77d4da06` (00:27). Every one
post-dates mutual enumeration; the fourth landed with a warning message **in flight**.
Enumeration was complete **for this pair** and changed nothing.

**Narrowed 2026-09-01, and the narrowing is confirming rather than damaging.** Enumeration was
not complete as a *population*. Measured on this host, in units matched to the question this
entry asks — *can the writing session see the peers who can reach its files?* — **`ListAgents`
returns 1 of the 4 live peer sessions whose `cwd` is inside this checkout**, while
simultaneously reporting 3 peers in an unrelated checkout. The instrument is wrong in both
directions at once: 75% of the peers who share this working tree are invisible, and 75% of what
it does report cannot reach it.

The falsifier needs only the **pair**, and the pair was mutually enumerable, so `IC-17` stands
on unchanged evidence. But the sentence above, read as a population claim, is refutable from
this project's own data — and a reader who refuted it would discard a correct partition.

Two consequences, both favourable to the split and neither available by argument. A 1-of-4
instrument makes **this** class considerably worse than the entry assumed, so the measurement
strengthens `IC-1` and leaves `IC-17` untouched — two halves of a former single class moving
independently under new evidence is what a correct split looks like. And the colliding pair sat
inside the visible quarter, so the falsification ran on the *most favourable sample available
for coordination* and coordination failed four times regardless.

*(Supersedes a `4 of 20` figure cited here for six minutes. That ratio compared a profile-scoped
instrument against a raw socket-file count of which 7 were dead processes — a units mismatch
whose danger is that 20% and 25% are close enough to read as agreement. Derivation and the full
count table: `cluster-promotion-session-log:F-3`.)*

**What that breaks is the *therefore* in this entry's own Claim.** The claim reads *"its peer
listing reaches only peers sharing its config profile. Coordination is **therefore** impossible
by construction"* — asserting the short listing is what makes coordination impossible. It is
not. Coordination failed with the listing complete, because knowing **who** your peer is does
not tell you **which lines in a shared tree are theirs**: `git status` returns a correct,
complete listing with nothing marking whose changes are whose. The binding constraint is a
missing **ownership field**, not a missing peer.

**So the two halves need separating, and they have different remedies** — the same test that
kept `IC-1`/`IC-2` and `IC-3`/`IC-15` apart. *Visibility*: the peer cannot be enumerated —
`cross-account-agents-cannot-see-each-other`, `listagents-omits-cross-profile-sessions`,
`peer-sessions-never-compares-start-time-to-build-time`; remedy is a better instrument, and
`OB-3` already carries it. *Coordination/ownership*: the peer **is** enumerable and the shared
resource has no owner — the six peer-capture instances, `entry_high_water_<PREFIX>`, `target/`;
remedy is isolating the resource, and `OB-8` carries it.

**Both sides of the split are now taken.** `OB-8` landed at `d710e58d`; `IC-17` was minted on
2026-09-01 and the 15 ownership members re-tagged through the catalog, verified
catalog-against-disk — 15 carrying the new slug, 3 the old, and the 3 are the instrument bugs
named above.

**The timing was re-verified independently before the split, because the whole argument rests
on it.** The reporting session put mutual enumeration at ~23:51; this session's own transcript
puts its `ListAgents` call at 2026-08-31T20:19:02Z — **23:19 local** — and its first outbound
message at 23:23. So the margin to the first capture is 34 minutes, not 2, and three of the
four captures are this session's own. Re-verification widened the evidence rather than
weakening it, which is the outcome to expect least often and to record most carefully.

**`IC-10`'s warning applies, and is discharged here rather than waived.** A re-partition is
exactly when a steady-looking count stops meaning what it did. `IC-1` reads n=3 today and read
n=18 yesterday, and **no bug was added, removed, fixed, or re-examined** — only the boundary
moved. Any trend drawn across 2026-09-01 on this row or `IC-17`'s is measuring this edit.
