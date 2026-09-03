---
id: '9383d8a81b041e26'
kind: bug
status: open
title: 'BUG: the rel_path filter is an alias onto an absolute column, so nine of its ten ops are wrong'
tags:
- cluster/selector-narrower-than-its-population
---

## Summary

`doc(action="find", filter={"rel_path": …})` compiles to SQL against the **`abs_path`**
column, whose stored value is an absolute path. Every caller writes a **relative**
argument, because that is what the field is named and what all four documented surfaces
show. `contains` is the only one of the ten ops that survives the mismatch. The other
nine return well-formed, wrong, error-free results — in **both** directions.

## Symptom (Effect)

Two live calls at `00ce1995`, on a repo holding 101 catalogued trackers.

**Over-exclusion.** The worked example printed in `src/prompts/guides/librarian.md:77`:

```
doc(action="find", filter={"rel_path": {"prefix": "docs/trackers"}})
→ {"count": 0, "items": [], "hints": {}}
```

No error, no hint, no `more_in_scope`. A clean zero for a query naming 100+ rows.

**Under-exclusion — the worse direction.** Ask for every tracker *except* one, then look
for the one:

```
doc(action="find", filter={"and": [
  {"kind":  {"eq": "tracker"}},
  {"rel_path": {"ne": "docs/trackers/issue-clusters.md"}},
  {"title": {"contains": "Issue Clusters"}}]})

→ count: 1
   docs/trackers/issue-clusters.md   ← the file the filter excluded
```

The excluded row is returned by the filter that excludes it.

## Reproduction

`git rev-parse HEAD` → `00ce1995`, branch `experiments`. Run either call above through
the live MCP server against any indexed project. No build step; this is a query defect.

## Environment

Linux, codescout `experiments` @ `00ce1995`, catalog schema v6+, stdio MCP transport,
project `codescout`.

## Root cause

`src/librarian/filter.rs:148` — measured 2026-09-04 by reading the function and running
the two calls above:

```rust
// rel_path was dropped in schema v6; abs_path is the DB column now.
// Remap here so documented filter examples continue to work.
let sql_field = if field == "rel_path" { "abs_path" } else { field.as_str() };
```

The remap is name-level only. It rewrites the *column* and never the *value*, so the
caller's relative argument is compared against an absolute stored path. Op by op:

| op | compiled predicate | against `/home/…/docs/trackers/x.md` |
|---|---|---|
| `contains` | `LIKE '%v%'` | **correct** — substring, position-independent |
| `prefix` | `LIKE 'v%'` | always false for a repo-relative `v` |
| `eq` / `in` | `= v` | always false |
| `ne` / `nin` | `<> v` | always **true** — returns the excluded rows |
| `gt` `lt` `gte` `lte` | lexicographic | compares against a different string entirely |

The comment names the reason the remap exists — keeping documented examples working —
and the documented example that most needs it is the `prefix` one it does not reach.

**A second half of the same feature disagrees about the representation.**
`rel_path_hint` (`src/librarian/tools/find.rs:463-481`) reads a `rel_path` leaf under
`contains` **or `prefix`** and feeds `scan_unindexed_md`, whose doc comment reads *"walk
the current project directory for `.md` files whose **repo-relative** path contains
`hint`"*. So the zero-result fallback path uses relative semantics while the query it
falls back from uses absolute. One field name, two meanings, inside one tool.

## Evidence

### The guard test covers the one safe op

`src/librarian/filter.rs:827-837`:

```rust
#[test]
fn rel_path_filter_compiles_to_abs_path_sql() {
    let node = parse(json!({"rel_path": {"contains": "docs/trackers"}}));
    let f = compile(&node).unwrap();
    assert_eq!(f.sql, "abs_path LIKE ?");
    assert_eq!(f.params, vec![Value::Text("%docs/trackers%".into())]);
}
```

Two properties, both named in `CLAUDE.md` § *Testing Discipline*:

