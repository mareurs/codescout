---
kind: bug
status: fixed
tags:
- workspace
- discoverability
- multi-project
- memory
closed: 2026-08-27
opened: 2026-08-26
owner: marius
related: []
severity: medium
---

# BUG: `workspace(list_projects)` reports DECLARED projects, not workspace members — auto-discovered sub-projects are invisible

## Summary

`workspace(action="list_projects")` and `workspace(action="status")` re-read
`.codescout/workspace.toml` from disk and report only its `[[project]]` entries.
`workspace(action="activate")` reports the **live** workspace, which includes
auto-discovered sub-projects. So a valid project id that `memory`, `symbols` and
`activate` all accept can be absent from the one surface documented as the way to find
valid ids.

## Symptom (Effect)

Measured on `codescout` itself, 2026-08-26, in one session with no state change between
calls:

```
workspace(action="list_projects")   → projects: [ codescout ]                    # 1
workspace(action="status")          → workspace.projects: [ codescout ]          # 1

# the same session's activation banner:
| codescout       | .                      | markdown, rust |                   # 2
| codescout-embed | crates/codescout-embed | rust           |

memory(action="list", project_id="codescout-embed")  → 5 topics                  # accepted
```

The last line is the tell. `resolve_memory_dir` validates the caller-supplied id against
`ws.has_project(id)` and returns `RecoverableError("No project '<id>'.")` with the valid
ids listed when it misses (`src/tools/memory/mod.rs`). It did not miss, so the live
`Workspace` **does** contain `codescout-embed` — while the surface that exists to
enumerate members does not show it.

## Reproduction

Any repo with a `.codescout/workspace.toml` that declares fewer projects than discovery
finds — here, one `[[project]]` entry and a cargo sub-crate at `crates/codescout-embed`.

```
cat .codescout/workspace.toml     # one [[project]]: id="codescout", root="."
```
Then compare `workspace(action="list_projects")` against `workspace(action="activate")`
and against `memory(action="list", project_id="<discovered-id>")`.

## Environment

`experiments` @ `2e43097e`, Linux, MCP stdio. Not platform-specific.

## Root cause

Two builders for the same field, reading two different sources.

| Surface | Source | Code |
|---|---|---|
| `status` / `list_projects` | `.codescout/workspace.toml`, re-read and re-parsed **from disk** | `src/tools/config/mod.rs:511-534` |
| `activate` | `ctx.agent.workspace_summary()` — the live `Workspace` | `src/tools/config/mod.rs:796-806` |

`list_projects` is not a distinct handler: it calls `ProjectStatus` and projects out the
`workspace` key (`src/tools/config/mod.rs:69-72`), so it inherits the disk-read.

Because the status path parses the TOML itself rather than asking the agent, it also never
runs discovery — `discovery_max_depth` and `exclude_projects` have no bearing on its
answer. It reports what someone wrote down, not what the server resolved.

Measured 2026-08-26 by reading both builders and by the three-call comparison above.

## Evidence

`.codescout/workspace.toml` on this host (gitignored, per-machine — see CLAUDE.md):

```toml
exclude_projects = ["fixtures"]
[workspace]
name = "codescout"
discovery_max_depth = 3
[[project]]
id = "codescout"
root = "."
```

One declared project; `crates/codescout-embed/Cargo.toml` supplies the second by
discovery.

## Hypotheses tried

1. **`memory` accepted an unknown id and silently created a directory** — the failure mode
   `docs/issues/archive/2026-08-08-memory-dir-for-project-materializes-any-id.md` fixed.
   **Test:** read `resolve_memory_dir`; it validates with `ws.has_project` and errors with
   the valid-id list. **Verdict:** rejected — the id is genuinely known.
2. **The activation banner is rendered from a different, staler source than the tool JSON.**
   **Test:** read both builders. **Verdict:** confirmed, but the polarity is the reverse of
   the guess: the *banner* is live and the *tool JSON* is the stale one.

## Fix

**Fixed, Option A: `list_projects`/`status` now report live workspace members.**

- **SHA:** `1af0dcde` (`experiments`)
- **patch-id:** `ddb2f0a7875f4b06f10ecff44394c2b14a9aee38`

