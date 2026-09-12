---
id: '13a1fbde684b4370'
kind: bug
status: taken
title: 'BUG: the documented-parameter guard bills only `key=value` args, so the manual''s signature and JSON forms are unscannable'
tags:
- cluster/guard-narrower-than-its-name
claimed_at: 2026-09-12
claimed_by: f3c594ce-c424-40d3-a603-9693cfef3f63
filed_by: b0b9bc40
---

## Summary

`tests/doc_tool_refs.rs` is the standing guard for "a present-tense document naming a tool
parameter that does not exist". It walks `docs/manual/**`. It was green for the entire life of
`57ecbac8f925d7b8` — a bug whose whole content is `docs/manual` naming `name_path` as
`references`' parameter, in three files inside that very corpus.

The guard is not wrong about what it checks. It collects a call's parameters only from
`NAMED_ARG` matches, and `NAMED_ARG` requires an `=`. Neither form the manual actually uses to
name a parameter has one.

## Symptom (Effect)

Measured this session: the default gate lane returned `exit=0` (5818 passed / 0 failed / 37
binaries) on a tree whose `docs/manual` contained all of:

```
docs/manual/src/concepts/tool-selection.md:32   **`references(name_path, path)`**
docs/manual/src/concepts/tool-selection.md:155  | Who calls a function | `references(name_path, path)` |
docs/manual/src/tools/tool-workflows.md:43      | 2 | `references(name_path, path)` | …
docs/manual/src/tools/tool-workflows.md:84      | 1 | `references(name_path, path)` | …
docs/manual/src/tools/symbol-navigation.md:249  "name_path": "AuthService/authenticate_user",
docs/manual/src/tools/symbol-navigation.md:289  "name_path": "Logger/log",
```

and `references` rejects that key on the wire:

```
references(name_path="Tool/param_aliases", path="src/tools/core/types.rs")
  -> {"ok": false, "error": "missing 'symbol' parameter"}
```

## Reproduction

Add `references(name_path, path)` to any file under `docs/manual/src/`, then run
`cargo test --workspace --test doc_tool_refs`. It passes. Change it to
`references(name_path=…, path=…)` and the same test reds.

The discriminator is the `=`, not the wrongness of the parameter.

### What the unscannable population actually costs, measured on the wire 2026-09-12

Both verified by running the call, not by reading a schema — a dropped key returns a
plausible answer rather than an error, so the source does not show it.

- `symbols(pattern="Cite", path="tests/doc_tool_refs.rs")` returned **all 23 symbols in the
  file**, unfiltered. `pattern` is not accepted, is silently dropped, and the call falls
  through to the overview path — so a reader following the manual gets a confident answer to
  a question they did not ask (`IC-15` riding on top of this one). Taught in **6** manual
  files, including `symbol-navigation.md`'s parameter TABLE.
- `tree(pattern="*.rs", path="scripts")` returned **all 54 entries**, including `.js`, `.sh`,
  `.py` and `.json`. The real key is `glob`. Taught in `file-operations.md` at :207, :215,
  :236. This one was flagged as *likely* by `b0b9bc40` with an explicit "don't take it from
  me, it's one call" — so it was run here rather than inherited, and it is a genuine defect.

**The population is `b0b9bc40`'s, re-derived rather than copied**, and their count corrected
an earlier reading of mine that said 4 files. The `references(name_path…)` instances that
motivated the filing are already GONE from the corpus, fixed in their own commit — which is
why `symbols(pattern)` and `tree(pattern)` are recorded here with their line numbers: they are
the fixtures that survive.

### Deliberately NOT defects — the discriminator set a widened guard must not red on

Also `b0b9bc40`'s, and the more valuable half of the population, because a guard that reds on
these is worse than the gap it closes:

- `api-redesign.md:19` — a rename MAPPING table, which names dead parameters on purpose.
- `api-redesign.md:61`, `:82`, `file-operations.md:154`, `:162` — `grep(pattern=…)`, where
  `pattern` is real.
- `augmentation-render-template.md:58`, `:74` — the JSON Schema `pattern` KEYWORD, not a
  codescout parameter at all.

