---
id: '1b5a080fe2efcb6b'
kind: tracker
status: active
title: Issue Clusters — the defect class a bug instantiates (IC-N)
owners:
- marius
tags:
- defect-classes
- issues
- clusters
- promotion
- mineable
topic: issue clusters and rule promotion
entry_prefix: IC
entry_high_water_IC: 11
---

> **Prefix:** `IC-N` — one **defect class** the bug corpus instantiates. Declared ledger; the
> `IC` namespace is project-wide (`docs/TAXONOMY.md`).

## What this ledger is for

`docs/issues/` answers *what is broken*. `docs/trackers/open-issue-work-queue.md` (`BL-N`)
answers *what to pick up next*. Neither answers **what these bugs have in common** — and that
is the question an architectural problem is visible in.

Each entry here is a **class**, stated as a claim that could be false. Membership is carried by
a reserved `cluster/<slug>` tag in each bug file's frontmatter, so a class's instances are a
query rather than a list:

```
artifact(action="find", kind="bug",
         filter={"tags": {"contains": "cluster/<slug>"}})
```

**This ledger never lists its members.** That is deliberate, and it is the whole reason this
surface exists rather than the three that preceded it. `docs/issues/INDEX.md` listed files and
was retired 2026-05-18. `open-issue-work-queue.md` § *Sequencing notes* listed `BL-1`, `BL-2`,
`BL-6`, `BL-8`, `BL-11`, `BL-12`, `BL-16` as two clusters — all seven are now `done` or
`done, archived`, and the prose still names them. A member list is a fact about an instant, the
same defect `docs/TAXONOMY.md` records for a bare SHA and a bare ordinal. A query is re-evaluated
on read and cannot go stale.

So: a `**Members:**` line carries the query, plus `n=<count>` and the date it was run. Trust the
query; re-run it before trusting the count.

Design: `docs/superpowers/specs/2026-08-31-issue-clusters-design.md`.

## The entry shape

Seven fields, and nothing else is required.

| field | content |
|---|---|
| `**Slug:**` | the `cluster/<slug>` tag value — the closed-set entry the gate checks against |
| `**Claim:**` | the mechanism, stated so it can be false |
| `**Members:**` | the query, plus `n=<count>` and the date it was run |
| `**Blind party:**` | who structurally cannot see it, and why — or `none — ordinary design defect` |
| `**Promotes to:**` | target surface, per the routing table below |
| `**Mechanism status:**` | `none yet` \| `designed` \| `shipped (<what>)` — borrowed from `OB` |
| `**Valid:**` | `invariant` \| `dated YYYY-MM-DD` \| `conditional — <event>` |

A **slug is claim-shaped, never topic-shaped**: `blast-radius-exceeds-visibility`, not
`concurrency`. A topic slug re-creates the tag soup this replaces; a claim slug can be false,
which is what makes the cluster promotable.

**One `cluster/` tag per bug file.** A bug spanning two classes names the one whose *mechanism*
it instantiates and cites the other in prose. Multi-membership makes counts non-additive, and
the counts are what drive promotion.

## How a cluster becomes a rule

**Threshold: three or more instances spanning two or more subsystems.**

Three is the count at which this corpus has repeatedly noticed itself unaided — three separate
open bug files carry a sentence of the form *"this is the third instance today of one
mechanism"*. The second condition is the load-bearing one: three bugs in **one** subsystem are a
broken subsystem and belong in a bug file; three across **two** are a mechanism and belong in a
rule.

`**Blind party:**` picks the target. **No new rule surface is created** — the space is covered:

| the cluster… | promotes to |
|---|---|
| has a blind party **and** fails with a plausible answer rather than an error | `OB` — `docs/trackers/observer-blindness.md` |
| needs a runtime gate or hook | `H` — `docs/trackers/codescout-usage-hookify.md`, or `I` in `docs/trackers/test-escape-hardening.md` |
| holds across every project, tool and model for this operator | `OP` — `docs/trackers/operator-rules.md` |
| is codescout-specific engineering discipline | `CLAUDE.md` |
| is a written claim that decayed with no repair trigger | `DC` — `docs/trackers/claim-decay.md` |

**A rule with no mechanism is a worklist item, not a rule.** `**Mechanism status:** none yet` is
the honest way to record that, and promotion without the field produces advice — which is the
failure mode this ledger exists to avoid.

**Promotion does not close the cluster.** The entry stays as the standing membership query;
`**Promotes to:**` gains the target's id, and new instances keep landing under the same tag.

## Relationship to the neighbouring trackers — read before adding a row

- **`OB` (observer blindness)** is a *narrower* ledger, not a parent. Its admission test is
  *"would a more careful version of the same party have caught it?"* — if yes it is not an `OB`.
  Several classes here fail that test and are ordinary design defects; they still belong here.
  Where a class passes it, this ledger's entry carries the class **membership query** and the
  `OB` entry carries the **class analysis**. Do not duplicate the analysis.
- **`BL` (open-issue work queue)** is the sequencing axis — readiness, blockers, what to pick up.
  Orthogonal. A bug has one `IC` class and may or may not have a `BL` row.
