---
id: f7ed94fbc1952bc5
kind: bug
status: open
title: 'BUG: artifact(find)''s more_in_workspace hint counts rows scope="all" can never reach — the 2026-07-17 fix gated on applied scope, so the self-reference survives one level down'
---

## Summary

At `scope="repo"` with an umbrella configured, `artifact(action="find")` emits
`more_in_workspace: <N>` and `expand: ["scope=\"all\""]`. Following that advice
aliases to umbrella and returns only the umbrella rows — the extra-umbrella rows
the hint just counted are unreachable by any `scope` value. The hint then
disappears, so nothing records that N rows were ever claimed to exist.

## Symptom (Effect)

Two calls, same filter, this repo, 2026-08-17:

```
artifact(find, kind="bug", filter={"status":{"in":["open","investigating"]}})
→ count: 10
  scope.applied: "repo",  scope.umbrella: "codescout-ecosystem"
  hints: { "more_in_umbrella": 2,
           "more_in_workspace": 23,
           "expand": ["scope=\"all\"", "scope=\"all\""] }

artifact(find, …same filter…, scope="all", limit=60)
→ count: 12
  scope.applied: "umbrella"
  hints: {}
```

10 + 23 = 33 rows claimed. Following the only offered expansion returns **12**.
Twenty-one rows are counted by the hint and reachable by nothing. The follow-up
`hints: {}` means the discrepancy leaves no trace at the destination.

Note also `expand` lists `scope="all"` **twice** — once for `more_in_umbrella`
(where it is correct) and once for `more_in_workspace` (where it is not).

## Reproduction

`git rev-parse HEAD` → `a1540c8c`, branch `experiments`, release binary built
2026-08-17 08:03. Requires a project with an umbrella whose members do not cover
every repo in the machine-wide catalog.

1. `artifact(action="find", kind="bug", filter={"status": {"in": ["open", "investigating"]}})`
2. Read `hints.more_in_workspace` and `hints.expand`.
3. Re-run with `scope="all"`, `limit` above the claimed total.
4. Observe `scope.applied == "umbrella"` and a count short of step 1's
   `count + more_in_workspace`.

## Environment

Linux, codescout `experiments` @ `a1540c8c`, stdio MCP, project
`/home/marius/work/claude/codescout`, umbrella `codescout-ecosystem`. The catalog
is machine-wide and spans repos outside the umbrella, which is what makes the
gap observable.

## Root cause

`build_hints` (`src/librarian/tools/find.rs:111-278`) gates the workspace hint on
the **applied scope** rather than on whether the suggested expansion can reach
what it counted:

```rust
if !matches!(applied.scope, Scope::All | Scope::Umbrella)
    && current.and_then(|c| c.umbrella.as_deref()).is_some()
{
    let in_workspace = count_for_scope(…, Scope::All, …)?;   // whole catalog
    …
    hints.insert("more_in_workspace".into(), json!(extra));
}
```

and then

```rust
if hints.contains_key("more_in_workspace") {
    expand.push("scope=\"all\"");
}
```

The two conditions are jointly unsatisfiable in the useful direction: the hint
requires `current.umbrella.is_some()`, and an umbrella present is exactly the
condition under which `scope="all"` aliases to umbrella. So whenever
`more_in_workspace` fires, its own suggestion is guaranteed not to reach the
rows it counted.

The comment immediately above the block (`find.rs:217-222`) states the mechanism
correctly — *"`scope=\"all\"` aliases to umbrella whenever the project has one …
it counts extra-umbrella catalog rows the alias can never reach"* — and then
applies it only to excluding `Scope::Umbrella` from firing the hint. The same
sentence is the argument for not emitting the hint at `Scope::Repo` either.

Measured 2026-08-17: the two live `find` calls quoted under **Symptom**, run
against the rebuilt binary — not inferred from source alone.

## Evidence

### The alias is asserted by an existing test

`src/librarian/tools/find.rs:934-990`,
`scope_all_does_not_self_reference_expand_hint`:

