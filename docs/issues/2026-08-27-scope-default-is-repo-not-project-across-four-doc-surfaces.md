---
id: b010f1812f5a74ed
kind: bug
status: fixed
title: 'BUG: scope has three different defaults across the librarian surface, and every documentation surface says "project" while find compiles to repo'
tags:
- librarian
- scope
- doc-drift
- prompt-surface
closed: 2026-08-27
opened: 2026-08-27
owner: marius
related:
- docs/issues/archive/2026-07-05-audit-doc-refs-scope-param-ignored.md
severity: medium
unverified: 'Not live-verified. The fix is committed and the suite is green, but this session''s MCP server runs a binary built before the change, so a live artifact(action="find") still reports applied: "repo". Observing the new default needs `cargo rb` plus `/mcp`. Committed and tested is not live — see reconnaissance-patterns:R-89 (freshness is a property of the copy that SERVES you).'
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

### The documentation surfaces, all saying "project" — three live, one inert

**Corrected 2026-08-27 (`bug-fix-session-log:F-73`).** This section originally
called all four live. `companion_hint.md` is not: its only reference in the whole
tree is `tests/librarian/companion_hint.rs:7`, which `include_str!`s it to assert
it names no unknown tools. Nothing under `src/` reads it — it is a leftover of the
crate dissolved in `d48bf992`. A grep cannot tell a live `include_str!` from a
test-only one, and this list was built by grep.
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
   **Verdict:** **resolved 2026-08-27 — not an endorsement.** `git log -S` on the
   enum found `649dc0a4` (*"one scope-resolution prologue, with the divergence
   declared (SD-10)"*), whose commit body states: *"the umbrella GUARD keys off
   the raw Option (a.scope == Some(All)) while the ALIAS keys off the defaulted
   value — equivalent today because the default is Repo, and **preserved
   regardless in case that default changes**."* The author who built
   `resolve_scope` knew the default was `Repo` and deliberately wrote the
   guard/alias split to survive it being changed. That is anticipation of this
   change, not a defence of the value — which is what moved the decision from (B)
   toward (A)/(C).
3. **Hypothesis:** `link_scan`'s divergence is a copy that fell behind.
   **Test:** `link_scan/mod.rs:29` imports `{apply_scope, Scope}` and deliberately
   not `resolve_scope`, so it never had the `unwrap_or_default()` path.
   **Verdict:** confirmed as intentional-looking, but undocumented.

## Fix

**Option C**, chosen by the owner over (A) *move the attribute* and (B) *correct
the docs*. Shipped in `bf3c2c78` on `experiments`.

- `Scope` loses `Default` entirely (`src/librarian/tools/scope.rs`). The default
  is a property of each SURFACE, not of the enum; a derive attribute cannot say
  which surface it speaks for. The enum's new doc comment says exactly that.
- `resolve_scope` takes `default: Scope` as a required argument. `find`,
  `context` and `workspace_state_at` each pass `Scope::Project` with a call-site
  comment. Same argument `UmbrellaPolicy` already makes, applied to the other
  half of the same decision.
- `link_scan` (`unwrap_or(Scope::Project)`) and `reindex` (`Project`-or-`All`)
  were already explicit and are unchanged.

**Three of the four documentation surfaces became true without being edited** —
`artifact.rs:70`, `guides/librarian.md:79` and `companion_hint.md:56` all already
said "active project". That is most of the argument for C over B. Only
`librarian.rs:61` changed, because `reindex` genuinely differs and one sentence
was covering four actions with three defaults.

No `all`-handling moved: the guard still keys off the raw `Some(All)` and the
alias off the defaulted value, and the `project`/`repo` → `all` fallback arm is
untouched.

**Fix commit — record both, they fail differently:**

- SHA `bf3c2c78e42a55d5433e4fbb93cc54227d38e770` on **`experiments`** (positional;
  dies when `experiments` is rebased)
- patch-id `9cf83021d688fc033493a92bdecf4a5d271e96d8` (`git show <sha> | git
  patch-id --stable`; content hash of the diff, survives rebase and cherry-pick)
## Tests added

`every_resolve_scope_call_names_project_as_its_default` —
`src/librarian/tools/scope.rs`, in the `tests` module beside the sibling DRY gate
`scope_fallback_arm_is_not_inlined_outside_resolve_scope`.

It scans **source text**, not behaviour, on purpose: a test that calls
`resolve_scope(None, .., Scope::Project)` and asserts it returns `Project` is
computed from the thing it judges and cannot fail. It asserts every
`resolve_scope` CALL under `src/` names `Scope::Project`, skipping the definition
and line comments, with the needles assembled character-wise so the test's own
source does not match them.

It carries a **positive control** — `assert!(checked >= 3)` — so a needle that
stops matching reads as a broken scan rather than a clean tree.

**Mutation-verified, not merely green.** Flipping `find.rs`'s argument to
`Scope::Repo` failed the gate with
`Offenders: ["librarian/tools/find.rs:704"]`; reverted after.

Also: `context::tests::repo_scope_excludes_other_repos` renamed to
`default_scope_is_project_and_excludes_other_repos`, its `applied` assertion
flipped `"repo"` → `"project"`. Worth noting for its fixture — it is the only
place in the suite where `abs_path != git_root`, the one shape that can tell the
two defaults apart, and a shape `scripts/probe_librarian_scope.py` measured at
**0 of 923** real recorded scope blocks. The suite is a better oracle for this
change than the usage log.

Gate at fix time: 4598 passed / 0 failed / 51 ignored; `cargo fmt --all --
--check` clean; `cargo clippy --workspace --all-targets -D warnings` clean.
## Workarounds
Pass `scope` explicitly. Currently 84.0% of scope-accepting calls omit it, so in
practice: read `$.scope.applied` on the response — every scope-aware surface
reports what actually resolved, and that field has been correct throughout.

## Resume

One step outstanding, and it is the reason `unverified:` is set rather than
cleared: **live-verify**. Run `cargo rb`, then `/mcp` to reconnect, then
`artifact(action="find", kind="tracker", limit=1)` and confirm
`$.scope.applied == "project"`. Until then the honest claim is *committed and
tested*, not *live* — this session's server still serves the pre-change binary
and still answers `"repo"`.

A second, optional confirmation once live:
`python3 scripts/probe_librarian_scope.py --crosstab` should start recording
`find <omitted> -> project` in its requested→applied table, where the 2026-08-27
baseline reads `find <omitted> -> repo 822`.
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
