---
id: b010f1812f5a74ed
kind: bug
status: open
title: 'BUG: scope has three different defaults across the librarian surface, and all four doc surfaces say "project" while find compiles to repo'
tags:
- librarian
- scope
- doc-drift
- prompt-surface
closed: null
opened: 2026-08-27
owner: marius
related:
- docs/issues/archive/2026-07-05-audit-doc-refs-scope-param-ignored.md
severity: medium
unverified: Hypothesis 2 deferred — whether the Repo default was deliberate is inferred from a missing doc comment, not established. Blast radius is currently zero (project == repo root in 923/923 observed scope blocks), so no wrong answer has been observed, only a false contract.
---

# BUG: `scope` has three different defaults across the librarian surface, and all four documentation surfaces say "project" — the compiled default for `find` is `repo`

## Summary
Every documented surface states that `scope` defaults to the active **project**.
The compiled default for `find` / `context` / `workspace_state_at` is
`Scope::Repo` (`#[default]` sits on `Repo`, `src/librarian/tools/scope.rs:26`).
`link_scan` defaults to `Project`, and `reindex` defaults to `Project`-or-`All`
depending on whether a project is active. One parameter name, three policies,
zero of which match the docs for all their actions.

## Symptom (Effect)
A `find` call passing no `scope`, run with `/home/marius/work/claude/codescout`
active, reports what actually resolved in its own response:

```json
"scope": {
  "applied": "repo",
  "abs_path": "/home/marius/work/claude/codescout",
  "git_root": "/home/marius/work/claude/codescout",
  "umbrella": "codescout-ecosystem"
}
```

`applied: "repo"` — not `project`. The tool schema for that same call says:

```
"description": "find: scope for listing. Defaults to active project."
```

## Reproduction
```
workspace(action="activate", path="/home/marius/work/claude/codescout")
artifact(action="find", kind="tracker", limit=1)
# read $.scope.applied  ->  "repo"
```
Observed on branch `experiments`, tip `2dc8cadb`, 2026-08-27.

Reproducible from the log too:
```
python3 scripts/probe_librarian_scope.py --crosstab
# "requested -> APPLIED" table:
#   find       <omitted>  -> repo     822
#   context    <omitted>  -> repo       4
#   link_scan  <omitted>  -> project    2
```

## Environment
Linux, codescout MCP over stdio, branch `experiments`. 13 registered project
roots across `~/work` and `~/agents`.

## Root cause
Three independent default policies, none of them wrong in isolation, never
reconciled — and a doc set written against the one that is least common.

- **`find` / `context` / `workspace_state_at`** call `resolve_scope`
  (`src/librarian/tools/scope.rs:59`), whose first line is
  `let scope = requested.unwrap_or_default();`. `Scope`'s `#[derive(Default)]`
  puts `#[default]` on the **`Repo`** variant (`scope.rs:24-30`) — so an omitted
  `scope` resolves to `Repo`.
- **`link_scan`** bypasses `resolve_scope` entirely and hardcodes the other
  answer: `let effective_scope = args.scope.unwrap_or(Scope::Project);`
  (`src/librarian/tools/link_scan/mod.rs:224`).
- **`reindex`** has a third policy: `a.scope.unwrap_or_else(|| if
  ctx.current_project.is_some() { Scope::Project } else { Scope::All })`
  (`src/librarian/tools/reindex.rs:83-88`).

measured 2026-08-27: `artifact(action="find", kind="tracker", limit=1)` with no
`scope` → `$.scope.applied == "repo"`, live, on the tip of `experiments`. The
same value appears on 822 logged `find` calls and 4 `context` calls across 13
`usage.db` files.

**Why nobody noticed.** In 923 of 923 recorded `scope` blocks, `abs_path ==
git_root` — the active project *is* its git repo everywhere it has been observed,
so `Repo` and `Project` select identical rows and the drift is unobservable. It
becomes real the moment a workspace has sub-projects: codescout's own
`.codescout/workspace.toml` declares `codescout-embed` at `crates/codescout-embed`,
and with that activated, the documented default would return its subtree while the
compiled default returns the whole repo.

## Evidence

### The four documentation surfaces, all saying "project"
| Surface | Text |
|---|---|
| `src/librarian/tools/artifact.rs:70` | `"find: scope for listing. Defaults to active project."` |
| `src/librarian/tools/librarian.rs:61` | `"context/reindex/workspace_state_at/link_scan: scope. … Defaults to active project."` |
| `src/prompts/guides/librarian.md:79` | `` - `scope="project"` (default) — active project only (artifacts under its path) `` |
| `src/librarian/prompts/companion_hint.md:56` | `default to **the active project's path** and **hide archived/superseded**` |

`librarian.rs:61` is the most misleading of the four: it names four actions in one
sentence, and that sentence is correct for `link_scan`, wrong for `context` and
`workspace_state_at`, and wrong-in-a-third-way for `reindex`.

