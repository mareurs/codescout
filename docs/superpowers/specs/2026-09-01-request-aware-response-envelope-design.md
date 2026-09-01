---
kind: spec
status: draft
title: Request-aware response envelopes
owners: [marius]
tags: [progressive-disclosure, envelope, context-cost, guides]
topic: response envelope composition
---

# Request-aware response envelopes

**Status:** draft, awaiting review · **Opened:** 2026-09-01 · **Branch:** `experiments`
· **HEAD at authoring:** `bb4688fd`

## Problem

Three of codescout's read surfaces — `read_file`, `read_markdown`, `artifact(get)` —
attach **advisory payload** to every response: a `preview` block, a compact `summary`, a
next-step `hint`, and an auto-injected `get_guide` section. All are composed at the
response layer, where the request's arguments are out of scope. So a caller who *narrows*
a request is billed the *un-narrowed* advisory, and the extra payload reads as
helpfulness rather than as an error.

The corpus has been instantiating this for months without naming it as one thing.

## Evidence

Measured 2026-09-01 on this repo. Counts are of **bug files**; derivations are given
because a cited count decays and a derived one can be re-run.

| Measure | n | Derivation |
|---|---|---|
| Bug files whose **slug** names one of the three tools | **20** | 9 `read-file`, 3 `read-markdown`, 8 `artifact-get`; verified no file matches two slugs |
| Bug files carrying one of 5 **envelope-shaped** cluster tags | **31** | anchored per-slug `git grep -cl` union (`^\s*-\s*cluster/<slug>\s*$`) over the IC-13/15/19/21/22 slugs, open + archive — 9+15+3+2+2, verified no cross-slug overlap; unanchored `grep -l` overcounts to 32 by also matching prose mentions |
| Files under `docs/issues/` mentioning any of the three | 154 files / 825 matches | any mention — an upper bound, not a claim |

Relevant classes in `docs/trackers/issue-clusters.md`: **IC-22**
(`hint-composed-without-the-request`), **IC-13** (`capped-result-presented-as-complete`),
**IC-15** (`accepted-parameter-silently-dropped`), **IC-19**
(`truncated-window-ordered-by-the-wrong-key`), **IC-21**
(`instrument-omits-the-dimension-that-grows`).

### The composition sites

| Mechanism | Site | Composed from | Request in scope? |
|---|---|---|---|
| `preview` | the `out["preview"]` assignment in `get::call` (`src/librarian/tools/get.rs`) | the artifact body | **No** — assigned before `body_selected` is read at the same site |
| compact `summary` | `LibrarianAdapter::format_compact` | the response | **No** |
| overflow `hint` | `LibrarianAdapter::json_path_hint` | the response shape | **Was no; fixed `bb4688fd`** |
| guide trigger | `relevant_guide_topic(&self, result: &Value)` | the response | **No, by signature** — 12 definitions: 11 tool-level overrides + the trait default |

The last row is the strongest evidence, because it is not an implementation slip. The
trait signature (`src/tools/core/types.rs:1453`) admits only the result, so
request-awareness is **unrepresentable** rather than merely unimplemented. That is IC-6 —
*an addressing scheme with no escape hatch* — applied to an internal API, and it predicts
the rows above it: when the only parameter in scope is the response, every advisory field
gets composed from the response. `src/tools/markdown/read_markdown.rs:488` returns
`Some("progressive-disclosure")` unconditionally and ignores `_result` for exactly this
reason.

### Cost, derived

One real call — `artifact(get, id=…, heading="## IC-22 …")` against
`docs/trackers/issue-clusters.md`:

