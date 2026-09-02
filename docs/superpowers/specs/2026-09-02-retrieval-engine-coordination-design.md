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

Seven retrieval engines are known; four ship, and only three are coordinated at all — the
fourth spends the same context window while participating in no ledger and no budget. This
spec is about the thing between them, which already exists in the tree, has no name, and
cannot be seen.

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

**Seven engines.** Six were enumerated 2026-08-27 by walking the surface inventory in
`src/prompts/README.md` § *Surfaces* plus the tracker corpus; the seventh was found
2026-09-02 by enumerating ledger write sites instead — see the note under the table, which
is also the reason to read this roster as open rather than closed. **This table is canonical and
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
`src/server.rs::a_p50_session_stays_under_the_committed_emission_byte_ceiling` (renamed from
`..._guide_byte_ceiling` by Plan 3 Task 2, which also widened what it counts — see § *Problem 4*
and § *Measurements this spec rests on*). The row was correct when written and decays with every
ship, because a per-engine spec is the wrong owner for cross-engine state.

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
seven.

> **Partly closed 2026-09-02.** Layer 1 shipped `src/engines/mod.rs`, so the engines *are*
> now enumerable at runtime and the pairwise test is subsumed by a property over the
> registry. The other three clauses stand: preview is Layer 3, the graph is Layer 4, and
> the registry still cannot see a path that bypasses the fan-out entirely (Gate 1).

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

> ⚠ **Updated 2026-09-02, Plan 3 Task 2.** The table and prose below describe the state this
> problem statement diagnosed. Two of its five rows are now covered — see the "Resolution"
> block that follows the table.

There was **one** byte budget, it covered **part** of the window, and two emitters were bounded
by nothing at all.

| emitter | bound (as diagnosed) | unit | status |
|---|---|---|---|
| guide sections (push) | `CEILING = 12_000` in `a_p50_session_stays_under_…` | bytes, p50 session | **covered** — always was |
| session opener (engine 7) | the same ceiling — it emits a `get_guide(` block | bytes | **still uncovered** — see Resolution |
| operator `always` (resident) | `SIZE_CEILING = 10`, compile time | **rule count**, and on a *disjoint* set | unchanged; a deliberately separate budget, not this one |
| operator `triggered` (per call) | **nothing** | — | **now covered structurally** — see Resolution |
| craft skills (engine 6) | **nothing** | — | **still uncovered**, and cannot be from here — see Resolution |

Engine 1 also caps each declared section at `MAX_DECLARED_SECTION_BYTES = 2500`.

**The exclusion was deliberate, not incidental.** The ceiling test's `shape_total` summed only
blocks containing `<!-- auto-injected get_guide(`; operator rules emit
`<!-- operator-rule OP-N …`. So the one real budget was *written* not to see the other
engine's bytes — which was defensible for a test named after guide injection, and indefensible
as the system's only accounting of what an agent receives.

Sharpening it: `GG-4` recorded engine 1 at **11,946 B against 12,000 — 54 B of slack**
(measured 2026-08-27; re-derive before relying on it). That margin was defended against guide
prose and against nothing else. Any triggered operator rule, and every skill body, landed in
the same window carrying no accounting whatsoever.

**Resolution, 2026-09-02 (Plan 3 Task 2).** `shape_total` no longer filters on the guide
marker — it sums every block in the call's `Content` array after the primary, renamed to
`a_p50_session_stays_under_the_committed_emission_byte_ceiling`. Measured at 12,116 B (up from
the guide-only 11,872 B), against a re-derived `CEILING = 13_300`. What that buys, precisely,
because a partial fix stated as a full one is the failure mode this section itself records:

- **`operator triggered`** is now structurally counted — a triggered rule's
  `<!-- operator-rule OP-N …` block is no longer filtered out. **Not exercised by the p50
  fixture**: none of its six shapes (`artifact` create/get/update/append_entry/find/move)
  matches a `serves:` declaration in `docs/trackers/operator-rules.md`, so the measured 12,116 B
  contains zero operator-rule bytes today. The mechanism is fixed; the fixture doesn't reach it.
- **`session opener` (engine 7) is still not covered**, and widening cannot change that:
  `call_tool_checked` routes through `warm_ledger`, which stamps `SESSION_OPENING_GUIDE` into
  the ledger **before every call**, so `emit_session_opener` finds its key already set and
  declines on all six shapes. This is a fixture property, not a filter — the ceiling still
  covers two of the three wired engines, not three.
- **`craft skills` (engine 6) remains structurally unreachable from this test**, `Mode::Unmanaged`
  — skill bodies reach the context window through the harness, never through an MCP response, so
  no assertion on `Content` blocks can see a byte of them.
