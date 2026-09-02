---
kind: convention
status: active
title: What a green suite is evidence for — the measurements behind the laws
owners: []
tags:
  - testing
  - mutation-testing
  - epistemics
---

# What a green suite is evidence for — the measurements behind the laws

**Audience:** anyone adding a test, trusting one, or deciding a property is covered. The **laws**
live in [`CLAUDE.md`](../../CLAUDE.md) § *Testing Discipline* and stay there because they change
what you do at the keyboard. This page holds the **derivations** — the mutation runs, the
counts, and the superseded formulations — so the executable copy stays readable.

Each law below was promoted from a measured failure, not from argument. There is deliberately **no
count** of the laws, here or in `CLAUDE.md`: a tally of the section's own contents is a premise that
every addition falsifies. That sentence read *"all four"* for an hour after a fifth law landed,
which is § *Observer Blindness*'s premise-moved-conclusion-didn't firing inside the section that
documents it. Removing the number removes the class; correcting it would only have reset the clock.

## Monotone assertions — the `e6414362` run

**Law:** a test cannot detect a change its assertion is MONOTONE under.

Absence assertions (`is_empty()`, `!exists()`) are monotone under **removal** — a dead mechanism
produces exactly the silence they assert. Existence assertions ("a region *containing* X is found")
are monotone under **widening** — over-reporting satisfies them. Both look like guards, and neither
fires in its own direction, so a property held by one of each is covered **zero** times, not
weakly. For each test ask which direction its assertion is monotone under, and mutate the *other*
way.

Measured (`e6414362`): a locator widened to swallow its whole section killed **none of six tests** —
the silence test and the positive test were both blind, and pairing them bought nothing.

**Superseded formulation, kept because it is falsified above:** an earlier version of this law
prescribed *"pair an absence test with a positive one."* That is exactly the configuration the
`e6414362` run killed six-for-zero. Full history in `reconnaissance-patterns` R-132 and R-133.

## Recording filters — the harder twin, and why "widen the sample" is a no-op

**Law:** a test cannot detect what its RECORDING filters out.

The monotone law is about *members*: a population selected so no member can falsify. Its fix is to
add one that could — the nine dead-code probes that all had visible callers become informative the
moment a tenth is drawn from the dispatch side. This one is about *observations*: the refuting
outcome leaves **no artifact**.

Measured 2026-09-01 — a ledger was about to publish *"of the near-miss numbers, four were caught by
re-derivation and zero by inspection"*, and that zero is unfalsifiable by construction, because a
reader who doubts a figure and re-counts it produces nothing. The wrong number never ships, so
nothing is recorded, so the population contains only the cases where doubt failed to occur.
**"Widen the sample" fixes member-selection and is a no-op here at any corpus size** — a bigger
corpus of ledger entries still contains zero catches-by-reading, forever. That is what makes it
worse than a small sample: the reflex answer looks responsive and changes nothing.

The remedy is to instrument the **doubt**, not the correction: when a re-derivation *confirms*,
publish the confirmation. Nobody is inclined to, because there is no finding to report — which is
exactly why the negative case has no record. One null result was published that day only because
someone said so out loud, and it is a **denominator**, not a catch: a confirming re-derivation never
entered the wrong-number population and bears on the base rate rather than on the hit rate.
Absorbing it as a near-catch would make the population look self-correcting.

## One mutation per guarded SITE — the `artifact_augment` run

**Law:** mutate once per guarded SITE, not once per feature.

A mutation run answers a question about one *line*. Where a law is implemented at N call sites, one
kill proves exactly one site is guarded and says nothing about the other N−1.

Measured: `artifact_augment` had two shape-writing paths, and mutating each separately killed
**different** tests, neither failing under the other's mutation — so a single mutation would have
supported "covered" with the second site unguarded.

## Loudness is a property of a PATH — the `BL-66` alarm nothing reached

**Law:** loudness is a property of a PATH, not of a failure.

An alarm nothing reaches is exactly as informative as no alarm: `BL-66` *aborts the process* —
maximally loud — and survived anyway, because every in-tree construction site installs the provider
first. When adding a guard, alarm, error return or `panic!`, name the concrete caller that reaches
it **and** the observer who acts on what it emits.