Nothing about token shape separates these from the defects above. `semantic_search(query)`
names a real parameter and `grep(regex)` does not; both are a bare lowercase identifier in
argument position. That is the whole difficulty of § Fix, and it is why this is filed rather
than fixed in passing.

## Environment

`experiments`, 2026-09-12. Reachable in the committed tree; not build- or feature-dependent.

## Root cause

`tests/doc_tool_refs.rs:67-68`:

```rust
const CALL_OPEN: &str = r"\b([a-z][a-z0-9]*(?:_[a-z0-9]+)*)\(";
const NAMED_ARG: &str = r"\b([a-z_][a-z0-9_]*)\s*=";
```

`calls_on_line` matches `CALL_OPEN`, walks a balanced span, and fills `params` from
`NAMED_ARG` over that span. Two consequences, and the manual's prose lives in both:

1. **Signature form** — `references(name_path, path)`. `CALL_OPEN` matches, the span is
   `name_path, path`, `NAMED_ARG` finds no `=`, so `params` is EMPTY and the call is billed
   nothing. The guard sees a call it fully parsed and has no parameters to check. This is the
   commonest way the manual names a tool's parameters: as a signature, in prose and in
   "you know / start with" tables.
2. **JSON payload form** — `{"tool": "references", "arguments": {"name_path": …}}`. There is no
   `identifier(` on the line at all, so `CALL_OPEN` never matches and no call is recorded. This
   is the form of every worked example in the manual's tool pages.

So the corpus the guard scans is real, the parse is correct, and the two shapes that carry
almost all of the manual's parameter claims are unrepresentable to it.

## Why this class

`cluster/guard-narrower-than-its-name` (IC-14). The guard's name and module header both state
the general claim — a document naming a parameter that does not exist — and its predicate holds
for a strict subset of the ways a document does that. A reader who sees it green concludes the
manual's parameter claims are checked.

Note the file's own header at `:62-66` already records one narrowing of exactly this shape: an
earlier draft anchored on `name\(\s*param\s*=`, saw only the FIRST argument, and reported green
over a live violation in the manual's Recommended Workflow table. Its conclusion — *"a guard
that checks the first argument and passes is worse than no guard, because the green tick is read
as coverage of the whole call"* — is this bug restated one axis over. That narrowing was found
and fixed; this one has the same shape and survived it.

## Fix

Not fixed. Two candidate directions, neither costed here:

1. Extend `calls_on_line` to bill BARE identifiers in a call span as parameter candidates when
   the span contains no `=` at all. Narrow enough to avoid billing `grep(regex)`-style prose
   that names a concept rather than a key — which is itself the hard part, since
   `semantic_search(query)` names a real parameter and `grep(regex)` does not.
2. Scan JSON `"arguments"` objects whose sibling `"tool"` key names a live tool. Structurally
   unambiguous, and covers every worked example in the manual, but it is a second parser rather
   than a widening of the first.

**A measurement hazard that must be read before either is attempted.** The instances that would
have proved a widened predicate are GONE from the corpus as of the fix for
`57ecbac8f925d7b8` — this session removed all six. A widened guard run against `experiments`
today goes green and proves nothing. Recover fixtures from that commit's parent, or better, use
`calls_on_line`, which is already extracted as a pure per-line function precisely so a case can
be written without planting one in the corpus the gate scans.

## Tests added

None yet. When one is written, it belongs against `calls_on_line` directly — the pure function —
and must be observed RED against today's constants.

## Resume

`f3c594ce-c424-40d3-a603-9693cfef3f63` held `tests/doc_tool_refs.rs` dirty when this was filed
and was told the mechanism directly, including the fixture hazard above. Check with that session
before widening the predicate; they may already be in it.

## References

- `tests/doc_tool_refs.rs:61-68` (the two constants and the header recording the prior narrowing)
- `tests/doc_tool_refs.rs:345-422` (`calls_on_line`)
- `tests/doc_tool_refs.rs:473` (`a_documented_tool_parameter_exists_on_that_tool`)
- `57ecbac8f925d7b8` — the instance this guard did not catch
