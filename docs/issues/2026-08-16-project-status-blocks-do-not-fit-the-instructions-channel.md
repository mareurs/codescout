---
id: fd54f2ae3cc7bdc8
kind: bug
status: open
title: 'BUG: Kotlin warnings, the workspace table and Custom Instructions cannot fit the MCP instructions channel, so they never reach the model'
tags:
- prompt-surfaces
- server_instructions
- mcp-channel
- progressive-disclosure
opened: 2026-08-16
owner: marius
related:
- docs/issues/archive/2026-08-15-server-instructions-truncated-before-reaching-the-model.md
severity: medium
---

# BUG: Kotlin warnings, the workspace table and Custom Instructions cannot fit the MCP instructions channel, so they never reach the model

## Summary

The MCP `initialize.instructions` channel is **2048 characters** (measured, BL-9). The
static `server_instructions` slice occupies ~1880 of them, leaving ~120 for the dynamic
`## Project Status` block. Three blocks that block can contain each exceed that budget on
their own:

- `KOTLIN_KNOWN_ISSUES` — injected for every Kotlin project
- the `## Workspace Projects` table — grows with project count
- `## Custom Instructions` — arbitrary user-authored text, unbounded by construction

Since BL-9 they are **trimmed with an explicit note** rather than cut silently mid-token,
which is a strict improvement — the agent now learns content was dropped. But it is still
dropped. The user-authored Custom Instructions case is the sharpest: a user writes project
rules, the server renders them, and the channel discards them.

This is not a regression introduced by BL-9. It is a pre-existing loss that BL-9's
measurement made visible: before, these blocks were cut by the *client*, mid-token, with no
signal at all.

## Symptom (Effect)

With BL-9's fitting in place, a Kotlin project's rendered instructions end:

```
## Project Status

- **Active project:** backend-kotlin at `/workspace/backend-kotlin`
- (status trimmed — it did not fit the MCP instructions channel)
```

`kotlin-lsp` known issues: absent. Same for the workspace table and any custom prompt.

## Reproduction

`src/prompts/mod.rs`, tests `build_with_kotlin_project_includes_kotlin_warnings`,
`build_with_workspace_appends_project_table`, and
`build_with_system_prompt_appends_custom_section`. All three were retargeted from
`build_server_instructions` to `build_project_status_block` on 2026-08-16 **precisely
because the rendered surface no longer carries them** — the renderer is correct, the
channel cannot deliver its output. Those three retargeted tests are the standing
reproduction.

## Environment

Linux, codescout `v0.15.0`, branch `experiments`, MCP stdio transport. Claude Code client.

## Root cause

Arithmetic, not a defect in any one function:

```
client limit                     2048 chars   (measured 2026-08-16)
− safety margin                    48
= usable                         2000
− static slice                  ~1880
= dynamic budget                 ~120 chars
```

A minimal status block (active project + languages + memories + index) is ~180 chars and is
itself already trimmed. `KOTLIN_KNOWN_ISSUES` alone is several hundred.

There is no ordering that fixes this, which is what separates it from BL-8 and BL-19 — those
were tail-cut problems where putting the important content first was the cheap fix. Here the
content does not fit at any position.

## Evidence

Measured by cutting the static slice as far as it reasonably goes. BL-9 removed the
`## Workspace gate` section (fully covered by `get_guide("workspace-state")`), de-aligned the
guide pointer list, and compressed Iron Laws 1 and 6 — 2081 → ~1880 chars, about 200
recovered. That was enough to restore five of the eight tests the fitting had broken. The
remaining three did not come back, and no plausible further trim of the static slice buys the
hundreds of characters they need.

## Fix

Not implemented — deliberately deferred. The decision taken on 2026-08-16 was to ship BL-9's
guarantee (static slice always intact, dynamic loss announced) and file this separately,
rather than expand BL-9 into a channel redesign.

**Proposed:** move `## Project Status` off the `instructions` channel entirely.

**Decision:** deliver project status through a channel with no 2 KB cap — the first tool
response, or an explicit `workspace` call — and let `instructions` carry only the static
slice.

**Context:** `instructions` is a fixed, small, session-start-only budget. Project status is
dynamic, unbounded, and cheap to deliver on demand. Putting unbounded content in a fixed
channel is the actual design error.

**Alternatives considered:**

- *Keep trimming.* Status quo after BL-9. Honest, but Custom Instructions — content the user
  wrote — is the thing most likely to be dropped, which is the worst possible priority.
- *Priority-order the trim* (drop memories and languages before Kotlin/custom). Cheap, and a
  strict improvement over tail order. Does not make anything fit; it only chooses better
  what to lose. Worth doing as an interim step.
- *Cut the static slice further.* Measured as insufficient — see § Evidence.

**Consequences:** now easier — nothing is silently lost, and the static slice stops competing
with unbounded content. Now harder — status arrives one call later than at session start, and
something must decide when to emit it.

**Change scenarios absorbed:** a user adds a long custom prompt; a workspace grows past a few
projects; a new per-language warning block is added.

**Confidence:** high on the diagnosis (arithmetic from a measured limit); medium on the fix
shape, since "first tool response" needs a concrete carrier and there may be a better one.

## Tests added

None — not fixed. The three retargeted tests named in § Reproduction already document the
gap: each asserts the renderer produces the block, and none asserts it survives the channel.
When this is fixed, the assertion to add is that a Kotlin project's *delivered* surface
contains `kotlin-lsp` again.

## Workarounds

- Kotlin known issues are also in codescout memory `gotchas` (LSP section) — read it directly.
- Custom instructions live in the project's own `system_prompt`; read it rather than relying
  on the MCP surface to carry it.
- `get_guide("workspace-state")` covers the workspace topology the table would have shown.

## Resume

Pick the carrier for project status. Then move `build_project_status_block`'s output to it,
leave `build_server_instructions` static-only, and drop `fit_dynamic_block` — it exists only
because the two share a channel.

Interim if that is not taken soon: make `fit_dynamic_block` drop by **priority** rather than
tail order, so Custom Instructions and the worktree banner outlive the memories list.

## References

- `docs/issues/archive/2026-08-15-server-instructions-truncated-before-reaching-the-model.md` — BL-9, where the 2048-char limit was measured
- `docs/architecture/mcp-channel-caps.md` § *Exact limit* — the measurement
- `src/prompts/mod.rs` — `build_project_status_block`, `fit_dynamic_block`

