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
closed: 2026-09-01
unverified: 'Both halves are committed and gated: codescout 019b1c5b on experiments, claude-plugins ac1b1fa on branch fix/subagent-guide-fetch-directive. That branch is NOT merged to main -- the only outstanding item. Do not archive until it merges.'
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

Fixed on `experiments` at **`019b1c5b`** (`fix(prompts): stop telling subagents to skip
guides they never received`), patch-id **`f75c181d83ae70b86918e250189883e621fcd6d5`** —
record the patch-id rather than relying on the SHA, which orphans on rebase.

The four-command gate is green at that commit, with one exception that is not this change:
`peer::server::tests::run_exits_after_idle_timeout_with_no_connections`, the known
load-sensitive flake filed as `ee9d8d80ad5ecdc8`. It passes in isolation in 1.13s against a
10s deadline, and `src/peer/server.rs` has not changed since 2026-08-18. Two reproductions
from this session were handed to that file's owner; note that the accompanying
"concurrent cargo held the target lock" hypothesis was **retracted** — one of the two runs
showed no lock wait at all, so lock contention is not a necessary condition and the pair is
two plain reproductions, not a discriminating condition.

**Not yet archived, and the reason is queryable in `unverified:` above.** The companion half
of the fix — `codescout-companion/hooks/subagent-guidance.mjs`, which is the only channel
that reaches a subagent directly, plus its three new cases in `tests/test-subagent-guidance.sh`
(39 passed, 0 failed) — is committed in the `claude-plugins` repo at **`ac1b1fa`**, patch-id
**`f7cbe7f484dce20a708f97e84f772550ebdbff79`**, on branch `fix/subagent-guide-fetch-directive`.
That branch is **not merged to `main`**, which is the only thing still outstanding. Archive
this file once it merges.

(The companion half is nonetheless already *live*: `claude-plugins` hooks resolve via
`CLAUDE_PLUGIN_ROOT` to the repo working tree, so the edit took effect on save rather than on
merge. The codescout half is live too, by a different and less comfortable route — a release
build by another session at 22:52 on 2026-09-01 compiled the then-uncommitted working tree into
the shared binary, so servers started after that timestamp ran this fix before it was
committed, while six older servers kept the pre-fix image.)

The residual that no wording change reaches — the ledger cannot key on an identity the MCP
protocol never carries — is filed as `OB-11` on `docs/trackers/observer-blindness.md`, which
cites this file. **That citation is a scheduled break:** archiving this bug re-keys it
(`id = sha256(abs_path)`), so re-point `OB-11`'s `**Rests on:**` and `**Instances:**` lines in
the same commit as the move.
