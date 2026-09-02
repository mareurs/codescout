---
id: ff63538dfc9b2d8b
kind: tracker
status: active
title: Resume queue — get_guide Section Grain Phases 2 and 3 (GG-N)
owners:
- marius
tags:
- resume-queue
- get-guide
- prompt-surfaces
- progressive-disclosure
topic: get_guide section grain
entry_high_water_GG: 9
entry_prefix: GG
---

# Resume queue — get_guide Section Grain Phases 2 and 3 (GG-N)

Work left after Phase 1 shipped
(`docs/superpowers/plans/2026-08-27-get-guide-section-grain.md`, artifact
`ff204e3f9a01ada6`).

**Spec:** `docs/superpowers/specs/2026-08-27-get-guide-section-grain-design.md`
(`17a82c8622f1dcdf`). **Code:** `src/prompts/guide_index.rs`, and the delivery
wiring in `src/server.rs`'s `call_content`.

**Phase 1 is live and observable.** A session on 2026-08-28 received
`librarian § Filter Syntax`, `§ Artifact Model`, `§ docs/trackers/ — Backing
Store, Not a Docs Folder` and `§ librarian(action=...) — Reference` as separate
sliced injections, each keyed to the call that triggered it — while
`tracker-conventions` arrived **whole**, because it is Phase 2. That contrast is
the cheapest live check that Phase 1 still works.

## How to use this queue

**To act:** scan the `## GG-N` headings — each carries a `**Status:**` line.
There is no index table to fall out of sync. Verify the byte figures below before
relying on them: they were measured 2026-08-27/28 and the corpus changes whenever
a guide is edited.

**To append:** one call, from the main checkout —

```
artifact(action="append_entry", id="<this artifact's id>", id_prefix="GG",
         anchor_heading="## Template for new entries", title=…, body=…)
```

**Deliberately unaugmented** — see `docs/conventions/cross-machine-catalog-resume.md`.

## Provenance

Opened 2026-08-28 from a full-surface partial-implementation sweep. GG-1, GG-2,
GG-4, GG-5 and GG-6 are transcribed from the Phase 1 plan's own
`## Out of scope for Phase 1` and `### Carried deferred minors` sections — the
SDD workspace that held those findings is deleted at finish, so this file is
their surviving home.

## GG-1 — Phase 2: `tracker-conventions` cannot declare until two sections decompose

**Status:** open — the next piece of work in this stream
**Valid:** dated 2026-08-27

`tracker-conventions` is the highest-traffic undeclared topic and it is blocked
on two sections that exceed `MAX_DECLARED_SECTION_BYTES = 2500`
(`src/prompts/guide_index.rs:272`):

| Section | Size |
|---|---:|
| § *Entry-level standard — the shape INSIDE a tracker* | 17,378 B |
| § *Bug files (docs/issues/)* | 10,170 B |

Both need decomposing at `###` before they can carry `serves:` declarations.
Note the cap's error message prescribes **decomposing further**, never merging —
see GG-4 for the case where merging was tried and reverted.

**Why it matters:** this topic is auto-injected on tracker and bug work, which is
a large fraction of sessions in this repo, and today it delivers ~28 KB whole.

**Next:** decompose the two sections, add `serves:` declarations, run the Phase 1
gates (coverage, dangling `requires:`, reachability — plan Task 9).

## GG-2 — Phase 3: eight topics still undeclared, four have never auto-injected

**Status:** open
**Valid:** dated 2026-08-27

The four that have never auto-injected at all: `iron-laws-detail`,
`librarian-runtime`, `untrusted-content`, `error-handling` — **28,955 B, 27% of
the corpus**.

A topic that never auto-injects is not merely un-sliced; it is invisible to the
just-in-time delivery path entirely, reachable only by an explicit `get_guide`
call the model has to think to make. That is the discoverability problem the
whole section-grain design exists to address, and it is untouched for a quarter
of the corpus.

