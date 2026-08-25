---
status: open
opened: 2026-08-25
closed:
severity: medium
owner: marius
unverified: 'Not yet reproduced inside an indexed codescout project — the primary measurement ran against a pytest tmp-dir tree that may never have been indexed. The confound is named in Evidence and must be closed before any of this is treated as characterized.'
related: []
tags: [lsp, python, references, impact-analysis]
kind: bug
---

# BUG: `references` dead-ends at a renaming re-export, so impact analysis silently under-reports callers

## Summary

When a package re-exports a function under a different name
(`from m import f as g`), `references(f)` reaches the re-export line but not the
consumers that call `g`. Continuing the chase requires querying `g` — and `g` is not
addressable by name, so `symbols(name="g")` returns nothing and there is no second hop
to take. The impact analysis stops one hop short and reports a complete-looking answer.

## Symptom (Effect)

Measured against a generated Python fixture with this shape:

```python
# src/intl/duties.py
def duty_multiplier() -> float: ...

# src/intl/__init__.py
from src.intl.duties import duty_multiplier as apply_duty

# src/orders/crossborder.py
from src.intl import apply_duty
def landed_cost(subtotal: float) -> float:
    return subtotal * apply_duty()
```

`references(symbol="duty_multiplier", path="src/intl/duties.py")` reaches
`src/intl/__init__.py` (the re-export line) and does **not** reach
`src/orders/crossborder.py`. Attempting the second hop:

```
references(symbol="apply_duty", path="src/intl/__init__.py")
  → {"ok": false, "error": "symbol not found: apply_duty"}

call_graph(symbol="apply_duty", direction="callers")
  → {"ok": false, "error": "symbol not found: apply_duty"}

symbols(name="apply_duty")
  → 0 matches
```

A corroborating check inside a project codescout definitely indexes
(`prompt-engineering`, measured 2026-08-25):
`tests/prompt_tdd/test_judge.py:2` is `import json as _json`, and
`symbols(name="_json", workspace="/home/marius/work/claude/prompt-engineering")` returns
15 hits — every one a fuzzy substring match on a name containing `json`
(`test_bad_json_response`, `mcp_json`, `to_json`, …) and **none** the alias binding at
`test_judge.py:2`. Import-alias bindings are not surfaced as document symbols.

The failure is silent: both calls that fail do so loudly, but the *analysis* that
stops after hop one returns a well-formed, plausible caller list.

## Reproduction

Not yet reduced to a standalone repro inside an indexed project — see `unverified:`.

The observation path: `/home/marius/work/claude/prompt-engineering`, branch `master`,
commit `eb33a45`. `.venv/bin/python scenarios/blast-radius/gen_fixture.py /tmp/blast-a`
emits the tree above; the failing queries were run against it during Task 1 fix round 2
of `codescout:docs/superpowers/plans/2026-08-25-unanchored-blast-radius-eval.md`.

## Environment

Linux, codescout MCP over stdio, Python LSP. Observed while the active codescout project
was `codescout` (Rust) and the queried tree lived under a pytest `tmp_path`.

## Root cause

Unknown — two candidate mechanisms, not yet separated:

1. **Aliases are genuinely not indexed.** Most `textDocument/documentSymbol`
   implementations omit imports, since an import is a binding rather than a definition.
   If codescout's Python symbol extraction inherits that, no alias is ever addressable
   by name, and the `_json` corroboration above is consistent with it.
2. **The tree was never indexed.** The primary measurement ran against a pytest tmp-dir
   tree outside any registered project root, so `symbol not found` may mean "not in the
   index" rather than "not indexable." This would make the fixture measurement worthless
   while leaving the `_json` observation standing.

`inferred from the two measurements above — the mechanism is not measured, and the
confound in (2) is not yet excluded.`

## Evidence

### The `_json` check (inside an indexed project — this one is clean of the confound)

`prompt-engineering` is a registered codescout project with `python` in
`.codescout/project.toml`. `symbols(name="_json", workspace=…)` → 15 matches, all fuzzy
name-substring hits, none at `tests/prompt_tdd/test_judge.py:2` where `_json` is bound.

Note the limit of this datapoint: `import json as _json` is a *module* alias. The bug is
about `from m import f as g`, a *function* re-export. A grep for
`^from [a-z_.]* import [A-Za-z_]* as ` across all non-vendored Python in
`prompt-engineering` returns **zero** hits, so the exact shape could not be tested there.

### The fixture measurement (carries the confound)

Recorded verbatim in
`codescout:.superpowers/sdd/2026-08-25-unanchored-blast-radius-eval/task-1-report.md`,
"Fix Round 2" section.

## Hypotheses tried

1. **Hypothesis:** `apply_duty` is absent because the alias is not indexed.
   **Test:** independent `symbols(name="_json")` against an indexed project.
   **Verdict:** consistent, not confirmed — the shape tested was a module alias, not a
   function re-export.
   **Evidence link:** *The `_json` check*.
2. **Hypothesis:** `symbol not found` is an artifact of an unindexed tmp tree.
   **Test:** none yet.
   **Verdict:** deferred — this is the confound the `unverified:` field names.

## Fix

None. Not characterized enough to prescribe one, and doing so before separating the two
mechanisms would be a prescription for a bug that may not exist in the form described.

## Tests added

None — deliberately. Adding a regression test before the mechanism is separated would
pin whichever behaviour happens to be current, which is the defect this repo's own
discipline calls out (`run the reproduction before reading the fix plan`).

## Workarounds

Chase the rename lexically. `grep` the original name to find the re-export line, read the
alias off it, then `grep` the alias. Two greps reach what one `references` chain cannot —
which is the finding that made the blast-radius eval abandon its "LSP-only" dependent
bucket as unconstructible and repartition its fixture by hop-cost instead.

## Resume

Build the minimal case **inside an indexed project** and re-run, which separates the two
mechanisms in one pass: add `pkg/mod.py` defining `f()`, `pkg/__init__.py` with
`from pkg.mod import f as g`, and `consumer.py` with `from pkg import g` calling `g()`,
somewhere under `/home/marius/work/claude/prompt-engineering` (already registered,
already Python). Then run, in order:

1. `symbols(name="g", workspace="/home/marius/work/claude/prompt-engineering")` — if this
   returns the binding, mechanism (2) was the whole story and the fixture measurement was
   an artifact; close as `wontfix-false-alarm`.
2. `references(symbol="f", path="pkg/mod.py")` — does it reach `consumer.py`?
3. `symbol_at(path="pkg/__init__.py", line=1, col=<column of `g`>)` then `references`
   from whatever it returns. **This is the check that decides severity:** if the alias is
   reachable positionally but not by name, the gap is discoverability (medium) rather
   than capability (high), and the fix is a `symbols` extraction change, not an LSP one.

## References

- `codescout:docs/superpowers/plans/2026-08-25-unanchored-blast-radius-eval.md` — the
  work that surfaced this.
- `codescout:.superpowers/sdd/2026-08-25-unanchored-blast-radius-eval/progress.md` — the
  ruling this measurement produced: an "LSP-only" dependent bucket is not constructible
  in Python for a function reference, so that eval partitions by hop-cost instead.
- `codescout:prompt-surface-measurement-session-log` — F-16..F-18, W-15, the same work
  stream.
