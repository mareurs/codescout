---
id: '86e44eba4bbdd891'
kind: bug
status: taken
title: Architecture probe counts Rust keywords as unresolved helpers
tags:
- architecture
- measurement
- cluster/addressing-without-an-escape-hatch
claimed_at: 2026-09-16
claimed_by: e5691fad-9f78-4cd1-ad14-edfdd1fee41f
---

# BUG: the probe counts Rust keywords as unresolved helpers, inflating the population 5.6x

## Summary

`scripts/architecture-boundary-probe.py`'s `unresolved_same_file_helpers` reports
**28** unresolved helper routes across 15 tools. **20 of the 28 are Rust syntax**,
not helpers: `let` (15 tools), `return`, `cfg`, `derive`, `drop`, `any`. A further
3 are local closures whose bodies the probe already walks. The genuine population
is **5** `(tool, helper)` pairs over 3 names.

The number is published as a defect population that a reader is expected to audit.
At 28 it looks like a multi-day sweep; at 5 it is an afternoon, and the 5 all have
the same cause.

## Symptom (Effect)

`context.tools.<tool>.unresolved_same_file_helpers` lists identifiers that are not
functions. Sample, verbatim from a 2026-09-16 run:

    run_command      ['cfg', 'count_lines', 'derive', 'drop', 'let']
    onboarding       ['let', 'return']
    workspace        ['any', 'let']
    tree             ['let']

`let` appears in **every** one of the 15 tools that report any unresolved helper.

## Reproduction

    python3 scripts/architecture-boundary-probe.py context --repo . --output /tmp/ctx.json
    python3 -c "import json;d=json.load(open('/tmp/ctx.json'))['context']; \
      print({k:v['unresolved_same_file_helpers'] for k,v in d['tools'].items() \
      if v['unresolved_same_file_helpers']})"

## Environment

codescout `experiments`, 2026-09-16. Probe exit 0, `worktree_changed_during_measurement: false`.

## Root cause

`scripts/architecture-boundary-probe.py:809`:

    elif "ctx" in body and helper not in {"if", "match", "while", "for"}:
        unresolved_helpers.add(helper)

The candidate regex is `(?<![.:])\b([a-z_][A-Za-z0-9_]*)\s*\(`, which matches any
lowercase identifier followed by `(`. Rust puts several **keywords** in exactly
that position:

- `let (a, b) = f();` — destructuring. This is the `let` in all 15 tools.
- `return (x, y);`
- `#[cfg(...)]`, `#[derive(...)]`, `cfg(any(...))`
- `drop(child)` — `std::mem::drop`, not a local helper

The exclusion is an **enumerated four-keyword denylist over an open namespace**.
Every keyword not in the list of four is reported as an unresolved helper, and the
list can only ever be extended by someone who already noticed the specific miss.

## Evidence

Verified rather than reasoned:

- `let` has **zero** `fn` definitions in `src/`; `let (` destructuring is present
  in the scanned tool files. So it is syntax, not a missed helper.
- The 3 non-keyword names with no `fn` definition are closures or `&dyn Fn`
  parameters — `plan_path` (`src/tools/symbol/edit_code.rs:410`),
  `collect_docstrings` (`src/tools/symbol/list_overview.rs:216`), `name_ok`
  (`src/symbol/query.rs:56`) — declared inside the very function bodies the probe
  already collects, so their `ctx` reads are captured and flagging them
  double-counts.
- The 3 genuine names each have **exactly two** definitions, which is why
  `len(candidates) == 1` fails and the probe correctly declines to guess:
  `uri_to_path` (`src/fs/mod.rs:366`, `src/lsp/client.rs:41`), `leading_ws`
  (`src/tools/markdown/edit_markdown.rs:1143`, `src/util/text.rs:36`),
  `count_lines` (`src/tools/command_summary.rs:451`, `src/util/text.rs:14`).

## Hypotheses tried

1. **Hypothesis:** the names are real helpers the probe failed to resolve.
   **Test:** grepped `fn <name>` for each. **Verdict:** rejected for the 6
   keywords — zero definitions; `drop` has 11, all `impl Drop for X`, none a local
   helper.
2. **Hypothesis:** the closure names are genuine unresolved routes.
   **Test:** located each definition. **Verdict:** rejected — all three are
   declared inside a body the probe already walks.

## Fix

Applied 2026-09-16, and **candidate 1 alone does not fix this bug's own case** — found by
running the reproduction before building on the plan.

**Why candidate 1 is insufficient.** It reports a name only if it resolves to a `fn`
definition somewhere but not uniquely. `drop` resolves to **11** anchored `fn drop(` in
`src/`, three of them in `run_command/inner.rs` itself, so it is non-uniquely resolvable and
survives the rule. All 11 are `impl Drop for X { fn drop(&mut self) }` — verified, `impl Drop`
occurs 11 times in `src/` too.

