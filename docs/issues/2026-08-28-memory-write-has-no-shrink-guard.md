---
id: d8a7d136a92ee5a2
kind: bug
status: open
title: 'BUG: memory(write) replaces a topic wholesale with no shrink guard — destroyed 83% of the gotchas memory and returned status ok, while the identical artifact operation is refused without force=true'
owners:
- marius
tags:
- memory
- data-loss
- guard-asymmetry
---

# BUG: `memory(write)` has no shrink guard

## Summary

`memory(action="write", topic=X, content=Y)` **replaces the whole topic**. There is
no append mode, no size check, and no warning. Writing two new sections to a
17-section memory deleted the other 15 and returned `{"status": "ok"}`.

The same shape of operation on an **artifact** is refused: `artifact(action="update",
patch={body})` has a 50% body-shrink guard that returns `RecoverableError` unless
`force=true`. Memories — durable, hand-curated project knowledge, auto-loaded at
session start — have no equivalent.

## Symptom (Effect)

Live, 2026-08-28, on this repo:

```
memory(action="write", topic="gotchas", content="## A Fresh Machine …\n## Co-Occurrence …")
→ {"status": "ok", "wrote_to": "/home/marius/work/claude/codescout"}
```

Before: **391 lines, 17 `##` sections.**
After: **66 lines, 2 `##` sections.**

An 83% reduction, silent. No `prev_bytes` / `new_bytes`, no `sections_removed`, no
warning field. The `## MCP Binary Symlink`, `## Cherry-Pick SHA Discipline`,
`## Kotlin LSP Circuit-Breaker` sections and twelve others were gone, along with the
file's `# Workspace Gotchas` H1.

## Reproduction

```
# 1. note the size
wc -l .codescout/memories/<topic>.md ; grep -c '^## ' .codescout/memories/<topic>.md

# 2. write content shorter than the existing topic
memory(action="write", topic="<topic>", content="## One New Section\n\nbody\n")

# 3. same two counts — the file is now just the new content
```

Deterministic. Nothing in the response distinguishes it from an append.

## Environment

Branch `experiments` @ `1e10840b`, linux, codescout 0.15.0, release build.

## Root cause

**Measured, not inferred** — `src/tools/memory/mod.rs` contains no shrink or
size-delta check. A `grep` for `shrink|prev_bytes|guard|truncat` over that file
returns four hits, none of them a write guard: three are doc-comments about
*output* truncation, and the fourth is an `OutputGuard` on the `recall` result
path. The write is a plain replace.

The asymmetry is the finding, because the artifact side already solved this:

| surface | wholesale body replace | guard |
|---|---|---|
| `artifact(action="update", patch={body})` | yes | **50% shrink guard**, `force=true` to override; emits a `field_patch` event with `prev_bytes`/`new_bytes`/`replaced_subsections` |
| `memory(action="write")` | yes | **none** |

`get_guide("librarian")` documents the artifact guard as existing because a
wholesale overwrite "caused a real ~600-line tracker body loss". A memory is at
least as load-bearing: memories are auto-loaded into every session, and CLAUDE.md
routes all durable cross-session facts here specifically *because* Claude Code's
own memory is per-profile and lossy.

## Evidence

### 1. The tool's own docs never say "replaces"

The `memory` tool description reads: *"Persistent project memory. Topic-based:
read/write/list/delete with path-like keys."* `write` is listed beside `read` with
no note that it is destructive, and there is no `append` action to reach for
instead. A caller who has just read a topic and wants to add to it has exactly one
verb, and it silently means "replace".

### 2. The loss was recoverable here only by luck of timing

`.codescout/memories/gotchas.md` is git-tracked, so `git checkout HEAD -- <file>`
restored it. That is the mitigation, not a defence: an uncommitted memory edit —
or any memory written since the last commit — has no such fallback, and the
catalog-side embedding is re-derived from whatever the file now says.

### 3. Anchors churn alongside

The write also rewrote `.codescout/memories/gotchas.anchors.toml` (14 `[[` entries),
so a naive `git checkout` of the `.md` alone leaves the anchors describing sections
that no longer match. Both files must be restored together.

## Hypotheses tried

1. **Hypothesis:** `write` appends and I mis-read the result.
   **Test:** counted `^## ` before (17) and after (2), and grepped for three known
   section titles — all absent.
   **Verdict:** rejected; it replaces.

2. **Hypothesis:** a guard exists but did not fire because the content was not
   "shrinking enough".
   **Test:** 391 → 66 lines is 83%, far past the artifact side's 50% threshold; and
   a source grep finds no guard at all in the write path.
   **Verdict:** rejected; there is no guard to fire.

## Fix

Not started. Two candidates, and they are not exclusive:

- **a. Port the artifact shrink guard.** Refuse a `write` that reduces the topic by
  more than 50% unless `force=true`, with the same error shape and hint. Cheapest,
  reuses a threshold already tuned on this repo, and fails loudly at the moment the
  caller can still fix it. Exempt topics under ~200 bytes, as the artifact side does.
- **b. Report the delta unconditionally.** Return `prev_bytes` / `new_bytes` /
  `sections_before` / `sections_after` on every write. A field that appears only on
  failure cannot confirm success, and this is the same argument
  `workspace(status)`'s serving-binary block was added under.

An `append` action would be a third option but is a bigger surface change; (a)+(b)
close the data-loss path without one.

## Tests added

None yet. The regression test is a write whose content is shorter than the existing
topic, asserting `RecoverableError` — and it must assert on the **refusal**, not on
the file being unchanged, since an unchanged file is also what a no-op produces.

## Workarounds

**Never call `memory(action="write")` to add to an existing topic.** Read the file,
append with a shell redirect, and verify the section count:

```
cat /tmp/new-sections.md >> .codescout/memories/<topic>.md
grep -c '^## ' .codescout/memories/<topic>.md      # must equal old + new
```

If already destroyed and the file is committed:

```
git checkout HEAD -- .codescout/memories/<topic>.md .codescout/memories/<topic>.anchors.toml
```

Restore **both** files — see Evidence #3.

## Resume

Write the failing test from *Tests added* against `src/tools/memory/mod.rs`'s write
path and confirm it fails today. Then implement (a), reusing the artifact guard's
error text and `force` parameter so the two surfaces read the same. Decide (b)
separately — it is useful regardless of whether (a) lands.

## References

- `src/tools/memory/mod.rs` — the unguarded write path
- `get_guide("librarian")` § *The shrink guard, `force`, and event forensics* — the artifact-side guard this should mirror
- `docs/issues/archive/2026-05-25-augmented-artifact-body-overwrite.md` — the artifact-side incident that motivated that guard
- Hit live during the 2026-08-28 cross-machine catalog resume; process at `docs/conventions/cross-machine-catalog-resume.md`

