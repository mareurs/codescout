---
id: efb6cea2d5c0cf7e
kind: bug
status: fixed
title: 'BUG: memory(write) replaces a topic wholesale with no shrink guard — destroyed 83% of the gotchas memory and returned status ok, while the identical artifact operation is refused without force=true'
owners:
- marius
tags:
- memory
- data-loss
- guard-asymmetry
closed: 2026-08-28
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

**Fixed 2026-08-28.** Option **(a)** — the ported shrink guard. Option (b),
unconditional delta reporting, was **not** done and stays open; see *Resume*.

`experiments` SHA `5b7b82cc`, patch-id `4477be7feb16fad3ff16b9dfabaa1e884a3ca53e`.
(Both, per CLAUDE.md: the SHA is positional and dies when `experiments` is rebased;
the patch-id is a content hash of the diff and survives rebase and cherry-pick.)

**Placement was the real decision, and it is NOT `MemoryStore::write`** — despite
that being the single chokepoint under both the MCP tool and the dashboard API.
Three measured reasons:

1. `src/tools/onboarding.rs:816,823` rewrites `onboarding` and `language-patterns`
   wholesale by design. A store-level guard blocks regeneration whenever the new
   summary is shorter.
2. `src/memory/mod.rs` has a test named `overwrite_replaces_content`.
   Replace-wholesale is the **specified** primitive semantics, not an oversight.
3. The precedent this bug asked to mirror puts the guard in
   `librarian/tools/update.rs` — a **tool** — with `force` as a *caller* argument,
   not in the catalog write primitive.

So the split is: `MemoryStore::shrink_check` (pure, non-mutating, no policy), the
refusal at the tool, and `write` untouched. This also keeps `src/memory/` from
depending on a tool-layer error type.

The check runs **inside each of the write action's two branches**, not hoisted
above them: the private and project stores are different directories, so one
check up top would read the wrong file — guarding nothing on one path, and
invisibly so.

Semantics mirror the artifact side byte for byte: `new * 2 < old`, a 200-byte
floor, `force=true` to override. The floor is a **separate constant** from
`librarian::tools::update::SHRINK_GUARD_MIN_BYTES` rather than a shared one — the
two answer different questions (frontmatter shells vs stub memories) and agreeing
today is not a reason to make moving one move the other.

The hint leads with *"`write` REPLACES the topic wholesale — it does not append"*,
because Evidence #1 is right that the failure mode is a wrong mental model rather
than a missing flag.

**Cost:** `TOOL_SURFACE_CHAR_BUDGET` had ~27 chars of headroom and the `force`
param needs ~280, so it breached on arrival. Raised 56_266 → 56_519 at the owner's
direction, against the gate's own advice, with a sweep owed. Recorded as debt in
the constant's doc comment and set to the exact measured total so the ratchet
still bites on the next byte.
## Tests added

**Ten**, all green. Five on the store (`src/memory/mod.rs`) and five on the tool
(`src/tools/memory/tests.rs`):

| test | pins |
|---|---|
| `shrink_check_flags_a_destructive_overwrite` | the measured 751 → 112 bytes, and `pct == 86` |
| `shrink_check_is_silent_when_the_write_grows` | no false positive on growth |
| `shrink_check_is_silent_for_a_new_topic` | a first write destroys nothing |
| `shrink_check_is_silent_below_the_byte_floor` | the 200-byte exemption |
| `shrink_check_permits_removing_exactly_half` | the `new*2 < old` boundary, both sides |
| `write_refuses_a_destructive_overwrite` | the refusal **and** the file being byte-identical |
| `write_with_force_permits_the_overwrite` | the documented escape works |
| `write_guard_also_covers_the_private_store` | the second branch is not silently unguarded |
| `write_guard_is_silent_on_a_first_write` | the tool stays usable |
| `schema_advertises_force` | the param is reachable, and its text says REPLACES |

### CORRECTION — this section's original test-design advice was wrong

It read:

> *"it must assert on the **refusal**, not on the file being unchanged, since an
> unchanged file is also what a no-op produces."*

The stated risk is real — an unchanged file alone cannot distinguish a working
guard from a write that silently did nothing. But the prescribed remedy inverts
the priority, and a mutation test run 2026-08-28 shows which assertion actually
carries the weight.

Patching the tool to **warn-but-write** — keep the error, write anyway:

```rust
if let Some(r) = store.shrink_check(topic, content) {
    store.write(topic, content)?;              // MUTATION
    return Err(shrink_guard_error(topic, &r).into());
}
```

leaves `expect_err` passing **and** `msg.contains("memory-shrink guard")` passing.
The suite fails on exactly one line — `tests.rs:897`, *"a refused write must leave
the topic byte-identical"*. Had the original advice been followed, that mutation
ships: an error the caller believes means "nothing happened", plus the data loss.

**Assert both.** `expect_err` rules out the no-op the original text feared;
file-unchanged rules out warn-and-write. Neither alone is sufficient, and the one
this section told the reader to drop is the one that caught the bug.
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

**(a) is done — see *Fix*.** Two things remain, neither blocking:

1. **Option (b), unconditional delta reporting, is not implemented.** A field that
   appears only on failure cannot confirm success. Still worth doing, still
   independent of (a).
2. **The `TOOL_SURFACE_CHAR_BUDGET` sweep.** The budget was raised to land this
   fix. The pass that pays it back must *lower* that line, and any pass that
   cannot is a pass that did not happen.

The **dashboard** write path (`src/dashboard/api/memories.rs:58`) is deliberately
still unguarded — it calls `MemoryStore::write` directly. That is a human surface
where the operator typed the content and can see it, whereas the loss this bug
records was silent to an agent. Adopting `shrink_check` there is a one-line change
if it ever proves wanted; it is a scope decision, not an oversight.
## References

- `src/tools/memory/mod.rs` — the unguarded write path
- `get_guide("librarian")` § *The shrink guard, `force`, and event forensics* — the artifact-side guard this should mirror
- `docs/issues/archive/2026-05-25-augmented-artifact-body-overwrite.md` — the artifact-side incident that motivated that guard
- Hit live during the 2026-08-28 cross-machine catalog resume; process at `docs/conventions/cross-machine-catalog-resume.md`
