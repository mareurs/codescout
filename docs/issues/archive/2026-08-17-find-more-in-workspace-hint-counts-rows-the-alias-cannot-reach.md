---
id: e8094b832491b358
kind: bug
status: fixed
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

Implemented — a **third** direction, cheaper and more honest than the two this
file originally proposed. Scouting killed both: direction 1 ("suppress the
hint") would have deleted behavior an existing test explicitly demands
(`scope_all_widens_to_workspace`: *"with umbrella → more_in_workspace hint must
appear"*), and direction 2 as written would have mislabelled reachable rows as
unreachable in that same fixture.

The actual defect is narrower than "the hint is wrong": the two hints were
measured from the **same baseline** when they describe different regions.

| hint | region | baseline | reachable? | in `expand`? |
|---|---|---|---|---|
| `more_in_umbrella` | umbrella minus current scope | `here` | yes, via `scope="all"` | yes |
| `more_in_workspace` | catalog **beyond** the umbrella | `in_umbrella` | no scope value | **no** |

Both blocks had identical guards, so they are now one block computing
`in_umbrella` once. `more_in_workspace` becomes `in_workspace - in_umbrella`
(was `in_workspace - here`, which double-counted the entire reachable umbrella
delta) and carries a new `more_in_workspace_hint` naming the action that does
work — activate the owning project. Its `expand.push` is deleted: `expand` is a
list of args that *fetch* what was counted, and it also stopped emitting
`scope="all"` twice as if two hints had two different remedies.

Why the umbrella is genuinely the ceiling, confirmed at
`src/librarian/tools/scope.rs::resolve_scope`: under `UmbrellaPolicy::Require`,
`scope="all"` aliases to `Umbrella` when the project has an umbrella and
**errors** when it does not. There is no configuration in which `scope="all"`
returns machine-wide rows for an active project, so no advice could have made
the old count reachable.

Change: `src/librarian/tools/find.rs::build_hints`, plus the two prompt surfaces
that documented the old contract (`src/prompts/guides/librarian-runtime.md`,
`src/librarian/prompts/companion_hint.md`).

Fix SHA `9cdb2f50` (**`experiments`**). `master` is a strict ancestor at archive
time (`git rev-list --left-right --count master...experiments` → `0 894`), so the
promotion path is fast-forward and this SHA already *is* the master SHA — no
second SHA to record later.
## Tests added

`librarian::tools::find::tests::scope_all_widens_to_workspace` — extended rather
than duplicated, because the bug was that this test could not see the bug. Its
fixture had one row in the repo and one inside the umbrella, and **nothing beyond
the umbrella** — so both hints were numerically identical and no assertion could
distinguish their baselines. Added a third row at `/other/ghost/c.md`, outside
every umbrella member, mirroring the ghost-repo and `/tmp` rows the real shared
catalog holds.

With one row per region the test now pins four things: `more_in_umbrella == 1`,
`more_in_workspace == 1`, `expand == ["scope=\"all\""]` exactly (once, and no
entry for the unreachable surplus), and the contract the old code broke —
following `expand` returns `count + more_in_umbrella`, asserted as a computed
value rather than the literal `2`, so it cannot drift back into agreement by
coincidence.

**Mutation-verified, not assumed.** Two independent mutations, each turning it
red with the expected diff:

- `saturating_sub(in_umbrella)` → `saturating_sub(here)` (the shipped defect):
  `more_in_workspace` becomes `2`, failing on `left: Some(2) / right: Some(1)`.
- restoring the deleted `expand.push`: `left: ["scope=\"all\"", "scope=\"all\""]`
  vs `right: ["scope=\"all\""]`.

`scope_all_does_not_self_reference_expand_hint` still passes untouched — the
umbrella-scope guard it protects is unchanged.
## Workarounds

Treat `more_in_workspace` as "rows exist somewhere on this machine", not as a
count you can retrieve. To actually read them, activate the owning project
(`workspace(action="activate", path=<other repo>)`) and query there, restoring
the home project afterward.

## Resume

N/A — verified on the wire after `cargo rb` + `/mcp`, not only by the suite.
At repo scope: `more_in_umbrella: 2`, `more_in_workspace: 21` (was 23 — the
reachable delta is no longer double-counted), `more_in_workspace_hint` present,
and `expand: ["scope=\"all\""]` with a single entry. Following it resolved to
`applied: "umbrella"` and returned 13, matching repo-total 11 + 2 exactly. The
remaining 21 rows are reported as beyond the ceiling and offered no scope
expansion, which is the corrected contract.
## References

- `src/librarian/tools/find.rs:111-278` — `build_hints`
- `src/librarian/tools/find.rs:934-990` — the umbrella-scope test
- `docs/issues/archive/2026-07-17-artifact-find-ignores-workspace-pin.md` — parent bug, sub-finding #2
- `docs/issues/2026-08-15-context-scope-all-crosses-umbrella-boundary.md` — the same alias seen from the other side (`librarian(context)` drops the alias entirely); marked `wontfix`