- **One site, ten ops, one mutation.** *"Mutate once per guarded SITE, not once per
  feature"* — the remap is one line serving ten ops and the test exercises the single op
  the remap cannot break.
- **It asserts on the generated SQL string, never on a returned row.** *"A second level
  asserting about its own re-implementation is indistinguishable from coverage until you
  break the thing that ships."* Deleting the remap entirely reds this test; changing
  `abs_path` to store relative paths, or fixing the value side, does not touch it. It
  cannot express either live failure above.

The test name states the property generally — *"the rel_path filter compiles to abs_path
sql"* — which is also true of the nine ops it never runs.

### The four surfaces, and what each one claims

| surface | text | status |
|---|---|---|
| `src/prompts/guides/librarian.md:77` | `{"rel_path": {"prefix": "docs/trackers"}}` as a Leaf example | **matches zero rows, always** |
| `src/prompts/guides/librarian.md:27,30` | `` `rel_path` \| string \| Path relative to repo root `` and *"use `rel_path` for filesystem-oriented lookups"* | establishes the relative-path model, 50 lines above the example that relies on it |
| `src/librarian/tools/artifact.rs:95` | `contains` example, then `Ops: … contains prefix` + `contains on strings = LIKE '%v%' (works on title, rel_path, etc.); prefix = LIKE 'v%'` | example safe; op list implies `prefix` works on `rel_path` |
| `src/librarian/prompts/companion_hint.md:51` | same shape as above | same |
| `src/librarian/tools/find.rs:463` | `/// Extract the first rel_path contains/prefix value` | reads `prefix` as meaningful, then applies **relative** semantics |

Five surfaces, and the two that matter most are in **one file**. The guide's Artifact
Model table (`:27`) states the field holds a path relative to the repo root and directs
the reader to use it for *"filesystem-oriented lookups"*; its Filter Syntax section
(`:77`) then shows the lookup that cannot work. A reader who follows the guide top to
bottom is given the model and the counter-example in the same read, with nothing marking
which half the code implements.

The guide is also the sharpest surface for a second reason: it is auto-injected into a
session's context on the first `find` call, so the failing form is **served**, not merely
available. It arrived in this session twenty minutes before the zero it explains.
### How it was noticed

Triaging `fixed`-but-unarchived bugs with
`filter={"and": [{"status": {"in": ["fixed","mitigated"]}}, {"rel_path": {"prefix": "docs/issues/2026"}}]}`.
Result: `count: 0` — read as "the archive backlog is clear". Seven files on disk carry
`status: fixed` in `docs/issues/`, and the catalog holds all seven with that status
(`doc(action="find", filter={"rel_path": {"contains": "read-markdown-silently-ignores-offset"}})`
returns the row, `status: fixed`). The zero was the filter, not the backlog.

## Hypotheses tried

1. **Hypothesis** — the catalog rows are stale; peers archived the files and the catalog
   never re-indexed (BL-48 territory).
   **Test** — `find` by `rel_path` `contains` on one of the seven filenames.
   **Verdict** — rejected. The row is present, current, and `status: fixed`.
2. **Hypothesis** — `rel_path` is stored with a repo-name prefix, per the `create`
   param's *"NOT including the repo name (use the `repo` field for that)"*.
   **Test** — `prefix: "codescout/docs/issues/…"`.
   **Verdict** — rejected, `count: 0`.
3. **Hypothesis** — `rel_path` is an alias onto an absolute column.
   **Test** — read `src/librarian/filter.rs:146-152`; then the two live calls in
   § *Symptom*.
   **Verdict** — confirmed, both by source and by observed behaviour in both directions.

## Fix

Not started. Three shapes, and the choice matters because two of them are silent:

- **(a) Normalise the value, not just the field.** Where `field == "rel_path"`, resolve
  the caller's argument against the scope's `git_root` before binding it, so all ten ops
  compare like with like. Correct, and the only option that makes the served guide
  example work as written.
