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
entry_high_water_IC: 16
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
>
> **When a count moves, re-derive every judgement that quotes it, in the same pass.** The
> 2026-08-31 backfill updated `**Members:**` and this table but not `**Promotes to:**` — the
> field that records the *decision* — so four entries carried a correct count beside a judgement
> reasoned from the superseded one, `IC-6` reading `not yet — n=2, below threshold` while holding
> the largest count in the corpus at 27. Because the ledger's own mining query is
> `grep -B4 'not yet'`, the effect is a class surfacing as owed-a-rule next to a reason telling
> the reader to dismiss it. Repaired for IC-4, IC-5, IC-6, IC-7 and IC-9; the sentence below the
> table carried the same defect, reading "six" through three rewrites because each rewrite listed
> only the classes it was adjudicating.
>
> **Re-derive against `git ls-files docs/issues`, not a bare recursive grep.** `docs/issues/` also
> holds untracked session-log directories (`.buddy/`, `.codescout/`) whose tool logs quote
> `cluster/<slug>` verbatim from the commands that counted them, so a recursive grep can read its
> own measurement back as corpus — observed 2026-08-31 inflating three cells at once. `git
> ls-files` is also the definition `tests/issue_clusters.rs` enforces, so it is the one that
> matches the gate.

| id | class | slug | n | promotes to | mechanism |
|---|---|---|---:|---|---|
| IC-1 | the blast radius of a write is wider than the set of peers you can see | `blast-radius-exceeds-visibility` | 18 | `OB` (OB-2, OB-3) | partial |
| IC-2 | a gate keyed on an event it cannot observe substitutes a proxy | `gate-keyed-on-unobservable-event` | 16 | `OB-6` — promoted 2026-09-01 | designed (exemplar shipped) |
| IC-3 | declaration is not execution | `declared-not-wired` | 20 | `OB` (OB-5 residual) | none yet |
| IC-4 | config propagation is additive | `config-propagation-is-additive` | 8 | `OB` — passes admission test; hook owed | none yet |
| IC-5 | the reproduction environment is not the gating environment | `repro-env-diverges-from-gate-env` | 11 | `H` — six subsystems; mechanism owed | none yet |
| IC-6 | an addressing scheme with no escape hatch | `addressing-without-an-escape-hatch` | 27 | `CLAUDE.md` § Parsers Over a Namespace — **landed** | shipped (partial) |
| IC-7 | lazy warm-up bills the first caller | `lazy-warmup-bills-the-first-caller` | 4 | not yet — 2 of 4 unconfirmed | shipped (partial) |
| IC-8 | a record asserts a completed action nothing re-checked | `record-asserts-an-unchecked-completion` | 5 | `DC` | none yet |
| IC-9 | an assertion over environment-controlled text is satisfiable by accident | `assertion-satisfiable-by-accident` | 1 | not yet — two tags withdrawn as misfits | none yet |
| IC-10 | authorship on a shared checkout is unrecoverable after the fact | `authorship-unrecoverable-after-the-fact` | 1 | not yet — below threshold | none yet |
| IC-11 | documentation denies a capability the code has since gained | `doc-contradicted-by-code` | 1 | not yet — n=1 taggable | none yet |
| IC-12 | transient shared state lies to every reader | `transient-shared-state-lies-to-readers` | 0 | not yet — 1 instance, untagged | none yet |
| IC-13 | a capped result is presented as complete | `capped-result-presented-as-complete` | 0 | not yet — count unverified | none yet |
| IC-14 | a guard's coverage is narrower than its name | `guard-narrower-than-its-name` | 0 | not yet — count unverified | none yet |
| IC-15 | a parameter is accepted then silently dropped | `accepted-parameter-silently-dropped` | 0 | not yet — count unverified | none yet |
| IC-16 | an assertion that cannot fail | `assertion-that-cannot-fail` | 0 | rule ALREADY in `CLAUDE.md`; membership owed | designed |

