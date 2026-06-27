---
kind: spec
status: draft
title: Out-of-scope write ack handle — preserve content, unify the @ack_ permission pattern
owners: []
tags:
  - path-security
  - output-buffers
  - pending-ack
  - write-tools
  - friction
created: 2026-06-27
---

# Out-of-scope write ack handle

## 1. Goal & the problem

All four write tools — `create_file`, `edit_file`, `edit_code`, `edit_markdown` —
resolve their target through `resolve_write_path` → `validate_write_path`
(`src/util/path_security.rs:270`). When the path is outside the project root and
no session write root covers it, `validate_write_path` bails:

> `write denied: '<path>' is outside the project root. Call approve_write('<dir>') first to grant write access for this session.`

The content the agent generated is **discarded**. The agent must call
`approve_write(dir)` and then **re-issue the whole write, regenerating the
payload** — for a several-hundred-line plan, that is a large, pure-waste token
spend plus a three-call round trip (reject → `approve_write` → re-write).

**Goal:** when a write is rejected solely for being outside the project scope,
stash the full write payload server-side and return an `@ack_*` handle plus a
hint (exactly as `run_command` already does for dangerous commands). The agent
acknowledges the handle in one cheap call; codescout grants the directory for
the session and replays the stored payload. The content is never regenerated.

This **unifies the permission pattern**: out-of-scope writes become the
write-side twin of `run_command`'s dangerous-command gate.

### The precedent being unified with

`run_command`'s dangerous-command gate (`src/tools/run_command/inner.rs:272`):

- On rejection it returns `Ok({pending_ack, reason, hint})` — **not** an error —
  after stashing the payload via `output_buffer.store_dangerous(...)`.
- Re-invoking `run_command("@ack_…")` hits an early dispatch
  (`src/tools/run_command/mod.rs:184`, `looks_like_ack_handle`) that retrieves
  the stored payload via `get_dangerous` and replays it with the gate bypassed.
- `resolve_refs` (`src/tools/output_buffer.rs:412`) already forbids
  *interpolating* `@ack_*` handles — they are execute-only. This guard covers
  the new write handles for free.

## 2. Scope

**In scope:** the out-of-scope (outside-project-root) rejection path of all four
write tools, and the buffer + path-security plumbing they share.

**Behavioral decisions (from the design dialogue):**

- **Ack semantics = approve dir for session + write.** Acknowledging a handle
  replays the stored content **and** adds the target directory to
  `session_write_roots` (the same effect as `approve_write`). Subsequent writes
  to that directory then succeed with no further handle. This is the maximal
  friction removal and mirrors `run_command` (ack = bypass + run).
- **Handle surface = reuse the `path` param.** `create_file(path="@ack_1a2b")`,
  mirroring how `run_command` reuses `command`. The early replay check
  short-circuits before the tool's other required params (`content`,
  `old_string`, `symbol`, …) are demanded.
- **Response shape = `Ok({pending_ack, reason, hint})`**, not an error envelope.
- **`approve_write` stays** — still useful when the agent knows up front it will
  write outside the project; the ack path is purely additive.

**Explicitly out of scope:**

- The other four `validate_write_path` failure modes (empty path, null byte,
  unresolved `..`, deny-listed/protected location). These are **hard denials**
  and must never mint a handle. See §5.
- `run_command`'s existing `@ack_*` flow — untouched except for the buffer type
  generalization in §4, which keeps its observable behavior identical.

## 3. Architecture decisions

### ADR-1 — Approach 1: shared capture + shared replay helpers

**Decision:** put both the capture (rejection → handle) and replay
(handle → approve + payload) logic in shared helpers in the layer that already
owns write-path security, not duplicated per tool and not in the server router.

**Rejected — Approach 2 (per-tool early dispatch, literal `run_command`
mirror):** self-contained per tool but duplicates the dispatch/approve/replay
boilerplate four times; drifts over time.

**Rejected — Approach 3 (centralize replay in the server tool router):** cleanest
separation but spreads a path-security concern into `src/server.rs` routing and
is harder to unit-test than a pure helper.

Approach 1 unifies the pattern in one testable place, keeps each tool's diff to
~3 lines, and — via the "replay by re-running the tool body" trick (§4.3) —
duplicates no write logic.

### ADR-2 — Typed write-path decision instead of error-string matching

**Decision:** add `classify_write_path` returning a `WritePathDecision` enum so
callers can distinguish the *approvable* "outside root" case from *hard* denials
without matching on `bail!` strings.

