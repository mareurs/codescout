---
kind: bug
status: open
tags:
- librarian
- mcp-timeout
- codescout-tool
- server
- cluster/selector-narrower-than-its-population
closed: null
opened: 2026-09-03
owner: marius
related:
- docs/issues/2026-09-03-a-long-reindex-cannot-be-distinguished-from-a-wedged-one.md
severity: medium
---

# BUG: `tool_skips_server_timeout` exempts `index` for being "an embedding loop that runs for many minutes" and omits `librarian`, which is one

## Summary

`librarian(action="reindex", reembed=true)` is an embedding loop that runs for
many minutes, and it is subject to the 60-second server-level tool timeout. The
exempt list names `index`, `index_library` and `run_command` and stops there
(`src/server.rs:1272`), while the doc comment two lines above states the very
criterion `librarian` satisfies. On any corpus whose full re-embed exceeds 60
seconds, **a full re-embed cannot be completed through the MCP surface at
default configuration.** The work continues server-side; only the caller's view
of it is destroyed, so the failure presents as an unexplained timeout rather than
as "this tool needs longer than the timeout".

## Symptom (Effect)

2026-09-03, attempting the re-embed that `6f032dbd` required:

```
Tool 'librarian' timed out after 60s. Increase tool_timeout_secs in .codescout/project.toml if needed.
```

The message is accurate and actionable, which is the problem: it reads as a
tuning nudge for an unusually slow call, not as *this tool is in the same class
as `index` and was left out of the exemption*. The run that eventually completed
took **12m10s** — 12× the default budget — and only after I raised
`tool_timeout_secs` to 3600 in `.codescout/project.toml`, ran it, and restored
the value to 60.

## Reproduction

```
git rev-parse HEAD   # 596a8d7ae3f67c901ee6d6c6c977ec4cc723cd46, branch experiments

# with default .codescout/project.toml (tool_timeout_secs unset -> 60)
librarian(action="reindex", reembed=true)
```

On this repo's corpus (1,457 artifacts / 28,612 chunks) it times out at 60s
every time. The catalog keeps filling afterwards, because the timeout drops the
caller's future arm and the work is already in flight — so a second call sees
partial progress and no error, which is a separate confusion.

## Environment

- Linux 7.1.9-zen1-2-zen, `experiments` @ `596a8d7a`
- MCP stdio transport, Claude Code client
- `tool_timeout_secs` unset → `default_timeout() == 60` (`src/config/project.rs:349-351`)

## Root cause

A hand-written allowlist whose membership test is a `matches!` arm list, and
whose stated criterion is broader than the list.

```rust
// src/server.rs:1264-1273
/// Returns true for tools that manage their own timeout internally and must not
/// be wrapped by the server-level `tool_timeout_secs` guard.
///
/// - `index` / `index_library`: embedding loops that run for many minutes.
/// - `run_command`: the caller supplies `timeout_secs` in the request params; ...
fn tool_skips_server_timeout(name: &str) -> bool {
    matches!(name, "index" | "index_library" | "run_command")
}
```

Dispatch consumes it as the whole decision:

```rust
// src/server.rs:1116-1124
let timeout_secs = if tool_skips_server_timeout(&req.name) {
    None
} else {
    self.agent
        .with_project_at(ctx.workspace_override.as_deref(), |p| {
            Ok(p.config.project.tool_timeout_secs)
        })
        .await
        .ok()
```

`None` means the timeout arm is not installed at all
(`src/server.rs:735-762`) — so exemption is total, and non-exemption is the 60 s
default. `librarian` takes the `else` branch.

The criterion in the doc comment — *"embedding loops that run for many minutes"* —
is satisfied by `librarian(reindex, reembed=true)`, whose loop
(`src/librarian/tools/reindex.rs:368-390`) awaits `svc.embed_artifact(...)` once
per queued chunk. Measured 2026-09-03: 27,762 vectors at ~38/sec ≈ 12m10s. It is
the same shape as `index`, one subsystem over.

*Read at the bytes this session:* `src/server.rs:1116-1124`, `:1264-1273`,
`:735-762`, `src/config/project.rs:349-351`,
`src/librarian/tools/reindex.rs:368-390`. *Measured 2026-09-03:* the 60 s timeout
observed live, and the 12m10s / 27,762-vector completion after raising
`tool_timeout_secs`.

## Evidence

### The complement test enumerates four tools and cannot see this

```rust
// src/server.rs:5335-5344
#[test]
fn other_tools_do_not_skip_server_timeout() {
    for name in &["read_file", "edit_file", "symbols", "semantic_search"] {
        assert!(!tool_skips_server_timeout(name), ...);
    }
}
```

