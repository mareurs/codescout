---
id: d0a4d6e530048d6a
kind: bug
status: fixed
title: 'BUG: the rel_path filter is an alias onto an absolute column, so nine of its ten ops are wrong'
tags:
- cluster/selector-narrower-than-its-population
closed: 2026-09-04
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

Fixed at **`5253297a`** on `experiments`, patch-id
**`66b4bcda49601dfee601c6a458d438f5d4891159`**.

Option **(a)** from the original plan, but implemented at a different site than that plan
named — and the difference is the whole point. The plan said *"resolve the caller's
argument against the scope's `git_root`"*, which would have meant threading a root through
`compile` → `compile_composition` → `compile_leaf` and updating every caller. Reading the
code first killed that: `compile` takes only a node, its one production caller
(`src/librarian/catalog/find.rs`, 4 sites) has no root either, and an **umbrella** query
spans several repos, so there is no single root to normalise against even in principle.

What shipped instead is **root-agnostic boundary anchoring**, gated on the column exactly
as BL-47 gated `tags`/`owners` twelve lines above:

| op | before | after |
|---|---|---|
| `contains` | `abs_path LIKE '%v%'` | unchanged — was already correct |
| `prefix` | `abs_path LIKE 'v%'` | `abs_path LIKE '%/v%'` |
| `eq` | `abs_path = v` | `abs_path LIKE '%/v'` |
| `ne` | `abs_path != v` | `NOT (abs_path LIKE '%/v')` |
| `in` | `abs_path IN (…)` | OR of the `eq` form |
| `nin` | `abs_path NOT IN (…)` | `NOT` of that OR |
| `gt` `lt` `gte` `lte` | lexicographic on the wrong string | **`RecoverableError`** |

The ordering ops are refused rather than repaired because there is no coordinate system in
which they mean what they read as. The refusal names the field and points at `prefix` /
`eq` / `contains`, and at `updated_at` / `created_at` if a range was the actual intent —
per `CLAUDE.md` § *Parsers Over a Namespace*, *"where no escape is affordable, say so at
the refusal site"*.

**Residual, stated at the site rather than hidden.** `%/docs/trackers%` also matches a
nested `…/vendor/docs/trackers/…`. Under the default `scope="project"` the AND'd scope
clause already pins the root, so this needs a second `docs/trackers` deeper in the same
tree to bite. Named in the code comment, not only here.

**`eval()` deliberately unchanged.** Its one production caller is
`src/librarian/tools/get.rs:537`, the `entry_filter` path over augmentation params rows —
where `rel_path` would be an ordinary user-defined field, not the catalog alias. Remapping
there would have been a new bug. The parity fixture `eval_matches_compile_on_fixture` now
carries a comment saying the two engines answer different questions for that one name on
purpose, so its silence on `rel_path` reads as a decision rather than a gap.

**Budget.** `TOOL_SURFACE_CHAR_BUDGET` raised 37 to the exact measured `56_513`, logged at
the constant. Gross addition was 180; 143 was paid on the spot by compressing the same
description's `LIKE '%v%'` / `LIKE 'v%'` idioms, which were verbose *and*, for `rel_path`,
describing a comparison that no longer happens.
## Tests added

`rel_path_filter_matches_rows_whose_stored_path_is_absolute`
(`src/librarian/filter.rs:839`). Watched RED first — `left: []`, `right: ["t1", "t2"]`,
which is the live symptom reproduced in a fixture.

It satisfies all three requirements this section stated before the fix:

- **Runs queries, asserts on ROWS.** An in-memory table holding absolute paths, the filter
  compiled and executed, ids compared. The pre-existing
  `rel_path_filter_compiles_to_abs_path_sql` asserts on `f.sql` and stayed green through
  every failure above; it is kept, because it still pins the field remap itself.
- **Both directions.** `prefix` / `eq` / `in` assert non-empty results; `ne` / `nin`
  assert the named row is absent. The second half is the one a prefix-only test is
  monotone under.
- **Per-op, not per-field.** Six ops across one remap site.

Plus `contains` as a **regression anchor** in the same test: without it, a fix could repair
the other five by breaking the one op every existing caller relies on, and nothing would
say so.

`rel_path_rejects_the_ordering_ops_it_cannot_mean` (`src/librarian/filter.rs:925`) — all
four ordering ops refused, each refusal asserted to name the field. Also watched RED
(``gt` on rel_path must be refused`).

The fixture's absolute paths carry an on-line annotation saying they are load-bearing and
what breaks if they are “tidied” to relative form — that edit would leave the test passing
and no longer discriminating, which no assertion can catch.
## Workarounds

**Use `contains` for every path filter.** It is the only op whose meaning survives the
absolute/relative mismatch, and the `find` shorthand already lifts a bare
`rel_path="foo"` param into exactly that form. To anchor a `contains` at a directory
boundary, include the leading separator in the value — `contains: "/docs/issues/2026-09"`
— which is not a true prefix match but excludes same-named subpaths in practice.

**Do not use `ne`/`nin` on `rel_path` at all.** It returns the rows it is asked to drop,
with no signal.

## Resume

N/A. **Verified live 2026-09-04 01:50** against the rebuilt release binary (built 01:47:49,
post-dating both the fix `5253297a` at 01:29:33 and HEAD `998f64d3` at 01:46:51), five
probes through the MCP surface:

| probe | before | after |
|---|---|---|
| `{"rel_path": {"prefix": "docs/trackers"}}` — the served guide example | `0` | **94** |
| `{"rel_path": {"ne": "docs/trackers/issue-clusters.md"}}` — rows kept | 101, nothing excluded | **100** |
| … same, `AND title contains "Issue Clusters"` | returned the excluded file | **0** |
| `{"rel_path": {"gt": "docs/trackers"}}` | lexicographic on the wrong string | **refused**, naming the field |
| the triage query in § *Evidence* → *How it was noticed* | `0` | **7** |

The third row is the one that carries the `ne` claim, and it needed the second beside it:
a zero there is *also* what a `ne` excluding everything would return, so the pair — 100
kept, 1 dropped — is the discriminating result and either alone is not.

The last row closes the loop on how this was found. The seven are byte-identical to what
`grep -l '^status: fixed' docs/issues/*.md` returns, and that agreement counts because the
two instruments have **different scopes**: one reads SQLite through the compiled filter,
the other reads the filesystem. Per `CLAUDE.md` § *Observer Blindness*, two instruments
agreeing is evidence only when they do not share a blind spot.

Build mtime was established *before* reading any probe result, deliberately: a stale binary
and a broken fix both return zero, so a negative would otherwise have been uninterpretable.
In the event every probe was positive, which is unambiguous on its own.

---

One adjacent defect noticed while reading `compile_leaf` and **not** fixed here, because it
is a different mechanism and mixing them would muddy the diff: `LeafOp::Contains` binds its
value with no `escape_like_pattern` call and emits no `ESCAPE` clause, while `LeafOp::Prefix`
immediately below it does both. Confirmed live rather than left as a lead —
`{"title": {"contains": "%"}}` returned 100 trackers, none of whose titles hold a percent
sign. Filed as `docs/issues/2026-09-04-contains-binds-percent-and-underscore-as-live-wildcards.md`
(`IC-6`, the no-escape half) at `22808be4`.
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