- payload: **3,210 bytes** (exact, from the response's own `body_meta.bytes`)
- preview block: **~2,611 bytes** (reconstructed from the file's real heading texts)
- ratio: **~81%**

*Exact:* the 20-entry cap, the heading texts, the payload size. *Approximate:* `summary`
modelled at 140 bytes, and the script counted only `##` headings (30) where the response
reported `total_headings: 32` across all levels. This is a derivation with a stated error
bar. **Nothing here rests on the precise value** — the defect stands at any ratio.

This is the measurement the shipped hint fix said was missing. Its `## Fix` records: *"The
`preview.headings` suppression was NOT done, and is not owed. The bug file proposed
measuring whether envelope metadata pushes otherwise-inlinable sections over the 9 KB
budget. That measurement was not run, so nothing here rests on it."* The measurement now
exists, so the deferral's own stated precondition is met.

### A correction, recorded because it changed the scope

This work's first pass claimed the heading-scoped `get` reproduced IC-22 live. **It did
not.** Re-reading the response, its `hint` correctly said `$.body` — the `bb4688fd` fix
was working. What was actually observed was the `summary` and `preview` carrying the full
section map. Change 2 is scoped down accordingly. The distinction matters operationally: a
reader who reproduces the *hint* defect at `HEAD` has found a regression, not this class.

## The invariant

> **An advisory field — `preview`, `summary`, `hint`, injected guide — is a function of
> *(request, response)*. When the request carries a selector, the advisory narrows to
> match it.**

## Change 1 — gate the preview on selector presence

**Site:** `src/librarian/tools/get.rs:535-540`. **Bug:**
`docs/issues/archive/2026-09-01-a-scoped-read-is-billed-the-full-heading-map.md`.

`Args` (`:88–116`) already carries every selector — `full` `:101`, `heading` `:103`,
`occurrence` `:107`, `headings` `:109`, `start_line` `:111`, `end_line` `:113`,
`entry_filter` `:115` — and `body_selected` already exists at `:538`. The change hoists
that computation above the preview assignment at `:536` and branches:

```json
"preview": { "shape": "default", "line_count": 1190, "total_headings": 32,
             "headings": "omitted (selector present) — call with no selector for the map" }
```

Retaining `total_headings` is deliberate: it reports the **magnitude** withheld rather
than merely its absence, which answers the IC-21 shape in passing.

### Change 1b — the compact summary, same discriminator

On a scoped read the summary leads with the full section list before reaching
`$.body starts:`. `LibrarianAdapter::format_compact` should lead with the requested
section instead, keyed on the **same `body_meta` discriminator** the shipped hint fix
established: `body_meta` is emitted only when the server scoped the read, so it is the
narrowest available signal for *"the caller named a part, and this is that part."*
Reusing it keeps one discriminator across hint and summary rather than inventing a second.


## Change 2 — hint: SHIPPED, and the remaining half probed clean. No work.

`bb4688fd` (patch-id `5e6ff450ad5eaf822283499492288b7ded15faf3`) already routes scoped
librarian reads to `$.body`. The two probes this spec made a precondition were run
2026-09-01 and **both came back clean**:

- **Buffer extraction is size-driven, not unconditional.** `read_file(@tool_*, json_path=…)`
  returned content **inline** for a 1-line scalar and for a 52-line array; only a 263-line
  extraction buffered. The "three calls to read one section" observed while authoring was
  progressive disclosure working correctly on a genuinely large section — **not** a
  request-blind hint. The first reading of that observation was wrong.
- **The non-librarian `json_path_hint` defaults name the new handle plus useful next
  steps**, which is the right answer for a payload the caller did not scope.

**Closed with no code change.** Recorded rather than deleted, per CLAUDE.md § *Testing
Discipline*: *"instrument the doubt, not the correction — when a re-derivation confirms,
publish the confirmation."* A confirming probe leaves no artifact by default, so the
population of "suspicions that turned out fine" is unfalsifiable unless someone writes one
down. This is one. It is a **denominator**, not a catch.
## Change 3 — CLOSED BY PROBE: already enforced centrally, and the prescribed fix would regress

**CLOSED BY PROBE 2026-09-02 — no work, and the prescribed fix would REGRESS.** Read this
before re-opening; everything below it is kept for the reasoning, not as a work item.

*Already enforced, centrally.* `src/tools/core/types.rs:1281-1294` (**at HEAD `4672799b`** — see
the coordinate note at the end of this block) gates the topic inside `call_content`, for **every**
tool:

```rust
let should = match topic {
    "progressive-disclosure" => {
        exceeds_inline_limit(&json)
            || val.as_object().and_then(|o| o.get("output_id"))
                  .and_then(|v| v.as_str()).is_some()
    }
    _ => true,
};
if !should { continue; }
```

So the six tools returning `Some("progressive-disclosure")` unconditionally — `grep`, `tree`,
`read_file`, `read_markdown`, `run_command`, `semantic_search` — already never ship it on a call
that did not overflow. The `continue` precedes `guide_blocks_for`, so a suppressed candidate has
**zero** delivery effect and **zero** ledger effect. `Symbols::relevant_guide_topic` says so in
its own comment — *"`call_content` gates it on overflow having actually happened, so returning it
there delivers nothing at all"* — and its `if` exists to **repurpose the slot** with
`symbol-navigation` (`BL-25`), not to suppress an injection. This spec read that pattern as the
missing fix; it is a second, different purpose that only the three symbol tools have.

*Already tested, with the exact pair § Test plan prescribes.*
`src/server.rs:7557 run_command_without_overflow_no_progressive_hint` is the absence side,
`:7579 run_command_with_overflow_emits_progressive_hint_once` its positive twin (which also pins
per-session dedup). Both green 2026-09-02. `run_command` is one of the six, so that pair is
direct proof the central gate suppresses an unconditional per-tool return.

*And the fix would regress.* Making those six return `None` on a non-overflowing result skips the
enclosing `else if let Some(content_topic) = self.relevant_guide_topic(&val)` branch **entirely**,
so `topic_declaring` is never consulted and the declaring-section **fallthrough** — added
2026-08-31, immediately below at `types.rs:1265-1272` (HEAD) — becomes unreachable for those
tools. The
unconditional `Some` is what keeps that path live. It is inert today only because
`grep -l 'serves:' src/prompts/guides/*.md` returns one file and `librarian.md` declares no
`grep`/`read_file` shape, so the regression is **latent**: it would land the day someone authors a
`serves:` section for one of the six. A no-op that is load-bearing.

*Fourth `reconnaissance-patterns:R-117` datapoint* — a fix naming a population asserts that
population is non-empty, and this is the form that fails **green**: it compiles, its tests pass
(written from the same wrong model), the diff is clean, and nothing anywhere reports a change. The
wrinkle this instance adds is that "harmless if unnecessary" was **also** false, so the usual
fallback reassurance did not hold either. Both halves were answerable by reading two call sites.

*Coordinate note, and it is a hazard worth stating rather than a tidy-up.* The line numbers in
this block are **HEAD's**, and the first draft of it cited a different set — 1076-1083 and
1063-1066 — taken from `grep`/`read_file`, which read the **working tree**. On this shared
checkout a peer held `src/tools/core/types.rs` mid-edit and **206 lines shorter than HEAD** (1321
vs 1527), so those coordinates described uncommitted code that exists in no commit and may never.
`symbols` disagreed with `grep` about the same file by 206 lines, and that disagreement is the
only reason it was caught: `symbols` reads the AST/LSP index, `grep` and `read_file` read the
worktree, `git show HEAD:` reads the commit. Three instruments, two worlds, no error from any of
them. **The conclusion is unaffected** — the gate and the fallthrough are present in HEAD with
identical semantics, verified at `git show HEAD:src/tools/core/types.rs` — but a citation into a
shared checkout is a claim about a worktree unless it says otherwise, and on this one that is not
the same thing as a claim about the code.

---

**The signature change shown below is withdrawn.** It is kept visible rather than deleted,
because a rejected design a later reader might re-propose is worth showing along with the
reason it lost:

```rust
fn relevant_guide_topic(&self, result: &Value) -> Option<&str>                    // before
fn relevant_guide_topic(&self, args: &Value, result: &Value) -> Option<&str>      // after
```

**Withdrawn because the codebase had already considered and rejected it**, with a stated
reason found only by reading the trait:

> *"A cheap projection of this call's shape, taken BEFORE `call()` consumes `input`.
> Deliberately not a clone of `input`: `create_file` and `edit_file` inputs carry whole
> file bodies, and a clone would be paid on 100% of tool calls to benefit the ~3% that
> inject a guide."* — `src/tools/core/types.rs:1433-1438`

`relevant_guide_topic`'s own comment says it from the other side: *"Result-based rather
than input-based because `call_content` moves `input` into `call()` before the hint is
computed"* (`src/librarian/adapter.rs:331-333`). The `selector_key` hook exists precisely
to carry a cheap projection across that boundary, and `call_content` already computes it
at `src/tools/core/types.rs:827`, before `input` is consumed.

**This spec's earlier framing was therefore an over-claim, corrected here.** This is *not*
IC-6 — *no escape hatch* — applied to an internal API. There **is** an escape hatch, it is
documented, and it is already plumbed. The signature encodes a deliberate cost tradeoff,
not an oversight.

**And the widening is unnecessary, which is the stronger reason to drop it.** Every rule
this spec wants is expressible from data the callee already receives:

| Rule | Discriminator | Where it already lives |
|---|---|---|
| preview stub on a scoped read | selector present | `Args` in `src/librarian/tools/get.rs` — no trait involved |
| summary leads with the section | `body_meta` present | the result |
| overflow-guide only on overflow | `output_id` present | the result |

The work is to **make the implementations read the `_result` they are already handed**, and
no signature changes. `src/tools/markdown/read_markdown.rs:488` returns
`Some("progressive-disclosure")` — the guide about *handling overflow* — unconditionally,
with `_result` ignored.

**Governing rule, unchanged:** a guide about handling overflow does not ship on a call that
did not overflow.

**Checked and deliberately NOT assumed: this does not enable section-grain delivery.**
Raised by a peer session 2026-09-01 and verified here independently. Section-grain is
blocked one layer *below* routing, at `selector_key`: `Shape::matches`
(`src/prompts/guide_index.rs:179`) opens `let Some(sel) = sel else { return false };`, and
its own comment states that rejecting `None` is deliberate and must not become a wildcard.
`selector_key` is overridden in only five tools — `src/librarian/adapter.rs` (×2),
`src/tools/create_file.rs`, `src/tools/edit_file/mod.rs`, `src/tools/memory/mod.rs` — so
`read_markdown` returns the trait default `None` and can never match a declared section.
Change 3 does not touch that layer and does not claim to.

**The trap this closes off.** The obvious next optimisation is *"declare `serves:` sections
on progressive-disclosure so a caller gets a slice instead of the whole guide."* That
configuration fails as a **silent downgrade to preamble-only** — a plausible result, not an
error — and is refused by a gate shipped at `b769277b`. Do not reach for it.

**A consequence, and it is why the cost figure above is right.** `grep -l 'serves:'
src/prompts/guides/*.md` returns exactly one file: `librarian.md`.
`progressive-disclosure.md` declares nothing, so it takes `guide_blocks_for`'s
non-declaring branch and ships **whole**. An unconditional route therefore costs a full
guide, not a preamble.

Established twice by routes with genuinely different scopes: directly observed in this
session (a `grep` call delivered the entire guide) and reached independently by the peer's
`selector_key` enumeration. That is what corroboration looks like when it counts — unlike
two per-profile instruments, which agree *because* of a shared blind spot.


**Cost, stated honestly rather than inflated.** `GuideLedger` deduplicates per topic per
session, so an unconditional return costs **one injection per session, not one per call**
— whichever tool fires the topic first pays it. That bounds this change far below Change 1,
which is per-call and unbounded. Recorded because this spec's first draft implied
otherwise: a fix sized by its headline rather than its measured cost is how effort goes to
the wrong place. The `symbols()` observation below is real but costs ~2.5 KB **once**, not
per call.

Its genuinely unbounded failure mode is the ledger being **cleared** mid-session — the
activation-flip defect recorded immediately below, which belongs to workspace-state rather
than to this change.

### An adjacent defect found while authoring — filed separately, not fixed here

The `project-activation-bootstrap` guide was injected **three times** in one session, each
copy asserting *"first call this session"* (~6 KB total). The same response carried
`_workspace_notice: no project has been explicitly activated`, though the session opened
with `workspace(action="activate")`; a later `create_file` was then hard-blocked with
*"git worktrees detected but workspace(action='activate') has not been called."*

The mechanism is a peer session on this shared checkout flipping the process-global active
project. The guide ledger is cleared on activate **by design**
(`src/tools/core/types.rs:1448-1450`), so the re-injection is downstream of the flip, not
a guide bug. This is a **workspace-state** defect surfacing *through* the guide channel;
widening this spec to cover it would confuse the two. Recorded so it is not lost, and
because it means guide-injection volume cannot be measured on a shared checkout without
first controlling for activation flips.

## Test plan

Three guarded **sites**, so three independent mutation runs. A kill on `get.rs` proves
nothing about the summary builder or the guide trigger.

Every gating assertion here is **monotone under removal** — a dead preview builder
satisfies "preview is stubbed" perfectly — so each ships with its positive twin. Neither
member of a pair covers the property alone.

| Absence-side | Paired positive | Note |
|---|---|---|
| `preview_stubbed_when_selector_present` | `preview_present_by_default` | twin **already exists**, `get.rs:1024` |
| `scoped_summary_leads_with_the_requested_section` | `full_read_summary_still_leads_with_the_map` | mirrors the shipped hint fix's four-row table |
| `no_guide_on_non_overflowing_scoped_read` | `guide_still_ships_on_first_overflowing_read` | |

**Compile-time guard.** The envelope builder takes the selector as a **required**
parameter, so a tool added later cannot omit it — it fails to compile rather than silently
leaking. Preferred over a policy someone must remember, per CLAUDE.md § *Observer
Blindness*: make the correct path end in a safe state so compliance leaves nothing armed.

**Every test must be shown to fail.** The shipped hint fix set the standard to match — it
recorded a mutation table and annotated which row is the discriminator that must not be
deleted as redundant. Same rigour here.

## Out of scope — decisions, not gaps

- **No tool renames, merges, or new parameters.** A unified read surface was considered and
  declined: it would touch all three prompt surfaces, the 1900-character cap, and every
  existing agent habit.
- **No caller-declared verbosity flag.** It puts the burden on the caller to know the
  preview is costing them, which is the failure mode itself — a trigger the model must
  notice is a policy, not a mechanism.
- **`artifact(get)` body-relative vs file-relative line numbers**
  (`docs/issues/2026-08-31-artifact-get-line-numbers-are-body-relative-not-file-relative.md`)
  is a payload *correctness* defect, not an envelope one.

## Open questions

1. **IC-22 grain.** The claim says a *next-step hint*; the new member is advisory payload
   that is not a hint. Admitting it either widens the claim to "system-authored advisory"
   or wants a sibling class. Flagged in both the ledger and the bug file rather than
   silently widened — the ledger owner's call.
2. ~~Change 2 probes must run before that section is written.~~ **Run 2026-09-01 — both
   clean; Change 2 closed with no code change. No longer open.**
3. Should the preview stub be omitted entirely rather than stubbed? Stubbing keeps
   `total_headings` discoverable; omission is cheaper. Recommend stubbing.

## References

- `docs/issues/archive/2026-09-01-a-scoped-read-is-billed-the-full-heading-map.md` — Change 1.
- `docs/issues/2026-09-01-heading-scoped-get-overflow-hint-points-at-metadata.md` — the
  shipped hint fix; its `## Tests added` is the mutation-table standard this spec adopts.
- `docs/trackers/issue-clusters.md` § IC-22, IC-6, IC-21.
- `src/librarian/tools/get.rs:536`, `src/tools/core/types.rs:1453`.
