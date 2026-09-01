---
id: '0021bead4e5a01e2'
kind: spec
status: draft
title: Retrieval engine coordination — one selector, one ledger, one operator surface
owners:
- marius
tags:
- prompt-surfaces
- retrieval-engines
- coordination
- dashboard
- operator-surface
- gate
topic: prompt-surfaces
---

# Retrieval engine coordination — one selector, one ledger, one operator surface

Six retrieval engines were enumerated 2026-08-27. Two of them ship. This spec is about the
thing between them, which already exists in the tree, has no name, and cannot be seen.

**Companion specs:** `2026-08-27-get-guide-section-grain-design.md` (engine 1),
`2026-08-27-operator-rules-engine-design.md` (engine 5, and the current home of the
family table), `2026-08-30-tracker-grain-and-corpus-topology-design.md` (§ *Relationship to
the GG-N queue — one graph, not two*, which reached half of this conclusion from the
tracker side).

---

## Purpose

The engines are not six products that happen to sit in one binary. They are built to be
**operated**: an operator must be able to see the connections, the graphs and the rules that
decide what an agent receives on a given call — and modify them. That requires a
**coordinator**, a **management system** and a **preview system** over a surface the engines
share.

That purpose has never been written down anywhere. This spec is its home.

## The family

Six engines, enumerated 2026-08-27 by walking the surface inventory in
`src/prompts/README.md` § *Surfaces* plus the tracker corpus. **This table is canonical and
lives only here** (§ *Gates*, gate 5); the operator-rules spec, which held it until
2026-09-02, now points at this section.

The discriminator is **the key you retrieve on**, because that determines the corpus, the
retrieval mechanism, and what the ledger must remember.

| Engine | Retrieval key | Corpus | Status (verified 2026-09-02) | On the shared surface? |
|---|---|---|---|---|
| 1. codescout guiding | call shape | compiled-in guides | **Phase 1 shipped.** Phases 2–3 open as `GG-1`…`GG-8` | **yes** — owns `selector_key` and `parse_shape` |
| 2. model helper | served model | empty by measurement | needs a base arm before a design | n/a |
| 3. project corpus | task topic / graph | librarian artifacts | unbuilt | n/a |
| 4. outcome coaching | *result* shape | `err_family`, already ranked | unbuilt | n/a |
| 5. operator | the human | `~/.claude*/CLAUDE.md` + `docs/trackers/operator-rules.md` | **shipped.** `OP-1` resident in 3 profiles; `triggered` routing live | **yes** — borrows 1's grammar, shares 1's ledger |
| 6. craft / domain | task intent | `SKILL.md`, buddy specialists | exists, own retrieval | **no** — see below |
| **7. session opener** | **session phase** | compiled-in guides (shared with 1) | **shipped** — `SESSION_OPENING_GUIDE`, one topic | **yes** — shares 1's namespace, deliberately |

> **Engine 7 was added 2026-09-02, found by implementing Layer 1 rather than by
> re-reading the roster.** The 2026-08-27 enumeration walked the *surface inventory*;
> this one only becomes visible when you enumerate the **ledger write sites**, because
> it stamps a bare topic name indistinguishable from engine 1's. It fires on the first
> eligible call of a session regardless of shape — *session phase*, not call shape — so
> under this table's own discriminator it is a separate engine, not a mode of engine 1.
>
> Two consequences worth carrying forward. **The discriminator works**: applied to the
> code rather than to the docs it produced a member the doc-level pass missed. And
> **"six" was never a count of a closed set** — it was a count of what one instrument
> could see, which is exactly the failure mode this repo names when a bound is published
> without its population. Read the roster as "the engines we have enumerated", and expect
> Layer 2 to find more when key construction routes through the registry.

`domain`, `model` and `task-shape` are **facets that cut across engines**, not engines. The
evidence is in engine 5's own corpus: *"Sonnet is the floor for subagent dispatch"* is a
**model** rule living in the **operator** corpus. Treating each facet as an engine yields an
engine per facet-combination.

