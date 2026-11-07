---
kind: bug
status: fixed
title: a memory topic whose file name holds a backslash lists under a rewritten name that reads back as nothing
tags:
- cluster/addressing-without-an-escape-hatch
closed: 2026-11-07
opened: 2026-11-07
unverified: 'Fixed, gated, with a failing test first and a killed mutant at both sites, and seen live for list and read after a rebuild. Not seen live: check_all_memories (the anchors report). The cfg(windows) arm of display_path is not run here, and the dashboard was not traced for other uses of a listed topic name.'
---

# BUG: a memory topic whose file name holds a backslash lists under a rewritten name that reads back as nothing

## Summary

On Linux a backslash is an ordinary file-name character. `sanitize_topic` (`src/memory/mod.rs`) keeps a topic such as `a\b` as one path component, so `MemoryStore::write("a\\b", …)` creates the file `a\b.md`. `MemoryStore::list` then rewrote each `\` in the file name to `/` and returned the topic `a/b`. Reading the listed name looks for `a/b.md`, which does not exist, so a topic that `list` reports cannot be read. `check_all_memories` (`src/memory/anchors.rs`) named the same non-existent topic in its `untracked` list.

## Class fit

`cluster/addressing-without-an-escape-hatch`, the same class as `2d8475ed5bc831e5`: the listed name is one the caller feeds back as an address, and the rewrite turns a legal file-name character into the separator.

## Symptom (Effect)

`write("a\\b", "content")` then `list()` returned `["a/b"]`; `read("a/b")` returned nothing.

## Reproduction

Run 2026-11-07, Linux, as a unit test on the old code (`memory::tests::a_topic_with_a_backslash_lists_under_its_own_name_and_reads_back`):

```
store.write("a\\b", "content")
store.list()   ->  ["a/b"]      expected ["a\\b"]
```

The assertion failed with `left: ["a/b"]`, `right: ["a\\b"]`.

## Environment

Linux 7.2, branch `experiments`, before `4099a06f5`.

## Root cause

`src/memory/mod.rs`, `MemoryStore::list`, and `src/memory/anchors.rs`, `check_all_memories`, each built the topic with `rel.with_extension("").to_string_lossy().replace('\\', "/")`. The replacement exists for Windows, where a nested topic `a/b` is a file `a\b.md` and its relative path uses `\` as the separator. On Linux it also rewrote a `\` that is part of a file name.

## Evidence

The failing test output quoted above, and the two code lines.

## Hypotheses tried

None.

## Fix

Committed 2026-11-07 (`4099a06f5`): both sites use `crate::util::fs::display_path`, which is the forward-slash form on Windows and the path unchanged elsewhere. A nested topic keeps its `/` form on Windows, as before.

## Fix provenance

- **SHA:** `4099a06f5` (`experiments`)
- **patch-id:** `637a3f7c413760c61cedc11f6fe0a5e8d6d0bf57`

Fix committed 2026-11-07, not pushed when this was written. Gate: FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0; LEAN 6209 and DEFAULT 9091 passed.

## Tests added

- `memory::tests::a_topic_with_a_backslash_lists_under_its_own_name_and_reads_back` (`src/memory/mod.rs`): the listed topic must be `a\b` and must read back.
- `memory::anchors::tests::an_untracked_topic_with_a_backslash_is_named_as_it_is_on_disk` (`src/memory/anchors.rs`): a file `a\b.md` is reported untracked as `a\b`.

Red first for the first test. Mutant: the old `replace('\\', "/")` restored at both sites failed both tests; reverted.

## Workarounds

Avoid a backslash in a memory topic name.

## Resume

Fixed in `4099a06f5` and seen live on 2026-11-07 after a rebuild and `/mcp` reconnect: in a scratch project, `memory(write, topic="a\b")` then `memory(list)` listed `a\b`, and `memory(read, topic="a\b")` returned the content. Before the fix `list` named `a/b`. See the `unverified` caveat in the frontmatter. Found while tracing the callers listed in `2d8475ed5bc831e5`.

## References

- `2d8475ed5bc831e5`, the class this belongs to.
- `src/memory/mod.rs` (`list`, `sanitize_topic`), `src/memory/anchors.rs` (`check_all_memories`), `src/util/fs.rs` (`display_path`).