**Every class at n≥3 clears the count threshold; spread is adjudicated per entry.** Read the `n`
column — that is the derivation, and it cannot go stale when a count moves. This sentence used to
publish the *value* alongside a hand-maintained list of which classes qualified, and it rotted
three times in one evening: each editor updated the list for the classes they were adjudicating
and let the number ride along, so "six of ten" became "six of eleven" became eight, the middle
one never re-derived by anyone. `CLAUDE.md` § *Observer Blindness* prescribes exactly this — ship
a claim's derivation rather than its value, so a reader re-checks it instead of re-deriving it
under a counting rule of their own choosing. **IC-6 is the
first to land its rule** — `CLAUDE.md` § *Parsers Over a Namespace*, 2026-08-31. **IC-5 and IC-6
are now adjudicated rather than flagged**, and both route away from `OB` — each declares
`Blind party: none`, which fails OB's admission test, so the count was never the only thing
stopping them. IC-5 spans six subsystems (cargo feature config, wine, shell env, workspace
resolution, toolchain, ambient embedder config), seven of its eleven members outside the
Windows/wine lane its old note said contained all of them. IC-6 spans five. **IC-4 routes the
other way on the same test**, adjudicated 2026-08-31: it names a blind party — the operator who
made the edit, whose successful check of the value that landed is *positive evidence for the
wrong proposition* — and it fails with a plausible answer rather than an error, so it satisfies
the routing table's first row. Its old field doubted this by conflating the recording surface
with the remedy; the `H` hook that diffs intended against effective config is the mechanism it
owes, not an alternative home. IC-7 still fails on
premise confidence, IC-8 routes to `DC` regardless of n, and IC-10 / IC-11 are newly opened at
n=1.

**IC-9's flag is withdrawn, and the error was mine.** I tagged two archive files into it that do
not instantiate its claim: `ollama_large_batch_exceeding_batch_size` was vacuous the day it was
written and `cross-process-write-lock-test-passes-when-it-does-not-run` is vacuous when skipped —
neither turns on **environment-controlled text**, which is the whole of IC-9's claim. Both were
matched from their titles, which read as "a test that passes when it shouldn't" — true of IC-9
and true of a wider family. Tags withdrawn, n back to 1, below threshold. Their own pre-existing
`vacuous-assertion` and `green-proves-nothing` tags say what they actually are.

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

**The candidate queue is now empty — all five became classes on 2026-09-01, and every one opened
at n=0.** `IC-13`, `IC-14` and `IC-15` are the backfill's three remaining shapes; `IC-12` is the
read-side window the git hooks introduced; `IC-16` is the vacuous-assertion family the `IC-9`
withdrawal exposed. The fourth backfill shape needed no entry — it had already been promoted to
`IC-11` on 2026-08-31, forced by a taggable instance arriving against a gate with no
`cluster/unclassified` escape hatch, rather than by its count.

**Every one of the five carries `n=0`, and that is the honest reading rather than a defect.**
Three were opened on a peer's backfill count this session did not independently verify, one on a
single instance written up inside another bug file, and one on a rule that exists in `CLAUDE.md`
with no corpus ever indexed against it. The ledger stores a query, so a class whose members are
not yet tagged reports zero — and the `**Members:**` field of each says exactly whose count it
rests on. **Do not read those zeros as evidence of rarity.** The work they name is tagging, and
until it happens no judgement should rest on them; `**Promotes to:** not yet` is set on all five
for that reason and not on spread.

`IC-16` inverts the usual direction and is worth reading for that alone: the rule came first, from
an SDD run, and lives in `CLAUDE.md` § *Testing Discipline* already. What never happened is
indexing the corpus against it — so *"which of our bugs instantiate the vacuous-assertion rule?"*
has no answer, and nobody can tell whether the rule is working. The class exists to make an
existing rule measurable rather than to earn a new one.
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
**Promotes to:** `OB-6` — *a gate collapses "cannot observe" into the confident answer*, `docs/trackers/observer-blindness.md`, promoted 2026-09-01 at n=16. **The `OB-4` reference this field used to carry was loose and is corrected here**: `OB-4` is about the `.worktrees/bench` liveness marker and never mentions the rendezvous gate, so it was never "this class's rendezvous half". The two are siblings on one axis — `OB-4` asks *why a proxy is trusted* (an accuracy record it later spends), `OB-6` asks *what the proxy does when wrong* (returns the confident value rather than admitting it cannot tell) — and their remedies differ, so neither subsumes the other.
**Mechanism status:** `designed`, with a shipped exemplar rather than a sketch. `b9cc75b4` (patch-id `5a5c761072c44b54dc80f224d89355dd2d31e498`) closed one member and demonstrates the three moves `OB-6` prescribes: a **third state** for *indeterminate* rather than defaulting to the strong claim; **omit rather than zero** an unmeasured quantity; and **extract the proxy→claim mapping into a pure function**, which is the check itself — the hardcoded `"up_to_date"` survived because its arm sat behind a live client constructor no test could reach, and a pure function admits a table test whose `None` row is the one that was missing. Nothing yet applies the pattern corpus-wide.
**Valid:** dated 2026-08-31