**Next:** declare in order of measured injection traffic, not alphabetically.
`error-handling` and `untrusted-content` both state rules other surfaces cite.

## GG-3 — Extract the delivery helpers out of the 408-line trait method

**Status:** done 2026-09-02 — `d0065423`, patch-id `13e5ac7b024e972aa1ead62bc1ed224fae5a2afa` (see below)
**Valid:** dated 2026-08-27

`guide_blocks_for`, `inject_hint`, `GuideDeliveryShape` and `guide_block` are
~190 lines nested inside a ~408-line trait method, and touch neither `self` nor
`ctx`. Extracting them to a module-private `guide_emit` turns three end-to-end
tests into unit tests.

The Phase 1 whole-branch reviewer promoted this but called it *follow-up, not a
merge blocker*: the code is correct, only its testability is the problem.

**Next:** pure refactor, no behaviour change. Do it before Phase 2 adds more
delivery paths through the same method.

**Done 2026-09-02** (`d0065423`). Moved verbatim to `src/tools/core/guide_emit.rs`;
`call_content` **634 → 428 lines**. The 408 above was measured 2026-08-27 and had
already been overtaken: operator-rules Phase 2 (`1bdf94bd`, 2026-08-31) added a
second engine's delivery path through the same method, so the "before Phase 2 adds
more" window had closed before this was picked up.

Two things the extraction surfaced that were invisible while everything lived in
one method:

- **`guide_block` has two callers**, not one — `guide_blocks_for`'s non-declaring
  branch, and `call_content`'s session-opener path, which delivers
  `SESSION_OPENING_GUIDE` whole and deliberately bypasses `guide_blocks_for`. It is
  `pub(crate)` for that reason, documented at the definition.
- **One invariant had zero coverage.** `guide_blocks_for`'s doc comment states that
  all-already-sent must return empty and must *never* fall back to the preamble, and
  nothing asserted it. Now covered by
  `all_sections_already_sent_returns_empty_and_does_not_fall_back_to_the_preamble`,
  verified by mutation: making the unmatched branch fire when every matched section
  is already stamped reds that test and **only** that test — the other six in the
  module pass under the mutation.

Seven unit tests total. The e2e tests are **kept, not converted**: they guard the
wiring, these guard the logic, and the reviewer's "turns three e2e tests into unit
tests" is read as *makes unit tests possible*.

## GG-4 — `librarian` § Body Editing Surfaces sits 54 B under the p50 ceiling

**Status:** open — a ratchet with almost no slack
**Valid:** dated 2026-08-27

Task 10's p50 ceiling test measures the current draw at **11,946 B against a
12,000 B ceiling** — a 54 B (0.5%) margin. Any prose added to a `librarian`
section that `serves: artifact.update` breaks the gate.

`artifact.update` alone is 3,265 B, the largest single shape, because **both**
`###` children of *Body Editing Surfaces* (`Choosing a mode — anti-patterns` and
`The shrink guard, force, and event forensics`) declare `serves: artifact.update`
and are delivered as two separate wrapped blocks.

**Merging them was attempted and reverted.** The merged section measured 2,696 B,
over `MAX_DECLARED_SECTION_BYTES = 2500`. The remedy is splitting each child into
smaller declaring sections, not consolidating them.