- **`DC` (claim decay)** takes the *written claim* that rotted. A bug whose defect is a decayed
  record belongs to both: `IC` for the mechanism, `DC` for the missing repair trigger.
- **Derived structure is somebody else's half.** `docs/superpowers/specs/2026-08-30-tracker-grain-and-corpus-topology-design.md`
  Layer A finds hubs by deriving them from `cites` edges. That method sees only what authors
  linked, and bug files barely link — 3 of 30 open files carry a non-empty `related:`, two of
  them pointing into `archive/`. Mechanism similarity is declared at file-open time or it is not
  available at all.

## How to mine this

```
# every live bug in one class, archive included
artifact(action="find", kind="bug", filter={"tags": {"contains": "cluster/<slug>"}})

# the same class, actionable only
artifact(action="find", kind="bug", filter={"and": [
  {"tags": {"contains": "cluster/<slug>"}},
  {"status": {"in": ["open", "investigating"]}}]})

# classes owed a rule: n >= 3 and Promotes to: is still `not yet`
grep -A6 '^## IC-' docs/trackers/issue-clusters.md | grep -B4 'not yet'

# a bug file that declares no class (what the gate will enforce)
grep -L 'cluster/' docs/issues/2026-*.md
```

## Index

> Hand-maintained reading surface. The `## IC-N — <title>` headings are what define the tokens
> and what `link_scan` resolves; this table is for scanning. `n` is a snapshot — re-run the
> `**Members:**` query before trusting it.

| id | class | slug | n | promotes to | mechanism |
|---|---|---|---:|---|---|
| IC-1 | the blast radius of a write is wider than the set of peers you can see | `blast-radius-exceeds-visibility` | 18 | `OB` (OB-2, OB-3) | partial |
| IC-2 | a gate keyed on an event it cannot observe substitutes a proxy | `gate-keyed-on-unobservable-event` | 16 | `OB` (OB-4) | none yet |
| IC-3 | declaration is not execution | `declared-not-wired` | 20 | `OB` (OB-5 residual) | none yet |
| IC-4 | config propagation is additive | `config-propagation-is-additive` | 8 | not yet — routing unsettled | none yet |
| IC-5 | the reproduction environment is not the gating environment | `repro-env-diverges-from-gate-env` | 11 | **re-adjudicate** — its own condition fired | none yet |
| IC-6 | an addressing scheme with no escape hatch | `addressing-without-an-escape-hatch` | 27 | **re-adjudicate** — now the largest class | shipped (partial) |
| IC-7 | lazy warm-up bills the first caller | `lazy-warmup-bills-the-first-caller` | 4 | not yet — 2 of 4 unconfirmed | shipped (partial) |
| IC-8 | a record asserts a completed action nothing re-checked | `record-asserts-an-unchecked-completion` | 5 | `DC` | none yet |
| IC-9 | an assertion over environment-controlled text is satisfiable by accident | `assertion-satisfiable-by-accident` | 3 | **re-adjudicate** — count now met | designed |
| IC-10 | authorship on a shared checkout is unrecoverable after the fact | `authorship-unrecoverable-after-the-fact` | 1 | not yet — below threshold | none yet |
| IC-11 | documentation denies a capability the code has since gained | `doc-contradicted-by-code` | 1 | not yet — n=1 taggable | none yet |

**Six of ten now clear the count threshold** (IC-1, IC-2, IC-3, IC-4, IC-5, IC-6), and three of
those changed status on the backfill rather than on new evidence — which is the ledger working:
the counts were floors, and the judgements resting on them were provisional. **IC-5, IC-6 and
IC-9 are flagged `re-adjudicate` rather than promoted**, because promotion is a reading of the
spread as well as the count, and this pass supplied only the count. IC-7 still fails on premise
confidence, IC-8 routes to `DC` regardless of n, and IC-10 is newly opened at n=1.

**IC-10 was split out of IC-1 on the remedy test**, not on a count. `IC-1` wants an ownership
protocol over a shared resource; `IC-10` wants a provenance channel. The
`buddy-compact-banner` bug moved with it, so IC-1's 18 is a different 18 than the backfill
reported — one gained (`nested-hook-state`), one lost. A count that holds steady across a
re-partition is the clearest argument for re-running the query rather than trusting the cell.

**Coverage, 2026-08-31.** The open corpus is no longer maintained by hand: `tests/issue_clusters.rs`
(shipped `522675a6`) fails when any **tracked** file directly under `docs/issues/` declares no
`cluster/` tag, more than one, or a slug this ledger does not define. So open-corpus coverage is
whatever the gate last allowed through, and there is no number here to go stale. It fired on its
first run and caught two files committed within the hour.

The **archive is deliberately outside the gate**: 78 of the 357 files dated 2026-07-01 or later
are tagged and 279 are deliberately untagged, because the classes were derived from the open
backlog and forcing a fit would corrupt the counts promotion reads. A further 137 pre-July files
are unbackfilled. **Every `n` above therefore remains a floor.** Covering the archive would need
an explicit `cluster/unclassified` slug meaning *looked, nothing fits* — a taxonomy decision,
not a gate one.

