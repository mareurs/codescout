---
id: db65023089245832
kind: tracker
status: draft
title: Prompt-surface measurement session log
tags:
- prompt-surface
- eval
- measurement
- librarian
topic: prompt surface budget measurement eval harness compaction
entry_high_water_F: 6
entry_high_water_W: 3
entry_prefix:
- F
- W
---

Work stream: measuring codescout's prompt/tool/guide surfaces against a captured
Claude Code request, building a token-aware eval harness, and planning compaction.
Opened 2026-08-23.

Entries are prose sections (`## F-N — title` / `## W-N — title`) — the only shape
`link_scan` binds a citation to. Index tables below are hand-maintained reading
surfaces, not the definition.

## Index — frictions

| id | title | status |
|---|---|---|
| F-1 | Fixed output path destroyed the evidence for the headline figure | fixed |
| F-2 | `json.dumps` defaults inflated every schema measurement by 3.8% | fixed |
| F-3 | A subagent's `workspace(activate)` mutated the parent's active project | open |
| F-4 | Every augmentation in the catalog is gone | open |
| F-5 | A stale recon finding was relayed as current | fixed |
| F-6 | 400k of context with no tracker or librarian use | fixed |

## Wins Index

| id | title | status |
|---|---|---|
| W-1 | Adversarial verify caught four fabricating checkers | validated |
| W-2 | Re-measuring beat withdrawing | validated |
| W-3 | Reading the surface README before compacting it | validated |

## F-1 — Fixed output path destroyed the evidence for the headline figure

**Valid:** invariant
**Status:** fixed

**Observed:** `capture.py` wrote every capture to one hard-coded filename. The
`--bare` run therefore overwrote the default-mode capture, and the copy taken
afterwards copied the *bare* file. Both artefacts ended byte-identical (md5
`77c6b52f…`), so the evidence for the 196k-char split had silently destroyed
itself.

**Why it hid:** identical files look exactly like a successful copy. Nothing
errored. The defect was only found because a verification agent md5'd them.

**Fix:** the output path is now required on argv and an existing file is refused
rather than clobbered. Re-captured and re-measured rather than patching the old
arithmetic.

**Rests on:** the general principle that a measurement script must not be able to
overwrite a prior measurement — a capture tool with a constant output path is a
one-shot instrument wearing a repeatable one's interface.

## F-2 — `json.dumps` defaults inflated every schema measurement by 3.8%

**Valid:** invariant
**Status:** fixed

**Observed:** schema sizes were computed with bare `json.dumps(schema)`. Two
defaults inflate the result versus what actually goes on the wire:
`ensure_ascii=True` escapes non-ASCII (+310 chars) and the default separators emit
`", "` / `": "` (+1,906 chars — six times larger, and the one nobody noticed).

**Impact:** every per-tool figure was ~3.8% high. `artifact` reported 13,203
against a wire-exact 12,727. The *shape* of the finding survived (tools dominate
the budget), but published numbers were wrong.

**Fix:** `measure.py` uses `ensure_ascii=False, separators=(",", ":")`. The repo's
own `tool_surface_report_lengths` was trustworthy all along and should be the
reference any ad-hoc script is checked against.

**Rests on:** `src/prompts/README.md` § *The tool-surface budget*, which already
names `advertised_surface()` as the thing that must match what `list_tools` builds.

## F-3 — A subagent's `workspace(activate)` mutated the parent's active project

**Valid:** conditional — until activation becomes caller-scoped
**Status:** open

**Observed:** a background workflow agent called
`workspace(action="activate", path=<sibling repo>)`. Active-project selection is
process-global in the codescout MCP server, so the parent session's project
changed underneath it and its next write — to a scratchpad path it had written to
five times in the same turn — was refused mid-turn.

**Full record:** `docs/issues/2026-08-23-subagent-activate-mutates-parent-active-project.md`
(severity high). Measured: exactly one activate across all agent transcripts, zero
`read_only` occurrences, identical `[security]` config in both projects.

**Dispatch defect on our side:** the briefing told agents to `activate`. Every
codescout tool already takes a per-call `workspace` parameter documented for
"concurrent subagents in different workspaces", which resolves per-caller without
touching shared state. Brief subagents with the parameter, never the activate.

## F-4 — Every augmentation in the catalog is gone

**Valid:** dated 2026-08-23
**Status:** open

**Observed:** `artifact(find, kind="tracker", augmented=true, scope="repo")`
returns zero rows. `f2ecdd76a6189efb` — the T-N ledger CLAUDE.md hard-codes and
prescribes `append_entry` / `update_entry` against — has `augmentation: null`.

**Mechanism:** augmentation lives only in the catalog SQLite DB and has no on-disk
form, so it is the one class of state `reindex` cannot rebuild. Any event that
recreates the DB destroys every augmentation at once while leaving all artifact
rows intact — the observed state exactly.

**Why it stayed invisible:** reindex *preserves* augmentation rather than
regenerating it, so a post-loss reindex reports healthy and repairs nothing. No
gate notices; `artifact(get)` returns `augmentation: null` without comment.

**Full record:** `docs/issues/2026-08-23-research-index-tracker-has-no-augmentation.md`
(escalated low → high, scope corrected from one tracker to repo-wide).

