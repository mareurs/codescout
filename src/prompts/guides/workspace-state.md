# Workspace state

How `workspace.activate` (alias `activate_project`) switches the codescout
MCP server's active project, and what state is shared across every tool
call — including across subagents that share the parent's MCP server.

## What `activate_project` does

A single call to `activate_project(path=...)` flips the server's active
project to the given root. The call has these side effects, in order:

1. **Resolves the path.** Bare project IDs (no `/`) inside a workspace
   are focus-switches — they skip step 2 entirely (the ledger is never
   touched), then still run steps 3–5 and return. Absolute paths trigger
   full activation: path must be an existing directory or you get a
   `RecoverableError` (`isError: false`, sibling calls survive) —
   raised *before* step 2, so a malformed or nonexistent path leaves
   the ledger untouched.
2. **Re-arms `guide_hints_emitted`** — the per-session set tracking
   which `get_guide(topic)` topics the model has been hinted about.
   Only reached by the full-activation path above. How much depends on
   the predicate in § Per-session state reset: same-project keeps it,
   a genuine switch re-arms one topic, no rendezvous clears it
   outright.
3. **Prewarms LSP** for the project's languages (background — does not
   block the response).
4. **Auto-registers dependencies** for cross-project navigation.
5. **Re-arms the path-relative banner** — resets the novelty gate so
   this `activate_project` call's own response carries
   `[codescout] paths are relative to <root>`; later responses in the
   same activation window omit it.

The response includes `project_hints` (primary language, manifest,
entry points, build commands) so the model has orientation context
even without running `onboarding`.

## The home/foreign distinction

The **first** project activated in an MCP session is the **home**
project. Every subsequent activation to a different path is a **foreign**
activation. The `read_only` param defaults differ:

- **home**: `read_only = false` (mutations allowed by default)
- **foreign**: `read_only = true` (read-only by default; pass
  `read_only=false` explicitly to enable writes)

**An explicit `read_only` wins at either root.** The two values above are
*defaults*; passing the flag overrides them in both directions —
`read_only=false` opens a foreign root, and `read_only=true` protects the
home one. Before 2026-09-02 the second half was not true: `true` was
silently dropped, which made it inert at every root, since a foreign root
already defaults to protected. On an older build, read the `read_only`
field echoed in the response rather than assuming the value took effect.

This matters because the MCP server is shared state across the session.
Activating a foreign project leaves the server pointed at it until you
explicitly restore home or end the session.

## Per-session state reset

Activation's effect on these per-session sets:

| State | Behavior |
|---|---|
| `guide_hints_emitted` | With a companion rendezvous active, a **same-project re-activation keeps the ledger** — nothing is cleared, so nothing re-emits. A **genuine project switch re-arms only the project-scoped topic** (`project-activation-bootstrap`), leaving the tool-contract guides the model already holds in place. **Without a rendezvous, every activation still clears the whole ledger** — that blunt behaviour is retained deliberately: absent the companion hook a `/clear` is invisible to the server, and being surgical would silently starve the new conversation of guides it has never seen. It is also always cleared on `workspace(post_compact=true)` (compaction re-arm); **persisted per session**, so most topics survive `/mcp` restarts within one conversation instead of re-injecting guide bodies the conversation already holds — with one deliberate exception: server construction re-arms the session-opening topic alone on any non-empty reloaded ledger, so that one guide body is re-sent on every `/mcp` reconnect regardless of project. Written by **both** an explicit `get_guide(topic)` fetch and the first-touch auto-inject of a hint-carrying tool — one shared keyspace, so either path suppresses the other's re-emit. After a full clear or a scoped re-arm, the next touch of an affected topic re-emits. |
| path-relative banner | Cleared on every activation. The activation's own response re-emits it; later responses in the same window omit it. |
| section-read tracking | NOT cleared. Persists across activations. |
| Output buffers (`@tool_*`, `@cmd_*`) | NOT cleared. Buffers from before the switch remain readable. |


**Identity behind `guide_hints_emitted`.** The ledger the table row above describes is keyed by **conversation identity**, not by project — it follows the conversation across activations and workspace switches within one MCP session, not whichever project happens to be active.