```rust
pub enum WritePathDecision {
    Allowed(PathBuf),
    OutsideRoot { resolved: PathBuf },   // approvable → eligible for pending_ack
    Denied(String),                      // empty / null / ".." / protected → hard error
}
pub fn classify_write_path(
    raw: &str,
    project_root: &Path,
    config: &PathSecurityConfig,
    session_roots: &[PathBuf],
) -> WritePathDecision;
```

`validate_write_path` becomes a thin wrapper preserving today's external
contract: `Allowed → Ok`, `OutsideRoot | Denied → bail!` with the existing
messages. Nothing currently calling `validate_write_path` changes behavior.

**Rejected — string-match the bail message** (`contains("outside the project
root")`): fragile against message edits and conflates the protected-location
denial (which shares the `write denied:` prefix).

### ADR-3 — Generalize the buffer pending-ack store to an enum

**Decision:** widen the buffer's pending-ack store from command-only to a tagged
enum so writes ride the existing handle-minting + LRU eviction machinery.

```rust
pub enum PendingAck { Command(PendingAckCommand), Write(PendingAckWrite) }
pub struct PendingAckWrite {
    pub tool_name: String,   // guards against cross-tool replay
    pub input: Value,        // full original tool input, incl. content
    pub approve_dir: PathBuf,// directory granted on replay
}
```

- `pending_acks: HashMap<String, PendingAck>` (was `…, PendingAckCommand>`).
- `store_dangerous` wraps in `PendingAck::Command`; `get_dangerous` matches and
  returns the `Command` variant only (`None` for a write handle).
- New `store_pending_write(tool, input, approve_dir) -> "@ack_…"` and
  `get_pending_write(handle) -> Option<PendingAckWrite>`.
- LRU / `max_pending` (20) / `@ack_{:08x}` minting are shared, unchanged.

**Rejected — a second parallel map** (`pending_writes`): duplicates eviction and
ordering logic that must stay in lockstep with `pending_acks`.

## 4. Components & data flow

### 4.1 Capture (rejection → handle)

`resolve_write_or_capture(ctx, tool_name, input, raw_path) -> Result<WriteOutcome>`
(new, `src/tools/core/write_ack.rs`), where:

```rust
pub enum WriteOutcome {
    Write(PathBuf),   // proceed: write to this resolved path
    Pending(Value),   // return this pending_ack envelope verbatim
}
```

Runs `classify_write_path` with the pinned session-root snapshot
(`session_write_roots_snapshot_for(workspace_override)`):

- `Allowed(p)` → `WriteOutcome::Write(p)` (proceed to write).
- `OutsideRoot { resolved }`:
  1. Compute the approve directory (the resolved file's parent).
  2. **Pre-validate** it with `validate_approve_path(dir, root, security)`. If
     that fails (e.g. `/` or `$HOME` "too broad"), return the hard error — never
     mint a handle that cannot replay.
  3. `store_pending_write(tool_name, input, dir)` → `@ack_…`.
  4. `WriteOutcome::Pending(json!({ "pending_ack": handle, "reason": "...outside the project root",
     "hint": "<tool>(path=\"<handle>\") to write it and approve <dir> for this session" }))`.
- `Denied(msg)` → `RecoverableError`.

### 4.2 Replay (handle → approve + payload)

`maybe_replay_ack(ctx, input, tool_name) -> Result<Value>` (new). If
`input["path"]` is an `@ack_*` handle:

1. `get_pending_write(handle)` — if absent, `RecoverableError("ack handle expired
   or unknown" + "regenerate the write to get a fresh handle")`.
2. Guard `stored.tool_name == tool_name` — reject cross-tool replay
   (acking a `create_file` handle inside `edit_markdown`).
3. `validate_approve_path(stored.approve_dir)` then
   `add_session_write_root_for(workspace_override, stored.approve_dir)`.
4. Return `stored.input` (content intact).

If `input["path"]` is not a handle, return `input` unchanged.

### 4.3 Per-tool integration (~3 lines each)

At the top of each write tool's `call()`:

```rust
let input = maybe_replay_ack(ctx, input, "create_file").await?;   // Phase A
// …extract path / content / old_string / symbol as today, from `input`…
let resolved = match resolve_write_or_capture(ctx, "create_file", &input, path).await? {
    WriteOutcome::Write(p)     => p,                 // Phase B
    WriteOutcome::Pending(env) => return Ok(env),    // pending_ack envelope
};
```

On replay, Phase A approves the directory and returns the original input, so
Phase B's re-resolution now lands in `Allowed` and the tool's normal write path
runs — **no write logic is duplicated**. Phase B replaces each tool's existing
`resolve_write_path` call site.

### 4.4 Worked example

```
create_file(path=/out/plan.md, content=<300 lines>)
  → { pending_ack: "@ack_1a2b",
      reason: "'/out/plan.md' is outside the project root",
      hint: "create_file(path=\"@ack_1a2b\") to write it and approve /out for this session" }

create_file(path="@ack_1a2b")                 # content NOT re-sent
  → "ok"   (file written at /out/plan.md; /out now a session write root)

create_file(path=/out/notes.md, content=…)    # same dir now approved
  → "ok"
```

Replay returns the tool's **normal** success result unchanged (`create_file`
returns `"ok"`); the directory grant is conveyed up front by the capture
envelope's `hint`, so no extra annotation is added to the success result.

