---
id: e3437bd1ec116dec
kind: bug
status: mitigated
title: 'BUG: Kotlin warnings, the workspace table and Custom Instructions cannot fit the MCP instructions channel, so they never reach the model'
tags:
- prompt-surfaces
- server_instructions
- mcp-channel
- progressive-disclosure
closed: 2026-08-21
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
− static slice                   1711         (measured 2026-08-17, `wc -m` on the fixture)
= dynamic budget                  289 chars
```

A minimal status block (active project + languages + memories + index) is ~180 chars.
`KOTLIN_KNOWN_ISSUES` alone is several hundred.

**The `~1880` above was wrong when written, and the conclusion drawn from it is wrong for
the common case.** `d2cf4449` sold a quickref row to buy Iron Law 1's overlap condition and
left the slice at **1711**, so the dynamic budget is **289**, not ~120 — and a ~180-char
minimal status fits inside it. Measured 2026-08-17.

The *unqualified* claim "there is no ordering that fixes this" is therefore false. It holds
only for the two genuinely oversized blocks: `KOTLIN_KNOWN_ISSUES` and an unbounded custom
prompt cannot fit at any position, and that is still what separates this from BL-8 and
BL-19. For everything else, ordering fixes it — measured directly in `30f3df81`'s red run:
the memories line (~137 chars) did not fit the ~225 of remaining room, while the
custom-instructions line (~70) would have. Ordering was not choosing the loss better; it
was recovering content that was being dropped.

## Evidence

Measured by cutting the static slice as far as it reasonably goes. BL-9 removed the
`## Workspace gate` section (fully covered by `get_guide("workspace-state")`), de-aligned the
guide pointer list, and compressed Iron Laws 1 and 6 — 2081 → ~1880 chars, about 200
recovered. That was enough to restore five of the eight tests the fitting had broken. The
remaining three did not come back, and no plausible further trim of the static slice buys the
hundreds of characters they need.

## Fix

**Interim fix landed `30f3df81` (experiments). The carrier decision is NOT taken — this
bug stays open on it.**

The alternative this file listed as *"Cheap, and a strict improvement over tail order. Does
not make anything fit; it only chooses better what to lose"* — priority-ordered trimming —
is shipped, and the parenthetical turned out to understate it. At the corrected 289-char
budget it does make things fit.

`fit_dynamic_block` now drops whole **segments** by priority instead of cutting lines from
the tail. The tiers come from this file's own § Workarounds rather than from taste — a
segment another surface reproduces is cheap to lose, one only this channel delivers is not:

| Tier | Segments | Why |
|---|---|---|
| `Substitutable` | languages, memories, index, workspace table, Kotlin issues | `memory(action="list")`, `get_guide("workspace-state")`, `index(action="status")`, memory `gotchas` |
| `UserAuthored` | custom instructions | the user wrote it; nothing else surfaces it |
| `Anchor` | header, active project, worktree banner | never dropped |

`Anchor` is not "most useful" but "cannot be re-derived, and its absence causes a wrong
**write**" — an agent that assumes the activated root is the canonical checkout commits to
the wrong branch. Within a tier the later segment still goes first, so the change only ever
reorders *across* tiers and reproduces the old behaviour inside one. A useful side effect:
the Kotlin block, the largest substitutable segment and unable to fit at any position, is
now dropped first rather than last.

The note names what went, capped at three plus a count. An agent told *which* segment was
dropped can take that segment's own route; "something was trimmed" only tells it to
distrust the whole block.

**BL-9's hard guarantee is preserved deliberately.** Anchors are never dropped, so anchors
alone could in principle overflow (a pathological project path, or a static slice grown to
its cap). `fit_dynamic_block` falls back to the pre-BL-37 line cut in exactly that branch,
because *the total never exceeds the channel* outranks segment integrity.

**Still proposed, still the maintainer's call:** move `## Project Status` off the
`instructions` channel entirely. Nothing above makes `KOTLIN_KNOWN_ISSUES` or an unbounded
custom prompt fit, and putting unbounded content in a fixed channel remains the design
error.
## Tests added

In `src/prompts/mod.rs`, red observed before the fix:

- `an_overflowing_status_keeps_the_user_s_own_text_over_a_substitutable_list` — the failing
  case. Its first assertion is a control: without it a fixture that happened to fit would
  pass while exercising none of the trimming path.
- `an_overflowing_status_keeps_the_worktree_banner` — the Anchor tier's load-bearing member.
  Passed *before* the fix too, because the banner sits early enough that tail-cutting spared
  it; it is here to stop a future reordering from losing it silently.
- `a_trim_names_what_it_dropped` and `the_trim_note_caps_the_names_it_lists` — both passed on
  first run, so `trim_note` was **mutated** back to the old generic note and both went red.
  Load-bearing, not decorative.

Unchanged and still passing: `production_render_fits_the_client_channel`, which already
carries the hostile fixture (30 memories, a 20×-repeated custom prompt, Kotlin) and is the
invariant the fallback branch exists to keep true; and
`a_status_block_that_fits_is_left_alone`.

The three retargeted renderer tests named in § Reproduction still assert on
`build_project_status_block`, which is now `#[cfg(test)]` — production renders from
segments. They remain this bug's standing reproduction for the part that is still open.
## Workarounds

- Kotlin known issues are also in codescout memory `gotchas` (LSP section) — read it directly.
- Custom instructions live in the project's own `system_prompt`; read it rather than relying
  on the MCP surface to carry it.
- `get_guide("workspace-state")` covers the workspace topology the table would have shown.

## Resume

**Pick the carrier for project status.** That is the whole remaining decision, and it is the
maintainer's: the interim fix is in, so nothing is silently lost and the common case now
fits, but `KOTLIN_KNOWN_ISSUES` and an unbounded custom prompt still cannot arrive at any
position.

**2026-08-21: closed as `mitigated`, carrier decision deferred deliberately.** Verified
`fit_dynamic_block` (`src/prompts/mod.rs:267-329`) is live and matches this file's
description exactly — priority tiers, per-drop label, the pre-BL-37 fallback branch. Asked
the maintainer whether to design a new carrier now; the answer was to leave the interim fix
in place rather than rush an MCP-surface architecture decision (which channel? a resource?
the first tool response? each has different client-support and freshness tradeoffs) at the
tail of an unrelated bug-sweep session. The remaining decision is exactly as described
above and nothing about it has changed — re-read this section, not just this correction,
when picking it up.

When a carrier is chosen: move `build_project_status_segments`' output to it, leave
`build_server_instructions` static-only, and delete `fit_dynamic_block` and the
`StatusPriority` tiers — they exist only because the two share a channel. The assertion to
add then is that a Kotlin project's *delivered* surface contains `kotlin-lsp` again.

One measurement worth repeating first, because it decayed once already and silently: the
289-char budget is `2048 − 48 − len(static slice)`, and the static slice moves whenever
`src/prompts/source.md` is edited. `source_md_under_cap` pins the slice at ≤1900, not at
1711, so a future edit can shrink the dynamic budget by ~190 chars without failing anything.
## References

- `docs/issues/archive/2026-08-15-server-instructions-truncated-before-reaching-the-model.md` — BL-9, where the 2048-char limit was measured
- `docs/architecture/mcp-channel-caps.md` § *Exact limit* — the measurement
- `src/prompts/mod.rs` — `build_project_status_block`, `fit_dynamic_block`