> **HALF DONE, 2026-09-02 — and the ceiling did fire in the interim, exactly as this
> entry predicted.** *"Any prose added to a `librarian` section that
> `serves: artifact.update` breaks the gate"* came true from a different direction: a
> peer's +555 B to a section serving `artifact.create` reddened it, against a ceiling
> re-derived down to 12,244 the same day. Record:
> `docs/issues/archive/2026-09-02-a-corrected-ceiling-reds-within-minutes-on-a-shared-checkout.md`
> (fixed and archived the same day at `6b1276fb`, patch-id
> `1ece0070092134f53547016c8200abf7b8e7d812`).
>
> **Resolved by decomposing the second child.** *The shrink guard, force, and event
> forensics* held a `field_patch` / `artifact_event(action="list")` paragraph addressed
> to `artifact_event` rather than to `artifact.update` — and duplicating the single
> fact an update caller needs from it, `replaced_subsections`, which the sibling
> anti-patterns section already states outright. Moved to § *artifact_event — Event
> Log*, whose kind list already names `field_patch`. Heading is now *The shrink guard,
> `force`, and `patch`'s accepted keys*.
>
> **`librarian.md` shrank 3 B; the served draw fell 445 B** — 12,330 → 11,885, margin
> 359 B against 12,244. Worth stating plainly because it is the whole point of grain:
> decomposition moves bytes to the action that needs them rather than deleting them, so
> whole-topic size is nearly useless as a progress signal here.
>
> **Still open:** *Choosing a mode — anti-patterns* (1,366 B), untouched. The 54 B
> margin in this entry's title is superseded; every figure above is the
> pre-decomposition state, kept for its derivation.

**Next:** split, don't merge. Treat the 54 B margin as the reason this is due
now rather than later.

## GG-5 — Phase 3 derivation: three surfaces still feed the index instead of projecting from it

**Status:** open — Phase 3, sequenced after GG-2
**Valid:** dated 2026-08-27

The spec has `topic_body()`, `GUIDE_TOPICS` and `GetGuide`'s `summaries` map all
become projections of the parsed corpus. Phase 1 deliberately inverted this: the
index is *built from* them, because deriving would have enlarged the diff across
the `get_guide` tool and its four pinning tests for no Phase-1 benefit.

Deferred to Phase 3, **when every topic declares and the derivation is total
rather than partial**. Doing it while topics are half-declared produces a
projection with holes.

**Next:** blocked on GG-2 completing. Do not start early.

## GG-6 — Latent correctness set: seven items, none live against today's corpus

**Status:** open — watch list, not a work item
**Valid:** conditional — any one of them becomes reachable

Recorded so a future change does not make one live without anyone noticing:

- Duplicate heading text under different parents yields one `ledger_key`;
  `guide_blocks_for` drops the second **permanently** for the session. Now gated
  by `no_topic_has_duplicate_section_headings`, so it fails the build.
- `call_tool_checked` keys only on `body.get("ok")`. `route_tool_error`'s
  LSP-transient branch returns `is_error: false` with `{"error":…, "hint":…}` and
  no `ok` key, defeating the predicate the same way. Not live: no p50 shape
  touches an LSP path.
- `re_arm`'s `key.len() > t.len() + 1` leaves a degenerate `topic#` empty-heading
  key unswept — the one place "prefer the duplicate" is not honoured.
- `parse_declarations` comma-splits `serves:`, so a `path~` substring containing
  a literal comma mis-splits. Fails loudly (unterminated predicate → `Err`),
  never mis-parses.
- `fence_run` does not enforce CommonMark's rule that a closing fence contain
  only the delimiter run; a ` ```rust ` line would be accepted as a closer.
- `#` and `####` are not section boundaries — worth knowing because Gate 3's
  failure message prescribes decomposition at `###`, so an over-cap `###` has no
  legal decomposition.
- Indented ATX headings are unrecognised (fence detection trims, heading
  detection does not).

**Next:** nothing, until one is reachable. Re-read this list before changing the
splitter or the matcher.

## GG-7 — Guide topics are atomic nodes in an unmodelled graph

**Status:** open — filed as a bug, sequenced with this stream
**Valid:** dated 2026-08-27

Bug `docs/issues/2026-08-27-guide-topics-are-atomic-nodes-in-an-unmodelled-graph.md`
(`7579b32b1cd2362f`): 63% of the corpus auto-injected in one session, and three
guides already cite sections the API cannot serve.

