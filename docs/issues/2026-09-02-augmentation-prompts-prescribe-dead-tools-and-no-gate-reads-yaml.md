---
id: db80a4adc712c971
kind: bug
status: open
title: 'BUG: 20 augmentation prompts prescribe dead tool calls, and no gate scans YAML'
tags:
- cluster/doc-contradicted-by-code
---

## Summary

`docs/augmentations/*.yaml` `prompt:` fields are **delivered to the model at refresh time** — they
are live instructions, not documentation. Twenty of them prescribe tool calls that no longer exist:
**6 prescribe `artifact_augment(…)`** and **14 prescribe `artifact(action=…)`**. No gate scans this
directory, and no task in the tool-surface-collapse plan claims it.

## Symptom (Effect)

An augmentation refresh delivers a prompt instructing the model to call a tool that was deleted. The
model either emits the dead call and receives an unknown-tool error, or silently works around it.
Nothing in the build reports the staleness.

## Reproduction

At `5da2537d` on `tool-collapse`:

```
grep -rlw 'artifact_augment' docs/augmentations/*.yaml     →  6 files
grep -rl  'artifact(action=' docs/augmentations/*.yaml     → 14 files
```

Named instances include `docs-research-README.yaml:42` and
`docs-trackers-tool-usage-patterns.yaml`. Measured 2026-09-02 by the Opus task review of `5da2537d`.

Then confirm no gate reaches them: `present_tense_surfaces()` (`tests/doc_tool_refs.rs`) enumerates
its scope explicitly and `docs/augmentations/` is not in it; `audit_doc_refs`' default paths are
`docs/**/*.md`, `CLAUDE.md`, `**/README.md` — all markdown, and these are YAML.

## Environment

Branch `tool-collapse` at `5da2537d`. The `artifact_augment` half is branch-specific (Task 5 deleted
that tool); **the `artifact(action=…)` half is already stale on `experiments` too**, since Tasks 1-3
renamed that tool to `doc` — but those tasks are also branch-local. Re-check the counts against
whichever branch you are fixing on: this is exactly the kind of number that differs per checkout.

## Root cause

**A model-facing surface that no instrument treats as one.** Three properties combine:

1. **It is executable prose.** An augmentation `prompt:` is delivered into a model's context at
   refresh time and is acted on. It has the consequence of code and the form of documentation.
2. **It is YAML.** Every doc-scanning instrument in this repo is markdown-scoped —
   `audit_doc_refs`' default globs, `present_tense_surfaces()`' explicit list, `link_scan`'s
   artifact corpus. A `.yaml` file carrying prose falls between them.
3. **No task owns it.** The plan assigns `src/`, `docs/manual/`, `docs/trackers/` and the guides.
   `docs/augmentations/` appears in none of them, so it is not "deferred" — it was never scoped.

Inferred from the scan scopes as written, **and measured** for the counts above; the *delivery*
claim (that these prompts reach the model) is read from the refresh path, not observed at runtime —
confirm before designing a fix.

## Evidence

### The shape that makes it invisible

`audit_doc_refs` would flag these immediately if it read them — it resolves symbol and path
references against the filesystem and LSP index. It does not read them because they are not
markdown, and the file-type restriction is a *performance and precision* decision that silently
became a *coverage* decision when prose moved into YAML.

### Why the count is split

`artifact_augment(…)` and `artifact(action=…)` are different staleness generations from different
tasks. Reporting them as one number of 20 would hide that the second group was already stale before
this task began.

## Hypotheses tried

1. **Hypothesis:** these prompts are inert — historical records rather than live instructions.
   **Test:** read the refresh path to see whether `prompt:` is delivered into model context.
   **Verdict:** rejected by reading — the augmentation prompt is the persistent instruction the
   refresh cycle is built around. **Not confirmed at runtime**; do that before fixing.
   **Evidence:** `artifact_refresh(action="gather")`'s contract.

2. **Hypothesis:** `audit_doc_refs` covers them via a `docs/**` glob.
   **Test:** read its default path list.
   **Verdict:** rejected — the defaults are `docs/**/*.md`, `CLAUDE.md`, `**/README.md`. The `*.md`
   suffix excludes YAML.
   **Evidence:** § Reproduction.

## Fix

Two independent halves; do the second even if the first is deferred.

1. **Repair the 20 prompts** — mechanical, once the target names are settled by the collapse
   programme. Do it **after** the renames land, not during, or it will be done twice.
2. **Bring the directory under a gate.** The cheapest form is adding `docs/augmentations/*.yaml` to
   an existing scanner's scope with a YAML-aware reader that checks only the `prompt:` field. The
   check that matters is the same one the markdown surfaces get: *does every tool call named here
   name a live tool?*

**Sequence matters.** Fixing the prose without wiring the gate leaves the next rename to rediscover
this by hand, which is how it reached 20.

Fix SHA: *(not yet fixed)*
Patch-id: *(not yet fixed)*

## Tests added

None yet. Acceptance is an **observed RED**: a `docs/augmentations/*.yaml` prompt naming a dead tool
must fail a gate. Today it fails nothing.

## Workarounds

Before trusting an augmentation's refresh output, check its `prompt:` for tool calls by hand:

```
grep -n 'artifact\|doc(action=' docs/augmentations/<name>.yaml
```

## Resume

Confirm at runtime that `prompt:` reaches model context on a refresh (the one unverified premise
here), then choose between extending `audit_doc_refs`' globs with a YAML branch and adding a
dedicated check. Do not repair the 20 prompts until the collapse programme's renames have landed —
`doc` is not the final surface until Task 13.

## References

- Found during the Opus task review of `5da2537d` (Task 5 of the tool-surface-collapse plan),
  2026-09-02, as review finding M6, flagged as programme-level rather than charged to Task 5.
- `docs/trackers/issue-clusters.md` § `IC-11`. This is a **new surface** for that class — its
  fifteenth member was noted as "a tracker worklist field, the first outside code and schema
  entirely"; an augmentation prompt is the first that is *model-facing and executable*.
- `docs/PROBES.md` — the instrument index. This directory appears in no row.