Four recurring shapes in the untagged 279 reached three or more instances and matched no existing
class — a capped result presented as complete; a guard whose coverage is narrower than its name;
documentation stating a behaviour the code contradicts; and an accepted parameter silently
dropped on some path. **Three remain candidates.** The third was promoted to `IC-11` on
2026-08-31 when a taggable instance arrived and the gate proved to have no escape hatch — there
is no `cluster/unclassified`, so an open bug whose shape is a known-but-unadded candidate cannot
be committed at all. Promotion was forced by that, not by the count. Adding the other three is
still a taxonomy decision, not a backfill one.
## IC-1 — the blast radius of a write is wider than the set of peers you can see

**Slug:** `cluster/blast-radius-exceeds-visibility`
**Claim:** A session's writes reach every peer sharing the filesystem; its peer listing reaches only peers sharing its config profile. Coordination is therefore impossible by construction, and the listing reports the short population as a definite count.
**Members:** `filter={"tags": {"contains": "cluster/blast-radius-exceeds-visibility"}}` — n=18, 2026-08-31, by query after archive backfill (8 confirmed, 2 consistent-but-unproven among the 10 pre-backfill members).
**Blind party:** the session doing the writing. Not carelessness — it *holds* the listing that would reveal the peer, and the listing is scoped narrower than the sharing. `ListAgents` answering *"Peer sessions (2)"* is a confident small number, which survives review in a way a suspicious zero would not.
**Promotes to:** `OB` — `docs/trackers/observer-blindness.md`. Partly there already: `OB-2` (shared `target/` left feature-clobbered) and `OB-3` (a peer listing is arbitrary w.r.t. the real population) are both members of this class seen from the observer side.
**Mechanism status:** partial — OS enumeration shipped for `OB-3`; the gate reorder shipped for `OB-2`. Nothing covers the *write* side: no ownership protocol on `target/`, the working tree, the git index, or the `entry_high_water_<PREFIX>` allocator.
**Valid:** dated 2026-08-31

What the instances share is not concurrency in the ordinary sense — there is no lock to take, because nothing models the resource as shared at all. `target/debug/codescout` is written by feature set and read by path; the working tree is written by whoever runs an editor and read by whoever runs `git commit -a`; `entry_high_water_IC` is read-modify-written by each host from its own committed copy. In every case a second writer is *representable* and simply not *represented*.

The two directions compound. You cannot build an ownership protocol over peers you cannot enumerate, and the enumeration is scoped to the config profile while the sharing is scoped to the filesystem. That is why `cross-account-agents-cannot-see-each-other` is not a nice-to-have adjacent to `peer-commit-captures-another-sessions-working-tree` — it is the reason the latter has no remedy available today.

Two members are **suspected, not proven**: `workspace-read-only-flips-mid-session` and `sdd-ledger-and-catalog-rows-vanished`. Both are unexplained state changes with no actor found in the owning session's own history, which is what an unseen peer looks like from inside. Neither has a peer identified, and they are tagged so the class is not silently credited with them.

**Falsified by** an instance where the writing session *could* enumerate the peer and still collided — that would move the defect from visibility to coordination and split this class in two.

**This class was demonstrated during its own writing.** The open corpus grew from 30 to 31 files while this entry was being drafted — a peer session in the same checkout filed `peer-sessions-never-compares-start-time-to-build-time`, and the measuring session had no signal that its count had changed. The new file is also a member on its own merits: `scripts/peer-sessions.sh` prints each peer's start time but never compares it to the served binary's build time, so **9 of 13 live processes serving pre-rebuild bytes read as healthy** (measured 2026-08-31T21:47). Same shape as `listagents-omits-cross-profile-sessions` — a peer instrument presenting an incomplete characterisation as a sufficient one.

## IC-2 — a gate keyed on an event it cannot observe substitutes a proxy, and the proxy fails silently

**Slug:** `cluster/gate-keyed-on-unobservable-event`
**Claim:** A gate whose condition is an event outside its observation boundary substitutes a proxy — a caller-supplied flag, a monotone stamp, a per-process map — and the substitution fails silently, because a proxy returns a plausible answer rather than an error.
**Members:** `filter={"tags": {"contains": "cluster/gate-keyed-on-unobservable-event"}}` — n=16, 2026-08-31, by query after archive backfill.
**Blind party:** the gate itself, and therefore every reader of its output. The server process cannot see a compaction, a dead hook, a `/clear`, or what a parent session already holds; each of those is a *conversation*-scoped or *harness*-scoped event, and the gate is process-scoped.
**Promotes to:** `OB` — `docs/trackers/observer-blindness.md`. `OB-4` (*a liveness marker with a good hit rate spends the trust it earned*) is this class's rendezvous half already.
**Mechanism status:** none yet for the class. Individual members have partial fixes; nothing addresses the shared shape.
**Valid:** dated 2026-08-31