**Engine 6 is the case that makes the surface necessary rather than merely tidy.** It ships,
it delivers guidance into the same context window as engines 1 and 5, and it does so through
a retrieval path none of them can see: skill invocation. It has no `selector_key`, no ledger
key, and no line in any budget. A coordinator that cannot account for it is measuring a
fraction of what the agent actually receives — and *neither* the p50 guide ceiling nor
`operator_rules::budget` counts a single byte of it.

Registering engine 6 is therefore not a Layer-1 afterthought; it is the first test of whether
the registry models the family or only the two members that happen to share an emission site.
## Problem

### 1. The family has no document, and its only table is stale

The six-engine enumeration lives in `§ Where this sits` of the *operator-rules* spec — a
per-engine document. Nothing else lists the family. That table's status column reads
`1. codescout guiding … 9 of 10 tasks`.

Measured 2026-09-02: engine 1's Phase 1 is **complete and shipped**, all ten tasks, and has
its own eight-item resume queue (`docs/trackers/resume-get-guide-section-grain-phases-2-3.md`,
`ff63538dfc9b2d8b`). Task 10's byte-ceiling test is live at
`src/server.rs::a_p50_session_stays_under_the_committed_guide_byte_ceiling`. The row was
correct when written and decays with every ship, because a per-engine spec is the wrong
owner for cross-engine state.

### 2. A shipped spec asserts an independence the code does not have

The operator-rules spec's scope section says of the other engines:

> Each is a key type plus a corpus and **ships separately**.

Verified false, 2026-09-02. Engines 1 and 5 already share four mechanisms:

| shared | where | what holds it |
|---|---|---|
| retrieval key | `Tool::selector_key` — default inverted to universal in `30b6fc41` | `every_registered_tool_supplies_a_selector_key` (`src/server.rs:3448`) |
| shape grammar | `prompts::guide_index::parse_shape` | imported by `operator_rules::render` (`render.rs:46`) |
| session ledger | `ctx.guide_hints_emitted` : `GuideLedger` | `op:OP-N` vs `<topic>#<heading>`, kept disjoint by `op_keys_collide_with_no_guide_key` |
| emission site | `Tool::call_content` (`src/tools/core/types.rs`) | one `selector_key` computed once, fanned out to both consumers before `val` is moved |

This is not incidental reuse. It is a coordinator — computed key, ordered fan-out, shared
dedup namespace, single emission point — written inline and called nothing.

### 3. The coupling is unmanaged, and that is the actual defect

Nothing enumerates the engines at runtime. Nothing can answer *"what would this call
draw?"* without issuing the call and reading the result. Nothing renders the graph. The
namespace disjointness that keeps the two ledgers from corrupting each other is defended by
**one hand-written pairwise test**, which is correct for two engines and does not scale to
six.

An operator's only instrument today is to make a call and inspect what came back — which
also stamps the ledger, so the instrument spends the thing it measures.

### 4. One byte budget, covering part of the window, and two emitters bounded by nothing

> ⚠ **Rewritten 2026-09-02 at the start of Layer 2. The first version of this section was
> wrong in every clause, and the error is recorded as
> `prompt-surface-measurement-session-log:F-46`.** It read: *"Engine 5 enforces its own
> `SIZE_CEILING` in `operator_rules::budget`. Both spend the same context window and neither
> knows about the other. Two budgets over one resource is not a budget."* I had not opened
> `budget.rs`. `SIZE_CEILING = 10` is a count of **rules**, checked at **compile time**
> (`operator_rules::mod:47`, `corpus.rs:40` — never on the delivery path), over the
> `always` set — which `route()` excludes **unconditionally**. It governs exactly the set
> that is never delivered per call. The corrected picture below is worse for the system and
> supports the same gate for a different reason.

There is **one** byte budget, it covers **part** of the window, and two emitters are bounded
by nothing at all.

| emitter | bound | unit |
|---|---|---|
| guide sections (push) | `CEILING = 12_000` in `a_p50_session_stays_under_…` | bytes, p50 session |
| session opener (engine 7) | the same ceiling — it emits a `get_guide(` block | bytes |
| operator `always` (resident) | `SIZE_CEILING = 10`, compile time | **rule count**, and on a *disjoint* set |
| operator `triggered` (per call) | **nothing** | — |
| craft skills (engine 6) | **nothing** | — |

Engine 1 also caps each declared section at `MAX_DECLARED_SECTION_BYTES = 2500`.