Every member is one substitution. `workspace(post_compact=true)` cannot check that a compaction happened, so it trusts the caller's flag and clears the whole ledger on a mistaken `/mcp` reconnect. The rendezvous gate cannot check that the companion hook is still alive, so it trusts a monotone stamp that nothing can un-set — and its twin defect is the same stamp never landing, which leaves the gate shut for the life of the process. The subagent ledger cannot see what the parent holds, so 84% of measured subagent sessions re-receive a topic their parent already has. `get_guide` cannot see which section a caller needs, so it serves the topic — or, since section grain shipped, serves the section attached to the response of the call it was meant to inform.

The unifying property is that **none of these fail loudly**. A wrong proxy produces a plausible delivery: an extra guide body, an open gate, a closed gate, a re-cleared ledger. Nothing throws, so nothing downstream fires either. That is what makes the class survive review chains that catch louder bugs, and it is why `rendezvous-slot-never-stamped` could be closed `wontfix` on the reasoning that the failure is invisible — correct about the observation, and exactly the property that should have counted against it.

Note the shape shared with `cluster/blast-radius-exceeds-visibility`: both are *a component reasoning about a scope larger than the one it can observe*. They are kept separate because the remedies differ — that one needs an ownership protocol over a shared resource, this one needs an authoritative signal for an event. Merging them would produce a class too abstract to prescribe anything.

**Falsified by** a member whose proxy failure surfaced as an error rather than a plausible result; that would belong to an ordinary-correctness class instead.

**The 16 members split on one question, and the split predicts their fate: does the substrate
that would answer the gate exist at all?** **Filesystem/repo-scoped (7)** — git sync, index
coverage, lock state, an external checkout, a `build.rs` snapshot, a worktree flag, a test-mode
env var — are cases where the process *could* have looked and did not. **All seven are closed**
(six fixed, one wontfix). **Harness/peer-scoped (9)** — a compaction, a `/clear`, an `/mcp`
reconnect, what a parent session holds, whether a companion hook is alive — are cases where the
process holds no channel to the fact. **Five of those nine are still live**, including all four
open members and the one `investigating`.

That asymmetry is the argument for `OB-6`'s first move rather than an excuse for the backlog.
Where the substrate exists, this class is an ordinary bug and the corpus fixes it. Where it does
not, **a third state is still available even when the event is not** — a gate that cannot tell
can say so, and the harness-scoped members are open precisely because each one instead picked a
proxy and reported its output as knowledge.
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
**Promotes to:** `OB` — `docs/trackers/observer-blindness.md`, adjudicated 2026-08-31. This field read *"three instances across three subsystems (MCP env, shell env, git config)"* until then — a count from before the archive backfill. It is **8**, spanning MCP env, shell env, git config, worktree gitdir, hook scripts, sweep scripts, the env-copy flow and memory keys. **Unlike `IC-5` and `IC-6`, this class passes OB's admission test**: `Blind party:` names one — the operator who made the edit — and the failure is a plausible answer (the additive half landing) rather than an error, which is the routing table's first row. The earlier *"arguably `H` rather than `OB`"* conflated *where the class is recorded* with *what the remedy is*: an `H` hook diffing intended against effective config is the mechanism this class owes, and `IC-1`/`IC-2`/`IC-3` likewise sit in `OB` with mechanisms outstanding. Record in `OB`; build the hook.
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
**Promotes to:** `H` — `docs/trackers/codescout-usage-hookify.md`, adjudicated 2026-08-31 after the archive backfill. **Not `OB`**: `Blind party:` is `none`, which fails OB's admission test, so the routing question was never open. The old note read *"clears the count but not the subsystem spread; all three sit in the Windows/wine lane… Revisit if a fourth lands outside it"* — seven have. The eleven members span **six** subsystems: cargo feature config (3), wine/Windows (4), shell environment (1), cargo workspace resolution (1), toolchain (1), ambient embedder config (1). **It promotes as a worklist item, not a rule**, because `Mechanism status:` is still `none yet` and this ledger holds that a rule without one produces advice. The mechanism shape is a check diffing the documented four-command gate against `ci.yml`'s matrix; the four Windows lanes red on 2026-08-31 are a live instance of exactly what that would have caught.
**Mechanism status:** none yet. `scripts/build-windows.sh` could assert the wine version CI packages, which would convert the divergence from silent to loud at ~3 lines.
**Valid:** conditional — a member appears outside the Windows/wine lane