"An external consumer we do not have in-tree" is a legitimate reason to keep one and is **not**
coverage of our own risk — say so at the test, or the green tick gets read as protection. The tell:
ask what an observer would *see differently* if this were broken right now; if the answer is
"nothing", the guard is decoration however loudly it is written.

## The law reaches past guards, to features — the un-registered tools

`ListFunctions` and `ListDocs` each implemented the `Tool` trait and were **registered nowhere** —
`src/server.rs` is where a tool joins the registry (`Arc::new(Grep)`), and neither name ever
appeared in it. Their module carried a full test suite that passed for months while no agent could
reach a line of it. Both were **deleted 2026-09-01**; the tree-sitter layer the module wrapped
(`crate::ast`) was untouched and remains load-bearing:

```text
src/tools/ast.rs — deleted 2026-09-01, along with ListFunctions and ListDocs
```

Keep `BL-66` above rather than treating this as its replacement: that is the **alarm** shape, a
`panic!` nothing arrives at, and it is the harder half to see. This is the **feature** shape, and it
is cleaner in exactly one way — `BL-66` had an out-of-tree consumer to argue about, and these had
none, so the green tick protected precisely nothing. Note what the unit is: the defect was the
*tests*, not the tools.

### Derive the count, don't cite it — one population, four defensible numbers

One population yielded **three** defensible numbers inside an hour, and each was the right answer to
a different question.

| number | question it answers |
|---|---|
| **18** | tests living in `mod tests` (19 functions less the `project_ctx_with_file` fixture) |
| **13** | tests exercising the tools by name — 12 there, plus `tests/integration.rs::workflow_analyze_ast` |
| **17** | tests guarding unreachable code — four formatter tests covered `format_list_functions` / `format_list_docs`, whose only production caller was the tools' own `format_compact` |

A first pass published **15**: 13 with two wrong inclusions — a JSON fixture containing the *string*
`"ListFunctions"`, and an e2e helper (`run_list_functions`) mistaken for a test. None of these was a
mistake about the code; they were four different units.

**A count of a defect population must arrive with its unit or not at all** — and note that 13, 15,
17 and 18 are near enough to each other that no reader would query any of them, which is §
*Observer Blindness*'s closing rule firing on the example added to illustrate it.

The deletion removed 18 tests across two lanes, 3424 → 3406 lean and 4991 → 4973 default: 17 from
the module plus `workflow_analyze_ast`, with the one test that covered a *reachable* feature —
`symbols(include_docs)` — moved to `src/tools/symbol/tests.rs` rather than lost. The matching delta
on both lanes is what proves nothing else went with them.

## Annotate the fixture — both directions

**Law:** annotate a fixture's load-bearing detail, on the fixture line.

The assertion states what must be true; nothing states which part of the *setup* is what makes the
test able to tell. Say what breaks if the detail goes — not in the test name, not in the assertion
message, and never as a bare "do not edit". A tidy-up that removes it leaves the test passing and no
longer discriminating, which no assertion can catch because that change is monotone too.

**The inverse is also owed, and is the worse of the two.** Annotate a fixture as **inert** where it
provides no coverage, so nobody credits it with coverage it does not have. One guards against silent
**removal**, the other against silent **credit** — and false coverage actively stops the next person
looking, where silent removal does not. Worked example: the `output_id` probe in
`every_declaring_topic_has_a_live_route_to_a_declared_section` (`src/server.rs`), annotated at the
fixture line as changing no outcome today and explicitly not to be cited as overflow coverage.
Promoted from `reconnaissance-patterns:R-161`.

## Scope, not direction — the p50 byte-ceiling run

**Law:** an assertion computed over a population cannot verify a claim about a member.

This is the axis the monotone law does not cover. That one asks which *direction* an assertion is
blind to; this one asks what *scope* it is computed over. A per-member claim checked against an
aggregate is vacuous for every member, and it reads as coverage rather than as a gap.

