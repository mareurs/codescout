---
id: '66e8a27d0ed97a90'
kind: bug
status: open
title: 'BUG: librarian(action="context") drops the umbrella alias and the no-umbrella guard, so scope="all" crosses into non-member projects'
tags:
- librarian
- scope
- umbrella
- duplication-drift
- context
topic: librarian scope resolution
opened: 2026-08-15
owner: marius
related: []
severity: high
---

# BUG: librarian(action="context") drops the umbrella alias and the no-umbrella guard, so scope="all" crosses into non-member projects

## Summary

Three librarian handlers resolve `scope` from the same `Scope` enum, but only two
of them run the full scope-resolution prologue. `src/librarian/tools/context.rs` carries a truncated
copy: it keeps the no-current-project fallback and drops both the `scope="all"`
umbrella guard and the `All → Umbrella` alias. The result is that
`librarian(action="context", scope="all")` applies **no scope clause at all** and
pulls artifacts from unrelated workspace projects into the returned bundle —
including projects that are not members of the configured umbrella.

## Symptom (Effect)

Same server, same active project, same word passed for `scope`, two different
meanings:

```
artifact(action="find",     kind="tracker", scope="all")
  → "scope": { "applied": "umbrella", "umbrella": "codescout-ecosystem" }
  → only codescout-ecosystem member artifacts

librarian(action="context", topic="...",   scope="all")
  → "scope": { "applied": "all",      "umbrella": "codescout-ecosystem" }
  → returned an artifact belonging to a workspace project that is NOT
    an umbrella member
```

The `find` side additionally *recommends* the wider scope in its own overflow
hint — `"expand": ["scope=\"all\""]` — so an agent that learns the meaning of
`scope="all"` from `find` (where it means "the umbrella") carries that meaning to
`context` (where it means "everything on the machine").

## Reproduction

At `1d22b715` on `experiments`, on a host whose active project declares an
umbrella and whose workspace also contains at least one non-member project:

1. `artifact(action="find", kind="tracker", scope="all", limit=3)`
   → `scope.applied == "umbrella"`.
2. `librarian(action="context", topic="<anything>", scope="all", max_tokens=300)`
   → `scope.applied == "all"`, and the returned `markdown` contains an artifact
   whose `abs_path` is outside every umbrella member root.

Both were run live in this session; step 2 returned a tracker from an unrelated
project. **The path is deliberately not reproduced here** — it names a third-party
client project and is a per-host absolute path, and neither belongs in a tracked
file. The check that matters is reproducible without it: compare the returned
`abs_path` values against the umbrella's member roots.

## Environment

- linux, MCP stdio transport, project `codescout`, branch `experiments` @ `1d22b715`.
- Requires a configured umbrella — see `memory(action="read", topic="local-environment", private=true)`
  for this host's values. On a host with no umbrella the alias is a no-op and only
  the missing guard divergence shows.

## Root cause

The scope-resolution prologue is written three times and one copy has drifted.

`src/librarian/tools/find.rs:524` and `src/librarian/tools/workspace_state_at.rs:98`
each open with the same ~30-line block, verbatim down to the line-wrap position of
the shared error string (`src/librarian/tools/find.rs:529`,
`src/librarian/tools/workspace_state_at.rs:103`):

1. `let requested_scope = a.scope.unwrap_or_default();`
2. reject `scope="all"` when the current project has no umbrella,
3. alias `All → Umbrella` when it does,
4. fall back to `All` when there is no current project at all.

`src/librarian/tools/context.rs:56` has steps 1 and 4 only. Steps 2 and 3 are
absent. Since `apply_scope` maps `Scope::All` to no clause at all
(`src/librarian/tools/scope.rs:75`, `Scope::All => None`), a `context` call that
would have been narrowed to the umbrella's member prefixes instead runs unfiltered.

Measured 2026-08-15 by running both calls against the live server and comparing the
`scope.applied` each reported — not inferred from the source alone.

### Two further divergences from the same truncated copy

- **Worktree shadow rows are not excluded.** All three `apply_scope` call sites in
  `src/librarian/tools/context.rs` pass `&[]` for `exclude_worktrees`, where
  `src/librarian/tools/find.rs` computes the real set from
  `crate::librarian::catalog::worktree::active_roots`. A `grep` for
  `exclude_worktrees|active_roots` across `src/librarian/**/*.rs` returns matches in
  four files; `src/librarian/tools/context.rs` and
  `src/librarian/tools/workspace_state_at.rs` are not among them. Another session's
  shadow artifacts can therefore appear in a context bundle.
