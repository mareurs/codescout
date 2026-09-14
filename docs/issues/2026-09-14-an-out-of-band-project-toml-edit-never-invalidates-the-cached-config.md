---
status: open
opened: 2026-09-14
closed:
severity: medium
owner: marius
related: []
tags:
- cluster/gate-keyed-on-unobservable-event
kind: bug
---

# BUG: an out-of-band `project.toml` edit never invalidates the cached config, and the write refusal's own remedy is that edit

## Summary

A project's config is loaded once when the project becomes resident and cached for the life of
the process. Only codescout's *own* write tools trigger a reload, so a `.codescout/project.toml`
edited in an editor, by `Bash`, or by any other process is never picked up. This matters most
for `security.file_write_enabled`, because the refusal message that fires when it is `false`
prescribes exactly the action that does not work: **"change the config."**

Both directions are broken and they fail differently. Turning writes **on** leaves the user
following a printed remedy that changes nothing and re-prints itself. Turning writes **off**
succeeds silently — the operator believes the project is locked down and writes keep landing
with no error at all.

## Symptom (Effect)

`false` on disk, cached `false` — refused, correctly:

```
File writes are disabled for <root> by security.file_write_enabled = false in its
.codescout/project.toml. Re-activating with read_only: false will NOT clear this — change the config.
```

Flip the file to `file_write_enabled = true`, re-run the identical call — **byte-identical
refusal**. The remedy the message names has been performed and the message has not changed.

The reverse, which is worse because nothing is printed. A project resident with
`file_write_enabled = true`, flipped to `false` on disk, then written to three times:

```
"ok"
"ok"     <- after the on-disk flag went false
"ok"     <- minutes later; not an eventual-consistency delay
```

## Reproduction

`HEAD = 9045c56a88899d46c16387f817ca440d110e0f72` (branch `experiments`), live MCP after
`cargo rb` + `/mcp`.

1. Create a throwaway project with a valid `[project]` section and `[security]
   file_write_enabled = false`. (A config *missing* `[project]` fails closed with
   `write gate: missing field 'project'` — correct, but a different path.)
2. `memory(action="write", topic=..., content=..., workspace="<throwaway>")` → refused,
   `ConfiguredOff`, naming the root.
3. Edit the file on disk: `file_write_enabled = true`. Re-run step 2 → **identical refusal**.
4. Fresh throwaway born with `= true`, same call → `"ok"`. Edit to `= false`, same call →
   still `"ok"`, and still `"ok"` minutes later.

Step 4 is the control that makes steps 2–3 mean anything: it proves the refusal is driven by the
config value rather than by the path being a throwaway outside any write root.

**Use a write tool outside `approve_write | create_file | edit_file | edit_code | library`.**
Those five are the ones `edit_file` reloads for, so a repro driven by them can accidentally
refresh the very cache under test.

## Environment

Linux, `experiments`, live MCP (stdio), release binary built 2026-09-14 11:49:52 from a tree
containing `a13b31c6`. Probe roots under the session scratchpad, each its own git repo.

## Root cause

Two mechanisms compose, and each is defensible alone.

1. `Agent::ensure_resident` (`src/agent/mod.rs:668`) is documented as "load + cache on miss".
   On a **hit** it can upgrade `read_only`, and does not re-read `project.toml`. So the config
   is whatever it was when the root first became resident.

2. `Agent::reload_config_if_project_toml_for` (`src/agent/mod.rs:901`) exists precisely to
   refresh it, but is keyed on `path == p.root/.codescout/project.toml` where `path` is **the
   current tool call's target**. Its only callers are codescout's own write tools —
   `src/tools/onboarding.rs:119,165,1071`, `src/tools/edit_file/mod.rs:289,675`,
   `src/tools/markdown/edit_markdown.rs:1558`.

So the real event is "the config file changed", which the server cannot observe; the proxy is
"one of my own write tools just targeted that path". The proxy is exactly right for the case it
was written for and blind to every edit that does not flow through those three tools — which
includes the normal one, a human in an editor.

No file watcher backs it up: `notify::` / `Watcher` / `watch(` appear nowhere in `src/config/`
or `src/agent/mod.rs`.

*measured 2026-09-14 via the four-step probe above (both directions, plus a delayed re-run);
mechanism read at the cited lines.*

## Evidence

Reload is keyed on the call's own target path, not on the file's state
(`src/agent/mod.rs:901-919`):

```rust
let toml_path = p.root.join(".codescout").join("project.toml");
if path == toml_path {
    if let Ok(fresh) = crate::config::project::ProjectConfig::load_or_default(&p.root) {
        p.config = fresh;
    }
}
```

## Hypotheses tried

1. **Hypothesis:** the refusal in step 3 is caused by the path being a throwaway outside any
   configured write root, not by the config value.
   **Test:** second throwaway born with `file_write_enabled = true`, identical call.
   **Verdict:** rejected — returned `"ok"`.
2. **Hypothesis:** the config reloads eventually (watcher with a debounce).
   **Test:** re-ran the write minutes after flipping the flag; grepped for a watcher.
   **Verdict:** rejected — still `"ok"`, and no watcher exists in the config or agent modules.

## Fix

Not implemented. Two candidate directions, not adjudicated:

- **Observe the file instead of proxying it.** Stat `project.toml` and compare mtime/size (or
  hash) before serving a security decision from cached config. Cost is a stat per gated call.
- **Fix the remedy rather than the cache.** If the staleness is accepted, the refusal must stop
  prescribing an edit that cannot work in-process and name an action that does — the
  measured-safe one being a re-activation or a server restart, whichever actually re-reads.

The second is strictly cheaper and addresses the half that misleads a human. It does **not**
address the silent direction, where nothing is printed to fix.

## Tests added

None yet — the bug is filed, not fixed. A regression test must drive the **out-of-band** edit
(write the file with `std::fs`, not through `edit_file`), or it re-triggers the reload it is
meant to prove absent and passes vacuously.

## Workarounds

Pass `workspace=` per call to a project whose cached config already says what you want, or
restart the MCP server (`/mcp`) after editing `project.toml`. Editing the config and retrying
is the one thing that does not work, and is what the error message tells you to do.

## Resume

Decide between the two Fix directions. If the stat-per-call route is taken, the seam is
`Agent::ensure_resident` (`src/agent/mod.rs:668`) on its cache-hit branch — that is where the
config is known-stale and no re-read happens. Confirm first whether any other cached config
field carries a security decision, since the same staleness would reach those too.

## References

- `src/agent/mod.rs:668` — `ensure_resident`, "load + cache on miss"
- `src/agent/mod.rs:901` — `reload_config_if_project_toml_for`, the proxy
- `src/util/path_security.rs:692` — the `ConfiguredOff` message whose remedy is the broken one
- `docs/issues/archive/2026-09-14-read-only-blocks-five-tool-names-not-the-writes-it-promises.md`
  — the sibling defect in the same gate; its fix is what made this one reachable by `memory`
