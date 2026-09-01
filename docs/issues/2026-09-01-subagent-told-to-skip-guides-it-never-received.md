---
id: fd378fa834757146
kind: bug
status: fixed
title: 'BUG: Iron Law 6 tells the parent to make subagents skip get_guide, and the repeat-fetch note tells them they already have it'
tags:
- get_guide
- subagent
- iron-law-6
- prompts
closed: ''
unverified: Fix is applied and gated in the working tree but NOT committed — the shared checkout held another session's in-flight work, so no fix SHA or patch-id exists yet. Do not archive until both are recorded.
---

## Symptom

A dispatched subagent (superpowers `subagent-driven-development` implementer, the
Explore/exploration agents) arrives with a brief saying, in effect, *"don't fetch
`get_guide` for librarian / tracker-conventions — they're already loaded."* It then does
librarian/tracker work with none of the contract text in context. Reported by the user
2026-09-01: *"This is true for the main agent but not the subagent … a new context needs
the guides as well unfortunately."*

## Root cause — the parent was following instructions

Not parent improvisation. `src/prompts/guides/iron-laws-detail.md` told it to. Three
sites, all verified at the bytes this session:

1. **`iron-laws-detail.md`, last bullet of *What "brief" means concretely*** — said that
   naming triggered topics lets the subagent *"predict its own injection behavior
   accurately and **short-circuit redundant `get_guide` calls**."* For a fresh context
   nothing is redundant. The section's **first** bullet already said the correct opposite
   ("Name the relevant `get_guide(topic)` the subagent should call"), so the guide
   contradicted itself and the harmful bullet read as the refinement.
2. **`iron-laws-detail.md`, *No tool gate enforces this*** — said *"a subagent whose first
   tool call is `get_guide(topic)` for a topic obviously needed by its task indicates the
   parent underbriefed."* This labelled the **correct** subagent behaviour as the symptom
   of a defect.
3. **`src/tools/guide.rs`, the repeat-fetch note** — read *"You already fetched
   `get_guide("X")` earlier this session … no need to re-read it."* The ledger is
   session-keyed and shared parent↔subagent, so a **subagent's very first fetch always
   lands on this branch**, with an empty context. The note was factually false for it and
   invited it to discard the body it had just received.

Compounded by the substrate: `workspace-state.md` § *Subagent semantics* confirms
parent-triggered hints never auto-inject for a subagent. So brief, auto-inject and
runtime note all pointed the same wrong way.

Superpowers is **not** the source — its `SKILL.md` correctly states subagents "should
never inherit your session's context or history."

## The lever

`src/tools/guide.rs` **never withholds the body** — both branches return `*body`, and the
comment says so deliberately (so post-`/compact` recovery works). An explicit
`get_guide(topic)` therefore always returns the full guide regardless of ledger state.
The fix is only to invert the prescription and stop misdescribing the caller: no pasting
of ~14 KB guide bodies into dispatch prompts.

## Fix

- `src/prompts/guides/iron-laws-detail.md` — final bullet inverted: state the triggered
  topics **and** tell the subagent to fetch them itself, because the auto-inject will not
  fire for it. Added an explicit "never tell it the guides are already loaded, and never
  tell it to skip a fetch as redundant". New bullet records that an explicit fetch always
  returns the full body. *No tool gate enforces this* reworded so the underbrief symptom
  is re-deriving known facts (paths, prior results, symbol names), and a subagent
  fetching its own guide is named as prescribed behaviour, not a defect.
- `src/prompts/source.md` — Iron Law 6 static slice: "Pass guide topics already
  triggered" → "Name the guides they must fetch themselves". +2 chars; the 1900-char
  `STATIC_SLICE_CHAR_BUDGET` is unchanged and `source_md_under_cap` /
  `production_render_fits_the_client_channel` both pass. Snapshot fixture
  `tests/fixtures/prompt_surfaces/server_instructions.md` regenerated (1731 → 1733 bytes,
  that line only).
- `src/prompts/guides/project-activation-bootstrap.md` § *When you dispatch subagents* —
  same inversion; it carried the identical ambiguous "guide topics triggered" framing and
  is the guide most likely to be auto-injected at activation.
- `src/tools/guide.rs` — repeat-fetch note made context-neutral: it now says the topic
  was already delivered in this session, *possibly to a different agent*, that the body
  above is authoritative, and that a caller lacking it should read it. Preserves the
  context saving for a genuinely re-fetching parent.

## Regression tests

`repeat_fetch_keeps_body_and_flags_static` (`src/tools/guide.rs`) — kept its two original
invariants (full body on both fetches; note differs between first and repeat) and gained
two guarding this defect directly: the repeat note must **not** contain "You already
fetched", and **must** tell a caller lacking the guide to read the body.

Companion-side, in the `claude-plugins` repo: `codescout-companion/hooks/subagent-guidance.mjs`
— the only channel that reaches a subagent directly — now directs an explicit `get_guide`
call and explicitly overrides an "already loaded" brief. Covered by three new cases in
`tests/test-subagent-guidance.sh` (39 passed, 0 failed).

## Status

Fix applied and gated in the working tree; **not committed**. The codescout checkout held
another session's in-flight work at the time (staged `src/librarian/adapter.rs`, modified
`src/server.rs`, `src/tools/core/*`, `src/tools/memory/*`, `docs/trackers/sdd-ruling-log.md`),
so committing would have swept up unrelated changes. No fix SHA or `patch-id` exists yet —
record both here and archive this file once it lands on `experiments`.

