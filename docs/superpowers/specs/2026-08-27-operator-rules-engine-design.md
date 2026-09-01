---
id: d2fad9fa5c012291
kind: spec
status: active
title: Operator Rules Engine — one addressable corpus for the rules that hold across every project, compiled where they must be resident and routed where they can be triggered
tags:
- operator-rules
- prompt-surface
- rules-engine
- claude-md
- injection
- engine-5
---

> **Engine 5 of a six-engine programme.** The programme is described in § *Where this
> sits*; every other engine is out of scope here and gets its own spec. This one is the
> pilot, chosen because it has a measured deficit, a live defect, and a corpus small
> enough to hold in one document.

## Problem

Operator rules — the working agreement between one human and every agent session they
run, independent of project, tool, or model — have no mechanism at all. They live as
prose in three hand-synchronised copies of `~/.claude*/CLAUDE.md`, and the synchronisation
is enforced by a sentence inside the very file it governs:

> When making config changes (plugins, settings, installed_plugins.json), always check and
> apply to ALL THREE instances.

**Measured 2026-08-27, this machine:**

| Profile | Bytes | md5 (first 8) |
|---|---|---|
| `~/.claude/CLAUDE.md` | 4639 | `b583ffaa` |
| `~/.claude-sdd/CLAUDE.md` | 4640 | `d52fc86c` |
| `~/.claude-kat/CLAUDE.md` | 4640 | `d52fc86c` |

They have drifted. `diff` reports `22a23 >` — a single blank line. Semantically nothing,
which is the point: **the corpus has no notion of rule identity**, so a stray newline makes
three copies "different" while a genuinely changed rule would be indistinguishable in kind
from that whitespace. There is no check that could tell the two apart, because there is
nothing to compare but bytes.

Two consequences, both live:

1. **Drift is undetectable in the direction that matters.** A rule added to one profile and
   not the others produces the same signal as a blank line.
2. **No rule carries its evidence.** `Conclude Last` was measured across 11 arms and is on
   record at 0% → 100%; `Sonnet is the subagent floor` rests on a single dated observation;
   two more rest on nothing recorded. All four read identically on the page, so a reader
   cannot tell a law from a hunch, and neither can a future edit.

### Not in scope

- **Cross-machine.** This machine's three profiles only. The store is a single git-tracked
  ledger in this repo; a second machine would need a sync design that does not exist and is
  not needed to validate the core. Revisit when a second machine runs these profiles.

  > **Trigger fired 2026-08-28.** A second machine now runs these profiles: this host was
  > compiled for the first time that day, its three profiles having still been byte-identical
  > to the § *Problem* baseline above — which is the proof it had never been onboarded.
  > Cross-machine is therefore in scope and undesigned. The asymmetry to design against: the
  > ledger is git-tracked and travels between hosts, the compiled block lives in untracked
  > `~/.claude*/CLAUDE.md` and does not, and nothing runs `--check` on either host. Recorded
  > as `CM-10` in `docs/trackers/resume-cross-machine-catalog-restore.md`.
- **The other five engines** (§ *Where this sits*). Each is a key type plus a corpus.

  > ⚠ **Superseded 2026-09-02.** This bullet used to end *"and ships separately."* That was
  > true when written and is now false in the tree: engines 1 and 5 share the retrieval key
  > (`Tool::selector_key`), the shape grammar (`operator_rules::render` imports
  > `prompts::guide_index::parse_shape`), the session ledger (`ctx.guide_hints_emitted`, with
  > `op:` vs `<topic>#<heading>` namespaces), and the emission site (`Tool::call_content`).
  > What remains out of scope *here* is designing those engines — the coupling itself is
  > owned by
  > [`2026-09-02-retrieval-engine-coordination-design.md`](2026-09-02-retrieval-engine-coordination-design.md)
  > (`0021bead4e5a01e2`).
- **The 44.4% `contradicted` rate** measured in `docs/evals/2026-08-27-guide-injection-use.md`.
  Guidance arriving and being violated is an enforcement problem; it is out of scope here
  for the same reason `get-guide-section-grain` excluded it.
- **Rewriting the existing rules' content.** Harvesting records their current text, shape
  and evidence status. Improving a rule's wording is a separate act, gated on its own arm.