Every member is one substitution. `workspace(post_compact=true)` cannot check that a compaction happened, so it trusts the caller's flag and clears the whole ledger on a mistaken `/mcp` reconnect. The rendezvous gate cannot check that the companion hook is still alive, so it trusts a monotone stamp that nothing can un-set — and its twin defect is the same stamp never landing, which leaves the gate shut for the life of the process. The subagent ledger cannot see what the parent holds, so 84% of measured subagent sessions re-receive a topic their parent already has. `get_guide` cannot see which section a caller needs, so it serves the topic — or, since section grain shipped, serves the section attached to the response of the call it was meant to inform.

The unifying property is that **none of these fail loudly**. A wrong proxy produces a plausible delivery: an extra guide body, an open gate, a closed gate, a re-cleared ledger. Nothing throws, so nothing downstream fires either. That is what makes the class survive review chains that catch louder bugs, and it is why `rendezvous-slot-never-stamped` could be closed `wontfix` on the reasoning that the failure is invisible — correct about the observation, and exactly the property that should have counted against it.

Note the shape shared with `cluster/blast-radius-exceeds-visibility`: both are *a component reasoning about a scope larger than the one it can observe*. They are kept separate because the remedies differ — that one needs an ownership protocol over a shared resource, this one needs an authoritative signal for an event. Merging them would produce a class too abstract to prescribe anything.

**Falsified by** a member whose proxy failure surfaced as an error rather than a plausible result; that would belong to an ordinary-correctness class instead.

## IC-3 — declaration is not execution — a surface declares a capability production never reaches

**Slug:** `cluster/declared-not-wired`
**Claim:** A surface declares a capability that production never reaches. Every piece is individually correct — the selector, the matcher, the ledger entry, the schema — and no call site connects them, so the declaration reads as a shipped feature and tests pass in isolation.
**Members:** `filter={"tags": {"contains": "cluster/declared-not-wired"}}` — n=20, 2026-08-31, by query after archive backfill.
**Blind party:** the author of the declaration, specifically. They hold the mental model in which the wiring exists — writing `**Serves:** edit_file(path~/.claude)` *is* the act of believing it is served. A more careful version of the same author writes the same line.
**Promotes to:** `OB` — `docs/trackers/observer-blindness.md`. `OB-5` already carries this as its *"Known-open residual — declaration is not execution"*; this class is the instance side of that residual and the two should be linked rather than duplicated.
**Mechanism status:** none yet. The remedy is a reachability check — for each declared selector, assert some production path emits it — and nothing implements one.
**Valid:** dated 2026-08-31

`op-4-path-predicate-can-never-fire` and `triggered-operator-rules-route-nothing-in-production` are the pure form: three operator rules declare `binding: triggered` against tools that emit no `selector_key` in production, so `route()` is never called with anything that could match them. The routing mechanism exists and is unit-tested; the tests construct the selector the production path never produces.

`cli-doctor-exposes-no-fix-flag` is the same shape at a different seam. `librarian(action="doctor", fix=…)` offers six repairs; `codescout doctor`'s clap struct offers none, so every repair is unreachable from the command line. Its own body names this as *"the third instance today of one mechanism: the CLI keeps its own clap structs and hand-marshals into the MCP tool's JSON"* — which makes it a member of both this class and a narrower CLI/MCP parity family. It is filed here because the *defect* is the unreachable capability; the hand-marshalling is the mechanism by which it became unreachable.

The reason ordinary testing does not catch this class is structural rather than accidental: a unit test constructs its own inputs, so it exercises the matcher with a selector production never emits, and passes. The test is not weak; it is *scoped to the half that works*. Only a check that starts from the production emission side can see it — which is the same shape as `CLAUDE.md` § *Testing Discipline*'s "name the concrete caller that reaches it".

**Falsified by** a member where the wiring existed and the declaration was merely wrong, which is an ordinary bug rather than this class.

## IC-4 — config propagation is additive — updates land, removals and renames do not

**Slug:** `cluster/config-propagation-is-additive`
**Claim:** Configuration propagates as an overlay rather than a replace. An added or changed key lands; a **removed** key, or a **renamed** path, does not — and the change that does land is read as confirmation that the whole edit applied.
**Members:** `filter={"tags": {"contains": "cluster/config-propagation-is-additive"}}` — n=8, 2026-08-31, by query after archive backfill.
**Blind party:** the operator who made the edit. They verify the change they can see — the new value — and that verification is *positive evidence for the wrong proposition*. Nothing in the successful check distinguishes "the edit applied" from "the additive half of the edit applied".
**Promotes to:** `not yet` — the class has three instances across three subsystems (MCP env, shell env, git config) and clears the threshold, but the routing field is unsettled: it is arguably `H` (a hook that diffs intended vs effective config) rather than `OB`. Decide before promoting.
**Mechanism status:** none yet.
**Valid:** dated 2026-08-31

`mcp-reconnect-applies-env-updates-but-not-env-deletions` is the sharpest statement of the class, because one edit made two changes and the outcomes diverged: `CODESCOUT_BM25_BOOST` changed from `3.0` to `5.0` and landed; `CODESCOUT_QUERY_PREFIX` was removed and is still set. The bug file's own title names the trap — *"the change that lands falsely confirms the one that did not"*.

