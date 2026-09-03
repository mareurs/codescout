---
id: e58da8c48f0dc7b1
kind: bug
status: fixed
title: 'BUG: `contains` binds % and _ as live SQL wildcards with no way to escape either'
tags:
- cluster/addressing-without-an-escape-hatch
closed: 2026-09-04
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

Fixed. The change is four lines in `compile_leaf`'s `LeafOp::Contains` arm — route the
value through `escape_like_pattern` and emit the `ESCAPE` clause, exactly as `LeafOp::Prefix`
does eight lines below:

```rust
let escaped = crate::librarian::util::escape_like_pattern(s);
Ok(SqlFragment {
    sql: format!("{sql_field} LIKE ? ESCAPE '\\'"),
    params: vec![rusqlite::types::Value::Text(format!("%{escaped}%"))],
})
```

**The pre-work this section demanded was run first, and it cleared.** The question was
whether any in-tree caller passes a `contains` value holding `%` or `_` and depends on the
wildcard match it currently gets — the only thing that could turn a four-line fix into a
behaviour change worth announcing.

Enumerated every `"contains":` construction across `*.rs` / `*.py` / `*.sh`. Most hits are
on `tags` / `owners`, which route through the `is_array_col` branch
(`EXISTS (SELECT 1 FROM json_each(col) WHERE value = ?)`) — **equality, not `LIKE`** — and
are unaffected. The `LIKE`-path production callers are five:

| site | value | verdict |
|---|---|---|
| `src/librarian/tools/audit_doc_refs/mod.rs:752` | `"docs/trackers/doc-ref-audit.md"` | hyphens only — unaffected |
| `src/librarian/tools/legibility_scan/mod.rs:320` | `"docs/trackers/legibility-backlog.md"` | hyphens only — unaffected |
| `src/cli/doc.rs:108` | user-supplied `--has-topic` | escaping is the correct behaviour |
| `src/librarian/tools/context.rs:755,760` | user-supplied `topic` | same |
| `src/librarian/tools/find.rs:604` | user-supplied `rel_path` shorthand | same |

No in-tree dependency on the wildcard reading; the three user-input paths are cases where a
caller typing `_` means a literal underscore. **No behaviour change to announce.**

**`eval` needed no change**, and that was checked rather than assumed: its `contains` is
already a literal substring test, so fixing `compile` is what brings the two engines back
into agreement. Confirmed by the three parity cases now passing.
## Tests added

> **CORRECTION 2026-09-04 — the test this section originally prescribed cannot fail.**
> It said: *"the cheap one is a single line in the existing parity fixture:
> `json!({"title": {"contains": "50%"}})` — RED today: eval finds 1, compile finds all 4."*
> That prediction is **wrong**, and it was written into the file at filing time as though
> it had been run. `contains "50%"` compiles to `LIKE '%50%%'`, and a trailing `%` adjacent
> to the wrapper `%` is semantically identical to `'%50%'` — so both engines return `["b"]`
> and the case is **inert**. Added to the fixture and observed passing before the fix.
>
> **A metacharacter only discriminates when it sits BETWEEN literal text.** The cases that
> do:
>
> ```rust
> json!({"title": {"contains": "Docs%Frog"}}),  // RED: eval [] vs compile ["a"]
> json!({"title": {"contains": "D_cs"}}),       // RED: eval [] vs compile ["a"]
> ```
>
> Both observed RED **independently** — the first assertion short-circuits the second, so
> the order was swapped once and the run repeated, rather than crediting the `_` half with
> a red it never showed. Left as an `IC-16` near-miss on the record: an assertion that
> cannot fail, prescribed in a bug file, would have shipped as coverage.

The shipped tests are two, and the split is deliberate.

**`contains_treats_percent_and_underscore_as_literal_characters`** (`src/librarian/filter.rs`)
asserts the **rows**, not parity. Parity is the cheaper test and is not sufficient here:
`eval` implements `contains` as a literal substring search, so a future change making *it*
wildcard-aware would restore parity while leaving both engines wrong. The expected id lists
are the claim; agreement is a consequence.