- **Keyed tier** — when a conversation id is obtainable (Claude Code's `CLAUDE_CODE_SESSION_ID`, today), the ledger persists to disk under that id and survives `/mcp` reconnects within the same conversation, other than the session-opening-topic exception noted above.
- **Anonymous tier** — when no identity is obtainable (every other client, or Claude Code before the companion hook has run), the ledger lives in-process only, is never persisted, and re-arms automatically after an idle interval — so a second conversation in a long-lived process isn't starved of guides forever.

A companion hook (Claude Code only) can refresh the keyed tier's id mid-process through a pid-keyed rendezvous slot the server publishes at construction. That is how the server detects `/clear`, which mints a new conversation id without restarting the MCP subprocess. The server stays fully correct without the hook: absent it, the ledger degrades to the anonymous tier's idle-TTL behavior rather than silently suppressing guides the new conversation has never seen. This is the same flag the table above checks — a keyed conversation id on its own is not enough for the surgical re-arm; the hook must actually have stamped the slot at least once.

## Path-relative annotation

After `workspace(action="activate")`, path fields in responses resolve against
the **new** root, and that first response carries a `[codescout] paths are
relative to <root>` banner naming it. Root-valued fields (`cwd`, `git_root`,
`project_root`, `repo_root`, …) stay absolute — they are the anchor the rest
resolve against.

That is the whole workspace-facing consequence. The canonical statement — the
`PATH_KEYS` / `ROOT_KEYS` allowlists, what is never rewritten (file content,
shell output, prose, error text), and how to read a path against catalog state —
is `get_guide("progressive-disclosure")` § Path-relative annotation, which
auto-injects the first time a call overflows into a buffer and so is usually
already in context.
## Cross-project workflow pattern

When you need to work in a sibling project briefly:

```
1. workspace(activate, path="/home/user/other-project")
2. <do the work — any number of tool calls>
3. workspace(activate, path="/home/user/code-explorer")   # restore home
```

Skip step 3 and the server stays pointed at the foreign project. The
next session inherits the foreign root as "active." This is the
**workspace gate** from `server_instructions` — restore home before
the turn ends.

For one-off reads, say so with `read_only=true`:

```
workspace(activate, path="/sibling", read_only=true)
```

On a foreign root that is already the default, so the flag is redundant
there — but it is not a no-op in general: on the home root it is the only
way to ask for the guard. Read-only mode blocks writes at the agent layer
regardless of the caller's intent — defense against accidental edits while
scouting.

## Subagent semantics

Subagents that share the parent's MCP server share:

- The same active project (no per-subagent override)
- The same `guide_hints_emitted` set (parent-triggered hints don't
  re-fire for subagents)
- The same `path_note_emitted_since_activation` flag

A subagent that needs the workspace pointed at a different root must
itself call `activate_project` — and then restore the parent's home
before returning. This is dangerous: the parent's next call after the
subagent returns will see whatever workspace state the subagent left.
Prefer not to switch workspace inside subagents; if you must, document
in the subagent's spawn prompt that it will restore home before exit.

## Per-call workspace pinning

`activate_project` flips the *one* shared active-project slot, so parallel
subagents that activate different workspaces race — last writer wins, and a
subagent can silently read another's worktree. The fix is to **pin per call**
instead of activating: pass `workspace=<absolute path>` on each tool call.

- The pin resolves that single call against the named workspace, regardless of
  the server's active project. Concurrent pins never collide — each call
  carries its own target.
- Single-workspace work omits the param; the server's active project is used.
- Pinning is the recommended path for parallel multi-workspace fan-out. Prefer
  it over calling `activate` inside a subagent (see Subagent semantics above),
  which leaves the shared slot pointed wherever the subagent last set it.

**When a peer has activated read-only, pin — do not re-activate.** A refused write
naming a project you never chose *is* this collision: another caller sharing the
process activated it with `read_only=true`, and activation is process-wide. The
remedy the refusal used to offer first — re-activating with `read_only=false` —
clears your block by flipping the substrate under whoever set it, mid-task. Passing
`workspace=<absolute path>` on the call resolves it for you alone and leaves their
activation intact. Measured 2026-09-01: the pin worked first try during an SDD run
where re-activation would have disrupted a running implementer
(`docs/issues/archive/2026-09-01-workspace-activation-is-process-wide-and-a-subagent-can-flip-it.md`).
## Anti-patterns

- **Forgetting to restore home.** Iron-Law-grade. Server is shared
  state; the next session sees your foreign activation as the active
  project. Symptoms: tools operate on the wrong codebase, semantic
  search returns unrelated results.
- **Switching workspaces inside a subagent without restoration.**
  Parent's next tool call lands in the subagent's workspace. Caller
  has no way to detect this without an extra `workspace(status)` call.
- **Relying on `guide_hints_emitted` to survive activation, restart, or compaction.** (On the keyed tier most topics *do* survive `/mcp` restarts — persisted per conversation identity, not per project — except the session-opening topic, which server construction re-arms on every reconnect; see "Per-session state reset" above.) Whether
  `activate_project` clears it, re-arms one topic, or leaves it untouched
  depends on the predicate in § Per-session state reset — don't assume it
  survives. If a hint was useful, capture the guide content in the
  parent's prompt or call `get_guide(topic)` again after activation.
- **Treating `read_only=true` as advisory.** It is enforced at the agent
  layer on any root, home included — tools that try to write fail with
  `RecoverableError`. Pass it deliberately for scout-only work, including on
  your own project, which is the case that silently did nothing before
  2026-09-02. What it does *not* do is protect you from a peer sharing the
  process: activation is process-wide, so pin per call (`workspace=<path>`)
  when the concern is someone else moving the default under you.

## Related

- `get_guide("error-handling")` — `RecoverableError` routing for
  invalid paths and read-only violations
- `get_guide("progressive-disclosure")` — `[codescout] paths are
  relative to <root>` mechanics, path stripping, buffer behavior
- Iron Law 6 in `server_instructions` — subagent dispatch discipline
  (parent must brief subagents about workspace state, among other
  context)