`stale-model-dir-env-masked-by-shell` is the same asymmetry one layer down, and it is worth noting that its two stale values were masked for *different* reasons, which is why a single-cause fix would have missed one. `core-hookspath-points-at-pre-rename-path` is the rename form: `.git/config` still points `core.hooksPath` at the repository's former name, so git finds no hooks at all and `.pre-commit-config.yaml` silently never fires — while `CONTRIBUTING.md` documents the hook's behaviour in the present tense. Its archived sibling (`bench-worktree-gitdir-points-at-pre-rename-path`) is the same mechanism, which is what makes the rename form recurrent rather than incidental.

The generalisation worth extracting: **a config surface that supports removal needs a check on the removal, because the update path is the one everybody exercises.** Silence after a deletion is indistinguishable from success, and the value that remains is a live setting nobody intends.

**Falsified by** an instance where a removal *did* propagate and the failure lay elsewhere.

## IC-5 — the reproduction environment is not the gating environment

**Slug:** `cluster/repro-env-diverges-from-gate-env`
**Claim:** The environment built so failures can be reproduced locally is not the environment that gates. While the two agree the divergence is invisible; when they disagree the local run is authoritative-looking and wrong, and a genuine platform defect is indistinguishable from an environment gap.
**Members:** `filter={"tags": {"contains": "cluster/repro-env-diverges-from-gate-env"}}` — n=11, 2026-08-31, by query after archive backfill.
**Blind party:** `none — ordinary design defect`. A careful engineer comparing wine versions catches this; nobody is structurally prevented from seeing it. Recorded so the class is not mis-promoted to `OB`, whose admission test it fails.
**Promotes to:** `not yet` — clears the count but not the subsystem spread; all three sit in the Windows/wine lane, so under this ledger's threshold rule they are *a broken subsystem*, not a mechanism. Revisit if a fourth lands outside it.
**Mechanism status:** none yet. `scripts/build-windows.sh` could assert the wine version CI packages, which would convert the divergence from silent to loud at ~3 lines.
**Valid:** conditional — a member appears outside the Windows/wine lane

`scripts/build-windows.sh` exists precisely so Windows failures are reproducible without CI round-trips, and that purpose holds only while the two wines behave alike. They do not: `ubuntu-latest` packages wine 9.0, a current dev box runs wine 11.16, and in a single day the gap produced two divergences — one costing a CI cycle, one still costing a skipped test.

The other two members are the downstream cost of that gap rather than separate defects. `wine-lane-flakes-under-load` records three tests that failed together and passed on the next identical run, and its own update narrows the file to one test after CI reproduced two of the three *with a different payload* — a distinction only visible because someone compared the two environments deliberately. `windows-ci-timing-flakes` is `zombie` for the honest reason: both flakes resolve only by recurring, so no amount of effort reaches them.

This class is deliberately kept even though it does not currently promote. Its value is the **threshold rule's worked negative example**: three instances is not enough when they share a subsystem, and recording that judgement is what stops the next reader counting to three and promoting anyway.

**Falsified by** the two wine versions being shown to agree on the divergent cases, which would relocate the defect to the tests themselves.

## IC-6 — an addressing scheme with no escape hatch and no disambiguator

**Slug:** `cluster/addressing-without-an-escape-hatch`
**Claim:** An addressing scheme interprets every token in its namespace and provides no way to write one literally, or to disambiguate two that collide. The scheme is correct on every input it accepts; the defect is the input it makes unrepresentable.
**Members:** `filter={"tags": {"contains": "cluster/addressing-without-an-escape-hatch"}}` — n=25, 2026-08-31, by query after archive backfill.
**Blind party:** `none — ordinary design defect`. The gap is visible to anyone who tries the unrepresentable input; nobody is structurally prevented from seeing it. Recorded so it is not mis-promoted to `OB`.
**Promotes to:** `not yet` — n=2, below threshold.
**Mechanism status:** `shipped (partial)` — `edit_markdown` and `artifact(get)` gained an `occurrence` selector, closing the heading half for librarian-managed files. The `link_scan` half has no escape syntax at all.
**Valid:** dated 2026-08-31

Two instances, one shape. `identical-headings-make-a-section-permanently-unaddressable`: two byte-identical headings make both sections unreachable, and the refusal is *correct* — the query is genuinely ambiguous. The defect was that no disambiguator existed, compounded by an error hint prescribing `start_line`/`end_line` parameters absent from the schema of a tool gated off `.md` files entirely. A wrong remedy in an error message is worse than none, because it costs the reader a round trip before they distrust it.

`an-entry-id-cannot-be-mentioned-without-citing-it`: `link_scan` derives a `cites` edge from any `\b[A-Z]{1,3}-\d+\b` token in a body, and there is no way to write an id as a literal. Inline backticks — the notation an author reaches for to mean *"this is a token, not a reference"* — are deliberately scanned. The only escape is a fenced block, which cannot sit mid-sentence. The consequence is narrow and self-demonstrating: a document about id allocation cannot name an id without citing it, which is why this ledger's own template writes its example inside a fence.