- **A discovery beyond this problem's original scope**: widening also swept in bytes from
  `post_process` (`src/server.rs`) — the once-per-activation `[codescout] paths are relative to
  …` banner and `## Project Status (details)` block, 244 B in this fixture. `post_process`
  predates the six-engine family and is not one of its emitters; it happens to land in the same
  `Content` array, so counting every block after the primary picks it up. Real bytes a p50
  session's first call receives, but not "a managed emitter" in this spec's sense — see the
  `CEILING` constant's own comment in `src/server.rs` for the full population note.
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

### Layer 2 — Coordinator: two phases, one of them total

> ⚠ **Rewritten 2026-09-02, after scouting the dispatch site.** The signature below
> replaces `engine.emit(sel, &val, &mut ledger)`, which made a pre-execution phase
> **unrepresentable** — `val` is the tool's *result* and does not exist before the call.
> The same scout re-aimed Gate 1 (see there), and found that three of the four things a
> blocking phase needs already ship, built for unrelated reasons.

The coordinator fans out **twice** around `self.call`, and the two halves live at
different sites because they need different data:

| phase | site | has | totality |
|---|---|---|---|
| **Pre** | `server.rs::call_tool_inner`, above `acquire_write_guard_if_writing` | `req.name`, `&input_for_record` | **total** — the one production dispatch, unoverridable |
| **Post** | `types.rs::call_content` | `&val` | one known bypass, `Onboarding::call_content` |

```
// server.rs::call_tool_inner
sel = tool.selector_key(&input_for_record)
(pre_blocks, verdict) = coordinator.run(Phase::Pre, sel, &input_for_record, &mut ledger)
if verdict == Block { record(blocked); return CallToolResult::success(pre_blocks) }
...acquire_write_guard_if_writing...

// types.rs::call_content
val = self.call(input, ctx).await?
post_blocks = coordinator.run(Phase::Post, sel, &val, &mut ledger)
```

`tool` is in scope at the dispatch site, so the pre phase calls the **trait method** rather
than `action_selector_key` directly — a tool that overrides `selector_key` must not see a
different key in the two phases. The two phases compute it independently rather than
threading it through `call_content`'s signature, which is safe **only** because
`selector_key` is required to be a pure function of the input. Nothing enforces that today;
Gate 1's neighbourhood is where it belongs.

Both phases pass a `&Value` — the input pre, the result post — so `Shape::matches` is
reused **verbatim**: its `tool`/`action` clauses read `sel`, and its `path~` clause reads
whichever `Value` the phase supplies.

That reuse is not wishful. `names_path_containing` checks top-level `rel_path` first, and
an `artifact(action="create", rel_path=…)` **input** carries `rel_path` at top level — so
`serves: pre artifact.create(path~docs/issues/)`, the motivating example, matches with no
matcher change at all. **It works by intersection rather than by design**, and the
difference is load-bearing: the function is named and documented for *responses*, and its
other two shapes (`items`, `violations`) are response-only. Input shapes need their own
enumerated list, and that function's doc comment already warns why — a missing shape
fails as a *wrong guide* rather than an error.

Three things move here that today have nowhere to live: **ordering** becomes registry
order rather than source order, and therefore reviewable; **the budget** becomes one
number over all engines; **ledger stamping** happens in exactly one place, so *"was this
delivered?"* has one answer.

#### Why the pre phase goes to the server

`src/server.rs:1122` is the **only** production call to `call_content` — every other hit
in the tree is a test. Placing the pre fan-out there closes the `Onboarding` bypass
**structurally** rather than by gate: the correct path becomes the only path, which is the
shape `73066479` established for gate ordering. It also puts the strongest guarantee under
the riskiest mechanism, and a blocked call never takes the cross-process write lock,
because the block returns above `acquire_write_guard_if_writing`.

Three preconditions were already satisfied by machinery built for other reasons:

| a blocking phase needs | already in the tree |
|---|---|
| a way to return without executing | `acquire_write_guard_if_writing`'s `Err(result) => return Ok(result)` (`server.rs:1113-1116`) — the write guard's contention refusal exercises it today |
| the input, before dispatch | `let input_for_record = input.clone();` (`server.rs:1101`) — unconditional, for usage recording |
| a bound on how often a rule fires | the ledger's dedup (`types.rs:1130`) — see below |

That unconditional clone also **scopes** the anti-clone argument in
`get-guide-section-grain` § 3: it is correct inside `call_content`, and moot at the
server, where the clone is already paid one line above.

#### Disposition, and why the breaker is the ledger