- **Cross-user vs cross-machine ledger scope — UNDECIDED.** The "Where this sits" table names
  the operator engine's retrieval key as "the human," but the actual corpus root
  (`docs/trackers/operator-rules.md`) is one git-tracked ledger in one repo checkout, and the
  routing corpus (`LEDGER_SRC`) is compiled in per build. Given the same human now confirmed
  to operate from more than one machine (see the Cross-machine bullet above and `CM-10`), it
  is not yet decided which identity the ledger's scope should actually follow — a prior,
  more basic question than `CM-10`'s sync-mechanism one:
  1. **Per-machine** (current design, unchanged). Each machine's build carries its own
     compiled ledger; a rule added on one host does not reach another until that host's
     checkout is updated and rebuilt. Simplest, but sits at odds with "the human" as the
     stated retrieval key.
  2. **Per-user, synced across machines.** The ledger becomes an identity-scoped resource
     every machine the human uses reads the same copy of. Closer to the stated key, but
     needs the sync design this spec already declines to build (Cross-machine bullet above).
  3. **Per-user base plus a per-machine override layer.** A shared ledger travels with the
     human; per-machine entries (e.g. rules that only make sense on a host running three
     profiles) layer on top. Most expressive of the three, also the most design and
     implementation debt.

  No option is recommended here — this bullet only records that the question is open, and
  keeps it distinct from `CM-10`'s narrower question of how syncing itself would work.

---

## Where this sits

The six-engine table **moved 2026-09-02** to
[`2026-09-02-retrieval-engine-coordination-design.md`](2026-09-02-retrieval-engine-coordination-design.md)
(`0021bead4e5a01e2`) § *The family*, which is now its only home — asserted by that spec's
gate 5.

It moved because a per-engine spec is the wrong owner for cross-engine state, and the decay
was already measurable: this table's engine-1 row read *"9 of 10 tasks"* while engine 1's
Phase 1 had shipped complete and accumulated an eight-item resume queue
(`docs/trackers/resume-get-guide-section-grain-phases-2-3.md`).

What this spec still owns, unchanged: **the operator engine is number 5, its retrieval key is
the human, and its corpus is `~/.claude*/CLAUDE.md` compiled from
`docs/trackers/operator-rules.md`.**

One finding from the enumeration stays here because it is about *this* corpus: `domain`,
`model` and `task-shape` are **facets that cut across engines**, not engines. The evidence is
local — *"Sonnet is the floor for subagent dispatch"* is a **model** rule living in the
**operator** corpus. Treating each facet as an engine yields an engine per
facet-combination.

---

## Design

### 1. The rule is the unit

A rule is a ledger entry in `docs/trackers/operator-rules.md`, `entry_prefix: OP`. It is a
prose ledger — entries are `## OP-N — <title>` body sections, allocated by
`artifact(action="append_entry")`, exactly as `reconnaissance-patterns` and the session logs
work. Nothing new is invented for storage, id allocation, citation, or decay.

Required fields:

| Field | Values | Why |
|---|---|---|
| `**Imperative:**` | one sentence, imperative mood | the delivered text; see § 2 |
| `**Binding:**` | `always` \| `triggered` | selects the delivery path (§ 3, § 4) |
| `**Shape:**` | `imperative` \| `guard` \| `procedure` \| `contract` | measured to matter; § 2 |
| `**Covers:**` | short kebab-case failure-mode slug | Gate 3(a) compares failure modes; without a field to compare, non-overlap is unenforceable. Added 2026-08-27 during planning — the first draft stated the gate and omitted the field it reads. |
| `**Serves:**` | selector, `triggered` only | grammar borrowed from § 4 |
| `**Evidence:**` | `measured: <arm> <base>% -> <shipped>% (n=N)` \| `unmeasured` | a law and a hunch must not read alike |
| `**Rests on:**` | citation to the arm, audit entry, or observation | existing ledger field |
| `**Valid:**` | `invariant` \| `dated <ISO>` \| `conditional — <event>` | existing decay vocabulary — recorded in the ledger for human review and `doctor`-style tooling; the Phase 1 engine does not parse or model this field (not on `Rule`) |
| `**Status:**` | `active` \| `candidate` \| `retired` | disposition |

`**Evidence:** unmeasured` is a first-class value, not an omission. It is what makes the
budget in § 3 enforceable and what stops a plausible sentence acquiring the authority of a
measured one by sitting next to it.

### 2. Shape is a measured field, not a style note

`prompt-hamsa-audit-log:A-21` ran 11 arms on this corpus's flagship rule and found the active
ingredient is **an unconditional imperative that binds at every claim**:

- `b2`, imperative-only — **100.0%**, best in grid
- `a2`, the full prose paragraph — 93.3%
- `a1`, bare — **0%**

with the mechanism stated: *"Conditional guards gate on the doubt a planted belief suppresses;
procedural detail only applies once checking has begun; labelling contracts produce honest tags
rather than checks."*

