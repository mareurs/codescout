---
kind: bug
status: fixed
title: Peer protocol forwards a caller `workspace` arg verbatim — read-scope escape past the addressed peer
owners: []
tags:
  - peer-delegation
  - security
last_observed: 2026-06-01
closed: 2026-06-01
---

## Symptom

A `peer(action="query", peer=<B>, tool=<read-tool>, args=...)` call is supposed to
delegate exploration of the peer's *own* workspace B. But the peer-serve endpoint
forwards the caller-supplied `args` map **verbatim** to dispatch, and codescout's
read tools (`symbols`, `grep`, `read_file`, `read_markdown`, `tree`,
`semantic_search`, …) accept a `workspace` field that pins project resolution to an
arbitrary absolute path. So a requester can smuggle:

```
peer(action="query", peer="codescout-main", tool="read_file",
     args={"path":"src/secret.rs", "workspace":"/home/u/some-other-project"})
```

and read a project *other* than the one the peer advertises. The peer addressing
model (registry `id → target workspace`) is silently bypassed.

## Scope / bound (why it is med, not high)

- **Reads only.** The deny-by-default `PEER_EXPOSED_TOOLS` allow-list rejects every
  write tool / `run_command` / `workspace` / librarian mutation before dispatch, so
  this cannot write or execute. It is a *confidentiality* scope escape, not RCE/write.
- **Loadable project roots only.** `with_project_at(Some(pin))` → `ensure_resident`
  loads the pinned root as a project; path-security still confines reads to that
  project root + its configured allowed paths. So it reads "any other loadable
  project on the host," not arbitrary files like `/etc/passwd`.

## Reproduction

1. Configure a peer entry pointing at workspace B.
2. From the requester, call `peer(query, peer=B, tool="tree", args={"workspace":"/path/to/UNRELATED/project"})`.
3. Observe the tree of the unrelated project returned (resolution pinned to the
   smuggled `workspace`, not B).

(Not yet exercised live — traced statically; see Evidence.)

## Root cause

`handle_tool_call_inner` (`src/peer/server.rs`) passes `params.args` straight to
`CodeScoutServer::call_tool_by_name(&tool, args)`. `call_tool_by_name`
(`src/server.rs:543`) rebuilds `CallToolRequestParams` from `{name, arguments: args}`
verbatim and routes through `call_tool_inner` — which honors the per-tool `workspace`
pin (`Agent::with_project_at(Some(workspace_override), …)`). Nothing in the peer layer
strips or rejects the `workspace` key, so the per-request pinning feature (intended
for in-process parallel subagents) is reachable by an external peer requester.

## Evidence

- `src/peer/server.rs::handle_tool_call_inner` — allow-list gate, then
  `ctx.server.call_tool_by_name(&tool, args)` with the caller's `args` unmodified.
- `src/server.rs::call_tool_by_name` (lines ~543-557) — `serde_json::from_value(json!({"name":name,"arguments":args}))`; no key filtering.
- Tool schemas for `symbols`/`grep`/`read_markdown` carry a `workspace` param:
  "Absolute path of the workspace this call targets, pinning project resolution …".

## Hypotheses tried

(none yet — static trace only)

## Fix

**APPLIED — experiments-side `6aaa21d4`** (re-cite the master SHA after the eventual cherry-pick). The peer layer now strips the caller-supplied `workspace` key from `args` in `handle_tool_call_inner`, before `call_tool_by_name` → `call_tool_inner` can extract it into `ctx.workspace_override`:

```rust
// in handle_tool_call_inner, after the allow-list gate, before dispatch:
if let Some(obj) = args.as_object_mut() {
    obj.remove("workspace");
}
```

The strip lives in the peer layer — not in `call_tool_by_name`, which has legitimate in-process pinning callers (parallel subagents). A peer is addressed by its registry id, which maps to exactly one served workspace, so a per-request `workspace` override is never peer-controllable.
## Tests added

`peer::server::tests::peer_tool_call_ignores_smuggled_workspace_override` — serves workspace A and sends a `tree` `tool.call` carrying `args.workspace=<B>`; asserts the result lists A's `served_alpha_dir` and NOT B's `foreign_beta_dir`. The escape was reproduced before the fix (the call returned B's directory listing, `src/peer/server.rs:817` assertion), green after.
## Workarounds

Only register peers you trust to behave; the requester is already a local codescout
instance. The escape matters once peers are less-trusted (cross-user / cross-machine,
explicitly deferred in the spec).

## Resume

Fixed 2026-06-01 (experiments-side `6aaa21d4`). The static trace was reproduced as a live exploit — the regression test failed showing the foreign workspace's listing — before the one-line strip closed it. Stays in `docs/issues/` until the fix ships to `master`; re-cite the master SHA in the Fix section after the cherry-pick, then `git mv` to `docs/issues/archive/`.
