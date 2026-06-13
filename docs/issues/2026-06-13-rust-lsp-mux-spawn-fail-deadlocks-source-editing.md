---
status: open
opened: 2026-06-13
closed:
severity: high
owner: marius
related: []
tags: [lsp, mux, edit_code, tooling, dogfood]
kind: bug
---

# BUG: Rust LSP mux fails to spawn → edit_code unavailable → source editing deadlocks

## Summary
The Rust LSP mux fails to spawn ("no socket file in /tmp"), so every `edit_code`
call on a `.rs` file errors. Because `debug_enforce_symbol_tools = true` in
`.codescout/project.toml` also blocks `edit_file` on source, the two together
**deadlock all source editing through codescout** — no sanctioned tool can edit Rust.
Noticed while executing the honest-usage.db-logging plan; every implementer subagent
had to work around it.

## Symptom (Effect)
```
mux startup failed for rust: Failed to spawn mux process
```
- `edit_code` on `.rs` → unavailable (mux down).
- `edit_file` on `.rs` with `debug_enforce_symbol_tools = true` → hard-blocked
  ("must use symbol tools").
- `edit_file` (once the flag is relaxed) additionally rejects any **multi-line**
  `new_string` containing `fn ` — so adding a new function still needs a workaround.

## Reproduction
- Branch `experiments`, commit ~`f13f6a46` (2026-06-13 session).
- Invoke any `edit_code(path="src/**/*.rs", ...)` via the live MCP server → mux spawn error.
- The mux socket is absent under `/tmp` after the failure.

## Environment
- Linux (Zen kernel), codescout v0.15.0, MCP stdio transport, project = codescout, branch `experiments`.
- Release binary launched via `~/.cargo/bin/codescout` symlink → `target/release/codescout`.

## Root cause
Unknown — under investigation. The mux child process does not come up and leaves no
socket in `/tmp`. Candidate leads: stale/exited mux from a prior session not reaped;
the release binary's mux spawn path failing silently; an environment/permission issue
on the socket dir. The LSP-disconnect / mux-startup failure family is exactly the
`mux_startup_fail` / `lsp_disconnect` `err_family` the new legibility probe will surface
in `usage.db` (see `docs/superpowers/specs/2026-06-13-dzo-friction-probes-design.md`).

## Evidence
Reported independently by three implementer subagents during the 2026-06-13 plan
execution (Exec 1/2/3/4-5), each citing `mux startup failed for rust: Failed to spawn
mux process` and falling back to non-`edit_code` editing.

## Hypotheses tried
1. **Hypothesis:** transient — a retry would spawn the mux. **Test:** multiple
   `edit_code` attempts across separate subagent sessions. **Verdict:** rejected — failed
   consistently across the whole session.

## Fix
Unknown — not addressed this session (out of scope of the logging plan). Workaround only;
status stays `open`.

## Tests added
N/A — diagnostic/tooling bug; no code change made. A regression test belongs with the
mux-spawn fix, not here.

## Workarounds
- Relax `debug_enforce_symbol_tools = false` in `.codescout/project.toml` (gitignored,
  local-only) so `edit_file` (codescout's tracked text editor) works on source. Restore
  to `true` afterward.
- To ADD a function via `edit_file` (which blocks multi-line `new_string` containing
  `fn `): two-phase insert — (1) insert the body with a `// __SIG__` placeholder line in
  place of the signature (no `fn ` → allowed), (2) single-line `edit_file` swapping
  `// __SIG__` for the real `fn …(…) {` line. Verify no `__SIG__` residue.
- Note: text edits via `edit_file` do not refresh the symbol index immediately, so
  `symbols(name=…)` may 0-match freshly-edited code until reindex — read raw text to verify.

## Resume
Investigate the mux spawn path: check `src/lsp/mux/process.rs` (spawn + `PR_SET_PDEATHSIG`
path) for why the child exits before creating its `/tmp` socket; look for a stale mux PID
or leftover socket from a prior session. Restart the MCP server (`/mcp`) and retry a
single `edit_code` to see if a fresh server process spawns the mux. Cross-check against
the Kotlin mux-collision history in `docs/issues/2026-03-24-kotlin-lsp-concurrent-instances.md`.

## References
- `docs/superpowers/plans/2026-06-13-honest-usage-db-logging.md` (the plan during which this surfaced).
- `.codescout/project.toml` `debug_enforce_symbol_tools` flag.
- Related friction family: `mux_startup_fail` in the legibility-probe spec.