That is an empirical ranking — `imperative` > `guard` > `procedure` > `contract` — across a
0%→100% spread. It is recorded per rule so that a rewrite can be proposed against it, and so
that a `contract`-shaped rule is visibly the weakest form rather than merely a different one.

**This spec does not rewrite any rule's shape.** It records it.

### 3. `always` rules compile; the budget is the design

`always` rules have no retrieval key — they must be resident. They compile into a delimited
block in each of the three profiles:

```markdown
<!-- BEGIN operator-rules (generated from docs/trackers/operator-rules.md — do not edit) -->
...
<!-- END operator-rules -->
```

- **Idempotent and lossless.** Everything outside the markers is preserved byte for byte. A
  profile with no markers gets the block appended once; thereafter only the block is
  rewritten.
- **`--check` mode** exits non-zero when any profile's block differs from the compiled
  output. This is the check that does not exist today, and its absence is the defect in
  § *Problem*.
- **Comparison is over rule ids, then over rendered bytes** — so "which rule is missing from
  which profile" is answerable, which a `diff` cannot do.

**The `always` set is hard-capped, because stacking is measured to dilute.** `A-20` records
*"Stacking diluted rather than added"*, and `a2` (paragraph) underperformed `b2` (imperative
alone). More resident rules is therefore not monotonically better, and an uncapped `always`
set is the failure mode this engine would otherwise ship.

**Two separate constraints, and conflating them is an error this spec made once.**

**(a) Non-overlap — what the dilution measurement actually supports.** No two `always` rules may address the same failure mode. `A-20`'s *"stacking diluted rather than added"* was measured on `a5-both`: `a3` (conclude-last) stacked with `a4` (claim-format), **two arms aimed at the same behaviour**. The result generalises to redundant guidance on one failure mode; it says nothing about unrelated rules, and reading it as a headcount limit is an over-generalisation. Overlap is the thing to gate, and it is a property of a candidate rule against the existing set, not of the set's size.

**(b) Size — a token budget, set by judgement.** Start at **3–5**; ceiling **5–10**. Beyond the ceiling, an addition requires either a base arm showing a deficit or the eviction of an existing rule — the same gate the reconnaissance skill's promotion routing imposes on its session-opening surface (*"a base arm … without it this is an addition with no shown deficit"*). Below it, (a) still binds: headroom is not licence to add a second rule about the same failure.

The corpus today holds four rules; § 4 projects one of them as `always`. So Phase 3 begins with real headroom, which makes the classification in § 4 a **decision informed by arms** rather than a forced choice — a rule projected as `triggered` may be promoted to `always` if its arm shows the trigger misses the moment of need.

A rule that does not earn an `always` slot is not rejected — it becomes `triggered`, the cheaper and more common outcome.

### 4. `triggered` rules route through the section-grain matcher

`triggered` rules declare a selector in the grammar `sdd/get-guide-section-grain` introduces:

```
shape := tool ["." action] ["(" pred ")"]
pred  := "path~" substring
```

Matching reuses `Tool::selector_key`, which projects a call's shape before its input is
consumed. **This is a second corpus fed to the same matcher, not a second matcher.** No new
retrieval mechanism is built, and the cross-topic-edge exclusion in that spec is untouched —
operator rules declare no `requires:` and form no graph in Phase 2.

The existing corpus splits cleanly:

| Rule | Binding | Selector |
|---|---|---|
| Conclude Last / always-verify | `always` | — |
| Sonnet is the subagent-dispatch floor | `triggered` | `Agent`, `Task` |
| Use codescout memory, not Claude Code memory | `triggered` | `memory.write` |
| Apply config changes to all three profiles | `triggered` | `edit_file(path~/.claude)`, `create_file(path~/.claude)` |

### 5. Ledger keys

Delivered `triggered` rules are stamped in `GuideLedger` under an `op:` key namespace
(`op:OP-7`). Task 7 of the section-grain plan already taught `re_arm` to sweep section keys
without crossing topic names; a third namespace is an extension of that prefix handling, not
a new mechanism.

`always` rules are **never** stamped. They are resident by construction, so a ledger entry
would assert a per-session delivery event that did not occur.

### 6. Harvest

Phase 3 backfills the existing corpus into the ledger: one `OP-N` per rule, with `**Shape:**`
classified against § 2's vocabulary and `**Evidence:**` set to the measurement if one exists
or `unmeasured` if not. Expected initial state, from what is already on record:

- `Conclude Last` — `measured: conclude-last/b2 0% -> 100% (n=35)`, rests on `A-20` + `A-21`
- the other three — `unmeasured`

Harvesting is **transcription with classification**, not authorship. A rule whose imperative
cannot be stated in one sentence is recorded as-is with `**Shape:** procedure` and flagged,
not rewritten.

---

## Gates

Falsifiable, each failing loudly:

1. **Round-trip.** Compile → parse → compile is byte-stable, and content outside the markers
   is preserved across a compile that changes every rule.
2. **`--check` discriminates.** Exits non-zero when a rule is present in one profile's block
   and absent from another; exits **zero** when the only difference outside the block is
   whitespace. This gate is the fix for § *Problem* and must be written before the compiler.
3. **Budget enforced, on both axes.** (a) A candidate `always` rule whose failure mode is
   already covered by an existing `always` rule fails the gate — this is the constraint the
   dilution measurement supports. (b) Adding beyond the size ceiling (5–10) fails unless the
   entry carries `**Evidence:** measured: …` or another `always` rule moves to `retired`.
4. **Selector validity.** Every `**Serves:**` parses under the § 4 grammar; an unparseable
   selector fails the gate rather than silently never matching. Reuses the section-grain
   parser's rejection tests.
5. **Key namespace disjoint.** No `op:` ledger key can collide with a guide topic or section
   key, asserted directly rather than by naming convention.
6. **Every rule has a disposition.** No `OP-N` lacks `**Status:**` or `**Evidence:**` — the
   field-presence sweep that `tracker-conventions` records as having left 39 of 57 entries
   unharvestable for three months when it was absent.

---

## Rollout

**Phase 1 — schema, ledger, compiler, `--check`. `always` rules only.** Fixes the measured
drift. No routing, no dependency on the section-grain branch. Independently shippable and
independently valuable.

**Phase 2 — `triggered` rules.** Blocked on `sdd/get-guide-section-grain` landing on
`experiments`, because it reuses `Tool::selector_key` and the prefix-aware `re_arm`. Do not
start before that merge; a parallel implementation of the matcher is the thing this phase
exists to avoid.

> **UNBLOCKED 2026-08-28.** That merge has landed on `experiments`. Both borrowed pieces are
> present and pinned: `Tool::selector_key` at `src/tools/core/types.rs:1205` (default `None`,
> so a tool opts in at zero cost) with `LibrarianAdapter`'s implementation at
> `src/librarian/adapter.rs:190`; and `GuideLedger::re_arm` at `src/tools/guide_ledger.rs:280`,
> whose prefix-awareness is pinned by `re_arm_does_not_sweep_a_topic_that_shares_a_name_prefix`
> (`:739`). Phase 2 may start.

**Phase 3 — harvest and backfill evidence.** Transcribe the four existing rules, then
commission arms for the three `unmeasured` ones. Each arm is a separate, small piece of work
and the results are what decide whether those rules keep their `always`/`triggered` binding
at all.

> **Sequencing overturned 2026-08-28 by `A-34`.** Phase 2 was to be sequenced after Phase 3
> on the grounds that `triggered` routing had an empty population. The population is not
> empty — it is **resident**. `OP-2` and `OP-3` are `triggered` rules whose text sits
> unconditionally in the profile today, and removing just those two sections recovers the
> whole 3-of-7 (~43%) loss in `OP-1`'s measured effect. Phase 2 is therefore the *measured*
> fix and goes first. The interim alternative — retiring `OP-2`/`OP-3` now — carries an
> honest cost worth stating: unlike the `Conclude Last` deletion, neither has a measured
> replacement, so they would go undelivered until routing exists.

---

## Verification

The spec is falsifiable on two predictions:

1. **Drift check inverts.** `--check` fails on the current three profiles (they differ) and
   passes after the first compile, and continues to pass across a subsequent single-profile
   hand-edit outside the markers. If it cannot distinguish that hand-edit from a rule change,
   Gate 2 has not been met and the design is wrong.
2. **The compiled block reproduces the measured result.** Re-running
   `scenarios/conclude-last` with the generated block in place reproduces the shipped arm's
   100% verified / 100% correct at n=35. A drop means compilation altered the rule's
   effective form — most likely its shape — and § 3's rendering is at fault.

Prediction 2 is the one that would embarrass this design, and it is cheap: the scenario, its
arms, and its checker already exist.