`scripts/build-windows.sh` exists precisely so Windows failures are reproducible without CI round-trips, and that purpose holds only while the two wines behave alike. They do not: `ubuntu-latest` packages wine 9.0, a current dev box runs wine 11.16, and in a single day the gap produced two divergences — one costing a CI cycle, one still costing a skipped test.

The other two members are the downstream cost of that gap rather than separate defects. `wine-lane-flakes-under-load` records three tests that failed together and passed on the next identical run, and its own update narrows the file to one test after CI reproduced two of the three *with a different payload* — a distinction only visible because someone compared the two environments deliberately. `windows-ci-timing-flakes` is `zombie` for the honest reason: both flakes resolve only by recurring, so no amount of effort reaches them.

This class is deliberately kept even though it does not currently promote. Its value is the **threshold rule's worked negative example**: three instances is not enough when they share a subsystem, and recording that judgement is what stops the next reader counting to three and promoting anyway.

**Falsified by** the two wine versions being shown to agree on the divergent cases, which would relocate the defect to the tests themselves.

## IC-6 — an addressing scheme with no escape hatch and no disambiguator

**Slug:** `cluster/addressing-without-an-escape-hatch`
**Claim:** An addressing scheme interprets every token in its namespace and provides no way to write one literally, or to disambiguate two that collide. The scheme is correct on every input it accepts; the defect is the input it makes unrepresentable.
**Members:** `filter={"tags": {"contains": "cluster/addressing-without-an-escape-hatch"}}` — n=27, 2026-08-31, by query.
**Blind party:** `none — ordinary design defect`. The gap is visible to anyone who tries the unrepresentable input; nobody is structurally prevented from seeing it. Recorded so it is not mis-promoted to `OB`.
**Promotes to:** `CLAUDE.md` § *Parsers Over a Namespace — owe an escape and a disambiguator* — **landed 2026-08-31**. Adjudicated the same day, after the archive backfill took it from n=2 to n=27, the largest class in the corpus. **Not `OB`**: `Blind party:` is `none`. It is codescout-specific engineering discipline, statable as one rule — *a parser over a namespace owes an escape for writing a token literally, and a disambiguator for two that collide.* Five subsystems: file-format navigation (`json_path`, `toml_key`), markdown editing (fences, heading-shaped content), the link/citation resolver (frontmatter delimiters, qualifiers, prefix collisions, doc-examples-read-as-citations), shell command gates (IL-3, dangerous-command, source gate, `run_command` — four separate gates, every one of them on heredocs), and symbol navigation (`name_path` with no disambiguator). Unlike IC-5 this one has partial mechanism already shipped, so the rule has something behind it.
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
**Promotes to:** `not yet` — the count is met but two of four members are `zombie` with unconfirmed root causes, so promoting now would rest a rule on unresolved premises. (This field read *"two of three"* until 2026-08-31, from before the archive backfill; the fraction moved, the argument did not.)
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
**Members:** `filter={"tags": {"contains": "cluster/assertion-satisfiable-by-accident"}}` — n=1, 2026-08-31, by query. Went to 3 in the archive backfill and back to 1 the same evening: two of those tags were misfits and were withdrawn, see **Promotes to**.
**Blind party:** `none — ordinary design defect`, but with an unusually strong *detection* asymmetry: at ~1-in-800 the failure is unreachable by local reproduction, so the author's evidence is necessarily circumstantial. The file records that honestly in its own `unverified:` field.
**Promotes to:** `not yet` — n=1. When it moves, the target is `I` (`docs/trackers/test-escape-hardening.md`), because the remedy is a standing check rather than a rule anyone remembers — but see **Mechanism status**: the check is not the grep this entry used to name.
**Mechanism status:** `none yet`. This field read `designed` until 2026-08-31, on the reading that the member *"names the check"*. The member names it and **records it as insufficient**: it ran the grep — 466 hits across 99 files under `src/` — and concluded *"not a worklist … the defect needs the haystack to embed environment text, which the regex cannot see. Recorded so the next person knows the bare grep does not narrow it and does not repeat the measurement."* Whether a haystack interpolates a `Path` is a dataflow question, so no text search can decide it; the starting population is not the finding set. **The mis-stated field cost exactly what the member predicted**: a reader took `designed` at face value on 2026-08-31 and re-ran the measurement (120 hits at the narrower `assert!(!x.contains(` form) before reaching the member that says not to. A real mechanism has to start from the emitting side — which formatters interpolate a `Path` — and work outward to their assertions.
**Valid:** dated 2026-08-31

