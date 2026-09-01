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

## Related

- The laws themselves: [`CLAUDE.md`](../../CLAUDE.md) § *Testing Discipline*.
- Superseded formulations and the route that got here: `reconnaissance-patterns` R-132, R-133,
  R-161.
- § *SDD Rulings*' "vacuous assertions cluster" and "demand a deliberate break" are instances of
  these laws, found before the general form was — `docs/trackers/sdd-ruling-log.md`.
- The epistemics of why care is the wrong instrument for these classes:
  `docs/trackers/observer-blindness.md`.