The generalisable point is that **an interpreting scheme owes a quoting mechanism**, and the cost of omitting one is paid by exactly the documents that discuss the scheme — the ones most worth writing.

**Falsified by** a third instance in an unrelated addressing scheme, which would raise this to threshold rather than falsify it; falsification proper would be showing the two members share no mechanism.

## IC-7 — lazy warm-up bills the first caller, and the bill can look like a negative result

**Slug:** `cluster/lazy-warmup-bills-the-first-caller`
**Claim:** Work deferred to first use is charged to whichever call arrives first, and that caller has no way to distinguish "cold" from "broken" — so the bill can surface as a timeout or as a negative result that looks like an answer.
**Members:** `filter={"tags": {"contains": "cluster/lazy-warmup-bills-the-first-caller"}}` — n=4, 2026-08-31, by query after archive backfill.
**Blind party:** the caller. It receives `symbol not found` or `0 matches` — well-formed, plausible results that are *indistinguishable from the true answer*. This is the property `docs/adrs/2026-08-27-negative-results-name-their-scope.md` exists to address, which makes the class an argument for that ADR rather than a new one.
**Promotes to:** `not yet` — the count is met but two of three members are `zombie` with unconfirmed root causes, so promoting now would rest a rule on unresolved premises.
**Mechanism status:** `shipped (partial)` — a false-zero guard covers the cold-start path for `references`; the deliberate cold-start reproduction showed the guard *does* fire there, which refuted the original filing.
**Valid:** conditional — either zombie member recurs

`post-compact-flush-leaves-first-nav-call-to-pay-cold-start` is the unambiguous member and the loudest: `workspace(post_compact=true)` flushes every LSP client and returns a hint promising *"no disruption to the session"*, then the first navigation call pays the entire cold start — on this 1697-file crate, past the 60s tool timeout, dying with no result. The mechanism is not in dispute; the hint's claim is simply false for the first caller.

The other two are `zombie` and are held here **with their uncertainty intact**, which is the point of recording them rather than the reason to leave them out. `references-symbol-not-found-while-lsp-warms` was filed with a root cause that has since been **refuted** — a deliberate cold-start reproduction showed the cold path produces the *guarded* false-zero, i.e. the opposite of the filed mechanism. `symbols-overview-include-body-ignored-and-search-flake` has one bug fixed and one mitigated-and-instrumented, unrecurred, root cause unconfirmed.

So the class is real and its evidence is thin in a specific way: one confirmed instance and two whose mechanism is unproven. That is exactly the state the threshold's second condition is meant to catch, and the honest disposition is to hold rather than promote. If either zombie fires again with instrumentation attached, this becomes promotable in one step.

**Falsified by** the two zombies resolving to unrelated mechanisms, which would leave one instance and no class.

## IC-8 — a record asserts a completed action that nothing re-checked

**Slug:** `cluster/record-asserts-an-unchecked-completion`
**Claim:** A record states that an action completed, and nothing is scheduled to re-check it. The assertion is written at the moment of *intent* and read forever after as *outcome*, so a step that silently did not happen leaves a closure note that reads exactly like a successful one.
**Members:** `filter={"tags": {"contains": "cluster/record-asserts-an-unchecked-completion"}}` — n=5, 2026-08-31, by query after archive backfill.
**Blind party:** the reader of the record, who has no signal distinguishing a verified closure from an asserted one. The author is not blind — they simply wrote what they intended to do.
**Promotes to:** `DC` — `docs/trackers/claim-decay.md`, whose `undecidable-green` and `premature-assertion` types already name this shape. This class is the bug-corpus entry point to that ledger, not a competitor.
**Mechanism status:** none yet.
**Valid:** dated 2026-08-31

One member, kept because the instance is unusually instructive. `bench-worktree-deletion-recorded-as-done-never-happened`: an archived bug file is `status: fixed`, `closed: 2026-08-16`, and its `## Fix` section states the worktree *"was removed with `git worktree remove --force .worktrees/bench`"*, closing with *"174 MB reclaimed, 163 MB of it regenerable `.codescout` index state."* The directory is still on disk. It is 174 MB, of which 163 MB is `.codescout`.

**The numbers match the live directory exactly, and that is the finding.** They were measured — correctly — *before* the deletion, then written in the past tense. So the record is not fabricated and not sloppy; it is a true measurement placed under a false verb. No plausibility check catches it, because every figure in it is right.

This is filed as a class of one deliberately rather than folded into `DC`. The threshold is not met and it will not promote on its own, but the *bug corpus* is where instances arrive, and a class with a defined slug is what lets the second instance find the first. If it stays at one for a long while, the correct disposition is to retire it here and keep the analysis in `DC`.

**Falsified by** the closure note turning out to have been written after a deletion that was later undone, which would make it a lost-work bug rather than an unchecked assertion.

## IC-9 — an assertion over environment-controlled text is satisfiable by accident, and fails open

