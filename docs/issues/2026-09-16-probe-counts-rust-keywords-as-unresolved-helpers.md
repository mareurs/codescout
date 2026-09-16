---
id: '86e44eba4bbdd891'
kind: bug
status: open
title: Architecture probe counts Rust keywords as unresolved helpers
tags:
- architecture
- measurement
- cluster/addressing-without-an-escape-hatch
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

Not implemented. The denylist is the wrong shape whatever it contains, so
extending it to six or ten keywords fixes this instance and not the class.

Two candidates:

1. **Allowlist instead of denylist** — report a name only if it resolves to a
   `fn` definition *somewhere* (`local_functions` or `global_functions`
   non-empty) but not uniquely. That is exactly the "genuinely ambiguous" case,
   and it excludes syntax for free because keywords have no definition. It also
   drops the closures, which is correct.
2. **Exclude the Rust keyword set** — cheap, and still an enumerated list over an
   open namespace. Prefer 1.

Candidate 1 also changes the meaning of the field from "names I could not follow"
to "names that are ambiguous between real definitions", which is the question a
reader of this field is actually asking.

## Tests added

None yet. A regression test wants a fixture body containing `let (a, b) = f();`
alongside one genuinely ambiguous helper, asserting the reported set is exactly
the latter — the `let` half is what reds today.

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