It carries a **positive control**, and that is the load-bearing part. The two wildcard
assertions are *absence* assertions (`== []`), which `CLAUDE.md` § *Testing Discipline*
names as monotone under removal — a `contains` broken to match nothing at all satisfies
both. So the test also asserts that a **literal** `%` is still findable
(`contains "50%"` → `["b"]`, `contains "50% off"` → `["b"]`) plus an ordinary substring
(`contains "Lotus"` → `["a"]`). Without those three, "escaped correctly" and "broken
outright" are the same green.

**The three parity cases** stay in `eval_matches_compile_on_fixture` — including the inert
`50%` one, kept deliberately and annotated as inert on the fixture line, so nobody re-adds
it later believing it discriminates.

The fixture titles carry an on-line note saying what they are for and why a trailing
metacharacter does not work, because that detail is invisible and a tidy-up would leave the
test passing and no longer discriminating.

**Two pre-existing SQL-text assertions needed updating**, and they earn a note rather than
a complaint: `rel_path_filter_compiles_to_abs_path_sql` and
`repair_fixes_inverted_op_keyed_leaf` both pin the generated SQL string, and both went red
on the added `ESCAPE` clause. That is a fair characterisation of what a SQL-text assertion
is good for — a poor *correctness* test (it is exactly the one that stayed green through
the whole `rel_path` defect) and a decent *change-detector*. Both roles are real; the
mistake would be crediting one with the other.
## Workarounds

None for a literal `%` or `_` — that is the defect. `prefix` and `eq` both escape
correctly, so anchor the query differently where the shape allows it. Note the
`rel_path` shorthand (`doc(action="find", rel_path="foo")`) lifts to `contains` and so
inherits this.

## Resume

N/A for the fix — shipped at **`5fc8005a`** on `experiments`, patch-id
**`0e4c069bed0275984895de55d6a1072d2f20b6b5`**.

**One thing owed, and it is a gate re-run rather than work.** The four-command gate was
not clean at commit time, for reasons in other people's files:

| command | result |
|---|---|
| `cargo fmt` | clean |
| `cargo clippy --workspace --all-targets --features local-embed -- -D warnings` | **RED** on `src/librarian/artifact_store.rs:610` — `clippy::for_kv_map` in a peer's test module. Reported to its author, not repaired. |
| `cargo test --workspace --no-default-features` | exit 0, and **vacuous for this change** — see below |
| `cargo test --workspace` | 5125 passed, 6 failed — all six an in-flight peer schema migration |

Re-run both once `artifact_store.rs:610` and the v12 migration land. Nothing about this
commit is expected to change.

**The lean lane is worth recording as a trap rather than a result.** It returned `exit 0`
and that is not evidence about this change: the librarian sits behind a default feature —
which is precisely *why* that lane produces a librarian-less binary — so
`--no-default-features` compiles `filter.rs` out entirely. `grep -c 'librarian::filter::tests'`
over its output returns **0**. The lane returns `exit 0` whether this change is correct or
catastrophically wrong, and it was one message away from being cited as a pass. This is
`CLAUDE.md` § *Testing Discipline* — *"a test cannot detect a change its assertion is
monotone under"* — reaching a whole **lane**: a suite that never compiles the code under
test is monotone under every change to it.

**What the default lane actually established.** All four tests from this work ran inside
the full default-feature workspace run and **passed**; zero `filter` / `catalog::find` /
`tools::find` tests failed. The six failures are one cause wearing six names — an identical
`assertion left: 12, right: 11` — a catalog schema version bumped to 12 by an unstaged peer
migration (`catalog/chunk.rs` +273, `catalog/mod.rs` +101) meeting six tests that hardcode
the expected version as a literal instead of deriving it from the constant. Six reds, one
fact, zero relation to this file.
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