`a_p50_session_stays_under_the_committed_guide_byte_ceiling` (`src/server.rs`) sums guide bytes
across six tool shapes and asserts `total <= CEILING` and `total > 0`. Measured 2026-09-02, per
shape:

| shape | bytes |
|---|---|
| `create` | 2785 |
| `get` | 0 — legitimate; `create` already delivered its section and the ledger dedups per session |
| `update` | 3193 |
| `append_entry` | 1643 |
| `find` | 2233 |
| `move` | 2018 |

Total 11,872 against `CEILING = 12_000`. `total > 0` is a sum of six non-negative addends, so it
fails only if **all six** are zero — insensitive to five of them by construction. That much is a
proof rather than a measurement.

**Both aggregates are blind in the same direction, which is worse than one blind spot.** A section
ceasing to be delivered *reduces* the total, so it makes `total <= CEILING` **more** comfortable.
The pair's failure modes agree, so content vanishing moves both the safe way.

**The test already computed the discriminating value and dropped it.** `shape_total` returned
`bytes`; all six call sites discarded the return, silently, because `usize` is not `#[must_use]`.
The author had even written the predicate in prose — *"`get` is the ONE shape expected to report 0 B
here … Any OTHER shape reporting 0 B is suspicious, not normal."* A guard that would have worked was
sitting in the function, unreferenced. (The adjacent mode *was* closed deliberately: every call goes
through `call_tool_checked` rather than `call_tool`, because a silently-failed call reports 0 B and
is character-identical to legitimate dedup.)

### Two remedies falsified, and that is the reusable part

Mutation used throughout: rename `serves: artifact.append_entry` in `src/prompts/guides/librarian.md`
so that shape's section no longer matches — a realistic serves-drift, not a synthetic break.

| remedy | result |
|---|---|
| `total > 0` (the existing assertion) | **GREEN** — absorbed |
| `bytes > 0` per shape, exempting `get` | **GREEN** |
| assert no block carries the declared failure marker | **RED**, naming serves-drift |

The second is the one worth knowing, because it is what the law's own diagnosis implies and two
readers agreed on it independently. It fails for a reason neither could reason to: a shape whose
section is gone does **not** report 0 B. It receives a 491-byte fallback whose own marker reads
`no section declares this call's shape`. Every shape therefore has a floor above zero, and **no
non-emptiness assertion at any grain can discriminate** — not the aggregate, not the per-member one.
Under mutation `append_entry` reads 491, not 0.

Hence two rules that generalise past this test:

- **A per-member assertion is only as good as the member's ability to reach the failing value.** A
  deliberate, self-describing fallback floor makes that value unreachable, so the assertion is
  vacuous for a second and independent reason.
- **Demand an observed RED, never an assertion's existence.** That acceptance bar is immune to the
  trap by construction, because it separates reachable from unreachable without anyone needing to
  know the system's floors in advance. It is what saved the sibling instance below: the same
  underspecified *"a per-tool expectation replacing the global sum"* was read as an exact-count
  table (which discriminates, 3 ≠ 0) rather than as `found > 0` (which would not have). The sound
  reading was chosen, not specified.

### What worked — cause-naming over magnitude-guessing

The injector announces the condition in the block it emits — `src/tools/core/guide_emit.rs:182`
writes `preamble — no section declares this call's shape` — so the discriminator was in the output
all along while both readers reached for a number. **Where a system already names its own failure
state, assert on the name rather than on a proxy for it.** It also needs no per-shape labels, no
call-site edits and no ordering assumption — all three of which the rejected shapes required.

**Annotating this fix's own weakness, per the fixture law above: it matches a STRING.** A reword of
that marker leaves the assertion passing and no longer discriminating, and no assertion can catch
that because the change is monotone. The robust form exists one layer down —
`GuideDeliveryShape::Preamble` (`src/tools/core/guide_emit.rs:41`) is the typed signal
`guide_blocks_for` already returns — but it is not reachable from this test's vantage, which sees
only the emitted `Content` blocks through `call_tool_checked`. So the string is the best available
discriminator here and the weakest link in the fix; a future change that surfaces the shape enum to
the test should replace it.