`fix(workspace): list_projects/status report live workspace members, not just declared config`

`ProjectStatus::call`'s workspace section (`src/tools/config/mod.rs`) now sources
its `projects` array from `ctx.agent.discovered_projects()` — the same live,
manifest-walked `Workspace.projects` that `activate`'s table already reads via
`Agent::workspace_summary` — instead of re-parsing `.codescout/workspace.toml`'s
bare `[[project]]` array. `list_projects` inherits this for free: it is a pure
projection of `ProjectStatus`'s response (`mod.rs:69-72`), not a separate
handler.

`depends_on` is the one field a manifest walk cannot supply — nothing on disk
states a dependency edge — so it is still looked up from the declared config,
by matching the *discovered* id against `ws.projects[].id`, exactly mirroring
the lookup `Agent::workspace_summary` already does for `activate`'s table.
`name` and `resources` are unaffected: those are genuinely declared-only
fields with no discovery equivalent, so they still come straight from the
parsed TOML.

**A real, deliberate side effect worth naming:** the declared `id` field on a
`[[project]]` entry was never actually consulted for *labelling* — `discover_projects`
assigns every project's id from its directory's basename
(`src/workspace.rs`), and the declared entry is only ever used as a lookup key
for `depends_on`. That was already true of `activate`'s table before this fix;
this fix makes `status`/`list_projects` consistent with it rather than
introducing new behavior. Concretely: a repo whose root directory name differs
from its declared root `id` will now show the directory-derived id here too,
where it previously echoed back whatever was written in `workspace.toml`.

Also fixed `ActivateProject`'s own error hint (`mod.rs:160`), which claimed
`list_projects` "shows the configured ones" — no longer true, and was arguably
never the right contract for a hint whose whole purpose is pointing an agent at
valid ids.

**Verified:** `cargo fmt`, `cargo clippy --all-targets -- -D warnings`,
`cargo test --lib` → 4415 passed, 0 failed, 8 ignored.
## Tests added

`project_status_reports_a_live_discovered_project_the_declared_config_omits`
(`src/tools/config/tests.rs`), shipped in `1af0dcde`.

Declares 2 projects (root + `declared-svc`, the latter with `depends_on =
["test"]`) but writes a 3rd manifest (`extra-service/package.json`) with no
declaration at all — the same shape as the bug's own reproduction
(`codescout-embed` was live but undeclared on this repo). Asserts:

- `projects.len() == 3`, not 2 — the live count, not the declared count;
- the undeclared `extra-service` project appears by its discovered id;
- the declared `declared-svc` project's `depends_on == ["test"]` still comes
  through — guards against the id-based lookup silently dropping metadata
  that used to be echoed verbatim.

**Verified red before green:** ran with the pre-fix disk-read code still in
place; failed with `left: 2, right: 3` — confirms the assertion actually
exercises the live-vs-declared gap rather than passing vacuously.

The pre-existing `project_status_shows_workspace_projects` test (2 declared
projects, both independently discoverable by manifest) still passes unchanged
— it only asserts `projects.len() == 2`, which holds under either the old or
new source, so it does not by itself distinguish declared from live. That gap
is exactly why the new test above declares fewer than are discoverable.
## Workarounds

Use `workspace(action="activate")`'s response, whose project table is live, or try the id
directly: `memory(action="list", project_id="<guess>")` errors with the full list of valid
ids when the guess is wrong, which makes it a better enumerator than `list_projects`.

## Resume

Fixed and archived. Nothing further planned. If a future report claims
`list_projects` is missing a valid id again, re-check with
`workspace(action="activate")` first — if `activate`'s table also omits it,
the defect has moved to `Agent::discovered_projects`/`discover_projects`
(`src/workspace.rs`) itself, not the surface this bug fixed.
## References

- `docs/issues/2026-07-07-memory-tool-hides-project-memories-after-workspace-activate.md` —
  found in the same pass and adjacent: that one is two memory *directories* for a
  sub-project, this one is the sub-project being unlistable in the first place. Both bite
  only when a workspace has a non-root project.
- `docs/issues/archive/2026-08-08-memory-dir-for-project-materializes-any-id.md` — where
  `resolve_memory_dir`'s id validation (the probe that proved the id is known) came from.
