---
kind: bug
status: fixed
tags:
- lsp
- python
- references
- impact-analysis
closed: 2026-08-26
opened: 2026-08-25
owner: marius
related: []
severity: medium
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

## Confound closed, and the diagnosis flips (2026-08-25)

The `unverified:` blocker on this file was that the primary measurement ran
against a pytest tmp-dir tree that may never have been indexed — and "not
indexed" produces output byte-identical to "not indexable". **Closed.**

The fixture was rebuilt at
`…/scratchpad/refrepo` with the exact shape from *Symptom*, then **activated as
a codescout project** (`workspace(action="activate")`, `index(action="build")`)
so it is registered and indexed. It reproduces identically:

```
references(symbol="duty_multiplier", path="src/intl/duties.py")
  → 2 references: src/intl/duties.py:1 (the def)
                  src/intl/__init__.py:1 (the re-export)
    — does NOT reach src/orders/crossborder.py, which calls apply_duty()

symbols(name="apply_duty")                                    → 0 matches
references(symbol="apply_duty", path="src/intl/__init__.py")  → symbol not found
references(symbol="apply_duty", path="src/orders/crossborder.py") → symbol not found
```

### It is NOT an LSP capability gap

This is the question *Resume* named as severity-deciding, and it resolves
against the original reading:

```
symbol_at(path="src/intl/__init__.py", line=1, col=48)
  def:   src/intl/duties.py:1  — def duty_multiplier() -> float
  hover: (function) def apply_duty() -> float
         Return the duty multiplier.
```

The language server resolves the alias **completely** — definition and hover —
at that position. What fails is *name-based* addressing:
`references` / `call_graph` resolve their `symbol` argument by name against
extracted **document symbols**, and an import alias is not one.

So it is a **name-addressability gap, not an LSP capability gap.** Severity
drops — the data is reachable — but the defect is real: the documented entry
point dead-ends, and there is no positional entry point to `references`.

### The recommended recovery is itself a dead end

The error hints "Use symbols(path) to list symbols." For this file that returns:

```
symbols(path="src/intl/__init__.py")
  → Variable  3  __all__
```

One irrelevant symbol, and never the alias. A caller who follows the hint
exactly is no closer.

### Aside, found while setting this up

Activating the fixture (read-only, `languages: []`, not yet indexed) **removed
`references`, `call_graph`, `symbol_at` and `library` from the tool list**
altogether; they returned on re-activating the home project. The tool set is
project-dependent. Worked around here by staying on the home project and passing
`workspace=<fixture>` to each call. Not filed separately yet — it needs its own
reproduction to establish whether the trigger is read-only, unindexed, or
no-language-detected.
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

`references` resolved its `symbol` argument one way only: look the name up in
the file's **document symbols**, take that symbol's `(start_line, start_col)`,
and ask the LSP for references at that position. The positional call was always
there; only the name→position step failed for a binding the server does not emit
as a document symbol.

`src/tools/symbol/references.rs` now falls back, when — and only when — the name
matched **no** symbol:

- `ident_positions(text, ident)` returns every word-boundary occurrence as a
  0-based `(line, col)`, counted in **UTF-16 code units** (LSP positions are
  UTF-16; a byte offset shifts the probe onto a neighbouring token on any
  non-ASCII line, and that token resolves to something plausible).
- `resolve_binding_by_position` walks those occurrences and validates each with
  `goto_definition` before using it.

Two guards carry the correctness argument, and both matter more than the
fallback itself:

- **Ambiguity stays an error.** The fallback runs only on a zero-match name
  (checked via `collect_matching_symbols`), never on an ambiguous one. Choosing
  a textual occurrence for an ambiguous name would silently pick one of several
  *real* candidates.
- **Unvalidated positions are never used.** An occurrence inside a comment or a
  string literal resolves to nothing under `goto_definition`, so it is skipped.
  Answering from one would turn a loud `symbol not found` into a quiet,
  well-formed, wrong reference list — which is the failure this bug is about,
  not one to commit while fixing it.

A `name_path` containing `/` is excluded: it addresses a nested symbol, and
there is no single identifier to probe for.

**Fix commit:** recorded below once committed.
## Tests added

**End-to-end, live LSP** — `refs_renaming_reexport_alias` in
`tests/fixtures/python-extensions.toml`, over new fixture files
`library/pricing/__init__.py`, `library/pricing/duties.py` and
`library/orders.py` (all new, so no existing expectation's containment
assertions could shift).

Measured both ways, with the fix stashed and restored:

```
without the fix:  FAIL  refs_renaming_reexport_alias:
                        symbol not found: apply_duty
                  24/25 passed for python   (90.65s, real pyright run)

with the fix:     PASS  refs_renaming_reexport_alias
                  25/25 passed for python
```

Run with `cargo test --features e2e-python --test e2e_tests python_e2e`.

**Unit** — three in `src/tools/symbol/tests.rs` covering `ident_positions`: the
exact alias line from this bug (column 47, the 0-based twin of the column 48
`symbol_at` resolved), identifier-boundary rejection, and UTF-16-vs-byte columns.

### An earlier claim in this file was wrong

This file briefly carried an `unverified:` marker saying the alias path *could
not* have a CI-enforced test because no live-pyright harness existed. That was a
negative search of a single file (`src/tools/symbol/tests.rs`), and it was false
— the harness is `tests/e2e/`, gated behind `--features e2e-python`, with six
pre-existing live-LSP `find_referencing_symbols` expectations. Recorded as
`bug-fix-session-log:F-61`; the scout that caught it is `W-50`.

Enabling the lane also surfaced `bug-fix-session-log:F-62` — all five e2e
language lanes had stopped compiling (`tests/e2e/harness.rs` missing
`ToolContext::workspace_override`), invisibly, because `cargo test` never builds
them. Fixed in its own commit; that fix is a prerequisite for this one's test.

Gate: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test`
— 4459 passed, plus `cargo test --features e2e --no-run` building all five lanes.
## Workarounds

Chase the rename lexically. `grep` the original name to find the re-export line, read the
alias off it, then `grep` the alias. Two greps reach what one `references` chain cannot —
which is the finding that made the blast-radius eval abandon its "LSP-only" dependent
bucket as unconstructible and repartition its fixture by hop-cost instead.

## Resume

N/A — fixed.
## References

- `codescout:docs/superpowers/plans/2026-08-25-unanchored-blast-radius-eval.md` — the
  work that surfaced this.
- `codescout:.superpowers/sdd/2026-08-25-unanchored-blast-radius-eval/progress.md` — the
  ruling this measurement produced: an "LSP-only" dependent bucket is not constructible
  in Python for a function reference, so that eval partitions by hop-cost instead.
- `codescout:prompt-surface-measurement-session-log` — F-16..F-18, W-15, the same work
  stream.