- **The `scope` block has a different shape.** `src/librarian/tools/context.rs`
  defines its own `scope_summary` rather than using `ScopeApplied::to_json`, so
  `context` reports `root` / `subdir` where `find` reports `abs_path` / `git_root`.
  Cosmetic, but it means the two tools' scope blocks cannot be compared by a caller
  without special-casing.

## Evidence

Prologue start lines, all three verified by grep at `1d22b715`:

```
src/librarian/tools/context.rs:56          let requested_scope = a.scope.unwrap_or_default();
src/librarian/tools/find.rs:524            let requested_scope = a.scope.unwrap_or_default();
src/librarian/tools/workspace_state_at.rs:98   let requested_scope = a.scope.unwrap_or_default();
```

The guard string exists in exactly two of the three:

```
src/librarian/tools/find.rs:529            "scope=\"all\" requires a configured umbrella — without one it crosses into \
src/librarian/tools/workspace_state_at.rs:103  "scope=\"all\" requires a configured umbrella — without one it crosses into \
```

The error text names precisely the failure that `context` then performs silently:
*"without one it crosses into unrelated workspace projects."*

## Hypotheses tried

1. **Hypothesis:** `context` does not accept `scope="all"`, so the divergence is
   unreachable.
   **Test:** read the `Scope` enum — `src/librarian/tools/scope.rs`, derives
   `Deserialize` with `#[serde(rename_all = "lowercase")]` and an `All` variant;
   `context`'s `Args.scope` is `Option<Scope>`, the same type. Then ran the call.
   **Verdict:** rejected — it deserializes and reaches `apply_scope`.

2. **Hypothesis:** the narrowing is deliberate, because `context` is a read-only
   bundler and wider reach is a feature.
   **Test:** looked for a comment, test, or doc stating it. `src/librarian/tools/context.rs` has one
   scope test (`repo_scope_excludes_other_repos`); nothing covers `all` or the
   umbrella alias, and no comment marks the omission.
   **Verdict:** deferred — cannot be confirmed from the code. If it *is*
   deliberate it needs a comment and a test, because the two sibling handlers
   encode the opposite intent in a shared error string.

## Fix

Not yet implemented. The shape the evidence points at: extract the prologue
(steps 1–4 above) into one function beside `apply_scope` in
`src/librarian/tools/scope.rs`, and call it from all three handlers. That
collapses the drift and the duplicated error string together, and it is the same
move already validated for the SQL-side descendant predicate in `31609aa5`
(`descendant_path_like`) — extract the whole shared unit, not the eye-catching
fragment.

Deliberately **not** done in the same change: whether `context` should also
exclude worktree shadow rows, and whether `scope_summary` should be retired in
favour of `ScopeApplied::to_json`. Both are behaviour changes to a second tool
and want their own decision.

## Tests added

None yet. The regression test this needs is a scope-parity test asserting that
`find`, `context`, and `workspace_state_at` resolve an identical
`(requested_scope, has_current_project, has_umbrella)` triple to the same
effective scope — a per-handler test would not have caught this, because each
handler is self-consistent.

## Workarounds

Pass `scope="umbrella"` explicitly to `librarian(action="context")` rather than
`scope="all"`. That reaches the `Scope::Umbrella` arm of `apply_scope` directly
and applies the member-prefix clause, which is what `find` would have done with
`scope="all"` anyway.

## Resume

Decide whether `context`'s wider reach is intentional (Hypothesis 2 is deferred,
not rejected). If it is not, extract the prologue from
`src/librarian/tools/find.rs:524` into `src/librarian/tools/scope.rs` beside
`apply_scope`, call it from all three handlers, and add the parity test named
under "Tests added". Run `cargo test --workspace` and re-run the two-call A/B
from "Reproduction" against a rebuilt server (`cargo rb`, then `/mcp`) to confirm
both report the same `scope.applied`.

## References

- `docs/trackers/structural-debt-refactor.md` — SD-3, whose "read the other three
  before proposing any extraction" instruction surfaced this.
- `src/librarian/tools/scope.rs` — `apply_scope` and the `Scope` enum.