```rust
// scope="all" aliases to umbrella → only the in-umbrella row is reachable.
assert_eq!(v["scope"]["applied"], "umbrella");
assert_eq!(v["count"].as_u64(), Some(1));
assert!(v["hints"]["more_in_workspace"].is_null(),
    "at umbrella scope there is nothing broader to reach; got hints: {}", v["hints"]);
```

The test constructs the case at `scope="all"` only. Its fixture has one
in-umbrella row and one outside — the identical shape to the repo-scope case,
never exercised from repo scope.

### A second live instance in the same session

```
artifact(find, kind="bug", filter={"title":{"contains":"scope"}}, include_archived=true)
→ count: 12, scope.applied: "repo"
  hints: { "more_in_workspace": 2, "expand": ["scope=\"all\""] }
```

## Hypotheses tried

1. **Hypothesis:** the 23 rows are reachable via `scope="umbrella"` explicitly
   rather than the aliased `"all"`.
   **Test:** `more_in_umbrella` is separately reported as 2, and the umbrella run
   returned exactly `10 + 2 = 12`.
   **Verdict:** rejected — umbrella reaches 2 of the 23; the other 21 are outside
   every member root.
   **Evidence:** Symptom, both calls.

2. **Hypothesis:** this is the already-fixed `2026-07-17` bug resurfacing.
   **Test:** read the archived file's sub-finding #2 and the test it shipped.
   **Verdict:** rejected — that fix is present and correct *at umbrella scope*.
   This is the untested sibling case one scope down.
   **Evidence:** `docs/issues/archive/2026-07-17-artifact-find-ignores-workspace-pin.md`
   (artifact `eb01cc2ba8270522`, status `fixed`); the test above.

## Fix

Not yet implemented. Two candidate directions, both small:

1. **Report only what is reachable.** Compute the workspace count with the alias
   applied — i.e. count at the scope `scope="all"` would actually resolve to. When
   an umbrella exists that equals the umbrella count, so `more_in_workspace`
   naturally stops firing and `more_in_umbrella` carries the signal alone. This
   removes the duplicate `expand` entry as a side effect.
2. **Keep the count, fix the advice.** Retain `more_in_workspace` as a genuine
   "rows exist that this session cannot reach" signal, but pair it with an
   accurate action — `workspace=<path>` or a new explicit scope — and never
   `scope="all"`. Preferable if the count is considered useful diagnostics.

Direction 1 is the smaller change and matches what the existing test already
asserts at umbrella scope. Direction 2 preserves information that is arguably
worth surfacing; it needs a reachable action to point at before it is honest.

Whichever lands, the `expand` builder must not push the same string for two
different hints — that alone is a legibility defect.

## Tests added

None yet. The regression test is the repo-scope twin of
`scope_all_does_not_self_reference_expand_hint`: same fixture (one in-umbrella
row, one outside), called at the default repo scope, asserting that whatever
`expand` offers, following it returns at least `count + more_in_workspace` rows —
or that the hint is absent.

## Workarounds

Treat `more_in_workspace` as "rows exist somewhere on this machine", not as a
count you can retrieve. To actually read them, activate the owning project
(`workspace(action="activate", path=<other repo>)`) and query there, restoring
the home project afterward.

## Resume

Decide direction 1 vs 2 above. Direction 1: in `build_hints`
(`src/librarian/tools/find.rs:216-236`), drop the `Scope::All` count when
`current.umbrella` is `Some`, and delete the corresponding `expand.push` at
`find.rs:270-272`. Then add the repo-scope regression test described under
**Tests added**, modelled on `scope_all_does_not_self_reference_expand_hint`
(`find.rs:934`) but invoked without a `scope` argument.

## References

- `src/librarian/tools/find.rs:111-278` — `build_hints`
- `src/librarian/tools/find.rs:934-990` — the umbrella-scope test
- `docs/issues/archive/2026-07-17-artifact-find-ignores-workspace-pin.md` — parent bug, sub-finding #2
- `docs/issues/2026-08-15-context-scope-all-crosses-umbrella-boundary.md` — the same alias seen from the other side (`librarian(context)` drops the alias entirely); marked `wontfix`

