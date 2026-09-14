---
kind: bug
status: open
tags:
- cluster/gate-keyed-on-unobservable-event
closed: null
opened: 2026-09-14
owner: marius
related: []
severity: medium
unverified: 'REMEDY-TEXT FIX COMPLETE 2026-09-14 across all four refusal sites: b21ad3b4 (file_write_enabled, patch-id cad267e7e761b85c94ae66c764e64b3ff044cfe8) plus 10a3c10d (indexing_enabled, shell_command_mode, max_index_bytes, patch-id 22af528ed42dd6c7fef1d65aea29eb4cf449c17e). The one-site reading in the earlier stamp was WRONG: project_security_config copies the whole SecuritySection, so the staleness reaches 14 production call sites including the shell gate -- see section Scope. Direction 1, observing the file so the cache cannot go stale, is NOT implemented, so the SILENT direction is untouched: a project resident with file_write_enabled = true (or shell_command_mode = warn) flipped on disk still permits, with no error. Record stays open for that half and is deliberately not archived.'
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

## Scope — the staleness was never scoped to `file_write_enabled`

*measured 2026-09-14; every hop below read at the bytes, this session.*

`ProjectConfig.security` is copied **wholesale** out of the resident project on every gated
call. `project_security_config` (`src/agent/mod.rs:436`) calls
`SecuritySection::to_path_security_config` (`src/config/project.rs`), which clones seven
fields — `profile`, `extra_write_roots`, `shell_command_mode`, `file_write_enabled`,
`indexing_enabled`, `shell_dangerous_patterns`, `max_index_bytes` — into the
`PathSecurityConfig` every gate then reads. So `file_write_enabled` was never the affected
field. It was the one that got noticed.

`call_graph(project_security_config, direction="callers", max_depth=2)` returns 28 edges
across 11 files, **14 of them production call sites**: both path gates (`src/fs/mod.rs:58`,
`src/fs/mod.rs:84`), the tool-access gate (`src/server.rs:662`), the shell tool
(`src/tools/run_command/mod.rs:169`), and ten others. The tool-access gate is reached from
the universal dispatcher at `src/server.rs:1290`, so it gates **every** tool call by name —
the refusals below are reachable, not decoration.

**The shell gate is the sharpest case, and it is worse than the write one.** A resident
project whose `project.toml` is edited on disk to `shell_command_mode = "disabled"` keeps
executing shell commands for the life of the process: `RunCommand::call` →
`security_config_for` → cached `shell_command_mode` → `run_command_inner` Step 3
(`src/tools/run_command/inner.rs:351`). That is the silent, fails-permissive direction again,
on a stronger gate than file writes.

Two fields outside `[security]` share the mechanism without being security decisions, named
here so nobody re-derives them as ones: `project.tool_timeout_secs` (`src/server.rs:1336`)
and `security.write_lock_timeout_secs` (`src/server.rs:770`).
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

**Direction 2 IMPLEMENTED 2026-09-14 (`b21ad3b4`) AT ONE OF FOUR SITES, AND COMPLETED AT
`10a3c10d`. Direction 1 is NOT implemented, and this record stays open for it.** The `ConfiguredOff` refusal at `src/util/path_security.rs` no longer ends in
*"change the config"* — it now states that the config is cached for the life of the process,
that editing the file and retrying will not work either, and names the restart that does.

What that does **not** buy, stated plainly so nobody reads the commit as a close: the cache is
unchanged, so the **silent direction is exactly as broken as before**. A project resident with
`file_write_enabled = true`, flipped to `false` on disk, still accepts writes and prints
nothing. No message exists on that path to correct, which is why the cheap repair cannot reach
it — and it is the direction an operator would care about more, since it fails toward
permitting writes rather than refusing them. § *Resume*'s question is **answered** — see § *Scope*, and the
answer widened this record rather than closing it.

**The one-site reading was wrong, and § *Scope* is why.** Four refusals prescribe a
`.codescout/project.toml` edit as their remedy, and the same cache defeats all four.
`b21ad3b4` repaired one. `10a3c10d` repaired the other three — `indexing_enabled`
(`src/util/path_security.rs`), `shell_command_mode` (`src/tools/run_command/inner.rs`) and
`max_index_bytes` (`src/tools/semantic/index.rs`, both messages) — through one documented
constant, `config::project::CONFIG_IS_CACHED_REMEDY`, rather than three copies of the
sentence. This is `CLAUDE.md` § *Testing Discipline*'s **mutate once per guarded SITE, not
once per feature** arriving as a defect rather than as a test: a per-site law was repaired
per-feature, and this record carried the overclaim in the interval between the two commits.

