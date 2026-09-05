---
id: 4e4762b735deb392
kind: bug
status: open
title: 'BUG: the prompt-surface gate is backtick-scoped, so the Iron Laws are invisible to it'
tags:
- cluster/guard-narrower-than-its-name
---

## Summary

`prompt_surfaces_reference_only_real_tools` extracts candidate tool names with
`` r"`([a-z][a-z_0-9]{2,})`" `` — **backtick-delimited only**. The Iron Laws in
`src/prompts/source.md` are written *without* backticks, so the single most load-bearing prose in
the system is invisible to the gate that exists to keep prose from naming dead tools.

Deleting `read_markdown` (Task 7 of the tool-surface-collapse plan) leaves Iron Law 4 reading
`NEVER read_file markdown → read_markdown (heading-addressed)` — an instruction to route to a tool
that does not exist — with **every** prompt-surface gate green.

## Symptom (Effect)

A tool is deleted. `server_instructions` continues to instruct every session to call it. No gate
fails.

## Reproduction

At `93fd8deb`+ on `tool-collapse`:

1. `src/prompts/source.md:14` reads:

   ```
   4. NEVER read_file markdown → read_markdown (heading-addressed).
   ```

   No backticks on either token.

2. `src/server.rs:3500`:

   ```rust
   let re = regex::Regex::new(r"`([a-z][a-z_0-9]{2,})`").unwrap();
   ```

3. Delete the `read_markdown` tool and run the gate. It passes.

Measured 2026-09-02 during the Task 7 pre-dispatch scout, by reading the regex and the surface
rather than by inferring from the gate's name.

## Environment

Branch `tool-collapse`. `source.md` and the gate are both shared with `experiments`; the *exposure*
is created by this branch, which deletes `read_markdown` and `edit_markdown`.

## Root cause

**Backticks are markup, and the gate treats them as a type declaration.** Backticking is how prose
formats a token; it is not a promise about what the token denotes, and nothing requires an author to
use it. `source.md`'s Iron Laws are written as terse imperative lines and deliberately do not
backtick — a style choice made years before this gate existed, in a file the gate reads.

So the population is not "tool names in the surface" but "tool names the author happened to
backtick", and the difference is invisible in the output because the gate reports only drift it
found, never the tokens it declined to consider.

## Evidence — THREE independent gates are blind to this one deletion, each for a different reason

This is the part worth more than the instance. `read_markdown` appears across three guarded surfaces,
and each guard misses it by its own mechanism:

| gate | where `read_markdown` lives | why it is missed |
|---|---|---|
| `prompt_surfaces_reference_only_real_tools` (`server.rs:3460`) | `source.md:14`, Iron Law 4 | regex is **backtick-scoped**; the Iron Laws are unbackticked |
| guide-body denylist (`prompts/mod.rs:2051`) | `iron-laws-detail.md` ×8, `librarian.md` ×2, `untrusted-content.md` ×1 | it is a **denylist** and `read_markdown` is not in `DEPRECATED_TOOL_NAMES` — nor were `artifact_event` / `artifact_augment` / `artifact_refresh` added when Tasks 4-6 deleted them |
| `companion_surfaces_reference_only_real_tools` (`server.rs:3708`) | `mcp__codescout__read_markdown` in the companion plugin | those tokens appear **only in `*.test.sh` files, which the gate skips by design** as stale-name sentinels |

Three surfaces, three guards, three unrelated blind spots, one deletion. **No single fix closes it**,
and any one of the three examined alone reads as adequate coverage.

## Hypotheses tried

1. **Hypothesis:** the companion gate will red and force the plugin update early.
   **Test:** checked whether the plugin dir is present (it is — the gate runs, not skips) and where
   `mcp__codescout__read_markdown` appears.
   **Verdict:** rejected — both occurrences are in `il4-deny-hook.test.sh` and
   `worktree-write-guard.test.sh`, and the gate's doc-comment states `*.test.sh` is skipped
   deliberately.
   **Evidence:** § Evidence.

2. **Hypothesis:** `prompt_surfaces_reference_only_real_tools` will red on `source.md`.
   **Test:** read the extraction regex and the line.
   **Verdict:** rejected — the line is unbackticked and the regex requires backticks.
   **Evidence:** § Reproduction.

## Fix

**Do not simply drop the backtick requirement.** The regex is backtick-scoped for a stated reason —
the gate's doc-comment explains it skips PascalCase to avoid an allowlist explosion, and an
unanchored `[a-z][a-z_0-9]{2,}` over prose would match hundreds of ordinary words. Removing the
anchor trades a silent gap for a gate nobody can keep green, which is the failure mode that gets a
gate deleted.

Two directions, both narrower:

- **Scan the Iron Laws block specifically.** It is a fixed, small, structured region of `source.md`
  with one instruction per numbered line. A check that every tool-shaped token in *that block* names
  a live tool needs no allowlist growth, and the block is exactly the surface whose staleness costs
  most.
- **Extend `DEPRECATED_TOOL_NAMES` as tools are deleted**, which closes the guide-body half and is
  owed already for three tools deleted in Tasks 4-6 (review finding M3 on `f7b7ff33`).

Neither reaches the companion plugin's `.test.sh` exclusion; that one is correct as designed and its
residue is Task 12's.

Fix SHA: *(not yet fixed)*
Patch-id: *(not yet fixed)*

## Tests added

None yet. Acceptance is an **observed RED**: with `read_markdown` deleted and Iron Law 4 unchanged, a
gate must fail. Today none does.

## Workarounds

When deleting a tool, grep the prompt surfaces for its **bare** name, not just the backticked form:

```
grep -rnw '<tool>' src/prompts/
```

`src/prompts/source.md` and `src/prompts/guides/*.md` are the surfaces the model actually reads.

## Resume

Add a scan of `source.md`'s Iron Laws block to `prompt_surfaces_reference_only_real_tools` (or a
sibling test), confirm it REDs against a deliberately-stale Iron Law before wiring it green, then
backfill `DEPRECATED_TOOL_NAMES` with `artifact_event`, `artifact_augment`, `artifact_refresh` and —
once Tasks 7/8 land — `read_markdown` and `edit_markdown`.

## References

- Found during the Task 7 pre-dispatch scout, 2026-09-02.
- `DEPRECATED_TOOL_NAMES` backfill was separately flagged as M3 by the Opus review of `f7b7ff33`.
- Siblings in the same class, different mechanisms: `bee04240275ee7d9` (citation filter),
  `db80a4adc712c971` (file type), `3f0e7733ae77c707` (directory enumeration),
  `ef3e685d69e34321` (label parsing). This one is **markup**.
- `CLAUDE.md` § *Testing Discipline* — "Loudness is a property of a PATH, not of a failure."