A rule carries a disposition. `advise` appends blocks alongside the result; `block`
returns them **instead of** the result, so the tool does not run and the agent re-issues
the call.

`block` × `post` is incoherent — the side effects have happened and the agent cannot see
them — so it is made **unrepresentable** rather than rejected: `PreVerdict { Proceed,
Block }` exists only in the pre fan-out. A validator rejecting the combination would be a
weaker statement of the same rule.

**The breaker is the existing ledger, and it gives N = 1 with nothing to tune.**
`types.rs:1130` reads `if emitted.contains(&key) { continue; }` before
`emitted.insert(key)`, so a triggered rule delivers **exactly once per session**, keyed
`op:OP-N`. A rule can therefore block **at most once per session, by construction**. No
rule can brick a session, because no rule can fire twice.

That is strictly stronger than the in-tree precedent it replaces: codescout-companion's
`pre-tool-guard.mjs` ships a `BREAKER_THRESHOLD = 3` stand-down, which still permits three
blocks and requires the threshold to be right. It is also the correct *semantics*. The
purpose is **"make sure you have seen the taxonomy before filing"**, not **"prevent
filing"** — an agent that re-issues the call has done the thing the rule wanted.
**Visibility is not authority** applies to our own engines, not only to peer sessions.

Three gaps the ledger does not close, each with its own mechanism:

| gap | mechanism |
|---|---|
| **TTL re-arm.** `expire_idle` re-arms keys after idle, so a re-armed rule would block a second time. | Block does not re-arm. Once a rule has blocked, it degrades to `advise` for the life of the process, marked in the ledger's non-expiring `notices`. |
| **Cross-session brick.** A bad rule blocks the first matching call in *every* session, and the ledger is per-session. `corpus.rs:16` is `include_str!` — **editing `operator-rules.md` does not disarm a rule without a rebuild**, which the site's own comment states: *"Only ROUTING is pinned to build time."* | `CODESCOUT_BLOCK=off`, read once at the edge per [`../../conventions/test-env-isolation.md`](../../conventions/test-env-isolation.md) option A, degrading every block to `advise`. Given compile-time routing this is the only disarm available *inside* a session, so it is mandatory rather than defensive. Default **on** — a switch that defaults off is a feature nobody runs. |
| **"Policy, or is the tool broken?"** The blocked agent is the observer and cannot tell the two apart. | The block text names the rule id and states that the call did not execute; Layer 3's `engines preview --selector artifact.create` answers the question without issuing the call. This is why Rollout makes block depend on Layer 3. |

**A blocked call must be recorded.** The block returns above `recorder.record_content`, so
without an explicit record it is invisible to `usage.db` — and therefore to
`/analyze-usage` and `docs/trackers/tool-usage-patterns.md`, which is precisely where
whether blocking works would be measured. An enforcement mechanism whose effects no
instrument can see is this project's own `Loudness is a property of a PATH`.

#### Phase is declared, at shape grain

`EngineDecl` gains `phases: &'static [Phase]`.

| engine | phases | why |
|---|---|---|
| `guide-sections` | Post | a 2,500 B section before every write is a byte disaster, and reference material is not time-critical |
| `session-opener` | Post | it already fires on the first eligible call |
| `operator-rules` | **Pre and Post** | declared per shape |
| `craft-skills` | none | `Mode::Unmanaged` |

The `serves:` grammar gains an optional `pre` marker at **shape** grain, defaulting to
`post`:

```
<!-- serves: pre artifact.create(path~docs/issues/), artifact.update -->
```

Shape grain rather than section or rule grain, because one section can serve
`artifact.create` (worth a pre) and `artifact.get` (post-only) — section grain was
refuted on exactly that case.

**Declared, not derived.** *When this arrives* is the most load-bearing fact about a piece
of injected guidance, and deriving it would put that fact where the corpus author cannot
read it — `OB-1` § *the third position*.