> **Ordering constraint:** `maybe_replay_ack` (Phase A) must run *before* any
> path-shape gate in the tool. `edit_markdown` rejects non-`.md` paths and
> `edit_file` redirects `.md` paths — both would choke on an `@ack_…` handle in
> `path`. Phase A runs first, swaps in the stored input (restoring the real
> path), then the gate sees the real path.

## 5. Error handling & security

- **Hard denials never mint a handle.** `Denied` (empty / null byte / unresolved
  `..` / deny-listed location) returns a `RecoverableError`. The deny-list check
  in `validate_write_path` runs **before** the outside-root check, so a path that
  is both external and protected is classified `Denied`, not `OutsideRoot`.
- **A minted handle is guaranteed replayable.** Capture pre-runs
  `validate_approve_path` (§4.1), catching the `/` and `$HOME` "too broad" cases
  that pass the deny-list but cannot be approved.
- **Cross-tool handle misuse rejected** via the `tool_name` guard (§4.2).
- **Expired / evicted handle** → `RecoverableError("…expired or unknown…")`; the
  agent regenerates — the same fallback `run_command` already uses. LRU cap is 20
  shared pending acks; large write payloads are stored as-is (the buffer already
  holds large `@file_*`/`@tool_*` content).
- **Net privilege is unchanged.** `approve_write` is already self-approving
  inside codescout (no user gate in-process), so folding approval into the ack
  grants nothing new — it removes a round trip and preserves content.
- **Documented caveat:** if a *client* permission config gates `approve_write`
  but auto-allows `create_file`, folding approval into the ack would skip that
  prompt. This is a no-op in the default setup (where `approve_write` is
  self-approving); noted here so a client author who gates `approve_write`
  knows to also gate the write tools.

## 6. Testing

**`path_security` unit:** `classify_write_path` returns `Allowed` for in-root,
`OutsideRoot` for an external non-protected path, `Denied` for a protected path /
empty / `..`. `validate_write_path` wrapper still bails with the unchanged
messages.

**`output_buffer` unit:** `store_pending_write` returns an `@ack_` handle;
`get_pending_write` round-trips `{tool_name, input, approve_dir}`; `get_dangerous`
returns `None` for a write handle and `get_pending_write` returns `None` for a
command handle; shared LRU eviction still holds.

**Per-tool integration** (`create_file` + at least `edit_markdown`):

1. External write → `Ok` with a `pending_ack` handle; the response carries no
   regenerated content and the original content is recoverable via the handle.
2. Replay with the handle → file written at the external path, directory present
   in `session_write_roots`, content byte-matches the original.
3. Second write to the same directory after replay → succeeds with no handle.
4. Expired / unknown handle → `RecoverableError`.
5. Cross-tool handle misuse → `RecoverableError`.
6. Protected path (e.g. under a denied root) → hard error, **no** handle minted.
7. Update `create_file_outside_project_rejected`
   (`src/tools/edit_file/tests.rs:1681`) to assert the new `pending_ack` shape.

**Prompt surfaces:** update the four write-tool descriptions to mention the ack
flow, and the progressive-disclosure guide's handle-kinds note
(`src/prompts/guides/progressive-disclosure.md:46`) so `@ack_*` reads as "dangerous
commands **and** out-of-scope writes." Gated by the existing
`prompt_surfaces_reference_only_real_tools` consistency tests.

## 7. Out of scope / future

- Auto-approving the *project's* sibling directories or a configurable
  allow-pattern — deliberately not pursued; the ack is per-rejection.
- Persisting session write roots across restarts — unchanged; ack grants are
  session-scoped like `approve_write`.
