---
id: 463b1cb984c715b0
kind: bug
status: mitigated
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

> **Re-measured 2026-09-11, and the headline claim is FALSIFIED.** § Summary says deleting
> `read_markdown` leaves a dead Iron Law 4 *"with **every** prompt-surface gate green"*. One gate
> is not green. The observation about `prompt_surfaces_reference_only_real_tools` being
> backtick-scoped still holds; the harm attributed to it does not.

**The reproduction, run rather than reasoned.** Iron Law 4 was reinstated in exactly the stale form
this file cites — `4. NEVER read_file markdown → read_markdown (heading-addressed).`, unbackticked:

| gate | result |
|---|---|
| `rendered_server_instructions_contains_no_deprecated_tool_names` | **FAILED** — *contains deprecated tool name: read_markdown* |
| `prompt_surfaces_reference_only_real_tools` | ok |

That first gate does a plain `rendered.contains(dead)` over `build_server_instructions`, which
includes the Iron Laws. Backticks are irrelevant to a substring check, so the surface this file
calls invisible is covered — by a **denylist**, not by the allowlist gate it names.

**Two sibling changes closed most of the mechanism after this was filed:**

- `DEPRECATED_TOOL_NAMES` now holds all five names § Fix says are owed — `read_markdown`,
  `edit_markdown`, `artifact_augment`, `artifact_event`, `artifact_refresh` — plus `artifact(`.
  That bullet is **done**.
- `prompt_surfaces_reference_only_real_tools` gained a **call-form** regex,
  `\b([a-z][a-z_0-9]{2,})\(`, which is *not* backtick-scoped
  (`docs/issues/2026-09-05-the-prompt-surface-gate-misses-a-backticked-tool-name-in-call-form.md`).
  So `symbols(path)` and `symbols(name=...)` in Iron Law 1 are validated against live tools today.

**What is still invisible, stated exactly:** a **bare, non-call-form** tool name in the Iron Laws —
`edit_file` and `edit_code` in Law 2, `run_command` in Law 3, `read_file` in Law 1 — is seen by
neither regex. It is caught only if someone adds the retired name to `DEPRECATED_TOOL_NAMES`. The
residual is therefore: *a tool deleted **without** a denylist entry, named bare and uncalled in the
Iron Laws.* Narrow, and the four names it covers are the most central tools in the system.
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

**§ Fix's first bullet is not affordable, and that was measured rather than argued.** It claims a
check over the Iron Laws block *"needs no allowlist growth"* because the block is small and
structured. Tokenising the real block with `[a-z][a-z_0-9]{2,}` yields **~60 distinct ordinary
words** — Law 6 is plain prose (*"Subagents see only what you brief them with. Name the guides they
must fetch themselves…"*), and even restricted to the arrow-bearing Laws 1-5 it is ~45. That is the
same allowlist explosion this file rejects the unanchored-regex option for, one paragraph earlier.
The proposed remedy has the property it disqualifies the alternative for.

**§ Fix's second bullet is done** — see § Root cause.

**What shipped instead, because it closes a hole that actually existed.** The denylist gate doing
the protecting is an **absence** assertion, so it is monotone under the surface shrinking: narrow
`build_server_instructions` to emit only the quickref and it passes over a text no longer
containing the Iron Laws, silently. Measured — deleting the block from `source.md` left
`rendered_server_instructions_contains_no_deprecated_tool_names` **green**.
`rendered_server_instructions_includes_the_iron_laws` now reds on exactly that, asserting both the
heading and its numbered body so a surviving header cannot satisfy it.

**The residual is deliberately not mechanised**, and the reason is this file's own argument turned
around: closing it needs the allowlist that § Fix correctly calls unsustainable. A gate nobody can
keep green gets deleted, and its deletion takes the working half with it.

Fix SHA: 86ce41b9 *(the vacuity pin only — no behaviour change; the exposure was closed by the
two sibling changes named in § Root cause)*
Patch-id: 4abdef288644de760b788749fc38193316cc7d35
## Tests added

One, `rendered_server_instructions_includes_the_iron_laws` (`src/prompts/mod.rs`), pinning that the
Iron Laws are inside the surface the deprecated-name denylist scans.

**Observed RED both ways, which is the whole point of adding it:**

- Remove the Iron Laws block from `source.md` → the new test **reds**, and
  `rendered_server_instructions_contains_no_deprecated_tool_names` stays **green**. That green is
  the defect: an absence assertion passing over a surface that no longer holds what it protects.
- Reinstate this file's stale Iron Law 4 → the denylist gate **reds**, the new test stays green.

The two failures are disjoint, which is why both tests are needed and neither subsumes the other.
## Workarounds

When deleting a tool, grep the prompt surfaces for its **bare** name, not just the backticked form:

```
grep -rnw '<tool>' src/prompts/
```

`src/prompts/source.md` and `src/prompts/guides/*.md` are the surfaces the model actually reads.

## Resume

Mitigated, not fixed, and the distinction is load-bearing: the title's claim about
`prompt_surfaces_reference_only_real_tools` remains literally true — it *is* backtick-scoped — while
the exposure that made it matter is closed by the denylist and the call-form regex.

**Do not implement § Fix's first bullet without re-running the token count.** It was falsified at
~60 words on 2026-09-11; a future `source.md` where the Iron Laws carry no prose would change that
answer, and it is a two-minute measurement.

The residual, restated so it is not mistaken for closed: a tool deleted **without** a
`DEPRECATED_TOOL_NAMES` entry, named bare and uncalled in the Iron Laws, is still invisible to every
gate. The four names in that position today — `read_file`, `edit_file`, `edit_code`, `run_command` —
are the system's most central tools, so the scenario requires deleting one of those *and* forgetting
the denylist entry in the same change.
## References

- Found during the Task 7 pre-dispatch scout, 2026-09-02.
- `DEPRECATED_TOOL_NAMES` backfill was separately flagged as M3 by the Opus review of `f7b7ff33`.
- Siblings in the same class, different mechanisms: `bee04240275ee7d9` (citation filter),
  `db80a4adc712c971` (file type), `3f0e7733ae77c707` (directory enumeration),
  `ef3e685d69e34321` (label parsing). This one is **markup**.
- `CLAUDE.md` § *Testing Discipline* — "Loudness is a property of a PATH, not of a failure."