Listed here because the fix is structurally part of this stream — `requires:` is
the edge type the graph needs, and it exists but is only used within a topic.
Cross-topic `requires:` is not modelled.

**Next:** read the bug file before Phase 2 design work; its conclusion may change
what GG-1 declares.

## GG-8 — Re-run the section-use probe once the post-refactor sample is large enough — pre-registered

**Status:** open — blocked on sample size, not on work
**Valid:** conditional — the sample trigger below fires (n >= 30 post-2026-08-31 main sessions)

**Observed.** The distribution probe that gated Phase 2 ran 2026-08-31 —
`scripts/probe_guide_section_use.py`, n=166 sessions across two machines (see
`docs/issues/2026-08-27-guide-topics-are-atomic-nodes-in-an-unmodelled-graph.md`
§ *The DISTRIBUTION probe ran*). Its findings are real but the sample is
**commit-mixed**: 84% of it predates the 2026-08-27 section-grain ship, and
against **today's** code the sample is **n=1 main, n=1 subagent**. So the two
weakest claims are the two most decision-relevant:

- the **subagent lever** (92.5% of delivered bytes never engaged, 2.3x the
  main-session waste) rests almost entirely on pre-27 data — post-ship n=5;
- the apparent post-ship improvement (main 49.0% -> 34.4% never-engaged) is
  **mix, not regime** — codescout-project sessions were flat at 28.0% -> 27.5%
  while their share of main sessions moved 38% -> 57%.

**Trigger — check it cheaply, do not eyeball the calendar.**

```
python3 scripts/probe_guide_section_use.py --kind main --split-at 2026-08-31
```

Read `n=` on the `>= 2026-08-31` slice. Thresholds, in order of what they buy:

| n (post-2026-08-31) | what it supports |
|---|---|
| main >= 15 | a provisional read; label it underpowered |
| main >= 30, of which >= 15 codescout-project | the codescout stratum is comparable to its n=12 baseline — this is the real bar |
| subagent >= 20 | settles the subagent lever, the weakest-evidenced and largest one |

At 2026-08-31 the counts were **main 1, subagent 1** (post-2026-08-27: main 21 of
which 12 codescout, subagent 5).

**Pre-registered — decided BEFORE the data exists, so the re-run is a test and not a
re-derivation.** Record each as held or falsified.

1. **codescout-stratum main never-engaged stays 28% +/- 8pp.** Falsified outside that,
   which would mean the mechanism did change something and the regime question re-opens.
2. **`Querying with the librarian` remains the top-engaged section.** It held in all four
   slices measured (laptop 89%, desktop 82%, pre-27 81%, post-27 90%) at 5.1% of the
   topic's bytes. Falsified if any other section outranks it.
3. **Subagent never-engaged stays >= 85%.** This is the one worth running for.
4. **tracker-conventions injections per session DROP**, because `50590b6c`'s fallthrough
   reroutes tracker-path-naming `doctor` calls to `librarian` § *doctor repairs* (1,490 B)
   instead of this topic (39 KB). **This is a prediction about our own fix that nobody has
   verified**, and it is the cheapest thing here to check.

**Three stratification rules — every one earned by a wrong number this probe produced
before it was split.** Never blend main with subagent (45% vs 92%); never quote a period
figure without holding project constant; never read a zero for a whole profile as absence
without confirming that profile receives *other* topics (`.claude-kat` genuinely never
receives this one — it takes `librarian` 64x).

**Next:**

1. **Re-copy the desktop transcripts first.** `~/work/claude/transcripts/` is a frozen
   snapshot taken 2026-08-31; without a fresh copy the re-run is laptop-only and loses the
   second observer, which is the check PROBES rule 6 prescribes for exactly this hazard.
2. Run both corpora with `--split-at 2026-08-31`, then stratify by project.
3. Record held/falsified against the four predictions above, in this entry.
4. If 3 holds, the subagent lever is settled — act on it. It is orthogonal to both of
   GG-1's Phase 2 blockers, so it needs no decomposition and no cross-topic addressing.

