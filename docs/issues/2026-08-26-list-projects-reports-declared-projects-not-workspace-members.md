---
status: open
opened: 2026-08-26
closed:
severity: medium
owner: marius
related: []
tags: [workspace, discoverability, multi-project, memory]
unverified: 'Mechanism established and measured; NOT fixed. Which answer is correct is a decision — `list_projects` may be intended to report the declared config, in which case the bug is that it is documented and pointed to as the way to discover valid project ids. No fix, no test.'
kind: bug
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

Not written. The mechanical change is small — have the status path ask
`agent.workspace_summary()` like `activate` does — but the choice is not, and this is why
it is filed rather than patched:

- If `list_projects` is meant to report **members**, it should use the live workspace, and
  today's answer is simply wrong.
- If it is meant to report the **declared config** (a reasonable thing to want — "what did
  I write down?"), then the bug is that it is named `list_projects`, described as
  "workspace members", and named in `ActivateProject`'s own error hint as the way to find
  a valid id. In that reading the fix is to report both, or to rename.

Either way the user-visible defect is the same: an agent that follows the hint to discover
project ids gets an incomplete list, and cannot reach a sub-project by id without already
knowing it exists.

## Tests added

None — no fix yet.

## Workarounds

Use `workspace(action="activate")`'s response, whose project table is live, or try the id
directly: `memory(action="list", project_id="<guess>")` errors with the full list of valid
ids when the guess is wrong, which makes it a better enumerator than `list_projects`.

## Resume

Decide which contract `list_projects` owes (members vs declared config), then either point
`src/tools/config/mod.rs:511-534` at `agent.workspace_summary()` or report both lists under
distinct keys. Whichever is chosen, update the tool description at
`src/tools/config/mod.rs:21` and the hint at `src/tools/config/mod.rs:160`, which currently
disagree with each other — the description says "workspace members", the hint says "the
configured ones", and only the hint is accurate today.

## References

- `docs/issues/2026-07-07-memory-tool-hides-project-memories-after-workspace-activate.md` —
  found in the same pass and adjacent: that one is two memory *directories* for a
  sub-project, this one is the sub-project being unlistable in the first place. Both bite
  only when a workspace has a non-root project.
- `docs/issues/archive/2026-08-08-memory-dir-for-project-materializes-any-id.md` — where
  `resolve_memory_dir`'s id validation (the probe that proved the id is known) came from.
