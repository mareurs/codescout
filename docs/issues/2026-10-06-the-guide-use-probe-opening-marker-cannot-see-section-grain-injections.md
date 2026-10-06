---
id: '9cf119d200328dfb'
kind: bug
status: open
title: 'BUG: the guide-use probe''s opening marker matches only the whole-topic injection, so section-grain injections of `librarian` are never counted'
tags:
- probe
- guides
- instrument
- cluster/selector-narrower-than-its-population
opened: 2026-10-06
owner: marius
related:
- docs/issues/2026-09-24-residual-section-use-signatures-for-nine-topics.md
severity: medium
---

# BUG: the guide-use probe's opening marker matches only the whole-topic injection, so section-grain injections of `librarian` are never counted

## Summary

`scripts/probe_guide_section_use.py` finds a guide injection by searching transcripts for one literal string. Since section-grain serving, `guide_emit.rs` writes a different opening line for a section. The probe's string does not match it. A topic that is served by section reads as having few or no sessions, and the probe prints that as a population.

## Symptom (Effect)

The probe builds its marker in `opening_marker`:

```
def opening_marker(topic: str) -> str:
    """The opening form only. The closing form is preceded by 'end ' (trap 1)."""
    return f"auto-injected get_guide('{topic}') — first call"
```

The whole-topic emitter writes that form (`src/tools/core/guide_emit.rs:113`). The section emitter writes this one (`guide_emit.rs:283`):

```
<!-- auto-injected get_guide('{topic}') § {} — first call this session that serves this section. ...
```

A real section injection reached this session. The `doc(get)` result carried:

```
<!-- auto-injected get_guide('librarian') § Artifact Model — first call this session that serves this section. Do NOT re-call get_guide for it. -->
```

The text ` § Artifact Model` sits between `get_guide('librarian')` and `— first call`, so the probe's string is not a substring of it.

## Reproduction

```
git rev-parse --short HEAD    # cdb383e4, branch experiments
```

1. Read `opening_marker` (`scripts/probe_guide_section_use.py:322-324`) and the section format string (`src/tools/core/guide_emit.rs:283`). The two are not compatible.
2. Run the probe for `librarian` over the transcripts and compare its session count with the number of sessions that hold a `get_guide('librarian') §` line.

Step 2 was not run by the author of this record. The signatures fork reported that the probe finds 37 `librarian` sessions, all from before section-grain serving. That figure is reported, not re-measured here. Counting lines in one transcript does not work as a check, because a session that quotes the string is counted too.

## Environment

Linux, python3, transcripts under `~/.claude*/projects/`. Branch `experiments`.

## Root cause

Inferred from the two source lines above and the live injection text; the probe's session count was not re-measured.

- `opening_marker` hard-codes the whole-topic wording.
- `guide_emit.rs` gained a second wording when `librarian` began to declare sections. A topic that declares sections now ships `§ <heading>` blocks, not one whole block.
- The probe's docstring names three traps it honours. A format added after it was written is not among them.

The consequence is the failure class of IC-18: the selector is narrower than the population it names, the unseen members are not counted, and the answer is well-formed.

## Evidence

- `scripts/probe_guide_section_use.py:322-324`, quoted above.
- `src/tools/core/guide_emit.rs:113` (whole-topic form) and `:283` (section form), read 2026-10-06.
- The live injection text above, observed 2026-10-06 in this session.
- The signatures fork's report (2026-10-06): 37 `librarian` sessions, all before section-grain serving. Reported, not re-run.

## Hypotheses tried

1. **Hypothesis:** the probe also handles the section form somewhere else. **Test:** `grep § scripts/probe_guide_section_use.py` (2026-10-06). **Verdict:** rejected. The only two hits are prose, at line 14 (`rubric-BRIEF.md § 2`) and line 397 (`tracker-conventions § Entry headings`). No marker logic matches `§`.

## Fix

Not started. Options:

- Match both forms, for example with a pattern that allows an optional `§ <heading>` after the topic. Add a test with one fixture per form.
- Give each section injection its heading as a key, so a per-section signature can be scored against its own sessions.

A key by `(topic, heading)` is also needed by the open residual bug about signatures for nine topics (`3ff543cad7b4fb22`).

## Tests added

N/A — not fixed. A fix needs a fixture of each emitter form and an assertion that the session count is not zero for the section form.

## Workarounds

None for the probe. Read `librarian` coverage from the injection text itself.

## Resume

Decide whether to key by `(topic, heading)` now. Then change `opening_marker` and add the two fixtures.

## References

- `docs/issues/2026-09-24-residual-section-use-signatures-for-nine-topics.md` (`3ff543cad7b4fb22`), whose fork found this.
- Class: `IC-18`.