**Slug:** `cluster/assertion-satisfiable-by-accident`
**Claim:** An assertion whose haystack embeds environment-controlled text — a path, a tempdir name, a hostname, a timestamp — can be satisfied by coincidence. It fails **open**: it passes on almost every machine and almost every run, so the green tick is evidence of luck rather than of the property.
**Members:** `filter={"tags": {"contains": "cluster/assertion-satisfiable-by-accident"}}` — n=3, 2026-08-31, by query after archive backfill.
**Blind party:** `none — ordinary design defect`, but with an unusually strong *detection* asymmetry: at ~1-in-800 the failure is unreachable by local reproduction, so the author's evidence is necessarily circumstantial. The file records that honestly in its own `unverified:` field.
**Promotes to:** `not yet` — n=1. When it moves, the target is `I` (`docs/trackers/test-escape-hardening.md`), because the remedy is a standing grep rather than a rule anyone remembers.
**Mechanism status:** `designed` — the member names the check: *negative `contains` assertions over anything that interpolates a `Path`*. Nobody has run it corpus-wide.
**Valid:** dated 2026-08-31

The single member states its own general form better than a summary would: *"This is not 'a flaky test'; it is an assertion whose input contains environment-controlled text. Any `!haystack.contains(needle)` where the haystack embeds a path, a hostname, a timestamp or a temp name has the same defect, and it always fails open."*

The direction matters and is the reason this is not a duplicate of `CLAUDE.md` § *Testing Discipline*'s monotone rule. That rule says an assertion cannot detect a change it is monotone under, and prescribes mutating the other way. This class is narrower and concerns the **haystack** rather than the assertion's direction: the positive form (`assert contains`) is safe here, because a coincidental match makes it pass *when it should already pass*. Only the negative form can be satisfied by an accident that the property being tested does not license. The two are complements, and the monotone rule is the more general of the two.

Kept as a class of one for the same reason as `IC-8`: the bug corpus is where the second instance will arrive, and a defined slug is what lets it find the first. The prescribed grep is cheap enough that running it once would either promote this class or close it.

**Falsified by** the corpus grep returning no other negative `contains` over interpolated paths, which would make this a one-off rather than a class.

## IC-10 — authorship on a shared checkout is unrecoverable after the fact, so every party infers it from proximity

**Slug:** `cluster/authorship-unrecoverable-after-the-fact`
**Claim:** On a shared checkout there is no attribution channel, so authorship cannot be recovered after the fact. Every party therefore infers it from proximity — who else was active, which file appeared when — and proximity is not evidence.
**Members:** `filter={"tags": {"contains": "cluster/authorship-unrecoverable-after-the-fact"}}` — n=1, 2026-08-31. The seed narrative below is not a bug file and is not counted.
**Blind party:** every party equally, which is what makes it different from an ordinary mistake. The information does not exist to be careless with: `git` collapses all sessions into one author string, and an untracked file carries no origin at all.
**Promotes to:** `not yet` — n=1. When it moves the target is likely `H` (a provenance channel is a mechanism, not a discipline), not `OB`.
**Mechanism status:** none yet.
**Valid:** dated 2026-08-31

**Split from `IC-1` deliberately, on the remedy test.** `IC-1` claims a write reaches further than the set of peers you can see, and its remedy is an ownership protocol over the shared resource. This class claims something narrower and later: once the write has happened, *who did it* is not recoverable. Its remedy is a provenance channel. Same substrate, different missing thing — which is the same test that keeps `IC-1` and `IC-2` apart despite both reducing to "a component reasoning about a scope it cannot observe". `buddy-compact-banner-names-a-peers-session-as-your-own` was filed under `IC-1` and is moved here: its defect is that `from=<sid>` names another live session as your own predecessor, which is misattribution, not blast radius.

**Seed evidence — an exchange between two sessions produced three misattributions, all while reasoning about this class.** Sessions `codescout-kat` and `codescout-23`, 2026-08-31, both actively working the `IC` ledger:

1. `codescout-kat` told `codescout-23` "your nested-hook-state bug reasons that session 3a6d634e… wrote `.buddy/`". The reasoning is in that file, but the file is not `codescout-23`'s.
2. `codescout-kat` warned `codescout-23` that "your untracked librarian-runtime bug file" would red the cluster gate. Also not theirs.
3. `codescout-23`, correcting the above, argued the file was `codescout-kat`'s because *"your own `2ed2e716` calls it 'my nested-hook-state bug'"*. `codescout-kat` authored exactly two commits that session, `351836a8` and `522675a6`; `2ed2e716` is neither.

The file belongs to a third session neither had enumerated. `git log` shows why the dispute was unresolvable from inside it: `2ed2e716`, `e14b230e`, `351836a8` and `522675a6` all read the same author and email, because git has no session dimension — the field is a constant and carries zero information. The one channel that did work was accidental: `.buddy/by-ppid/<pid>/session_id` on disk, which exists for unrelated reasons and is untracked.

Note the pattern is `OB-1`'s — *"the author, specifically"*. All three attributions were made by parties who had just read the evidence, in messages *about* attribution failure. Knowing the class prevented none of them, which is the standing argument against answering this kind of defect with care rather than mechanism.

