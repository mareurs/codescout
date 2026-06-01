---
kind: bug
status: open
title: Peer protocol forwards a caller `workspace` arg verbatim — read-scope escape past the addressed peer
owners: []
tags:
  - peer-delegation
  - security
last_observed: 2026-06-01
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

## Fix (proposed, not yet applied)

Defensive strip in the peer layer — force every peer-served call to resolve the
served (default) workspace only:

```rust
// in handle_tool_call_inner, before call_tool_by_name:
let mut args = args;
if let Some(obj) = args.as_object_mut() {
    obj.remove("workspace"); // peer calls are scoped to the served workspace
}
```

~3 lines. Belongs in the peer layer (not call_tool_by_name, which has legitimate
in-process pinning callers). Pair with a test: a peer `tool.call` carrying
`args.workspace=/other` still resolves the served root.

## Tests added

(none yet)

## Workarounds

Only register peers you trust to behave; the requester is already a local codescout
instance. The escape matters once peers are less-trusted (cross-user / cross-machine,
explicitly deferred in the spec).

## Resume

Noticed 2026-06-01 during the security trace of Phase 1.5 Task 2 (RO-convergence).
Separable from the auto-spawn work. If the user wants it folded into Phase 1.5, the
fix is the 3-line strip above + one test, naturally co-located with Task 4 (peer-tool
wiring) or as a small dedicated task.