**Two tags were withdrawn on 2026-08-31, and the mistake is instructive.** The archive backfill put `ollama_large_batch_exceeding_batch_size` and `cross-process-write-lock-test-passes-when-it-does-not-run` in here, taking n to 3 and appearing to meet the threshold. Neither instantiates the claim: the first was vacuous the day it was written and the second is vacuous when skipped, and **neither turns on environment-controlled text**, which is the entire content of this class. They were matched from their titles — both read as *"a test that passes when it shouldn't"*, which is true of this class and true of a wider one. That is precisely how a class inflates past its threshold on members that do not instantiate it, which is the failure this ledger's own *"a wrong class corrupts the counts that promotion reads"* names. Their real family, *an assertion that cannot fail*, is recorded as a candidate class in the Index; their own `vacuous-assertion` and `green-proves-nothing` tags already said so.

The single member states its own general form better than a summary would: *"This is not 'a flaky test'; it is an assertion whose input contains environment-controlled text. Any `!haystack.contains(needle)` where the haystack embeds a path, a hostname, a timestamp or a temp name has the same defect, and it always fails open."*

The direction matters and is the reason this is not a duplicate of `CLAUDE.md` § *Testing Discipline*'s monotone rule. That rule says an assertion cannot detect a change it is monotone under, and prescribes mutating the other way. This class is narrower and concerns the **haystack** rather than the assertion's direction: the positive form (`assert contains`) is safe here, because a coincidental match makes it pass *when it should already pass*. Only the negative form can be satisfied by an accident that the property being tested does not license. The two are complements, and the monotone rule is the more general of the two.

Kept as a class of one for the same reason as `IC-8`: the bug corpus is where the second instance will arrive, and a defined slug is what lets it find the first.

**Falsified by** an emitting-side sweep finding no other negative `contains` over an interpolated path, which would make this a one-off rather than a class.

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

**A TOOL commits this class too, which is the strongest evidence the ledger has for it.** The three misattributions above were made by agents, so "be more careful" remains an available (wrong) reading. `pre-commit` removes it. Its post-hook check is an unconditional whole-tree diff — `files_modified = diff_before != diff_after`, `pre_commit/commands/run.py:203-206`, no per-hook opt-out in 4.6.2 — and it reports any difference as *"files were modified by this hook"*. The tool has no way to ask who wrote them, so it attributes by **proximity in time**, exactly as the agents attributed by proximity in the working tree.

Measured 2026-09-01: it refused a push on a **green** `cargo test --workspace` run, naming `docs/trackers/claim-decay.md` — a file nothing under `src/` writes (`claim-decay` appears there three times, all citations in comments) and which a peer session was editing at that moment. The false-failure window is the hook's runtime: sub-second for the commit-stage checks, 30-80s for the workspace test. The push stage was withdrawn for this reason (`5fbc65fb`), and the reasoning is inline in `.pre-commit-config.yaml` so nobody re-adds one without meeting it.

