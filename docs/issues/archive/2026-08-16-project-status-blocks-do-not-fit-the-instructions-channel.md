---
id: e3437bd1ec116dec
kind: bug
status: fixed
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
channel cannot deliver its output. Those three retargeted tests were the standing
reproduction.

**Superseded 2026-08-21.** All three are retargeted again, and one changed meaning rather
than address: `build_with_kotlin_project_includes_kotlin_warnings` became
`no_language_specific_warnings_are_pushed_at_any_project` — an inversion, since the block
it asserted is gone. That test's history is the tell this bug should have been read by
earlier: it had ALREADY been retargeted once, from `build_server_instructions` to the
renderer, specifically so it could keep passing while the segment it described reached
nobody. A test that survives a change by being pointed at a surface the user never
receives is a signal, not a fix.

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

## RESOLVED 2026-08-21 — the carrier was chosen

`113c10df` (patch-id `92ec3bea`, `experiments`) and `7c3245d7` (patch-id `f37302ff`,
`experiments`).

The `Substitutable` tier now rides `post_process`'s once-per-activation response banner —
the mechanism the path-relative note already uses, which has no character ceiling and
already handles the `--project` auto-activation case where no `activate` call ever happens.
Measured after the move: 220 characters free in the persistent channel on an ordinary
project, 143 with a worktree banner, and no trim at all. The three blocks this file is
named for now arrive — verified live, not only by test: a `tree` call on 2026-08-21
returned the workspace table and the Kotlin block in full.

**Two deliberate divergences from what § Resume prescribed.** Recorded because the
prescription was written before the constraint that changes it, and following it literally
would have introduced a defect.

1. **`build_server_instructions` is NOT static-only, and `fit_dynamic_block` /
   `StatusPriority` are NOT deleted.** The plan assumed the split was about SIZE. It is
   about PERSISTENCE: `server_instructions` rides the system prompt, re-sent on every
   request, so it survives compaction — and a tool response does not. `Anchor` therefore
   has to stay, because its absence causes a wrong WRITE and a response-carried banner
   arrives too late for a first tool call that IS the write. `UserAuthored` stays for a
   weaker but real version of the same argument. Both tiers can still overflow, so
   `fit_dynamic_block` is still the guarantee that the channel never does.

2. **The assertion § Resume asked for is inverted.** It said *"the assertion to add then is
   that a Kotlin project's delivered surface contains `kotlin-lsp` again."* Instead the
   block was deleted: `detect_fatal_stderr` (`src/lsp/client.rs`) already raises a
   `RecoverableError` naming the condition and the remedy when it actually happens, which
   the block's own last line conceded — *"codescout detects this and fails fast with a clear
   error."* Its trigger was wrong too: `languages` is what a repo CONTAINS, so codescout —
   a Rust project with Kotlin fixtures — served itself the block on every session, observed
   live on 2026-08-21. Pre-loading an explanation of a self-announcing error is cost
   without benefit at any trigger, so narrowing it to Kotlin-only projects would have kept
   the cost and bought nothing.

**The measurement § Resume warned would decay silently is now pinned.** That section noted
`source_md_under_cap` caps the slice at 1900 rather than its actual 1711, so an edit could
shrink the dynamic budget by ~190 characters without failing anything.
`the_tier_split_leaves_real_headroom_in_the_persistent_channel` now asserts >=120 chars
free on the worst ordinary case, which fails if the slice grows back into that space.

**Also lifted:** `MAX_MEMORY_NAMES = 8`, whose stated reason was *"unbounded is exactly
what a fixed channel cannot carry"* — falsified by this very move. It was withholding 14 of
this repo's 22 memory names.
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

N/A — fixed. See § RESOLVED under Fix for the carrier, the two deliberate divergences from
what this section previously prescribed, and the live verification.
## References

- `docs/issues/archive/2026-08-15-server-instructions-truncated-before-reaching-the-model.md` — BL-9, where the 2048-char limit was measured
- `docs/architecture/mcp-channel-caps.md` § *Exact limit* — the measurement
- `src/prompts/mod.rs` — `build_project_status_block`, `fit_dynamic_block`