> **Both predictions resolved 2026-08-28. The design survives; the cost is elsewhere.**
>
> **Prediction 1 — confirmed in full.** `--check` reported DRIFT on all three profiles before
> the first compile, `all 3 profiles current` after, and kept passing across a hand-edit
> outside the markers. Gate 1 held alongside it: a second `compile` wrote nothing.
>
> **Prediction 2 — confirmed; the wrapper is INERT.** `A-31`, n=35/arm, 0 errored:
> `b2-imperative-only` and `s2-compiled-block` are per-cell identical on 6 of 7 cells, the
> sole difference being one run on a control; excluding the stale `t2-cat-gate` trap, 30/30
> vs 29/30. Compilation does not alter the rule's effective form — § 3's rendering and the
> `generated … do not edit` marker framing both stand.
>
> **What the same chain found instead is a cost this spec did not predict.** `A-32`: the
> block stacked beside the hand-written `Conclude Last` prose collapsed 7/10 → 2/10, so Gate
> 3(a)'s blindness to *unmanaged prose already resident in the target file* is not merely
> theoretical. `A-33`: deleting that prose helped and did not hurt, but left a residual (`s5`
> 4/10 against `s2`'s 7/10). `A-34` decomposed it: the mechanism is **instruction
> competition, not position** — `s7`, with `OP-2`/`OP-3`'s sections removed and the block
> still at the END, lands exactly on `s2`'s 7/10, while moving the block to the top (`s6`)
> buys one run and makes `wrong+unchecked` worse. **The remedy is not available to the
> compiler**, which is why it changes the phase order rather than the renderer.

---

## Measurements this spec rests on

Every figure below was read this session, from the named artifact.

- **Profile drift** — `wc -c` + `md5sum` over three files, 2026-08-27; `diff` → `22a23 >`.
- **`prompt-hamsa-audit-log:A-20`** — verify-before-assert prose 93.3% vs **0% bare**;
  *"Stacking diluted rather than added."* **Scope, because this quote is easy to over-read:**
  the stacking arm was `a5-both` — `a3` and `a4`, two treatments of the *same* behaviour. It
  is evidence about redundant guidance on one failure mode, **not** about a headcount of
  unrelated rules. This spec read it as a headcount in its first draft and set the cap at 1;
  § 3 records the corrected reading and the two constraints it separates into.
- **`prompt-hamsa-audit-log:A-21`** — `b2` imperative-only **100.0%**, beating `a2` at 93.3%;
  active ingredient is an unconditional imperative binding at every claim; closed 2026-08-16,
  shipped and re-measured at n=35 (`iron-laws-detail` `43fac6c8`, bootstrap `5917e37e`).
- **`docs/evals/2026-08-27-guide-injection-use.md`** — 44.4% contradicted (excluded, § *Not in
  scope*); `librarian` 5.6% utilisation; *"small and targeted is adopted immediately; large
  and general is not"*, which is why the unit here is a rule and not a document.
- **`docs/issues/2026-08-27-guide-topics-are-atomic-nodes-in-an-unmodelled-graph.md`** —
  *"edges … must be traversed by the substrate, not by the model"*; a prose edge was followed
  **0 times in 91 sessions**. This is why § 4 routes server-side and never asks the model to
  fetch a rule.
- **`sdd/get-guide-section-grain` spec** — the `serves:`/`requires:` grammar, `selector_key`,
  prefix-aware `re_arm`, and the deliberate exclusion of cross-topic edges.
- **`src/prompts/README.md` § Surfaces** + `prompt-engineering:scenarios/surface-budget` —
  the surface inventory, and the finding that Claude Code 2.1.241 **defers MCP tool schemas**
  (57,713 chars → 1,175 tokens, 7.3%), so the cost unit is chars-fetched-per-session, not
  chars-in-prefix. `always` rules bypass this entirely: they ride CLAUDE.md, which is not an
  MCP surface.
- **`memory: fable-tuning`** — engine 2's corpus is empty *by measurement*, not neglect;
  requested ≠ served model often enough to need a daily `llm-mismatch-watch.timer`. Recorded
  here because it is why engine 2 was not chosen as pilot.

### Field note — what this spec inherited rather than discovered

The exploration that produced this document found that four of its six load-bearing
constraints were already on record and unconnected: the graph-traversal conclusion (bug file,
2026-08-27), the shape ranking (`A-21`, 2026-08-15), the dilution finding (`A-20`), and the
matcher (`sdd/get-guide-section-grain`, in progress in a worktree at the time of writing).
None was found by designing; all four were found by reading before designing —
`reconnaissance-patterns:R-120` records the pass, and `prompt-surface-measurement-session-log:F-43`
records what the same session got wrong by not reading far enough on an earlier attempt the
same day.