That this arrived *inside infrastructure built by the session that opened the class, four hours after opening it*, is the `OB-1` signature again — and it is why the remedy field reads `H` (a provenance channel) rather than any amount of care.

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

## IC-12 — transient shared state lies to every reader, and the standard diagnostic confirms the lie

**Slug:** `cluster/transient-shared-state-lies-to-readers`
**Claim:** One session's tooling mutates shared state for the duration of an operation. Every other session's read is wrong for that window, and the standard diagnostic reports the lie as truth rather than as an outage — so the symptoms are indistinguishable from permanent loss.
**Members:** `filter={"tags": {"contains": "cluster/transient-shared-state-lies-to-readers"}}` — n=0 tagged, 2026-09-01. One measured instance, written up as *"The read-side twin"* inside `docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md` rather than as its own file, so there is nothing to tag yet.
**Blind party:** the *reading* session, and note the inversion — every other class here blinds a writer. Here the writer is fine and the reader is deceived, by an operation it did not initiate and cannot see.
**Promotes to:** `not yet` — one instance, and the remedy so far is knowledge rather than mechanism.
**Mechanism status:** none yet. Documented at the point of use (`scripts/pre-commit-unreviewed-content.sh` header, `0b763983`), which is a knowledge fix and by this ledger's own rule a worklist item rather than a rule.
**Valid:** dated 2026-09-01

Measured 2026-08-31, within a minute of git hooks being enabled on this shared checkout. The pre-commit framework stashes unstaged changes while hooks run, and that stash covers **every** session's in-flight work, not only the committing one's. For the sub-second duration of a peer's commit, a session observed its own edited file revert to HEAD content, `git status` report it clean, and a `grep` for text it had just written return nothing.

**The detail that makes it a class rather than a footnote: `git stash list` is EMPTY throughout.** pre-commit writes a patch under `~/.cache/pre-commit` instead of using `git stash`, so the obvious way to detect a stash reports that there is not one. The reader is not merely misinformed — the instrument they would reach for to check confirms the false reading. There is no opt-out; `pre-commit run --help` exposes no stash flag and the stash is unconditional when unstaged changes exist.

**The danger is not the window, it is reacting inside it.** Rewriting a section from memory races the restore and can genuinely lose or duplicate work while "recovering" from a problem that has already fixed itself. So the remedy is an oracle, not a fix: for a librarian artifact, `artifact_event(action="list")`'s `field_patch` byte counts, which no git operation touches; for anything else, `wc -c <path>` against `git show HEAD:<path> | wc -c`. Never `git status`.

**Kept apart from `IC-1` on the observer, not the substrate.** `IC-1` is a write reaching further than the set of peers you can see; here no write collides at all and the shared state is correct at both ends of the window. It generalises past `pre-commit` to anything that transiently mutates shared state — a formatter run, a build that moves files, a script that checks out.

**Falsified by** an instance where the standard diagnostic correctly reported the transient state as unavailable rather than as settled truth. That is an outage, which is a different and much safer thing.

## IC-13 — a capped result is presented as complete, so a partial answer reads as the whole one

**Slug:** `cluster/capped-result-presented-as-complete`
**Claim:** A result is truncated by a limit — a page size, a byte budget, a display cap — and returned without a marker saying so. The caller reads a partial answer as the whole answer, and a **zero** from a capped scan reads as "not present" rather than "not reached".
**Members:** `filter={"tags": {"contains": "cluster/capped-result-presented-as-complete"}}` — n=0 tagged, 2026-09-01. **Provenance: three or more instances identified by the 2026-08-31 archive backfill pass (`8b13b5f3`), which I have not independently verified.** The count is a peer's, the membership is unassigned, and the query is honest about that: it returns nothing until the files are tagged.
**Blind party:** the caller, who has no way to distinguish a short list from a complete one. Also the *author of a downstream count*, since an aggregate computed over a capped scan is wrong in a direction nothing signals.
**Promotes to:** `not yet` — opened on an unverified count; needs its members tagged before any judgement rests on it.
**Mechanism status:** none yet, though the shape of one is known — `link_scan` already carries a per-array `counts.truncated` flag, and `run_command`'s buffer envelope carries `unfiltered_truncated`. Both are the pattern to copy.
**Valid:** conditional — the archive members are tagged, at which point the count becomes real and this entry should be re-adjudicated