**Byte figures in GG-1 have drifted — re-measure before acting on them.** Measured
2026-08-31, fence-aware: the topic is **38,870 B** attributed across 7 sections (39,106 B
on disk, less 236 B of `##` heading lines), not the "~28 KB" GG-1 states;
§ *Entry-level standard* is **17,323 B** (GG-1: 17,378) and § *Bug files* is **11,319 B**
(GG-1: 10,170). The queue's own header says to verify these first; this is that check,
done.

## GG-9 — This engine now has a coordinator, and GG-3 is its step 0

**Valid:** dated 2026-09-01

**Status:** open — a dependency added to this queue from outside it

Recorded 2026-09-02, after the operator asked for the engine family to be
operable: a coordinator, a management system and a preview system over a
surface the engines share, with the graphs and rules visible and modifiable.

That surface is designed in
`docs/superpowers/specs/2026-09-02-retrieval-engine-coordination-design.md`
(`0021bead4e5a01e2`). **Its Rollout step 0 is `GG-3`** — extracting
`guide_blocks_for` / `inject_hint` / `GuideDeliveryShape` / `guide_block` out of
the ~408-line trait method. You cannot register an engine that is inlined.

**What changed about GG-3's justification.** GG-3 was promoted by the Phase 1
whole-branch reviewer as *follow-up, not a merge blocker* — testability only,
correct code. It now also blocks a second work stream. Its own "do it before
Phase 2 adds more delivery paths through the same method" is already overtaken:
Phase 2 of the **operator** engine added one on 2026-08-31 (`1bdf94bd`), so the
method today carries two engines' delivery paths, not one.

**Verified 2026-09-02, the four shared mechanisms:**

- `Tool::selector_key` — default inverted to universal in `30b6fc41`; held by
  `every_registered_tool_supplies_a_selector_key` (`src/server.rs:3448`).
- `prompts::guide_index::parse_shape` — imported by `operator_rules::render:46`.
- `ctx.guide_hints_emitted` : `GuideLedger` — `<topic>#<heading>` vs `op:OP-N`,
  held apart by the single pairwise test `op_keys_collide_with_no_guide_key`.
- `Tool::call_content` — one selector computed once, fanned out to both.

**Two items in this queue gain a second consumer:**

- `GG-4` (54 B of slack on the 12,000 B p50 ceiling) — the coordination spec's
  gate 3 extends that ceiling into one budget over *all* emitters. Do not spend
  the 54 B assuming the margin is defended more broadly than it is: `shape_total`
  sums only blocks containing `<!-- auto-injected get_guide(`, so triggered
  operator rules and craft/domain skill bodies land in the same window counted by
  **nothing**. (Corrected 2026-09-02 — this bullet previously called
  `operator_rules::budget` "a second, independent ceiling over the same context
  window". It is not: `SIZE_CEILING = 10` counts *rules*, at compile time, over the
  `always` set `route()` never delivers. See
  `prompt-surface-measurement-session-log:F-46`.)
- `GG-7` (topics are atomic nodes in an unmodelled graph) — that graph is what
  the spec's `GET /api/engines/graph` route renders. GG-7 stops being purely a
  correctness bug and becomes the data model for the operator surface.

**Next:** do `GG-3` as a pure refactor, no behaviour change. It serves both
streams and blocks neither.

## Template for new entries

```
## GG-N — <one-line title>

**Status:** open | in-progress | done | deferred
**Valid:** dated YYYY-MM-DD | invariant | conditional — <event>

**Observed.** <what you ran, and what it returned>

**Next:** <the concrete action>
```

## History

### 2026-08-28 — opened

Seeded GG-1..GG-7 from the Phase 1 plan's `## Out of scope for Phase 1` and
`### Carried deferred minors` sections plus one open bug. Phase 1 delivery
re-confirmed live the same day.