**Falsified by** an attribution dispute on a shared checkout that a party could settle from committed state alone.

## IC-11 — documentation denies a capability the code has since gained, because the prose was true when written

**Slug:** `cluster/doc-contradicted-by-code`
**Claim:** A document states a behaviour the code contradicts. The statement was *true when written*; the code later gained or lost the capability. Nothing checks prose against code systematically, and the corrective pass that *does* happen is a hand-enumerated sweep whose completeness is unfalsifiable — it reports the surfaces it changed, never the ones it missed. Unlike a wrong statement, this defect has no authoring error to find.
**Members:** `filter={"tags": {"contains": "cluster/doc-contradicted-by-code"}}` — n=1, 2026-08-31. Three or more further instances are known to sit in the untagged 279 archived files: this is the third of the four candidate shapes recorded above the `IC-1` entry, promoted to a class here because a taggable instance arrived and the gate has no escape hatch for "looked, nothing fits".
**Blind party:** the *reader*, routed to the document by its own scope claim and given no signal to cross-check. The author of the prose is not blind — they wrote something true. The author of the *code* change is differently blind: gaining a capability gives you no reason to search prose for sentences your feature just falsified.
**Promotes to:** `not yet` — n=1 taggable. The likely target is `DC` (`docs/trackers/claim-decay.md`): a true-when-written claim that silently decayed is that ledger's subject, and this class is the bug-corpus entry point to it rather than a competitor — the same relationship `IC-8` declares.
**Mechanism status:** none yet, and the nearest existing mechanism does not cover it. `librarian(action="audit_doc_refs")` lints *references* — paths, symbols, line numbers, link targets — so a document may cite every path correctly and still assert the opposite of what the code at those paths does. The remedy would have to check claims, not refs.
**Valid:** dated 2026-08-31

Seed instance: `2026-08-31-librarian-runtime-guide-denies-the-augmentation-sidecar`. The served `librarian-runtime` guide states augmentation has *"**No** — there is no on-disk representation"* and that sharing it is *"local-only by design"*. Both sentences were accurate when written. The sidecar shipped as `e799f29d` on 2026-08-30, and a deliberate sweep the **same day** — `e1b91221`, *"state that augmentation shape now travels, in the three places that said otherwise"* — corrected `CLAUDE.md`, `docs/conventions/cross-machine-catalog-resume.md` and `tracker-conventions.md`. Not this guide. So the drift is **one day old**, and the mechanism is an enumeration produced from memory, not neglect: "three places" reads as a finding and is a list. The guide mentions `sidecar`/`expects_augmentation` zero times; `tracker-conventions` mentions them thirteen.

**The cost is not that a reader is misinformed — it is that the reader stops.** A sentence saying a capability does not exist terminates the search that would have found it. Measured downstream the same day: a consumer repo held two augmentations in a machine-local catalog with no sidecar and no declaration, one clone away from silent loss, because the guide consulted for exactly that question said there was nothing to export. `doctor`'s `augmentation_declared_but_absent` could not report it either — that check fires only on a *declared* sidecar that is missing, so undeclared-and-unexported reads identically to nothing-to-declare.

**The same guide, the same section, fifteen days earlier — and it is already tagged.**
`docs/issues/archive/2026-08-16-librarian-runtime-guide-claims-move-preserves-id.md` is
`status: fixed`, and reports that this *same* § *Where catalog state lives* section claimed
`artifact(action="move")` "preserves `id`" when a move necessarily re-keys the row. It
carries the tag `doc-vs-code` — the shape had an informal label before it had a class, which
is the ordinary way a class announces itself. It stays untagged for `cluster/` purposes under
the archive policy, so it does not raise the count. Two of the three open files carrying
`doc-vs-code` are correctly filed under `IC-2`; the tag is a secondary descriptor there, not
the primary defect.

**Kept apart from `IC-3` and `IC-8` on their own falsifiers, not on judgement.** `IC-3` is a surface declaring a capability production never reaches; this is its mirror — production reaches a capability the surface denies — and `IC-3`'s falsifier explicitly ejects the mirror case (*"the wiring existed and the declaration was merely wrong, which is an ordinary bug rather than this class"*). `IC-8` is an assertion written at the moment of intent and read forever after as outcome; this prose was not intent, it was correct observation, which is why no plausibility check catches either one.

**Falsified by** an instance where the documentation was wrong on the day it was written. That is an ordinary authoring error with an author to find, and it does not belong here.

## Template for new entries

```
## IC-N — <the class, stated as a claim>

**Slug:** `cluster/<slug>`
**Claim:** <the mechanism, in mechanism-language>
**Members:** `filter={"tags": {"contains": "cluster/<slug>"}}` — n=<count>, <YYYY-MM-DD>
**Blind party:** <who structurally cannot see it, and why> | `none — ordinary design defect`
**Promotes to:** `not yet` | <target ledger + id>
**Mechanism status:** none yet | designed | shipped (<what>)
**Valid:** dated <YYYY-MM-DD>

<Two or three paragraphs: what the instances share, what a fix would have to change, and
what would falsify the claim.>
```