This class is opened deliberately **before** its membership exists, which is a departure worth stating. The ledger's rule is that a member list rots and a query does not; the cost of that rule is that a class identified but not yet tagged reads as n=0. Recording the provenance in `**Members:**` is the compromise — a reader sees both the claim and the fact that nothing has been assigned to it yet, and cannot mistake the zero for evidence of rarity.

The archived instances are not yet cited here because I have not read them. What is independently visible is that this repo has treated the shape as real for some time: `truncate_compact` cutting from the tail and destroying the overflow signal, `grep` printing a self-refuting *"Showing N of N"* when collection hit the cap, and `link_scan`'s dangling count being prefix-gated so a whole namespace could read as healthy — all closed, all the same claim.

**Falsified by** the backfill's three instances turning out to share a subsystem rather than a mechanism, which would make this a broken component rather than a class.

## IC-14 — a guard's coverage is narrower than its name, so the name is what everyone reasons with

**Slug:** `cluster/guard-narrower-than-its-name`
**Claim:** A guard's name states the property; its implementation covers a subset of it. Everything the name promises is believed protected, the uncovered remainder is protected by nothing, and the guard's own green result is what conceals the gap.
**Members:** `filter={"tags": {"contains": "cluster/guard-narrower-than-its-name"}}` — n=0 tagged, 2026-09-01. **Provenance: three or more instances identified by the 2026-08-31 archive backfill pass (`8b13b5f3`), unverified by me.**
**Blind party:** everyone downstream of the name. The implementer knows the scope at the moment they write it; every later reader knows only the name, and the name is what they reason with. This is `OB-1`'s shape — the parameter the author's context supplied for free.
**Promotes to:** `not yet` — opened on an unverified count.
**Mechanism status:** none yet.
**Valid:** conditional — the archive members are tagged

Distinguish this carefully from `IC-3` (*declaration is not execution*), which they are easy to merge and should not be. In `IC-3` the mechanism is **never reached** — a selector production does not emit, a CLI flag that does not exist. Here the mechanism runs, does real work, and returns a true result about a **smaller domain than its name claims**. `IC-3` fails at zero coverage; this fails at partial coverage, which is strictly harder to see because the guard demonstrably works every time you test it.

