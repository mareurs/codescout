---
id: '66e4b3c0b0625ef0'
kind: bug
status: open
title: 'BUG: `contains` binds % and _ as live SQL wildcards with no way to escape either'
tags:
- cluster/addressing-without-an-escape-hatch
---

## Summary

`{"<field>": {"contains": v}}` binds `v` into a SQL `LIKE` pattern **without** calling
`escape_like_pattern` and without emitting an `ESCAPE` clause, while `prefix` — the branch
immediately below it in the same `match` — does both. So `%` and `_` inside a `contains`
value are live wildcards, there is no way to write either literally, and the in-memory
`eval` twin disagrees with the SQL side about every such value.

## Symptom (Effect)

Live at `1c16d410`, against a catalog where **no** tracker title contains a percent sign:

```
doc(action="find", filter={"and": [{"kind": {"eq": "tracker"}},
                                   {"title": {"contains": "%"}}]}, limit=3)

→ count: 3, hints: {"more_in_scope": 97}
   "Issue Clusters — the defect class a bug instantiates (IC-N)"
   "Open-Issue Work Queue (BL-N)"
   "Retrieval Benchmark — pinned 25-TC log"
```

100 matches for a substring present in none of them. `contains: "%"` is `LIKE '%%%'`,
which matches every row.

There is no escape. `contains: "\\%"` binds a literal backslash-percent into a pattern
with no `ESCAPE` clause, so the backslash is an ordinary character and the `%` is still a
wildcard — the workaround makes the match *narrower and still wrong*, rather than correct.

## Reproduction

`git rev-parse HEAD` → `1c16d410`, branch `experiments`. Run the call above through the
live MCP server against any indexed project holding >0 artifacts. No build step.

Second, cheaper shape — `_` is the single-character wildcard, so this matches any title
with a `d` and an `y` two apart:

```
doc(action="find", filter={"title": {"contains": "d_y"}})
```

## Environment

Linux, codescout `experiments` @ `1c16d410`, catalog schema v6+, stdio MCP transport.

## Root cause

`src/librarian/filter.rs`, `compile_leaf`. Measured 2026-09-04 by reading the two adjacent
arms and running the probe above:

```rust
LeafOp::Contains => {
    let s = value.as_str()...;
    Ok(SqlFragment {
        sql: format!("{sql_field} LIKE ?"),                        // no ESCAPE clause
        params: vec![Value::Text(format!("%{s}%"))],               // no escaping
    })
}
LeafOp::Prefix => {
    let s = value.as_str()...;
    let escaped = crate::librarian::util::escape_like_pattern(s);  // escaped
    Ok(SqlFragment {
        sql: format!("{sql_field} LIKE ? ESCAPE '\\'"),            // ESCAPE clause
        params: vec![Value::Text(format!("{escaped}%"))],
    })
}
```

The escape helper exists, is correct, and is called eight lines away. `contains` was
simply never routed through it.

**Second-order: the two filter engines now disagree.** `eval` (`filter.rs:434`, the
in-memory twin used by `doc(action="get", entry_filter=…)` via
`src/librarian/tools/get.rs:537`) implements `contains` as a plain case-insensitive
substring test, where `%` is an ordinary character. So one filter value produces two
different answers depending on which surface reads it — artifact-grain `find` treats it as
a pattern, entry-grain `entry_filter` treats it as text. Neither surface says which it is.

## Evidence

### The parity test holds the exact fixture that would catch it, and tests the other op

`src/librarian/filter.rs`, `eval_matches_compile_on_fixture` — a test whose entire purpose
is asserting `eval` and `compile` return identical id sets:

```rust
("b", Some("done"), 3, "50% off sale"),
...
json!({"title": {"prefix": "50%"}}), // %-escape parity
```

A row deliberately titled `"50% off sale"`, and a `%-escape parity` case built on it —
routed through `prefix`, the op that escapes. `contains` never receives the same value.
The fixture is one line away from red.

This is `CLAUDE.md` § *Testing Discipline*'s **"mutate once per guarded SITE, not once per
feature"** read from the other end: the author correctly identified `%`-escaping as a
property worth pinning, pinned it at one of the two sites that implement it, and the
untested site is the one that lacks the behaviour.

### Blast radius is every `contains` caller, including the documented ones