> ⚠ **Corrected 2026-09-02, by a commit from another work stream.** This paragraph used to
> close: *"Deriving it from `is_write` specifically would also inherit that predicate's
> open hole (…), and `is_write` is tool-level where this needs per-action resolution."*
> The elided parenthetical was the bug file's 16-hex catalog id; it is dropped rather than
> quoted, because `id = sha256(abs_path)` re-keys on the archive move and a *quotation* of
> a retired sentence is not a citation worth dangling — the resolver cannot tell the two
> apart. The durable pointer is the fix itself, below.
>
> **Both legs have expired.** The hole was closed by `354ffac4` (patch-id
> `d4e6237ea3526776bc5b4441abd4677632624c0b`), which classifies librarian writes by
> exclusion rather than enumeration; and the same commit made the classification
> **per-action**, so the grain objection went with it. Verified here rather than accepted:
> `LibrarianAdapter::is_write` (`src/librarian/adapter.rs:300`) now reads the `action`
> field, and `is_write_classifies_every_action_outside_a_declared_read_set_as_a_write`
> (`src/server.rs:5752`) is its regression test.
>
> The conclusion stands, on a reason that never depended on the implementation and should
> have been the stated one: **`is_write` answers "does this call mutate?", which is not the
> question phase asks.** Phase asks "should this guidance arrive *before* the call?" — and
> the two come apart in both directions. A read can deserve a pre (an `artifact.get` under
> `docs/issues/` wants the taxonomy as much as a `create` does); a write can not need one
> (an `edit_file` on a scratch path). Deriving phase from mutation-ness conflates a
> **safety** predicate with a **pedagogy** predicate, and would do so however correctly
> `is_write` is implemented.
>
> Recorded rather than silently rewritten because the superseded form was an argument from
> a *defect*, and an argument that dies when someone fixes the defect was never load-bearing
> — the durable one was sitting underneath it the whole time.

**Phase 2a must be byte-identical.** Post-only, pure refactor, no new behaviour.

### Layer 3 — Preview: ask without spending

```rust
pub fn preview(phase: Phase, sel: Option<&str>, value: &Value)
    -> Vec<(EngineId, Vec<Block>, Option<PreVerdict>)>;
```

Given a phase, a selector key and the `Value` that phase would see — the input pre, the
result post — return what **every** engine would emit, without emitting it and without
stamping the ledger.

The verdict is in the return type because *"what would **block** this call?"* is the
question a blocked agent and its operator most need answered, and it is the reason
Rollout makes step 2c depend on this layer rather than the other way round. A preview
that could only show `advise` blocks would answer everything except the question with
teeth.

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

1. **Totality over call PATHS, not over ledger keys.**

   > ⚠ **Re-aimed 2026-09-02 by the Layer 2 scout.** This gate read *"every code path
   > that can stamp `GuideLedger` is a registered engine"*, which catches an
   > unregistered **writer**. The live hole is an unregistered **bypasser**:
   > `Onboarding::call_content` (`src/tools/onboarding.rs:294`) is the tree's only
   > override of the trait method, calls `self.call(input, ctx)` directly, and therefore
   > receives no selector, no ledger, no guide block and no rule. Nothing in the tree
   > declares that deliberate — and it is the *onboarding* tool, whose whole job is
   > orienting a session that has just arrived.
   >
   > Neither shipped gate can see it. `every_live_ledger_key_has_a_registered_owner`
   > enumerates **keys**; `engines_over_different_corpora_own_disjoint_key_spaces`
   > enumerates **corpora**. A bypasser writes no key, so it is invisible to both — the
   > exact mirror of the shortfall `src/engines/mod.rs` already admits about writers,
   > and a second instance of a gate returning a plausible pass rather than an error.

   Layer 2 closes the **pre** half structurally: the pre fan-out sits at
   `server.rs:1122`, the single production dispatch, which no tool can override. The
   **post** half keeps a declared exemption list — today of size one — where each entry
   states its reason, on `PULL_ONLY_GUIDE_TOPICS`' convention that a waiver is written
   down rather than assumed.
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
   >
   > **Done, 2026-09-02, Plan 3 Task 2 — for two of the two.** `shape_total` in
   > `a_p50_session_stays_under_the_committed_emission_byte_ceiling` (renamed from
   > `..._guide_byte_ceiling`) no longer filters on the guide marker, so `operator triggered`
   > is now structurally counted (not exercised by the p50 fixture's shapes today, but no
   > longer excluded by construction). `craft skills` is **not** covered and cannot be from
   > this test — `Mode::Unmanaged` bodies never pass through an MCP `Content` block. § *Problem
   > 4*'s table carries the row-by-row status; do not read this gate as "every emitter" —
   > it is two of the three wired engines, plus incidental bytes from the pre-existing
   > `post_process` onboarding hints that are outside the engine family entirely.
4. **Preview fidelity — and note which assertion discriminates.** The obvious gate is
   *"preview leaves the ledger unchanged"*, and it is **monotone under removal**: a preview
   that returns nothing at all passes it. The discriminating assertion is that preview's
   output is **byte-equal to what the live path emits for the same selector and result**;
   the ledger-unchanged check rides along and catches the opposite direction. Mutating
   `preview` to return `vec![]` must red the suite.
5. **Family table has one home.** The engine table exists in exactly one file, and a test
   asserts no other spec carries a copy — the failure mode being cured is a stale duplicate,
   which is what § *Problem 1* is an instance of.