Three tests cover this function (`:5318`, `:5329`, `:5335`). All three assert
about a **hand-listed** set of names. None asserts a property that a new
long-running tool would have to satisfy, so the suite is green on every future
omission of exactly this kind — including this one. The guard's population is
authored, and nothing derives it.

### The exemption is total, not extended

`timeout_secs: Option<u64>` — `None` skips the `tokio::time::timeout` arm
entirely (`src/server.rs:735`, `:763`). So the fix is not "give librarian a
longer number"; it is a binary membership decision, which is what makes the
design fork below load-bearing.

## Hypotheses tried

1. **Hypothesis:** `librarian` routes through a different dispatch path that does
   not consult `tool_skips_server_timeout`.
   **Test:** read the single call site, `src/server.rs:1116`.
   **Verdict:** **rejected** — there is one call site, and it branches on
   `req.name` for every tool.

2. **Hypothesis:** the default is higher than 60 and the observed timeout came
   from local config.
   **Test:** `symbols(name="default_timeout")` → `fn default_timeout() -> u64 { 60 }`
   (`src/config/project.rs:349-351`).
   **Verdict:** **rejected** — 60 is the shipped default.

## Fix

*Plan only — not implemented. The fork below is a real design decision, not a
detail, and this bug does not settle it.*

**Option A — exempt `librarian` wholesale.** One word in the `matches!` arm.
Cheapest, and wrong in one direction: `librarian` is a multiplexer over ten
actions, of which `reindex` is the only long one. `doctor`, `link_scan`,
`context`, `audit_log` are ordinary calls that would silently lose their
runaway-protection, so a hung `link_scan` on a large corpus would park forever
instead of returning an error.

**Option B — exempt per-action.** `tool_skips_server_timeout` takes only
`&str` name today; the request params are available at the call site
(`req.name` is read there, so `req.arguments` is too). Exempting
`librarian` **only when `action == "reindex"`** matches the real population.
Costs a signature change and makes the function's contract "name + params"
rather than "name", which is a wider blast radius than it looks — the three
existing tests all call it with a bare name.

**Option C — derive the list instead of authoring it.** The durable fix for the
class: let a tool *declare* that it manages its own timeout (a trait method
defaulting to `false`), so the answer travels with the tool rather than with a
list in `server.rs` that no one edits when adding one. This is the only option
under which the next long-running tool is exempt by construction.

Recommendation: **B now, C when a third long-running tool appears.** A is a trap
— it trades a caller-visible timeout for an invisible hang on nine other actions.

SHA: *(not fixed)*
patch-id: *(not fixed)*

## Tests added

None yet. When fixed, note that a test of the form
`assert!(tool_skips_server_timeout("librarian"))` is the same shape as the three
that already exist and would not have caught this bug — it asserts about the name
the author was already thinking about. The test with discriminating power asserts
the **criterion**: every tool that can run past the default budget is exempt, with
the population derived rather than typed.

## Workarounds

Raise the budget for the duration of the call and put it back:

```toml
# .codescout/project.toml
[project]
tool_timeout_secs = 3600
```

then `/mcp` to reconnect, run the reindex, restore `60`, reconnect again. Or run
the equivalent from the CLI, outside the MCP timeout entirely.

## Resume

`src/server.rs:1116` (the call site) and `:1264-1273` (the list). Read
`src/server.rs:5316-5344` first — the three existing tests define what "covered"
currently means here, and the fix should change that definition, not extend the
list they check.

## References

- `docs/issues/2026-09-03-a-long-reindex-cannot-be-distinguished-from-a-wedged-one.md`
  — the sibling. That one is about a long reindex being **unobservable**; this one
  is about it being **unsurvivable**. They compound: the timeout kills the
  caller's view, and no status surface exists to recover it.

### Cluster adjudication

Tagged `cluster/selector-narrower-than-its-population` (`IC-18`). The selector is
`tool_skips_server_timeout`; the population it names is stated in its own doc
comment — *"tools that manage their own timeout internally"*, *"embedding loops
that run for many minutes"* — and the `matches!` arm enumerates a strict subset of
it. `IC-18`'s own mechanism note reads *"nothing reaches an author-written
selector"*, and a `matches!` arm list is exactly that shape.

Near miss considered and rejected: `IC-14`
(`cluster/guard-narrower-than-its-name`). This function is not a guard — it
refuses nothing and protects nothing; it selects a set. The remedy differs
accordingly: `IC-14`'s is to widen or rename the guard, `IC-18`'s is to re-derive
the selector from the property, which is Option C above.