## F-5 — A stale recon finding was relayed as current

**Valid:** invariant
**Status:** fixed

**Observed:** a recon agent reported at ~19:00 that the eval harness could not run
— no `.venv`, no `~/.prompt-tdd`. That was relayed to the user as the current
blocker. Both had been created by the *build* agent at 19:45, 45 minutes later and
in the same workflow.

**Root cause:** a workflow's phases observe the world at different times, and a
recon finding about mutable environment state has a shelf life measured in
minutes. Findings about *code* survive the run; findings about *environment* may
not.

**Rule:** before relaying any environment claim from an earlier phase, re-check it.
A one-command `ls` would have prevented this.

## F-6 — 400k of context with no tracker or librarian use

**Valid:** invariant
**Status:** fixed

**Observed:** user observation, and correct. Across roughly 400k of context this
session produced two bug files but zero session-log entries, and never queried the
librarian until prompted — despite the work being exactly what these surfaces
exist to capture (measurements that decay, mechanisms worth citing later).

**Consequence:** F-1 through F-5 were all reconstructed at the end from
conversation memory rather than logged on notice, which is the failure mode
CLAUDE.md's capture-on-notice rule exists to prevent. Two of them (F-3, F-4) were
already bug-filed; the other three would have been lost at compaction.

**Second-order:** the librarian query that eventually ran (`augmented=true`) is
what exposed F-4. Querying the catalog is not only bookkeeping — it is a
diagnostic. The tracker discipline would have surfaced a repo-wide defect hours
earlier.

## W-1 — Adversarial verify caught four fabricating checkers

**Valid:** dated 2026-08-23
**Status:** validated

**Practice:** the compaction workflow's final phase spawned two independent
reviewers tasked with *refuting* the build, defaulting to unsound when uncertain.

**Counterfactual, concrete:** the new eval harness shipped with a correctness
predicate that demanded the wrong answer. The fixture unpacks twice — plain at the
root, and into `other/` with `sed s/with_tax/apply_levy/` — and the predicate
demanded `with_tax`, the local copy's name. It rewarded reading the wrong project
and failed every correct answer, guaranteeing a 0/10 floor behind a plausible
`wrong-answer` class. That would have been published as "the base prompt fails at
workspace routing" — a fabricated finding about codescout's own prompt surface.
Three sibling defects (dead `distinct` guard, truncated denominator, a null control
that passed denials) were found in the same pass.

**Promote-when:** a second workflow's verify phase changes a headline conclusion.
Then promote "adversarial verify defaults to unsound" into the workflow template.

## W-2 — Re-measuring beat withdrawing

**Valid:** invariant
**Status:** validated

**Practice:** when F-1 and F-2 invalidated the published numbers, the response was
to fix both instruments and re-capture from scratch, not to annotate the old
figures as withdrawn or patch the arithmetic.

**Counterfactual:** the alternative was a deliverable whose headline table read
"withdrawn — no surviving artefact", and a baseline that could never be trusted.
Re-capture cost one command and produced numbers that independently reproduced the
workflow's own figure for the librarian/artifact family (62.1% vs 62%), which is
also a cross-check the patched arithmetic could never have supplied.

**Rests on:** the project's Iron Rule that an unmeasured mechanism is a hypothesis
wearing a conclusion's clothes.

## W-3 — Reading the surface README before compacting it

**Valid:** invariant
**Status:** validated

**Practice:** `src/prompts/README.md` was read in full before planning any edit to
the surfaces it governs.

**Counterfactual:** the stated task was "compact the system prompt". The README
establishes that `server_instructions` is injected once per MCP session, is already
hard-capped at 1900 characters, and — decisively — that `fit_dynamic_block` refills
the channel to 2,000 chars, so every character freed is immediately consumed by the
Project Status block. Compaction there yields **exactly zero net tokens**. Without
that read, the effort would have gone into the one surface that cannot pay, and a
character-count metric would have scored it a success.

**Also recovered from it:** the ~12.5-turn break-even that makes schema→guide
relocation a loss, which is a sharper result than anything in the external source
that prompted the work.

## Follow-ups

- **Measure the librarian itself.** User request, 2026-08-23: the librarian is
  subjectively helpful and that should be quantified, likely as a sibling eval to
  the hidden-information one rather than a variant of it. Candidate axes — does an
  agent with catalog access find prior decisions faster than one grepping
  `docs/`; does provenance (`**Valid:**` / `**Rests on:**`) change whether an agent
  trusts a stale claim; does `librarian(context)` beat `semantic_search` for
  "what did we decide about X". Note the confound: the librarian family is 62.1%
  of codescout's tool-surface footprint, so any librarian eval is also the
  strongest test of whether that footprint earns itself.
- **Re-plan compaction against the guide corpus.** 97,690 chars over 10 topics with
  no cap, no counter, and no gate — the only surface with five-figure headroom. The
  existing 14-step plan was reviewed unsound.
- **Restore the augmentations** once F-4's fix direction is decided.

## Template for new entries

```
## F-N — <title>
**Valid:** invariant | dated YYYY-MM-DD | conditional — <event>
**Status:** open | fixed | mitigated | validated
**Observed:** …
**Rests on:** …
```