Three more arrive with `block` (Rollout step 2c), and none of them is a byte gate — a
control surface is not priced in bytes:

6. **A `block` rule names an explicit action.** `serves: artifact` plus `block` bricks a
   whole tool for the first matching call of every session; only a shape carrying an
   action (`artifact.create`) may block. Checked in `validate`, against the parsed
   `Shape`, not against the source text.
7. **Block budget, separate from `SIZE_CEILING`.** Blocks are counted as control
   surfaces and capped at **3**. Gate 3's byte ceiling says nothing about them, and
   `SIZE_CEILING` counts a disjoint set (`always` rules) — folding either into the other
   is the mistake § *Problem 4* records.
8. **A block is self-describing and observable.** Its rendered text names the rule id and
   states that the call did not run; the blocked call appears in `usage.db`. Both
   asserted on the emitted bytes and the recorded row — not on presence, which is
   monotone under the failure being guarded.

---

## Rollout

| step | content | blocked on |
|---|---|---|
| 0 | **GG-3** — extract the delivery helpers out of the 408-line trait method. Pure refactor. | — **done** `d0065423` |
| 1 | Layer 1 registry + gates 1 and 2 | — **done** `src/engines/mod.rs` |
| 2a | Layer 2 coordinator, **Post phase only**, byte-identical + gate 3 | Layer 1 |
| 2b | **Pre phase** at `call_tool_inner`, `advise` only + gate 1 | 2a |
| 3 | Layer 3 preview + `codescout engines` CLI + gate 4 | 2a |
| 2c | **Block**: disposition, `CODESCOUT_BLOCK` switch, gates 6–8 | 2b **and 3** |
| 4 | Layer 4 dashboard routes | 3 |
| — | Move the engine table here; leave a pointer in the operator spec + gate 5 | — (do first, it is one edit) |

**2c depends on Layer 3, and the order is deliberate.** Preview is the instrument that
answers *"policy, or broken tool?"* for a blocked agent. Shipping block first would leave
the only observer of a misfire with no recourse but the env switch — which disarms every
rule at once and so cannot distinguish the bad one.

Steps 0–2b are worth doing even if block, Layer 3 and Layer 4 never ship: they replace a
pairwise test with a property, extend one byte accounting to emitters that have none, make
the fan-out reviewable, and close the `Onboarding` bypass.

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

**Layer 2 scout, 2026-09-02** — the facts the two-phase design rests on, each read this
session:

- `src/server.rs:1122` (`CodeScoutServer::call_tool_inner`) is the **only** production
  call to `call_content`. 76 matches across 10 files; every other one is a test.
- `src/tools/onboarding.rs:294` is the tree's **only** override of `Tool::call_content`,
  and it calls `self.call(input, ctx)` directly — no selector, no ledger, no injection.
- `let input_for_record = input.clone();` — `server.rs:1101`, unconditional.
- The return-without-executing path already exists: `server.rs:1113-1116`,
  `Err(result) => return Ok(result)` from `acquire_write_guard_if_writing`.
- Per-session rule dedup: `types.rs:1130-1133`, `contains` then `insert` on `op:OP-N`.
- `OPERATOR_RULES` is `include_str!`'d (`src/operator_rules/corpus.rs:16`); the site's
  comment states *"Only ROUTING is pinned to build time"* — `compile`/`check` read disk.
- `Shape::matches` (`src/prompts/guide_index.rs:179`) reads `tool`/`action` from the
  selector and `path~` from the supplied `Value`.
- `names_path_containing` (`src/util/librarian_response.rs:36`) scans top-level
  `abs_path`/`rel_path`, then `items[]`, then `violations[]`.

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
- **One budget, derived 2026-09-02.** The p50 session's total managed emission is
  12,116 B across six shapes, counting every block after the primary rather than only those
  carrying the `<!-- auto-injected get_guide(` marker. Ceiling set to 13,300 B. Covers
  `guide-sections` (exercised by this fixture) and `operator-rules` (structurally counted,
  but this fixture's shapes trigger none — a corpus fact, not a filter gap); **does not**
  cover `session-opener` (`call_tool_checked` stamps its ledger key before every call, so it
  always declines) or `craft-skills`, whose bodies never travel through an MCP response. The
  widened total also absorbs 244 B from `post_process`'s pre-existing, once-per-activation
  onboarding hints (the path-relative-to banner and `## Project Status (details)`), which
  predate and sit outside the six-engine family — real bytes a session's first call receives,
  not a managed emitter. Re-derive with
  `cargo test --lib a_p50_session_stays_under -- --nocapture`.