**The exclusion is deliberate, not incidental.** The ceiling test's `shape_total` sums only
blocks containing `<!-- auto-injected get_guide(`; operator rules emit
`<!-- operator-rule OP-N …`. So the one real budget is *written* not to see the other
engine's bytes — which is defensible for a test named after guide injection, and indefensible
as the system's only accounting of what an agent receives.

Sharpening it: `GG-4` records engine 1 at **11,946 B against 12,000 — 54 B of slack**
(measured 2026-08-27; re-derive before relying on it). That margin is defended against guide
prose and against nothing else. Any triggered operator rule, and every skill body, lands in
the same window carrying no accounting whatsoever.
---

## Design

Four layers. Each is separately shippable, and each is a precondition for the next.

### Layer 1 — Registry: engines become values

An engine declares itself rather than being an inlined branch:

```rust
pub struct EngineDecl {
    pub id: &'static str,          // "guide-sections", "operator-rules"
    pub key: RetrievalKey,         // CallShape | Model | Topic | ResultShape | Operator | Intent
    pub ledger_prefix: &'static str, // "" for `<topic>#<heading>`, "op:" for rules
    pub mode: Mode,                // Push | Pull | Both
    pub corpus: CorpusRef,
}

pub static ENGINES: &[EngineDecl] = &[ /* … */ ];
```

**`ledger_prefix` becomes declared rather than defended.** The pairwise
`op_keys_collide_with_no_guide_key` test generalises into a property over the registry:
no two registered prefixes may be prefixes of one another. That is a real improvement to an
existing guard, not new scaffolding — and it is the reason Layer 1 is worth building before
engine 3 exists rather than after.

**Precondition: `GG-3`.** `guide_blocks_for`, `inject_hint`, `GuideDeliveryShape` and
`guide_block` are ~190 lines nested inside a ~408-line trait method and touch neither `self`
nor `ctx`. You cannot register what is inlined. GG-3 is already the queue's
*"highest-value item on the deferred list"* on testability grounds alone; it is also this
spec's first task.

### Layer 2 — Coordinator: the fan-out, named

Extract what `call_content` already does into a `retrieval::Coordinator`:

```
selector_key(input)  ──┐
                       ├─→ for each ENGINE (registry order)
result value      ──┘        └─ engine.emit(sel, &val, &mut ledger) -> Vec<Block>
                             ↓
                       one ordered block list, one budget, one ledger