- **(b) Refuse every op but `contains` on `rel_path`,** with a `RecoverableError` naming
  the absolute-storage reason and suggesting `contains`. Cheapest, loud, and consistent
  with `CLAUDE.md` § *Parsers Over a Namespace* — *"Where no escape is affordable, say so
  at the refusal site"*. Breaks any caller relying on today's `ne`, which is a caller
  already getting wrong answers.
- **(c) Documentation only.** Rejected as a standalone: it leaves `ne`/`nin` silently
  returning excluded rows, and § *Observer Blindness* is explicit that a doc fix for a
  silent wrong answer publishes to an audience that does not know to read it.

Recommend **(a)**, with **(b)** as the fallback for ops that cannot be normalised
meaningfully (the four ordering ops, whose lexicographic meaning over paths is unclear
either way). Either way the guide example at
`src/prompts/guides/librarian.md:77` must be re-verified live, not just re-read.

## Tests added

None yet. What a real regression test needs, given the two laws the existing one trips:

- It must **run a query and assert on returned rows**, not on `f.sql`. A SQL-string
  assertion cannot express either failure.
- It needs **both directions**: a `prefix` case asserting a non-empty result, and a `ne`
  case asserting the excluded path is absent. The `prefix` half alone is monotone under
  the fix and would pass a change that repaired nothing about `ne`.
- Per-op, not per-field: nine ops share one remap site.

## Workarounds

**Use `contains` for every path filter.** It is the only op whose meaning survives the
absolute/relative mismatch, and the `find` shorthand already lifts a bare
`rel_path="foo"` param into exactly that form. To anchor a `contains` at a directory
boundary, include the leading separator in the value — `contains: "/docs/issues/2026-09"`
— which is not a true prefix match but excludes same-named subpaths in practice.

**Do not use `ne`/`nin` on `rel_path` at all.** It returns the rows it is asked to drop,
with no signal.

## Resume

Decide between fix (a) and (b) — see § *Fix*. If (a): the change is at
`src/librarian/filter.rs:146-152`, where `compile_leaf` needs the scope's `git_root` in
hand to normalise the bound value; check whether `compile()` has it or whether the root
must be threaded in from `find.rs`. Write the two-direction test in § *Tests added*
first and watch both halves fail. Then re-run
`doc(action="find", filter={"rel_path": {"prefix": "docs/trackers"}})` live and confirm
it returns ~101 rows, because that exact call is the served guide example.

Also reconcile `rel_path_hint` (`src/librarian/tools/find.rs:463`) with whichever
semantics win — it currently accepts `prefix` and applies relative matching, so under
fix (b) it accepts an op the query layer refuses.

## References

- `src/librarian/filter.rs:146-152` (remap), `:827-837` (the narrow guard test)
- `src/librarian/tools/find.rs:463-481` (`rel_path_hint`), `:483-490` (`scan_unindexed_md`)
- `src/prompts/guides/librarian.md:77` — the served example that matches nothing
- `src/librarian/tools/artifact.rs:95`, `src/librarian/prompts/companion_hint.md:51`
- `docs/adrs/2026-08-27-negative-results-name-their-scope.md` — the ADR this zero violates
- `CLAUDE.md` § *Testing Discipline* — the two laws the existing guard test trips
- Cluster: `IC-18` `selector-narrower-than-its-population`. Adjudication: the claim
  fits — a well-formed answer over a population the selector could never reach, where
  *"a zero reads as 'not present' rather than 'not looked at'"*. One difference worth
  recording rather than smoothing: IC-18's members narrow by **enumeration** (a glob, an
  `--include` list, a heading level) and examine a genuine subset. This one narrows by
  **representation mismatch** — the row set examined is complete and the comparison value
  is in the wrong coordinate system — so `prefix` reaches the empty set rather than a
  subset, and `ne` widens to the *whole* set instead of narrowing. A class whose members
  all under-report gains one that over-reports on five of its ten ops; if that reads as a
  second mechanism rather than a variant, it belongs in its own class.