### Requested → applied, from the responses' own scope blocks
```
find           <omitted>    -> repo          822
find           repo         -> repo           28
find           umbrella     -> umbrella       24
find           all          -> umbrella       22
link_scan      project      -> project        17
find           project      -> project         8
context        <omitted>    -> repo            4
link_scan      <omitted>    -> project         2
```
(`scripts/probe_librarian_scope.py --crosstab`, 13 dbs, 6,445 calls, 24d.)

### The blast radius today is zero, and that is a fact about the corpus
`project == repo root` in 923/923 recorded scope blocks. No observed call would
have returned different rows under the documented default. This bug is about a
contract that is false, and a latent divergence, not an observed wrong answer.

## Hypotheses tried
1. **Hypothesis:** the docs are right and the host injects `scope="project"`.
   **Test:** ran `artifact(action="find", kind="tracker", limit=1)` live and read
   `$.scope.applied`; also confirmed the logged `input_json` carries no `scope` key.
   **Verdict:** rejected — nothing injects it; `applied` is `repo`.
2. **Hypothesis:** `#[default]` on `Repo` is deliberate and the docs are simply stale.
   **Test:** read `scope.rs:24-30`; the attribute carries no comment, while the
   neighbouring `UmbrellaPolicy` enum (`scope.rs:39-50`) has a long doc comment
   explaining exactly why *its* two answers both exist.
   **Verdict:** deferred — the asymmetry suggests the `Repo` default was not a
   documented choice, but that is an inference from a missing comment, not proof.
   Whoever fixes this should decide the intended default rather than assume.
3. **Hypothesis:** `link_scan`'s divergence is a copy that fell behind.
   **Test:** `link_scan/mod.rs:29` imports `{apply_scope, Scope}` and deliberately
   not `resolve_scope`, so it never had the `unwrap_or_default()` path.
   **Verdict:** confirmed as intentional-looking, but undocumented.

## Fix
*Plan — decide first, then make it uniform.* Two coherent endpoints:

- **(A) Make the code match the docs.** Move `#[default]` to `Project`. Narrower
  by default, matches every documentation surface and the principle already
  adopted for `doctor`'s worklist. Behaviour change is nil on every observed
  call (project == repo everywhere measured) and only bites sub-project setups —
  where it is arguably the correct answer anyway.
- **(B) Make the docs match the code.** Leave `Repo` and correct all four
  surfaces. Cheaper, but leaves three defaults for one parameter and leaves
  `librarian.rs:61` needing per-action wording.

**(A) is preferred**, with `link_scan` then already correct and `reindex`'s
`Project`-or-`All` fallback kept but documented (its `All` arm is the no-active-
project case, which `resolve_scope` handles as `scope_fallback` for the others).

Either way, the enum's `#[default]` should carry a doc comment saying which
answer it is and why — the neighbouring `UmbrellaPolicy` is the model, and its
comment exists because *"a deliberate choice indistinguishable from a truncated
copy"* was the exact failure mode (`scope.rs:41-44`).

Not started. No SHA, no patch-id yet.

## Tests added
None yet. A regression test should assert `resolve_scope(None, Some(&cp),
UmbrellaPolicy::Require).0 == <the decided default>` and — more valuable — a test
that pins **all three** entry points to the same answer, so the next divergence
fails a test rather than a reader. `scope.rs`'s existing
`scope_fallback_arm_is_not_inlined_outside_resolve_scope` (`scope.rs:512`) is the
precedent for pinning a policy rather than a value.

## Workarounds
Pass `scope` explicitly. Currently 84.0% of scope-accepting calls omit it, so in
practice: read `$.scope.applied` on the response — every scope-aware surface
reports what actually resolved, and that field has been correct throughout.

## Resume
Decide (A) or (B) with the user — the choice is a product decision, not a
mechanical fix, and Hypothesis 2 is explicitly unresolved. If (A): move
`#[default]` from `Repo` to `Project` in `src/librarian/tools/scope.rs:26`, run
`cargo test` and expect fallout in `scope.rs`'s own test module (several tests
construct `Scope` via `..Default::default()`-adjacent paths), then re-run
`python3 scripts/probe_librarian_scope.py --crosstab` and confirm the
`find <omitted> -> ...` row reads `project`. If (B): correct the four surfaces in
the Evidence table, giving `librarian.rs:61` per-action wording.

## References
- `src/librarian/tools/scope.rs:24-30` — the `#[default]` on `Repo`
- `src/librarian/tools/scope.rs:59` — `resolve_scope`'s `unwrap_or_default()`
- `src/librarian/tools/scope.rs:39-50` — `UmbrellaPolicy`, the model for documenting a two-answer choice
- `src/librarian/tools/link_scan/mod.rs:224` — `unwrap_or(Scope::Project)`
- `src/librarian/tools/reindex.rs:83-88` — the `Project`-or-`All` third policy
- `src/librarian/tools/artifact.rs:70`, `src/librarian/tools/librarian.rs:61`,
  `src/prompts/guides/librarian.md:79`, `src/librarian/prompts/companion_hint.md:56` — the four drifting surfaces
- `scripts/probe_librarian_scope.py` — `--crosstab` prints the requested → applied table
- `docs/issues/archive/2026-07-05-audit-doc-refs-scope-param-ignored.md` — prior bug of the same class (scope declared, behaviour different)