Two live examples visible from this session without consulting the archive. `cargo test --lib` in this repo's own pre-commit config was named "cargo test" and could not reach `tests/` at all, so the cluster gate it appeared to protect was never run (`4e5f060e`). And `doctor`'s `augmentation_declared_but_absent` fires only on a *declared* sidecar that is missing, so undeclared-and-unexported — the actually dangerous state — reads identically to nothing-to-declare (`IC-11`'s member). Both are guards whose names are broader than their reach.

**The tell, and it is cheap:** read the guard's name as a claim, then ask what input satisfies the name but not the implementation. If such an input exists and no other guard covers it, the name is the defect. Renaming is a legitimate fix here and is often the honest one — a guard called `cargo-test-lib` misleads nobody.

**Falsified by** an instance where the name and implementation agreed and the failure lay in the property itself being wrong.

## IC-15 — a parameter is accepted at the boundary and silently dropped downstream

**Slug:** `cluster/accepted-parameter-silently-dropped`
**Claim:** A parameter is accepted at the boundary — it validates, the call succeeds — and some path downstream discards it. The caller has positive evidence the value was set, because nothing rejected it, and no later observation distinguishes "applied" from "accepted and dropped".
**Members:** `filter={"tags": {"contains": "cluster/accepted-parameter-silently-dropped"}}` — n=0 tagged, 2026-09-01. **Provenance: three or more instances identified by the 2026-08-31 archive backfill pass (`8b13b5f3`), unverified by me.**
**Blind party:** the caller, and specifically because acceptance is the only feedback the interface gives. Rejection is loud; silent discard is indistinguishable from success at every point they can observe.
**Promotes to:** `not yet` — opened on an unverified count.
**Mechanism status:** none yet.
**Valid:** conditional — the archive members are tagged

The class is well-attested in this repo outside the backfill. `artifact(create)`'s `augment` silently discarded five of its seven fields; the CLI's `artifact create`/`update` dropped `time_scope` and `extra`; `read_file`'s `force=true` was silently discarded on whole-file reads; `update_entry`'s entry-param guard fired only when `fields` was absent. All closed, all the same claim — a value the caller passed and the system took, then did not use.

**The frontmatter defect filed today is the same shape at document grain rather than parameter grain** (`docs/issues/2026-08-31-a-body-that-already-has-frontmatter-becomes-two-blocks.md`): the keys in the orphaned block were accepted onto disk and dropped from the catalog, so `status: fixed` in a file read `open` to every query. It is filed under `IC-6` because its *mechanism* is the absent escape hatch, and it is cited here rather than double-tagged — the one-tag rule.

Note the asymmetry that makes this worth a class rather than a bug-by-bug fix: the remedy is almost always to **refuse** rather than to start honouring the value. Honouring a long-dropped parameter changes behaviour for every existing caller who has unknowingly relied on it being ignored; refusing is loud, immediate, and tells them the truth. The frontmatter bug's `## Fix` argues exactly this and is the worked example.

**Falsified by** an instance where the parameter was honoured and the defect lay in what it did.

## IC-16 — an assertion that cannot fail is zero coverage wearing a passing test's clothes

**Slug:** `cluster/assertion-that-cannot-fail`
**Claim:** An assertion has **no input that would make it fail**. It is not weak coverage — it is zero coverage wearing a passing test's clothes, and it is added most often in the very commit that closes a missing-guard finding.
**Members:** `filter={"tags": {"contains": "cluster/assertion-that-cannot-fail"}}` — n=0 tagged, 2026-09-01. Named as a candidate by the 2026-08-31 backfill and by the `IC-9` tag withdrawal; at least two archive members are identified (`ollama_large_batch_exceeding_batch_size`, vacuous the day it was written; `cross-process-write-lock-test-passes-when-it-does-not-run`, vacuous when skipped) and `CLAUDE.md` records four more from a single SDD run.
**Blind party:** the reviewer, structurally — a passing test is the evidence they are given, and vacuity is invisible in exactly that evidence. `CLAUDE.md` measures it: of four found in one run, *"the fourth only because the final reviewer was told to hunt for one."* Care does not find these; a changed question does.
**Promotes to:** `not yet` for the ledger's purposes, but note it is **already promoted in substance** — `CLAUDE.md` § *Testing Discipline* and § *SDD Rulings* both carry it (*"Ask 'what mutation would make this test fail?', never 'does it pass?'"*, and *demand a deliberate break*). What is missing is the membership query, not the rule.
**Mechanism status:** `designed` — the rule exists and is written down; nothing enforces it. Mutation testing per guarded site is the mechanism, applied by hand today.
**Valid:** conditional — the identified members are tagged

**Boundary against `IC-9`, which is a strict sub-case and must not absorb this.** `IC-9`'s assertion *can* fail — roughly 1-in-800, when a random tempdir name happens to contain the needle. Its mechanism is environment-controlled text in the haystack. This class is the harder one: **no input fails it at all**, so no run frequency, no environment and no amount of CI time will ever surface it. An `IC-9` member is a flake; a member here is a permanent zero.

That distinction is why the two withdrawn tags were withdrawn rather than left. Both read from their titles as *"a test that passes when it shouldn't"* — true of `IC-9` and true of this class and true of several others — and title-matching is what produced the misfit. The claim, not the title, is the admission test.

**This one is deliberately opened despite the rule already existing**, which reverses the usual direction: normally a cluster accumulates until it earns a rule. Here `CLAUDE.md` got the rule first, from an SDD run, and the *corpus* was never indexed against it — so the question *"which of our bugs are instances of the vacuous-assertion rule?"* has no answer, and nobody can tell whether the rule is working. Opening the class is what makes the existing rule measurable rather than merely stated.

**Falsified by** the identified members turning out to have a failing input after all, which would move each of them to `IC-9` or to an ordinary coverage gap.

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
