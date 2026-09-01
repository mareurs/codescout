---
kind: spec
status: draft
title: get_guide section grain — declare what each section serves, deliver only that
tags:
- prompt-surfaces
- get_guide
- guide-injection
- grain
- gate
topic: prompt-surfaces
---

# get_guide section grain

Auto-injected guides are delivered whole. This spec makes the **section** the unit of
delivery, selected by a declaration the section itself carries.

Evidence base: [`../../evals/2026-08-27-guide-injection-use.md`](../../evals/2026-08-27-guide-injection-use.md)
(method) and `docs/issues/2026-08-27-guide-topics-are-atomic-nodes-in-an-unmodelled-graph.md`
§ *Measured — USE* (results). Corpus measurements new to this spec are reproduced in
§ *Measurements this spec rests on*.

---

## Coordination — this engine is not alone in the pipeline

Added 2026-09-02, after this spec's Phase 1 shipped.

The machinery below is now **shared**. `operator_rules::render` imports `parse_shape` from
`prompts::guide_index`; `operator_rules::route` matches on the same `Tool::selector_key` this
spec introduces; and both engines stamp the same `GuideLedger` under disjoint key namespaces
(`<topic>#<heading>` here, `op:OP-N` there). All four hand-offs happen inside
`Tool::call_content`.

Two consequences for anyone reading this spec as a build guide:

- **`selector_key` is no longer this engine's private projection.** Its default was inverted
  to universal in `30b6fc41`, and `every_registered_tool_supplies_a_selector_key`
  (`src/server.rs:3448`) now holds every tool to it. A tool returning `None` is invisible to
  *both* engines, not just this one.
- **The p50 byte ceiling in Task 10 is not the whole budget.** `operator_rules::budget`
  enforces a second, independent ceiling over the same context window, and engine 6
  (craft/domain skills) is counted by neither.

The coordinator, its registry, the preview surface and the dashboard routes are designed in
[`2026-09-02-retrieval-engine-coordination-design.md`](2026-09-02-retrieval-engine-coordination-design.md)
(`0021bead4e5a01e2`). Its Rollout step 0 is `GG-3` from this engine's own resume queue — the
two streams meet there.
## Problem

`Tool::relevant_guide_topic(&self, result) -> Option<&str>` returns a **topic name**;
`guide_block()` feeds it to `topic_body()`, a hardcoded `include_str!` match. There is no
unit smaller than "entire topic" anywhere in the pipeline, and `GuideLedger` dedups on that
same string — so a topic fires once per session, whole, and never again.

Measured consequences (n = 81 injections over 10 sessions; delivery census n = 1,705
sessions):

| | |
|---|---|
| `U0_UNUSED` | **66.7%** |
| section-grain utilisation | **15.5%**, an upper bound; median per-injection **0.0%** |
| `librarian` bytes never touched | **94%** (utilisation 5.6%) |
| `TRIGGER_ONLY` — governed class never recurs | **51%** |
| arrived at first contact | **11%** |
| guide text as share of session context | median **14.0%** |
| clean delivery per session | **31,899 B**, median 3 injections |

The existence proof for the fix is in the same data: `symbol-navigation` (3,145 B, narrow)
landed on a turn-29 result and the agent's *very next* `symbols` call adopted its verbatim
`name_path=` form — absent from the prior 30 turns — and held it for 15 of the remaining 21
calls. Small and targeted is adopted immediately; large and general is not.

### Not in scope

The **44.4% `contradicted`** rate — guidance that arrives and is then violated. That is an
enforcement problem, not a grain problem, and wants a separate design. This spec is
accountable only for grain, and is falsifiable on the prediction in § *Verification*.

Topic→topic edges are also out of scope. The measurement says topic *selection* is not what
fails; section→section edges within a topic are in scope, because slicing is what creates
that boundary (§ *`requires:` edges*).

---

## Design

```
call_content(input, ctx)
  ├─ sel    = self.selector_key(&input)        // "artifact.append_entry" — BEFORE the move
  ├─ val    = self.call(input, ctx).await?
  ├─ slices = GUIDE_INDEX.match(sel, &val)     // ordered, most-specific first
  ├─ closed = transitive_requires(slices)
  ├─ new    = closed.filter(|s| !ledger.contains(s.key()))   // key = "topic#heading"
  └─ emit primary + one guide block per new slice
```

### 1. The corpus contract

