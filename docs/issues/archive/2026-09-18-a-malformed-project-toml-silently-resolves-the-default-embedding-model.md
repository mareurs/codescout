---
id: bcb1d662862dbfc9
kind: bug
status: fixed
title: 'BUG: a malformed project.toml silently resolves the default embedding model instead of erroring'
tags:
- cluster/unclassified
- embeddings
- config
- error-handling
---

## Summary

`resolve_embed_fields_with` (`src/retrieval/config.rs`) resolves a project root's
`.codescout/project.toml` via `root.and_then(|r| ProjectConfig::load_or_default(r).ok())` —
the `.ok()` turns a **parse error** (e.g. a `[embeddings]` table with no `[project]` table,
which `ProjectSection.name` requires) into `None`, which then falls through
`merge_embed_config`'s `unwrap_or_else(default_embed_model)` to the **built-in default
model**, silently. A project whose `project.toml` fails to parse gets `local:AllMiniLML6V2Q`
with no error and nothing in the response naming the substitution — the same shape
`f73130523241a666` (archived, fixed) found in the workspace-pin memory-read path, but this
is a **separate call site** in a **separate subsystem** (embedding config resolution, not
project-pin memory lookup) that the same fix did not touch.

## Reproduction

Tree at this bug's filing. Any `.codescout/project.toml` with an `[embeddings]` table but no
`[project]` table (or any other required-field violation) reproduces it:

```
[embeddings]
model = "local:JinaEmbeddingsV2BaseCode"
```

`ProjectConfig::load_or_default(root)` on that root returns `Err("missing field project")`.
`RetrievalConfig::from_env_and_project(Some(root))` on the SAME root returns
`Ok(RetrievalConfig { model: "local:AllMiniLML6V2Q", .. })` — the built-in default, not an
error, and nothing distinguishes this from a project that genuinely never configured a model.

Found while writing `resolved_chunk_budget_reflects_the_projects_configured_model`
(`src/tools/memory/tests.rs`, 2026-09-18): the first draft used exactly this malformed shape
as a test fixture and both "differently configured" roots silently produced the SAME
(default) budget — the test's own bug, but it only revealed itself by directly probing
`ProjectConfig::load_or_default`'s return value rather than trusting the assertion.

## Hypotheses tried

None yet — filed on notice per `CLAUDE.md` § Bug Tracking, not investigated for a fix. The
existing archived fix (`f73130523241a666`, `c1c8763a`) is the template if this one is picked
up: propagate the error with `?` rather than `.ok()`, at whichever layer between
`RetrievalConfig::from_env_and_project` and its callers is the right place to surface it —
`resolve_embed_fields_with`'s own doc comment already says it is "a layer this module does not
own and cannot edge-resolve away," which is the reason NOT to fix it inline as part of Task 8.

## Fix

**FIXED 2026-09-19** — `e635d4dab44a4b434a300ceab2b8402088d43a4d`, patch-id `89bf0a9bbd81be5a32efa790eb8dbfc912e5b9ca`. `resolve_embed_fields_with` propagates with `?` as a `RecoverableError` instead of `.ok()`-ing the parse failure into `None`. The missing-file path is unchanged and now pinned separately — its red was observed under mutation, since it cannot red on the production path.

Landed separately from Task 8, as originally scoped — fixed alongside the unrelated `c222737eedb69850` (under-isolated env tests in the same file set), because the second bug was what made the first invisible on the fixing session's own machine.

## Tests added

`tests/retrieval_unit.rs` gained a dedicated malformed-`project.toml` case alongside a
broader fix: five env-touching tests in that file (three pre-existing, all of them
under-isolated — `c222737eedb69850`) were unified onto one `with_isolated_retrieval_env`
helper deriving its var list from `embedding_env::all_names()`, plus
`the_isolation_helper_clears_every_declared_embedding_name` guarding that derivation itself
(mutation-probed: deleting the loop gives KILLED). Verified in both directions — 15 ambient
vars set (the world the bug lives in) and cleared (what CI sees) — 12 passed, 0 failed
each way.

## Workarounds

None needed for a correctly-formed `project.toml`; this only bites a malformed one, which
already fails loudly on every OTHER config read (`Agent::with_project_at`, the archived
sibling bug's write path) — only the embedding-config resolution path stays silent.

## Resume

Pick up by reading `resolve_embed_fields_with`'s own doc comment first — it already names the
two env-var families and the layering constraint; the open question is which layer should own
the propagated error without breaking the `root: None` (no-project) case, which legitimately
has no project.toml to fail to parse.

## References

- `src/retrieval/config.rs` — `resolve_embed_fields_with`, `.ok()` at the `and_then`.
- `f73130523241a666` (archived) — the sibling instance, workspace-pin memory-read path.
- `docs/plans/2026-09-17-embedding-config-consolidation.md` Task 8 — the work this was found during.
