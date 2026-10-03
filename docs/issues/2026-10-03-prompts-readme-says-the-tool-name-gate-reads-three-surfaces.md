---
id: '85ea79df030f3512'
kind: bug
status: open
title: 'BUG: src/prompts/README.md says the tool-name gate reads three prompt surfaces; it reads four, and the docs disagree on which surface is "the fourth"'
tags:
- cluster/doc-contradicted-by-code
closed: null
opened: 2026-10-03
owner: marius
related: []
severity: low
---

# BUG: src/prompts/README.md says the tool-name gate reads three prompt surfaces; it reads four, and the docs disagree on which surface is "the fourth"

## Summary

`src/prompts/README.md` calls itself the canonical home for which prompt surfaces exist, and says
`server::tests::prompt_surfaces_reference_only_real_tools` catches stale tool-name mentions "across
the three surfaces". Per that test's own doc comment, it has also read `.codescout/system-prompt.md`
since 2026-09-11, and the README never names that file. Separately, the README and
`scripts/probe_tool_surface.py` call `tools/list` "the fourth prompt surface", while the test's doc
comment and `CLAUDE.md` § *Prompt Surface Consistency* give that ordinal to
`.codescout/system-prompt.md`. Nothing executes wrongly because of it; a reader sent to the README to
learn which surfaces exist gets a set that omits the one injected into every session in this repo.

## Symptom (Effect)

Measured 2026-10-03 at HEAD `a4fef08d`:

```
$ git grep -n -e 'the three surfaces' -e 'fourth prompt surface' -e 'fourth surface' HEAD -- src/prompts/README.md scripts/probe_tool_surface.py src/server.rs
HEAD:scripts/probe_tool_surface.py:4:`tools/list` is the fourth prompt surface and the only one with a PER-REQUEST cost, so it
HEAD:src/prompts/README.md:5:[...] The build-time test `server::tests::prompt_surfaces_reference_only_real_tools` catches stale tool-name mentions across the three surfaces; `prompts::tests::claude_md_contains_no_deprecated_tool_names` guards `CLAUDE.md`. [...]
HEAD:src/prompts/README.md:54:`tools/list` is the fourth prompt surface and the only one with a **per-request** cost. [...]
HEAD:src/server.rs:5438:    /// **The fourth surface is read from DISK, and that is why it was ungated until
HEAD:src/server.rs:5509:                "cannot read the fourth prompt surface at {fourth_path}: {e}\n\
```

`[...]` marks where a long line was cut; the quoted text between cuts is verbatim.
`git grep -c 'system-prompt.md' HEAD -- src/prompts/README.md` prints nothing: zero mentions.

## Reproduction

Run the two `git grep` commands above at any HEAD where `src/prompts/README.md` is unchanged.

## Environment

Docs only. Branch `experiments`, HEAD `a4fef08d`.

## Root cause

Two enumerations of "prompt surfaces" grew apart, and each numbers its own members. The README's
§ *Surfaces* lists the two `source.md` slices, `build_system_prompt_draft()` and `tools/list`, so
`tools/list` is its fourth. `CLAUDE.md` and the test count the two slices, the builder output and
`.codescout/system-prompt.md`, so that file is theirs. When the file became the test's fourth input,
the test's doc comment (`src/server.rs:5438`) recorded it and the README did not, so the README still
states the coverage the test had when the README was written. Inferred from the texts at HEAD
`a4fef08d` and the test's doc comment — not bisected.

## Evidence

The Symptom block. And the same drift one surface over: `CLAUDE.md` § *Docs* still called
`.codescout/system-prompt.md` ungated ("the one no other index in this repo names") until `0d282a94`
corrected it on 2026-10-03 — the sentence the test's doc comment says the test replaced.

## Hypotheses tried

N/A — the defect is in the texts, read directly.

## Fix

Not done. Proposed: make the README's § *Surfaces* the single enumeration. Add
`.codescout/system-prompt.md` with the two gates that read it
(`prompt_surfaces_reference_only_real_tools`, `reader_docs_contain_no_retired_call_forms`), say the
tool-name gate reads four surfaces and, per its doc comment, not `tools/list`, and replace "the
fourth" with the surface's name in the README, `scripts/probe_tool_surface.py:4` and the test's
messages. An ordinal is a position in a list each document draws for itself, which is how two of
them can each be right about "the fourth" and still disagree.

## Tests added

N/A — open. Pinning README prose would be the sentence-pinning `CLAUDE.md` § *Testing Discipline*
advises against; removing the ordinals is the durable fix.

## Workarounds

For which files the tool-name gate reads, trust the doc comment on
`prompt_surfaces_reference_only_real_tools` in `src/server.rs`, not the README.

## Resume

Edit `src/prompts/README.md` line 5 and § *Surfaces* as in Fix; replace the ordinal at
`scripts/probe_tool_surface.py:4`. Then
`git grep -n 'fourth' -- src/prompts/README.md scripts/probe_tool_surface.py src/server.rs CLAUDE.md`
should return no ordinal standing in for a surface's name.

## References

- `CLAUDE.md` § *Prompt Surface Consistency*, and `0d282a94` (stage 2d, the `CLAUDE.md` side)
- `prompt-surface-compaction-session-log:F-14`
- `docs/trackers/issue-clusters/IC-11-doc-contradicted-by-code.md` — this README already has one
  IC-11 member, `prompts-readme-cites-a-guide-guard-test-that-was-deleted` (2026-10-01)