A section declares the call shapes it serves, in an HTML comment directly under its heading:

```markdown
## Entry ids
<!-- serves: artifact.append_entry, artifact.update_entry -->
<!-- requires: Declaring a ledger -->
```

**Grammar, deliberately minimal.** Extending it requires amending this spec.

```
serves-decl   := "serves:" shape ("," shape)*
shape         := tool ["." action] ["(" pred ")"]
pred          := "path~" substring
requires-decl := "requires:" heading
```

Note: `requires-decl` is deliberately **not** comma-split, unlike `serves-decl` — a
heading is prose and commonly contains its own commas (e.g. "docs/trackers/ — Backing
Store, Not a Docs Folder"), so one `requires:` line names exactly one heading. Multiple
requirements are multiple `<!-- requires: ... -->` lines under the same section. The
implementation (`parse_declarations`, `src/prompts/guide_index.rs`) enforces this; an
earlier grammar draft here showed `requires:` as comma-separated like `serves:`, which
was never implemented and would have been wrong to implement.

- `tool` is the codescout tool name without the `mcp__codescout__` prefix.
- `action` matches the `action` field of the call's input, via `selector_key`.
- `path~<substring>` matches against path-valued fields of the **response**, using the
  existing `names_tracker_path` machinery in `src/librarian/adapter.rs` generalised to an
  arbitrary substring.
- `requires:` names sibling headings **within the same topic**. Cross-topic requires are
  not permitted; they would reintroduce the topic-graph this spec excludes.

**Declarations may appear at `##` or `###`.** A `###` declaration overrides its parent
`##`. This is what lets `librarian` (14 well-sized sections) declare at `##` while
`tracker-conventions` decomposes its two oversized sections at `###` without a rewrite.

**The parser MUST skip fenced code blocks.** Guides teach this syntax by example, and a
worked example is not a declaration. This is the same rule `link_scan` already applies to
`**Valid:**` detection. It is called out because the omission is not hypothetical: the
first three section-size measurements taken while drafting this spec were wrong, because
`^## ` matched a line inside a fence in `tracker-conventions`, inflating its section count
by one and mis-splitting a 17,378 B section into a phantom 12,099 B one.

A second measurement defect, caught in the same review and worth the same care: reading
the corpus in Python text mode counts **characters, not bytes**, and these guides are dense
with em-dashes and arrows. Every figure in this spec is bytes, matching the injection
study. In a codebase whose `server_instructions` cap is deliberately a *character* cap, the
two units must never be quoted interchangeably.

### 2. The parsed index

Guides are parsed once into `Vec<Section { topic, heading, level, byte_range, serves,
requires }>`. Three hand-maintained tables become projections of it:

| today | becomes |
|---|---|
| `topic_body()` — `include_str!` match arm per topic | index lookup |
| `GUIDE_TOPICS` — const array | derived from files on disk |
| `GetGuide`'s `summaries` map (`src/tools/guide.rs:62-117`) | derived from section headings |

`get_guide(topic)` itself is unchanged: an explicit fetch still returns the whole topic
inline (`force_inline()` stays `true`). This spec changes the **push** path only.

### 3. `selector_key` — seeing the call

`call_content` (`src/tools/core/types.rs:669`) moves `input` into `call()` on its first
line, which is why `LibrarianAdapter` must infer intent from the *response* today. That
inference is why a read (`artifact(get, headings=[…])`) is indistinguishable from bug-file
authoring and draws the archive flow and the compaction ladder.

Add a trait method projecting a small owned key **before** the move:

```rust
/// Projected BEFORE `call()` consumes input. Default: None (tool opts out, zero cost).
fn selector_key(&self, _input: &Value) -> Option<String> { None }
```

```rust
// LibrarianAdapter
fn selector_key(&self, input: &Value) -> Option<String> {
    let action = input.get("action")?.as_str()?;
    Some(format!("artifact.{action}"))
}
```

Not a full `input` clone: `create_file` and `edit_file` inputs carry whole file bodies, and
a clone would be paid on 100% of tool calls to benefit the ~3% that inject.

### 4. Ledger at section grain

`GuideLedger`'s key becomes `topic#heading`. Dedup is **per-section and unlimited**: a
session doing varied work receives different slices as it goes, rather than one dump at
first contact. This is what converts the 51% `TRIGGER_ONLY` finding — today one fire spends
the topic for the whole session.

**Migration.** The ledger persists to `~/.local/state/codescout/guide_hints/` keyed by topic
string. The file gains a format-version field; an unrecognised or absent version reads as
**empty**, not as "everything already sent". A half-migrated session must fail toward
re-delivery, never toward silence.

### 5. `requires:` edges

Sections are not independent. `tracker-conventions` § *Entry ids* states the resolver's
token grammar and the server-allocates rule — but whether any of it applies depends on
§ *Declaring a ledger*, which establishes that a ledger is far narrower than a tracker
(measured: 27 trackers, 3 ledgers). Delivered alone, § *Entry ids* is entry-id law with its
precondition stripped out: individually true, jointly misleading.

`requires:` is closed transitively before emission, and the closure is deduped against the
ledger so an already-delivered prerequisite is not re-sent.

This is the `advisors:` edge from the buddy specialist-graph refactor, placed at the grain
where it earns its cost. At topic grain it would address a failure the measurement does not
find; at section grain it addresses the boundary this spec creates.

### 6. Fallback

An unmatched shape receives the **topic preamble plus a `get_guide(topic)` pointer** —
283 B for `librarian`, 663 B for `tracker-conventions` — never the whole topic, never
silence.

Falling back to the whole topic was considered and rejected: it makes the change
strictly-improving, but every byte of the win then depends on declaration coverage being
complete, which is unfalsifiable. Coverage is instead a **finite checklist** (§ *Gate 2*),
which is what makes the narrow fallback safe.

**Starvation degrades to "late", never "never".** If a needed section is undeclared for the
shape that first wanted it, per-section-unlimited dedup means a later call of any matching
shape still delivers it. Today's per-topic ledger has the opposite property. The new
failure mode is strictly weaker than the current one.

---

## Gates

All five fail the build.

1. **Malformed `serves:` / `requires:` fails.** Never a silent skip. This project has been
   bitten precisely here — a checker missing its exec bit reported a clean `0/N`,
   character-identical to a genuine floor.
2. **Coverage.** Every shape in the committed census (§ *Measurements*, 88 rows) has at
   least one declaration or an explicit waiver carrying a reason. **Replaces**
   `every_guide_topic_is_triggered_or_declared_pull_only` (`src/server.rs:3187-3252`),
   preserving `PULL_ONLY_GUIDE_TOPICS`' convention that a waiver states its rationale, at
   88× the resolution.
3. **Size cap — 2,500 B per declared section.** Over-cap fails, naming `###` decomposition
   as the remedy. This is the gate that stops the win eroding, and the need is not
   theoretical: `tracker-conventions` gained bytes on 2026-08-27 mid-study, and
   `iron-laws-detail` gained a further 769 B (`5d3f8ebe`) during the half hour this spec
   was being drafted — pushing its § *Iron Law 3* section from under the cap to 3,602 B.
4. **No dangling `requires:`** — a heading named by a `requires:` must exist in the same
   topic.
5. **Reachability.** Every section is declared, transitively required, or explicitly marked
   pull-only. No fourth state. This is what stops `librarian-runtime`'s 9,774 B being one
   waived line.

### Runtime tests

- Per-section dedup: two differently-shaped calls yield two different slices; the same
  shape twice yields one.
- Unmatched shape yields the preamble only — asserted on bytes, not on presence.
- Stale topic-keyed ledger file reads as empty, not as fully-emitted.
- `selector_key` default `None` leaves a tool byte-identical to today. This is what lets
  Phase 1 ship touching only `LibrarianAdapter`.
- Fenced `<!-- serves: … -->` inside a code block is **not** parsed as a declaration.
- **Golden byte budget:** a scripted 6-shape session asserts total delivered bytes against
  a committed ceiling. The project already does this for the 1900-character
  `server_instructions` slice; it is the mechanism that stops a corpus edit quietly undoing
  the work.

---

## Rollout

**Phase 1 — `librarian` only.** 14 sections, median 1,616 B, 2 over cap. Declarations for
the six shapes that are 89% of artifact traffic, plus the tail. Only `LibrarianAdapter`
implements `selector_key`; every other tool keeps the `None` default and behaves exactly as
today. Ship, accumulate sessions, re-measure.

**Phase 2 — `tracker-conventions`.** Blocked on decomposing § *Entry-level standard*
(17,378 B) and § *Bug files* (10,170 B) at `###`. That is a corpus edit, separable from the
mechanism and reviewable on its own.

**Phase 3 — the remaining eight topics**, including the four that have never auto-injected
(`iron-laws-detail`, `librarian-runtime`, `untrusted-content`, `error-handling` — 28,955 B,
**27%** of the corpus; the study's 28,186 B was measured before `5d3f8ebe`). Gate 5 names their sections individually rather than accepting a bulk
waiver.

Corpus-wide the decomposition debt is **6 sections over cap out of 67**.

---

## Verification

`scripts/probe_guide_injection.py` already measures bytes-per-session, injections-per-session
and duplicate rate on real transcripts; no new instrument is needed. After Phase 1:

> **`librarian`'s contribution falls from ~20,545 B to ~10,000 B at p50** (6 distinct shapes
> × ~1,616 B median, and lower once shapes collapse onto shared sections), with injection
> **count up** and total bytes **down**.

If bytes do not fall, the declarations are too broad. If sessions start missing guidance
that used to arrive, Gate 2's census was under-covered — both are diagnosable from the same
probe output.

---

## Measurements this spec rests on

Taken 2026-08-27 against `~/.claude`, `~/.claude-sdd`, `~/.claude-kat`; 1,740 transcripts,
138 main sessions at ≥20 assistant turns, of which 105 use artifact/librarian tools. The
138 is consistent with the injection study's 128 eligible, which added model and
injection filters on top of the same turn threshold.

**Distinct call shapes per session** — the bound on slices delivered, since distinct shapes
is an upper bound on distinct sections:

| | p50 | p75 | p90 | p99 | max |
|---|---|---|---|---|---|
| artifact/librarian shapes | 6 | 8 | 11 | 17 | 17 |
| all codescout shapes | 15 | 23 | 30 | 44 | 47 |

**Global shape census** — 88 distinct shapes across 170,465 calls; 25 touch
artifact/librarian. Six of those are 9,695 of 10,920 artifact/librarian calls (**89%**):
`artifact.update` (3,820), `artifact.get` (3,148), `artifact.find` (1,583),
`artifact.append_entry` (401), `artifact.create` (385), `artifact.move` (358).

**Guide corpus, fence-aware, byte-exact** — 67 `##` sections, 106,755 B. `preamble` is what
an unmatched shape receives under § *Fallback*:

| guide | bytes | sections | preamble | median | max | over 2,500 B |
|---|---|---|---|---|---|---|
| `tracker-conventions` | 35,492 | 6 | 663 | 2,142 | 17,378 | 2 |
| `librarian` | 20,545 | 14 | 283 | 1,616 | 4,080 | 2 |
| `iron-laws-detail` | 12,007 | 7 | 357 | 1,664 | 3,602 | 1 |
| `workspace-state` | 10,355 | 9 | 239 | 772 | 3,274 | 1 |
| `librarian-runtime` | 9,774 | 10 | 564 | 914 | 2,315 | 0 |
| `progressive-disclosure` | 5,669 | 5 | 118 | 1,205 | 1,793 | 0 |
| `untrusted-content` | 5,317 | 5 | 217 | 837 | 1,840 | 0 |
| `symbol-navigation` | 3,145 | 3 | 343 | 1,036 | 1,483 | 0 |
| `project-activation-bootstrap` | 2,594 | 5 | 197 | 533 | 829 | 0 |
| `error-handling` | 1,857 | 3 | 79 | 636 | 846 | 0 |

Reproduce: `scripts/probe_guide_injection.py` for delivery; the shape census and
section-size table are one-off scripts recorded in this spec's tables rather than kept as
instruments, because both are inputs to a design decision rather than ongoing signals.

### Field note — the session that wrote this spec

Drafting this document, before a single design question was asked, the session received
four topics totalling **46,900 B** — `project-activation-bootstrap` (2,594),
`symbol-navigation` (3,145), `progressive-disclosure` (5,669), `tracker-conventions`
(35,492). That is 47% above the clean-session figure of 31,899 B, and 44% of the entire
guide corpus, drawn by four exploratory calls.

The 35,492 B arrived because `artifact(action="get", headings=[…])` — a **read of one
heading** — names a path under `docs/issues/`, and `names_tracker_path` cannot tell a read
from an authoring call. Under this spec that call matches `artifact.get`, and the reader
receives the sections that serve reading.