**The constant deliberately does not cover the fourth site, and the asymmetry is
load-bearing.** It offers editing the config through codescout's own `edit_file`, which does
trigger `reload_config_if_project_toml_for` and genuinely works — but that escape is
unreachable exactly when `file_write_enabled = false`, because writes are the thing disabled.
Unifying all four would hand the write refusal a remedy its reader cannot perform:
confidently wrong rather than merely unhelpful.

## Tests added

**For the loud half, added 2026-09-14** — two assertions on the existing
`configured_off_refusal_rejects_the_reactivation_remedy`
(`src/util/path_security.rs`): one that the message does **not** contain `— change the config.`,
one that it **does** name `/mcp`. Both were mutation-verified in an isolated worktree via
`scripts/mutation-probe.sh`, because an assertion's existence is not coverage — reverting the
message to its old wording kills the first (panic at `:2609`), and removing `/mcp` while leaving
the rest intact kills the second independently (panic at `:2617`). Gate green on both lanes,
9778 passed / 0 failed; the test is present in the lean lane as well as the default one, so it
is not vacuous under `--no-default-features`.

**For the site asymmetry, added 2026-09-14 (`10a3c10d`).**
`the_write_refusal_withholds_the_edit_file_escape_the_indexing_refusal_offers`
(`src/util/path_security.rs`) reads BOTH refusals in one test and asserts they differ: the
indexing message names the `edit_file` escape, the write message must not. Nothing else in
the suite can express this — every other test here reads one message alone, so the difference
is invisible to all of them, and the regression it guards is a *tidy-up* rather than a bug.
Mutation-verified in an isolated worktree **once per guarded assertion**, not once for the
test: rewording the write message to say `edit_file` kills it at `:2722`, and replacing the
shared constant in the indexing message with text that keeps `/mcp` but drops the escape
kills it independently at `:2717`. Both reported `KILLED (rc=101, 1 test(s) ran)` — the count
is part of the verdict, since a mutation that never compiled would wear the same word with
0 tests run.

**For the silent half — still none, and this is the part that matters.** A regression test there
must drive the **out-of-band** edit (write `project.toml` with `std::fs`, not through
`edit_file`), or it re-triggers the very reload it is meant to prove absent and passes
vacuously. That test cannot be written against today's code, because there is no seam that
re-reads; it belongs with whatever Direction 1 ships.
## Workarounds

Pass `workspace=` per call to a project whose cached config already says what you want, or
restart the MCP server (`/mcp`) after editing `project.toml`. Editing the config and retrying
is the one thing that does not work, and is what the error message tells you to do.

## Resume

**The precondition this section used to name is ANSWERED — see § *Scope*.** Yes, and not
marginally: the whole `SecuritySection` is served stale across 14 production call sites,
including the shell gate. Verified at the bytes 2026-09-14.

**That settles the direction question rather than leaving it open, and settles it toward
Direction 1.** Direction 2 can only ever repair *messages*, and the two gates that matter
most — writes and shell — both fail **silently in the permissive direction**, where no message
exists to repair. Every remedy-text fix is now shipped (`b21ad3b4`, `10a3c10d`), so Direction
2 is exhausted: what remains is entirely Direction 1.

The seam is `Agent::ensure_resident` (`src/agent/mod.rs:668`) on its cache-hit branch —
verified this session: the fast path and the re-check under the write lock **both** return
`Ok(())` after at most upgrading `read_only`, and neither touches `p.config`. A
stat-per-gated-call would sit there.

One caveat for whoever takes it, cheap to state and expensive to rediscover: the refusal
messages now shipped assume the `edit_file` reload path stays reachable. If Direction 1
removes or reroutes `reload_config_if_project_toml_for`, then
`CONFIG_IS_CACHED_REMEDY` and its guard test must move with it.
## References

- `src/agent/mod.rs:668` — `ensure_resident`, "load + cache on miss"
- `src/agent/mod.rs:901` — `reload_config_if_project_toml_for`, the proxy
- `src/util/path_security.rs:692` — the `ConfiguredOff` message whose remedy is the broken one
- `docs/issues/archive/2026-09-14-read-only-blocks-five-tool-names-not-the-writes-it-promises.md`
  — the sibling defect in the same gate; its fix is what made this one reachable by `memory`

## Fix provenance

**Partial — the remedy-text direction only, now complete across all four sites. This bug is
NOT closed by it; the silent direction is untouched.**

- **SHA:** `b21ad3b4` (experiments) — site 1 of 4, `file_write_enabled`. Positional; does not survive a rebase of `experiments`.
- **patch-id:** `cad267e7e761b85c94ae66c764e64b3ff044cfe8`
- **SHA:** `10a3c10d` (experiments) — sites 2-4, the shared constant, and the asymmetry guard.
- **patch-id:** `22af528ed42dd6c7fef1d65aea29eb4cf449c17e`

If either SHA stops resolving, recover the commit by its patch-id.