Derivation, reproducible by anyone who wants to disbelieve it, run in an isolated worktree with its
own `target/` so no shared build lock was involved:

```text
A. unmutated + marker guard                  -> GREEN
B. `serves: artifact.append_entry` renamed   -> RED, naming serves-drift as the cause
C. reverted                                  -> GREEN
```

Held on branch `p50-absorption-demo` (`13ee893b`, patch-id
`95fd1e5230f052448d99346801c659993d1941b9`) rather than landed: `src/server.rs` carried another
session's uncommitted instrumentation inside that same function at the time — an aggregate-only
`eprintln!` of `total`, which is the blindness this fix removes.

**Provenance, because the parts came from different sessions.** The class was named by a peer session
from a symptom in its own gate (`total_aliases_found`, a global accumulator summed across 26 tools,
fixed separately with both demonstration steps observed RED). The instance here, the two falsified
remedies and the marker fix were derived in this run. A candidate offered first —
`tool_surface_under_budget` — was **not** an instance: it asserts a population claim against a
population accumulator, which is honest, and per-tool is separately covered by
`every_tool_description_under_cap`. That correction is kept because a class shown once is an
incident, and a mis-assigned member is a different error from a short count.

### A symbol name is a claim scoped to a TREE — and three doubts that confirmed

**The correction above was itself published without its scope.** Both greps were right.
`every_tool_description_is_under_its_cap` exists on branch `tool-collapse`
(`src/server.rs:2445`, where a refactor replaced the `experiments` test) and **nowhere on
`experiments`**; `every_tool_description_under_cap` exists on `experiments` (`:3354`) and **nowhere
on that branch**. So "zero definitions" and "one definition" were both correct answers to different
questions, and `0d2ab2b1`'s commit message — which reads *"has ZERO definitions … one was wrong"* —
is true of this tree and uncharitable about the peer who cited it from theirs. **When citing a
symbol across sessions on a shared repo, name the branch.** On a checkout with live worktrees that
is not pedantry: it is the same shape as everything else on this page, a claim true inside a
boundary and published without it.

**Three doubts raised in that run all confirmed, and they are recorded because confirmations are
the denominator this page's recording-filter law asks for.** None is a catch; absorbing them as
catches is what makes a population look self-correcting.

| doubt raised | outcome |
|---|---|
| "the per-tool cap is 1800, but a commit message argues from 300 — is one wrong?" | **Both real, different populations.** `tool_descriptions_stay_under_budget` (`src/server.rs:2449`) asserts `d.len() <= 300` and skips the librarian family via `is_librarian_tool`; `every_tool_description_under_cap` (`:3354`) asserts `CAP = 1800` over everything. A non-librarian tool is bound by 300, so the 304-char breach that commit describes was exact. |
| "`over.is_empty()` in the per-tool cap test is an absence assertion, monotone under an empty population" | **Closed by a pair.** `server_registers_all_tools` (`:2228`) and `server_tool_count_is_l3_target` (`:2278`) pin the count independently, so a broken or empty `server.tools` reds there first. |
| "is the aggregate/member split covered elsewhere, or is this systemic?" | **Generally covered by a pair, where anyone has looked.** Both cases above are two-level. The p50 test is the one where the pair was missing — which is what makes it an instance rather than the norm. |

Note the shape of the second and third: *the aggregate/member split is usually guarded by two tests
at different levels.* That is the remedy pattern, and its absence — not the presence of an
aggregate — is what identifies an instance. An aggregate assertion is not a defect; an aggregate
assertion **standing alone** where a per-member claim is being made is.

## Related

- The laws themselves: [`CLAUDE.md`](../../CLAUDE.md) § *Testing Discipline*.
- Superseded formulations and the route that got here: `reconnaissance-patterns` R-132, R-133,
  R-161.
- § *SDD Rulings*' "vacuous assertions cluster" and "demand a deliberate break" are instances of
  these laws, found before the general form was — `docs/trackers/sdd-ruling-log.md`.
- The epistemics of why care is the wrong instrument for these classes:
  `docs/trackers/observer-blindness.md`.
