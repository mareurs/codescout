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
entry_high_water_GG: 7
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

**Status:** open — highest-value item on the deferred list
**Valid:** dated 2026-08-27

`guide_blocks_for`, `inject_hint`, `GuideDeliveryShape` and `guide_block` are
~190 lines nested inside a ~408-line trait method, and touch neither `self` nor
`ctx`. Extracting them to a module-private `guide_emit` turns three end-to-end
tests into unit tests.

The Phase 1 whole-branch reviewer promoted this but called it *follow-up, not a
merge blocker*: the code is correct, only its testability is the problem.

**Next:** pure refactor, no behaviour change. Do it before Phase 2 adds more
delivery paths through the same method.

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