`contains` is the **recommended** op for path filters — `find`'s own `rel_path` shorthand
lifts a bare `rel_path="foo"` param into `{"rel_path": {"contains": "foo"}}`
(`src/librarian/tools/find.rs:577`), and
`docs/issues/archive/2026-09-04-rel-path-filter-is-an-alias-onto-an-absolute-column.md`
§ *Workarounds* tells readers to prefer it. Paths rarely contain `%`, but `_` is
extremely common in identifiers, filenames and titles — and `_` is a wildcard here too,
so every `contains` on a value holding an underscore is silently broader than written.

## Hypotheses tried

1. **Hypothesis** — `contains` is safe because SQLite only honours `%`/`_` when an
   `ESCAPE` clause is present.
   **Test** — the live probe in § *Symptom*.
   **Verdict** — rejected. SQLite's `LIKE` treats `%` and `_` as wildcards unconditionally;
   `ESCAPE` only *designates an escape character*. 100 rows matched.
2. **Hypothesis** — a caller can escape it themselves with `\%`.
   **Test** — read the compiled fragment: no `ESCAPE` clause is emitted on the `contains`
   arm, so `\` has no special meaning.
   **Verdict** — rejected; there is no escape, which is what makes this `IC-6` rather than
   an ordinary correctness bug.

## Fix

Not started. The shape is settled and small — route `contains` through
`escape_like_pattern` and emit the `ESCAPE` clause, exactly as `prefix` does:

```rust
let escaped = crate::librarian::util::escape_like_pattern(s);
Ok(SqlFragment {
    sql: format!("{sql_field} LIKE ? ESCAPE '\\'"),
    params: vec![Value::Text(format!("%{escaped}%"))],
})
```

Two things to settle before writing it, neither obvious:

- **Does any live caller depend on the wildcard behaviour?** Grep for `contains` filter
  values holding `%` or `_` across `src/`, `scripts/` and the trackers. A caller passing
  `_` in a filename fragment today gets a broader match than it asked for and may be
  relying on the result set. This is the only part that could turn a one-line fix into a
  behaviour change worth announcing.
- **`eval` must move in the same commit or the parity gap inverts.** Fixing only `compile`
  makes the two engines agree again (both then treat `%` as a literal), so `eval` likely
  needs **no** change — verify that rather than assume it, since `eval`'s `contains` is
  also case-insensitive while SQL `LIKE` is case-insensitive only for ASCII.

## Tests added

None yet. The cheap one is a single line in the existing parity fixture:

```rust
json!({"title": {"contains": "50%"}}),   // RED today: eval finds 1, compile finds all 4
```

That is the whole regression test, and it belongs beside the `prefix: "50%"` line already
there. Add a `_` case too — `contains: "d_y"` — because `%` and `_` are separate wildcards
and escaping one does not escape the other.

Per `CLAUDE.md`, watch it RED before the fix: a parity assertion that passes on the day it
is written proves the two engines agree, not that either is correct.

## Workarounds

None for a literal `%` or `_` — that is the defect. `prefix` and `eq` both escape
correctly, so anchor the query differently where the shape allows it. Note the
`rel_path` shorthand (`doc(action="find", rel_path="foo")`) lifts to `contains` and so
inherits this.

## Resume

Run the two greps in § *Fix* first — specifically, whether anything in-tree passes a
`contains` value holding `_`. Then add the two fixture lines to
`eval_matches_compile_on_fixture` (`src/librarian/filter.rs`), watch them RED, and apply
the four-line change to the `LeafOp::Contains` arm of `compile_leaf`.

## References

- `src/librarian/filter.rs` — `compile_leaf`'s `LeafOp::Contains` / `LeafOp::Prefix` arms;
  `eval` at `:434`; `eval_matches_compile_on_fixture`
- `src/librarian/util.rs` — `escape_like_pattern`, the helper already written
- `src/librarian/tools/get.rs:537` — `eval`'s one production caller
- `src/librarian/tools/find.rs:577` — the `rel_path` shorthand that lifts to `contains`
- `docs/issues/archive/2026-09-04-rel-path-filter-is-an-alias-onto-an-absolute-column.md` —
  found while reading `compile_leaf` for that fix; its § *Workarounds* recommends the
  affected op
- Cluster: `IC-6` `addressing-without-an-escape-hatch`, the **no-escape** half. A caller
  cannot write a literal `%` or `_` in a `contains` value — the input is not merely
  mishandled, it is unrepresentable, which is why ordinary testing never reached it: you
  cannot write a test for a case the grammar cannot express. The class's own framing
  applies almost verbatim — *"before shipping one, answer two questions in the code rather
  than in your head: how does a caller write this token literally, and what happens when
  two collide?"* The first question has an answer here (`escape_like_pattern`) and it was
  answered for the sibling op only.

