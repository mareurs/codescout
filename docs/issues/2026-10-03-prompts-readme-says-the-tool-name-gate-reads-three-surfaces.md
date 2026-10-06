---
id: '85ea79df030f3512'
kind: bug
status: fixed
title: 'BUG: src/prompts/README.md says the tool-name gate reads three prompt surfaces; it reads four, and the docs disagree on which surface is "the fourth"'
tags:
- cluster/doc-contradicted-by-code
closed: 2026-10-06
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

Done in `527fafa1` (2 files, 4 insertions, 3 deletions), on `experiments`:

- `src/prompts/README.md` line 5 now says `prompt_surfaces_reference_only_real_tools` catches stale tool-name mentions "across the four surfaces it reads" (the `server_instructions` and `onboarding_prompt` slices, the `build_system_prompt_draft()` output and `.codescout/system-prompt.md`) and says it does **not** read `tools/list`.
- § Surfaces gains a bullet for `.codescout/system-prompt.md` naming the two gates that read it (`prompt_surfaces_reference_only_real_tools`, `reader_docs_contain_no_retired_call_forms`).
- The "fourth" ordinal for `tools/list` is dropped from the README (two places) and from the `scripts/probe_tool_surface.py` docstring.

Not done: `src/server.rs` still says "The fourth surface is read from DISK" (line 5438), "the fourth surface's ..." (5480, 5486) and a panic message "cannot read the fourth prompt surface" (5509), because another agent owned that file during the sweep. Those ordinals now agree with `CLAUDE.md` § *Prompt Surface Consistency* (where `.codescout/system-prompt.md` is the fourth), so no document calls two different surfaces "the fourth" any more; they are still ordinals rather than the surface's name, which the bug's proposal wanted replaced.

## Tests added

none: documentation fix. Pinning README prose would be the sentence-pinning `CLAUDE.md` § *Testing Discipline* advises against, as this bug said when opened; removing the ordinals is the durable fix.

## Workarounds

For which files the tool-name gate reads, trust the doc comment on
`prompt_surfaces_reference_only_real_tools` in `src/server.rs`, not the README.

## Resume

Closed on 2026-10-06 by `527fafa1`. Residual follow-ups (listed, not filed):

- `src/server.rs:5438`, `5480`, `5486` and the panic message at `5509` still use "fourth surface" / "fourth prompt surface" for `.codescout/system-prompt.md`; replace with the file's name when `src/server.rs` is next touched.
- Check: `git grep -n 'fourth' -- src/prompts/README.md scripts/probe_tool_surface.py CLAUDE.md` returns no ordinal standing in for a surface's name (verified at HEAD `fda10a31`: the only `fourth` hits among the four files named in the original Resume are in `src/server.rs`).

## Fix provenance

- **SHA:** `527fafa1` (`experiments`)
- **patch-id:** `24c9a1df5dc9125334cd6cc65dcd5e8ee8cc31d6`

## References

- `CLAUDE.md` § *Prompt Surface Consistency*, and `0d282a94` (stage 2d, the `CLAUDE.md` side)
- `prompt-surface-compaction-session-log:F-14`
- `docs/trackers/issue-clusters/IC-11-doc-contradicted-by-code.md` — this README already has one
  IC-11 member, `prompts-readme-cites-a-guide-guard-test-that-was-deleted` (2026-10-01)
