---
id: 3648e50b78f3229a
kind: bug
status: wontfix
title: 'BUG: librarian(action="context") drops the umbrella alias and the no-umbrella guard, so scope="all" crosses into non-member projects'
tags:
- librarian
- scope
- umbrella
- duplication-drift
- context
topic: librarian scope resolution
closed: 2026-08-15
opened: 2026-08-15
owner: marius
related:
- d31233700ca979c2
severity: high
---

# BUG: librarian(action="context") drops the umbrella alias and the no-umbrella guard, so scope="all" crosses into non-member projects

## Summary

**Closed `wontfix` — the behaviour is intended. See Fix.**

`librarian(action="context", scope="all")` applies no scope clause and draws
candidates from every project in the catalog, where
`artifact(action="find", scope="all")` narrows to the configured umbrella. The
divergence is real and was measured; the *judgement* that it is a defect was
wrong. `context` is an orientation tool, and reaching across everything when
explicitly asked for `all` is the point of it.

What this investigation leaves behind is not this behaviour but two things it
uncovered on the way: the `find` / `workspace_state_at` prologue duplication
(SD-10 in `docs/trackers/structural-debt-refactor.md`) and an un-deduplicated
worktree overlay, which has its own file —
`docs/issues/archive/2026-08-15-context-and-state-at-never-dedup-the-worktree-overlay.md`.

The original report is kept below unedited. It was right about the mechanism and
wrong about what the mechanism means, which is worth being able to read back.
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
   **Verdict:** CONFIRMED by the owner, 2026-08-15 — it is deliberate, and the
   rationale is broader than "read-only bundler": `context` exists to give
   cross-project, cross-session visibility. The code genuinely could not settle
   it, and that half of the finding stands — nothing in the file said so. Both
   consequences named here still apply: it wants a statement of intent (better
   still, a typed policy at the call site than a comment) and a test.

## Fix

**WONTFIX — the wider reach is intentional.** Owner decision, 2026-08-15:
`librarian(action="context")` can and should reach across every project when
asked for `scope="all"`. The whole value of an orientation tool is
cross-project, cross-session visibility — other sessions' state, recorded
sessions, and whatever else orientation needs to see.

The two handlers are not inconsistent once you read them as different kinds of
tool. `find` narrowing `all` to the umbrella is a safety default appropriate to a
*search* surface, where a caller usually wants their own work and a silent
cross-project hit is noise. `context` taking `all` literally is appropriate to an
*orientation* surface, where the caller has explicitly asked to look further than
usual. Both are correct for what they are; what was missing was any statement
that this was a choice.

So two pieces of work survive the decision — tracked on SD-10, not here:

1. **Make the divergence explicit at the call site.** When the prologue is
   extracted, give it a policy parameter (e.g. `UmbrellaPolicy::Require` for
   `find` and `workspace_state_at`, `UmbrellaPolicy::Literal` for `context`) so
   the intent is stated in the signature. A comment can rot; an absence says
   nothing at all, which is precisely how this file came to be opened.
2. **Pin the behaviour with a test.** An intended behaviour that nothing defends
   is indistinguishable from an accident — and will be re-reported.
## Tests added

N/A for a `wontfix` — but see Fix item 2, which is a real gap rather than a
formality. `src/librarian/tools/context.rs` carries exactly one scope test
(`repo_scope_excludes_other_repos`); nothing covers `all`, and nothing covers the
umbrella alias in either direction. The behaviour this file now blesses is held
up by no assertion anywhere.
## Workarounds

N/A — nothing to work around. A caller who wants the narrower pool can pass
`scope="umbrella"` to `librarian(action="context")` and get exactly what
`artifact(action="find", scope="all")` would have given.
## Resume

N/A — closed. Follow-up lives in two places: SD-10 in
`docs/trackers/structural-debt-refactor.md` (extract the prologue with an
explicit umbrella policy; add the `context`-literal-`all` test), and
`docs/issues/archive/2026-08-15-context-and-state-at-never-dedup-the-worktree-overlay.md`
for the one finding this decision does not settle.
## References

- `docs/trackers/structural-debt-refactor.md` — SD-3, whose "read the other three
  before proposing any extraction" instruction surfaced this.
- `src/librarian/tools/scope.rs` — `apply_scope` and the `Scope` enum.
