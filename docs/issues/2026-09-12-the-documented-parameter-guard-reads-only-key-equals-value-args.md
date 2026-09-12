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

### The full unscannable population, measured off the WIRE 2026-09-12 — hard-code these, they are about to vanish

sessionId `b0b9bc40`'s census, recorded here verbatim because their fix removes every live
instance and this guard's future test cannot discover what no longer exists. Method was not a
grep against a schema file: they spawned the binary, took the real `tools/list` **after**
`availability()` filtering, and diffed every JSON payload and signature form in `docs/manual`.
**38** claims not in the schema, of which these survive classification:

| claim | sites / files | what it actually is |
|---|---|---|
| `symbols(pattern)` | 14 / 6 | dropped silently → overview path |
| `"tool": "index(action: build)"` | 4 | the TOOL NAME does not resolve — the call cannot dispatch at all |
| `"tool": "workspace(action: status)"` | 3 | same shape |
| `symbols(project)` | 1 | `symbol-navigation.md:18-26`, a whole passage teaching a param the activation banner explicitly says `symbols` does not have |
| `memory(project)` | 1 | real key is `project_id` |
| `grep(regex)` | 2 | real key is `pattern` |
| `edit_file(…, content)` | 2 | an accepted alias of `body` since `f909a160`, non-canonical |
| `symbols(dir)` | 1 | same table as `symbols(pattern)`; real key is `path` |

**The two `"tool": "<name>(action: …)"` families are worse than a wrong parameter and neither
session had them.** A wrong parameter is dropped and the call still runs; an unresolvable tool
name cannot dispatch. 7 sites across 4 files, and they are invisible to this guard for the
second reason in § Summary — `CALL_OPEN` never matches a JSON payload.

**And that second reason is a SUPPRESSOR, not just a miss — the sharpest thing in this file.**
An unresolvable tool name stops anything from checking the ARGUMENTS inside it. Measured by
`b0b9bc40` on the second census pass: `{"tool": "workspace(action: status)", "arguments":
{"threshold": 0.3}}` concealed a second, independent defect behind the first — no tool has a
`threshold` parameter and no per-file drift score exists anywhere in codescout. So the 7
unresolvable-name sites were hiding an unknown number of argument defects, and **both** census
runs reported each as a single finding. A guard blind to JSON payloads is therefore not missing
7 things; it is missing 7 things plus everything they contain, with no way to bound the second
number from outside.

### The census corrected ITSELF, and the correction is the reusable part

`b0b9bc40` re-ran after widening the instrument, and the final figure is **75 claims → 36**, all
36 in the classified-out set, with **39 real sites fixed across 11 files** — nearly double the
first pass. What v1 could not see: it read only fenced ` ```json ` blocks carrying a `"tool"`
key, so every parameter TABLE and every bare argument example on a tool's own page was
invisible — the manual's most authoritative surface. **v1 reported "38 findings, 0 errors" and
nothing in that output suggested it had not looked.** Three kinds surfaced only after the widen:

- `symbols(project)` — `symbol-navigation.md` carried an entire `### Workspace project scoping`
  section teaching a parameter `symbols` has never had; the activation banner says so in as many
  words. The project selector belongs to `semantic_search` and `memory`, and is `project_id`.
- `index(path=…)` — `library-navigation.md` taught building a library index by pointing `index`
  at a root. `index` has no `path`: register with `library(action="register", path=…)`, then
  build with `scope: "lib:<name>"`.
- `tree`/`grep` `max_results` — real key is `limit` on both, confirmed on the wire.

The widen happened because a single unverified report (`tree(pattern)`, § below) was run instead
of dropped. That is the whole argument for this file's own method: **an instrument that returns
a clean number is the failure mode, because a zero-error report is what both a complete scan and
a half-blind one produce.**

**Also classified OUT, and that half is load-bearing:** `symbols(main)`, `grep(old_name)` and
`get_guide(librarian)` are argument VALUES, not parameter names. A widened predicate that reds
on them is worse than the gap.

### Two calls run here rather than inherited

Both verified by running them, not by reading a schema — a dropped key returns a plausible
answer rather than an error, so the source does not show it.

- `symbols(pattern="Cite", path="tests/doc_tool_refs.rs")` returned **all 23 symbols in the
  file**, unfiltered — `IC-15` riding on top of this one.
- `tree(pattern="*.rs", path="scripts")` returned **all 54 entries**, including `.js`, `.sh`,
  `.py` and `.json`. The real key is `glob`. Taught in `file-operations.md` at :207, :215, :236.

**`tree(pattern)` is NOT in the census above, and the reason is a finding about the census.**
`b0b9bc40`'s JSON parser reads only fenced ` ```json ` blocks, and those three sites are prose —
so an instrument that measured 38 claims off the live wire still has a shape it cannot see. They
declined to take it on this session's say-so and will re-derive before touching it, which is the
correct handling in both directions.

### Deliberately NOT defects — the discriminator set a widened guard must not red on

- `api-redesign.md:19` — a rename MAPPING table, which names dead parameters on purpose.
- `api-redesign.md:61`, `:82`, `file-operations.md:154`, `:162` — `grep(pattern=…)`, real.
- `augmentation-render-template.md:58`, `:74` — the JSON Schema `pattern` KEYWORD, not a
  codescout parameter at all.

**The census made the discriminator HARDER, not easier, and this is the sharpest statement of
the § Fix difficulty.** `grep(regex)` is a real defect (the key is `pattern`) and
`semantic_search(query)` is correct — and the two are byte-identical in shape: a bare lowercase
identifier, alone, inside a call's parens. No token-level rule separates them. Whatever the
remedy is, it has to consult the schema per call, not pattern-match the citation.

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
