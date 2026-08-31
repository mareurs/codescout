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
entry_high_water_IC: 9
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
| IC-1 | the blast radius of a write is wider than the set of peers you can see | `blast-radius-exceeds-visibility` | 10 | `OB` (OB-2, OB-3) | partial |
| IC-2 | a gate keyed on an event it cannot observe substitutes a proxy | `gate-keyed-on-unobservable-event` | 6 | `OB` (OB-4) | none yet |
| IC-3 | declaration is not execution | `declared-not-wired` | 3 | `OB` (OB-5 residual) | none yet |
| IC-4 | config propagation is additive | `config-propagation-is-additive` | 3 | not yet — routing unsettled | none yet |
| IC-5 | the reproduction environment is not the gating environment | `repro-env-diverges-from-gate-env` | 3 | not yet — one subsystem | none yet |
| IC-6 | an addressing scheme with no escape hatch | `addressing-without-an-escape-hatch` | 2 | not yet — below threshold | shipped (partial) |
| IC-7 | lazy warm-up bills the first caller | `lazy-warmup-bills-the-first-caller` | 3 | not yet — 2 of 3 unconfirmed | shipped (partial) |
| IC-8 | a record asserts a completed action nothing re-checked | `record-asserts-an-unchecked-completion` | 1 | `DC` | none yet |
| IC-9 | an assertion over environment-controlled text is satisfiable by accident | `assertion-satisfiable-by-accident` | 1 | not yet — below threshold | designed |

**Three of nine clear the promotion threshold** (IC-1, IC-2, IC-3) and all three route to
`OB`, where two-and-a-half already have partial entries. The six that do not clear it fail on
different conditions — subsystem spread (IC-5), count (IC-6, IC-8, IC-9), premise confidence
(IC-7), unsettled routing (IC-4) — and each records which, so the next reader does not
re-derive the judgement under a rule of their own choosing.

**Coverage, 2026-08-31:** 32 of 32 open bug files carry exactly one `cluster/` tag; the
catalog query returns the same counts as the on-disk tags. The archive (494 files) is not yet
backfilled, so every `n` above is a floor — the `concurrency` topic tag alone reaches 14 rows
once `archive/` is included, against 10 live in IC-1.
## IC-1 — the blast radius of a write is wider than the set of peers you can see

**Slug:** `cluster/blast-radius-exceeds-visibility`
**Claim:** A session's writes reach every peer sharing the filesystem; its peer listing reaches only peers sharing its config profile. Coordination is therefore impossible by construction, and the listing reports the short population as a definite count.
**Members:** `filter={"tags": {"contains": "cluster/blast-radius-exceeds-visibility"}}` — n=10 (8 confirmed, 2 consistent-but-unproven), 2026-08-31, by read-through; the query is authoritative once backfill lands.
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
**Members:** `filter={"tags": {"contains": "cluster/gate-keyed-on-unobservable-event"}}` — n=6, 2026-08-31, by read-through; the query is authoritative once backfill lands.
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
**Members:** `filter={"tags": {"contains": "cluster/declared-not-wired"}}` — n=3, 2026-08-31, by read-through; the query is authoritative once backfill lands.
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
**Members:** `filter={"tags": {"contains": "cluster/config-propagation-is-additive"}}` — n=3, 2026-08-31, by read-through; the query is authoritative once backfill lands.
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
**Members:** `filter={"tags": {"contains": "cluster/repro-env-diverges-from-gate-env"}}` — n=3, 2026-08-31, by read-through; the query is authoritative once backfill lands.
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
**Members:** `filter={"tags": {"contains": "cluster/addressing-without-an-escape-hatch"}}` — n=2, 2026-08-31, by read-through; the query is authoritative once backfill lands.
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
**Members:** `filter={"tags": {"contains": "cluster/lazy-warmup-bills-the-first-caller"}}` — n=3, 2026-08-31, by read-through; the query is authoritative once backfill lands.
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
**Members:** `filter={"tags": {"contains": "cluster/record-asserts-an-unchecked-completion"}}` — n=1, 2026-08-31, by read-through; the query is authoritative once backfill lands.
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
**Members:** `filter={"tags": {"contains": "cluster/assertion-satisfiable-by-accident"}}` — n=1, 2026-08-31, by read-through; the query is authoritative once backfill lands.
**Blind party:** `none — ordinary design defect`, but with an unusually strong *detection* asymmetry: at ~1-in-800 the failure is unreachable by local reproduction, so the author's evidence is necessarily circumstantial. The file records that honestly in its own `unverified:` field.
**Promotes to:** `not yet` — n=1. When it moves, the target is `I` (`docs/trackers/test-escape-hardening.md`), because the remedy is a standing grep rather than a rule anyone remembers.
**Mechanism status:** `designed` — the member names the check: *negative `contains` assertions over anything that interpolates a `Path`*. Nobody has run it corpus-wide.
**Valid:** dated 2026-08-31

The single member states its own general form better than a summary would: *"This is not 'a flaky test'; it is an assertion whose input contains environment-controlled text. Any `!haystack.contains(needle)` where the haystack embeds a path, a hostname, a timestamp or a temp name has the same defect, and it always fails open."*

The direction matters and is the reason this is not a duplicate of `CLAUDE.md` § *Testing Discipline*'s monotone rule. That rule says an assertion cannot detect a change it is monotone under, and prescribes mutating the other way. This class is narrower and concerns the **haystack** rather than the assertion's direction: the positive form (`assert contains`) is safe here, because a coincidental match makes it pass *when it should already pass*. Only the negative form can be satisfied by an accident that the property being tested does not license. The two are complements, and the monotone rule is the more general of the two.

Kept as a class of one for the same reason as `IC-8`: the bug corpus is where the second instance will arrive, and a defined slug is what lets it find the first. The prescribed grep is cheap enough that running it once would either promote this class or close it.

**Falsified by** the corpus grep returning no other negative `contains` over interpolated paths, which would make this a one-off rather than a class.

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