**The missing discriminator is the RECEIVER, and it is structural rather than a name list.** A
free call cannot reach a method taking `self`. The free-call regex
`(?<![.:])\b([a-z_]\w*)\s*\(` already rules out `x.f()` and `T::f()` by lookbehind, so its
resolution set should never have contained methods. Confirmed on the live corpus: the three
genuine names are all free — `fn uri_to_path(uri: &str)`, `fn leading_ws(s: &str)`,
`fn count_lines(s: &str)` — and every `drop` is `fn drop(&mut self)`.

**Two changes, both in `scripts/architecture-boundary-probe.py`:**

1. `named_function_bodies(text, *, free_only=False)` — opt-in exclusion of `self`-receiver
   methods. **Opt-in is load-bearing and the first cut got it wrong:** `references` reports a
   THIRD consumer, `methods_for_type`, which uses this same index to resolve `self.f()` and
   whose every wanted method has a receiver. Filtering globally would have emptied it and
   killed that path silently. The two free-call sites pass `free_only=True`; `methods_for_type`
   keeps the default.
2. The `elif` reports only names **ambiguous between real free-function definitions**
   (`len(candidates) > 1`), which retires the enumerated `{"if","match","while","for"}`
   denylist entirely — keywords and closures have no `fn` definition, so they fall out for free
   rather than by enumeration.

**A SECOND FAILURE MODE THIS FILE DID NOT RECORD, and it is the more expensive one.** With
several `impl Drop` in a file the name resolves ambiguously and lands in the published list —
noise. With **exactly one**, it resolves UNIQUELY, the probe walks the trait body, and that
body's `ctx` reads are attributed to the tool. That inflates `context_fields`, which is the
measurement the field exists to support, and **nothing flags it**: the unresolved list stays
clean precisely when the corruption happens. Pinned by its own test.

**Measured effect, with the control that makes it a comparison.** 28 `(tool, helper)` pairs over
12 names → **5 pairs over 3 names**, stable across three runs. `src/` is byte-identical between
the two measured corpora (`2d33e3ea..852fa5e7` touches 2 files, both docs) and the probe in
HEAD is unchanged across them, so the delta isolates the analyser. Both runs report
`worktree_changed_during_measurement: true` — 12 peer-dirty paths on a shared checkout, not this
change; the probe measures `git archive` of HEAD, and the figure was stable 3/3 rather than
taken once.

The surviving 5 are exactly this file's predicted population: `uri_to_path` (edit_code,
references, symbol_at), `leading_ws` (edit_file), `count_lines` (run_command).
## Tests added

Three, in `tests/test_architecture_boundary_probe.py`, all three observed RED against unmodified
production code before the fix.

- `test_unresolved_helpers_exclude_syntax_and_self_receiver_methods` — one fixture carrying
  `let (a, b) = split(...)` (keyword + undefined name), `drop(guard)` against two `impl Drop`
  blocks, one genuinely ambiguous helper, and one uniquely-resolvable helper. Asserted by
  **equality**, so it reds on over-reporting AND on losing the genuine member; an `assertNotIn`
  would be monotone under dropping `ambiguous` too. Red as
  `['ambiguous', 'drop', 'let', 'split'] != ['ambiguous']`. Carries a control that the unique
  helper is still FOLLOWED, so a change that stopped resolving free calls at all cannot pass it.
- `test_a_free_call_does_not_resolve_to_a_trait_method_of_the_same_name` — the second failure
  mode. One `impl Drop`, so `fn drop` resolves uniquely and gets walked. Red as
  `['agent', 'lsp'] != ['agent']`: the trait body's `ctx.lsp` read attributed to the tool.
- `test_a_self_method_call_is_followed_through_the_type_index` — **added because mutation found
  the gap, not because the fix needed it.** Flipping `free_only`'s default to `True` left **16 of
  16 green**, because no fixture exercised `self.f()` at all, so nothing pinned the one
  parameter that keeps `methods_for_type` working. With this test the same mutation KILLS
  (`1 failed, 16 passed`).

**Mutation run in an isolated copy outside the repo** — never the shared tree, where a mutation
publishes a red indistinguishable from a real regression. Control green unmutated (16/16, then
17/17), and each pattern asserted to occur **exactly once** before sed, since a mutation that
never applied is indistinguishable from one that survived.

**M2 SURVIVED and is semantically inert, which is a different verdict from untested.**
`len(candidates) > 1` → `>= 1` left 16/16 green — correctly, because the preceding
`if len(candidates) == 1` branch consumes the only value that distinguishes them, so the two
expressions are identical on every reachable input. No test can kill it and none should be
written for it.
## Workarounds

Filter the reported list against `fn <name>` definitions before treating it as an
audit population. The 2026-09-16 follow-up measurements did exactly this and
recorded 5 rather than 28.

## Resume

The corrected population is in `docs/trackers/architecture-boundary-measurement.md`
under *Follow-up measurements — 2026-09-16*, together with the finding that all
5 genuine routes read **no** `ToolContext` field, which is what discharged slice
3's prerequisite. Fixing this bug does not change that conclusion — it changes the
number a future reader has to audit to reach it.