```

Three things move here that today have nowhere to live:

- **Ordering** is currently the source order of two inlined blocks. It becomes registry
  order, and therefore reviewable.
- **The budget** becomes one number over all engines, absorbing Task 10's ceiling rather
  than sitting beside it. § *Gates* makes this the shape of the gate.
- **Ledger stamping** happens in exactly one place, so *"was this delivered?"* has one
  answer.

Behaviour must be byte-identical on the first commit. This layer is a refactor.

### Layer 3 — Preview: ask without spending

```rust
pub fn preview(sel: Option<&str>, result: &Value) -> Vec<(EngineId, Vec<Block>)>;
```

Given a selector key and optionally a result shape, return what **every** engine would emit
— without emitting it and without stamping the ledger.

This is the management primitive, and the precedent is already shipped:
`codescout operator-rules check` is exactly this for one engine, complete with a `Drift`
type and a non-zero exit. Generalise the surface:

```
codescout engines list
codescout engines preview --selector artifact.update
codescout engines check                  # drift across every engine, one exit code
```

Preview is also a **test** primitive. GG-3's stated win — *"turns three end-to-end tests
into unit tests"* — is the same lever pulled once for the whole family.

### Layer 4 — Operator surface: the dashboard already exists

`src/dashboard` is an axum server behind the `dashboard` feature: 13 routes, a static
JS/CSS frontend, and — importantly — `/api/memories/{topic}` already has a **`post`**
handler and a `delete`. Read-write operator surfaces are precedent here, not a new idea.

Three routes:

| route | serves |
|---|---|
| `GET /api/engines` | Layer 1's registry: id, key type, corpus, prefix, mode, live counts |
| `GET /api/engines/preview?selector=…` | Layer 3, rendered |
| `GET /api/engines/graph` | nodes = sections + rules; edges = `serves:` (shape→node) and `requires:` (node→node) |

The graph route is the one worth naming carefully: **it is the graph `GG-7` says is
unmodelled.** GG-7's finding is that guide topics are atomic nodes in a graph nobody
modelled, and that three guides already cite sections the API cannot serve. Rendering it is
how those three become visible instead of inferred.

**Modification, not just viewing.** The `serves:` / `requires:` declarations live in
markdown comments in the guide corpus, and the operator rules live in a markdown ledger.
Both are already text an editor can write. What the dashboard adds over an editor is
*validation before write* — Layer 3 previewing the consequence of a declaration change
before it is committed.

> ⚠ **Gate hazard.** Everything under `src/dashboard` is feature-gated. `cargo test --lib`
> and `cargo clippy` **silently skip it** — reporting "filtered out", not failures — unless
> `--features dashboard` is passed. The project gate in `CLAUDE.md` does not include it.
> Any Layer 4 task must state which command actually exercises its tests, or it ships
> untested behind a green gate. This is `Loudness is a property of a PATH` in its
> cheapest form.

---

## Inter-engine communication — scoped, and mostly declined

The purpose statement raises it, so it gets an answer rather than silence.

**Today the engines do not need to talk. They need to not collide, and to share a budget.**
Layers 1 and 2 deliver both. Direct engine→engine calls are declined for the same reason
`get-guide-section-grain` declined topic→topic edges and the topology spec declined a second
graph model: an edge type added before a measurement demands it is a guess with a
maintenance cost.

What is *not* declined, and is deferred to a measurement rather than a design:

- **Suppression.** If an operator rule and a guide section carry the same imperative on the
  same call, the agent receives it twice. Whether that is dilution (`A-20`'s finding) or
  reinforcement is **unmeasured**. The coordinator is the place a suppression rule would
  live; do not build one until an arm shows the cost.
- **Cross-engine `requires:`.** A rule that only makes sense given a guide section is
  expressible today only by restating the section. Same posture: record, do not build.

Both belong to the coordinator if they are ever built, never to an engine — which is the
design consequence, and it is available now.

---

## Gates

1. **Registry totality.** Every code path that can stamp `GuideLedger` is a registered
   engine. Mirrors `every_registered_tool_supplies_a_selector_key`; fails the build on a
   new unregistered writer.

   > **Shipped at partial resolution, and the shortfall is named rather than hidden.**
   > `engines::tests::every_live_ledger_key_has_a_registered_owner` asserts every key the
   > *registered* corpora emit has an owner. A brand-new engine writing a brand-new
   > namespace passes it trivially — the gate cannot see a writer it was never told
   > about. Closing that needs key **construction** to route through the registry, so
   > that an unregistered engine cannot mint a key at all. That is Layer 2's job, and it
   > is the difference between a gate and a convention.
2. **Disjointness, conditioned on corpus.** Two engines drawing on **different**
   corpora must own disjoint key spaces. Replaces `op_keys_collide_with_no_guide_key`
   at N engines instead of 2. Shipped as
   `engines::tests::engines_over_different_corpora_own_disjoint_key_spaces`.

   > ⚠ **Corrected 2026-09-02, before implementation, by enumerating the write sites.**
   > This gate first read *"no registered `ledger_prefix` is a prefix of another"*. That
   > is **wrong**, and it would have failed on correct code the day it landed.
   >
   > Production has **six** ledger writers, not two. Four belong to `guide-sections`
   > (whole / preamble / section / explicit `get_guide` fetch), and a fifth is the
   > **session opener** — an engine the spec's own six-engine enumeration missed,
   > because it retrieves on *session phase* rather than call shape and stamps a bare
   > topic name indistinguishable from `guide-sections`'. That overlap is deliberate
   > and argued at the site: keying the opener finer *"would desync this trigger from
   > what `GuideLedger::re_arm` actually re-arms."*
   >
   > A collision **within** one corpus re-delivers the same bytes; a collision **across**
   > corpora lets one engine's stamp silence another's unrelated content. Only the
   > second is a defect, so only the second is gated. `RetrievalKey::SessionPhase` and
   > the `Corpus` field both exist because of this find.
3. **One budget.** A p50 session's **total** emission across all engines is under one
   committed ceiling, counted by **emitter** rather than by comment marker. Absorbs Task
   10's `CEILING`; does not sit beside it.

   > **Corrected 2026-09-02** (`prompt-surface-measurement-session-log:F-46`). This gate
   > used to read *"absorbs Task 10's ceiling rather than sitting beside it"* on the premise
   > that a second byte ceiling existed to reconcile. It does not. There is one ceiling and
   > two unbudgeted emitters, so the work is **extending** an accounting to emitters that
   > have none — not merging two numbers. A gate that summed the guide ceiling's bytes and
   > `SIZE_CEILING`'s rule count would have passed, and been cited afterwards as proof the
   > budgets were unified.
4. **Preview fidelity — and note which assertion discriminates.** The obvious gate is
   *"preview leaves the ledger unchanged"*, and it is **monotone under removal**: a preview
   that returns nothing at all passes it. The discriminating assertion is that preview's
   output is **byte-equal to what the live path emits for the same selector and result**;
   the ledger-unchanged check rides along and catches the opposite direction. Mutating
   `preview` to return `vec![]` must red the suite.
5. **Family table has one home.** The engine table exists in exactly one file, and a test
   asserts no other spec carries a copy — the failure mode being cured is a stale duplicate,
   which is what § *Problem 1* is an instance of.

---

## Rollout

| step | content | blocked on |
|---|---|---|
| 0 | **GG-3** — extract the delivery helpers out of the 408-line trait method. Pure refactor. | — **done** `d0065423` |
| 1 | Layer 1 registry + gates 1 and 2 | — **done** `src/engines/mod.rs` |
| 2 | Layer 2 coordinator, byte-identical behaviour + gate 3 | Layer 1 |
| 3 | Layer 3 preview + `codescout engines` CLI + gate 4 | Layer 2 |
| 4 | Layer 4 dashboard routes | Layer 3 |
| — | Move the engine table here; leave a pointer in the operator spec + gate 5 | — (do first, it is one edit) |

Steps 0–2 are worth doing even if Layers 3 and 4 never ship: they replace a pairwise test
with a property, give two competing budgets one number, and make the fan-out reviewable.

## Not in scope

- **Designs for engines 2, 3 and 4.** This spec says what they must plug into, not what
  they are. Engine 2 still `needs a base arm before a design`.
- **Rewriting any engine's corpus.** Guide decomposition is `GG-1`/`GG-2`; rule wording is
  the operator spec's own exclusion.
- **Cross-machine sync** (`CM-10`) and the **cross-user vs cross-machine ledger scope**
  question the operator spec records as UNDECIDED. Both are upstream of this and unchanged
  by it.
- **The 44.4% `contradicted` rate.** Enforcement, not coordination — excluded by both
  companion specs for the same reason.

## Measurements this spec rests on

Verified 2026-09-02 by reading the tree unless marked otherwise.

- Engine 1 Phase 1 complete: ten tasks traced to commits `1cb4d588` (splitter) →
  `00020b88` (grammar) → `52265dfc` (index) → `34f5ad44` (matching + closure) →
  `fed362cd` (librarian declarations) → `94396e00` (gates) → Task 10's ceiling test at
  `src/server.rs:7981`. The plan file shows 0/50 boxes ticked; the boxes are wrong, the code
  is there.
- `librarian.md` carries 13 `serves:` declarations; the other nine guides carry zero.
- `MAX_DECLARED_SECTION_BYTES = 2500` (`guide_index.rs:272`).
- `src/dashboard`: 96 KB, 13 routes (`routes.rs:35-53`), `post` and `delete` handlers on
  `/api/memories/{topic}`.
- Operator engine live: block resident in all three profiles, byte-identical at 3,845 B;
  `operator-rules check` → `all 3 profiles current`. One `always` rule (`OP-1`); `OP-2`,
  `OP-3`, `OP-4` are `triggered`; `OP-5` retired.
- **Inherited, dated 2026-08-27, re-derive before use:** guide corpus 106,755 B / 67 `##`
  sections; `librarian` 20,545 B; `tracker-conventions` 35,492 B; the p50 draw of 11,946 B
  against the 12,000 B ceiling (`GG-4`).
