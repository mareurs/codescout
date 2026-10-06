---
id: '699b03cc445776d3'
kind: bug
status: open
title: 'BUG: doc update_entry cannot address rows keyed by key, and its Known ids hint prints an empty list for them'
tags:
- librarian
- update_entry
- augmentation
- cluster/addressing-without-an-escape-hatch
opened: 2026-10-06
owner: marius
related:
- docs/issues/2026-09-21-legibility-auto-close-cannot-tell-repair-from-detector-removal.md
severity: low
---

# BUG: `doc(action="update_entry")` cannot address rows keyed by `key`, and its "Known ids" hint prints an empty list for them

## Summary

`update_entry` finds a row only by its `id` field. The legibility backlog's rows carry `key` and no `id`, so the action cannot reach any of them. Its refusal then prints `Known ids:` followed by nothing, which looks like a bug in the hint and gives no route to the real problem. The guide prescribes `update_entry` for a per-row edit, so the prescribed path does not exist for this tracker.

## Symptom (Effect)

Measured 2026-10-06 at `10e935e3`:

```
doc(action="update_entry", id="cd886c414f6751b4", entry_collection="candidates",
    entry_id="src/ast/parser.rs::extract_rust_symbols", fields={"closed_reason": "refactored"})
→ {"ok": false, "error": "update_entry: no entry `src/ast/parser.rs::extract_rust_symbols` in `candidates`", "hint": "Known ids:  (+42 more)"}
```

The row exists. The hint's list is empty, and the `(+42 more)` count is the whole collection.

## Reproduction

```
git rev-parse --short HEAD    # 10e935e3, branch experiments
```

1. Run the call above with `workspace="/home/marius/work/claude/codescout"`. A refused call writes nothing.
2. Read `src/librarian/tools/legibility_scan/mod.rs:199-200`. `CandidateRow` has `key: String` and no `id` field.

## Environment

Linux, MCP over stdio, `experiments` at `10e935e3`. The artifact's augmentation declares `entry_collection: "candidates"`.

## Root cause

Measured 2026-10-06 (the call above) and read from `src/librarian/catalog/augmentation.rs`:

- Line 393: `.position(|e| e.get("id").and_then(|v| v.as_str()) == Some(entry_id))`. The row lookup tests `id` only.
- Lines 399-413: the "Known ids" list is built from `e.get("id")` and skips rows without one. For 42 rows with no `id` the list is empty, and `elided = entries_total - known.len()` is 42.
- `append_entry` (line 730) and `resolve_cite_ref` (line 2315) also read `id` only.

So an entry collection is addressable only when its rows carry `id`. Nothing declares or checks that when the augmentation sets `entry_collection`.

## Evidence

The legibility bug (`docs/issues/2026-09-21-legibility-auto-close-cannot-tell-repair-from-detector-removal.md`) advised a per-row retag "never a `params` array rewrite". Its 2026-10-06 update records that the advised route does not exist for these rows, and that the retag was done as one whole-array `doc(action="augment", merge=true)` with `params_path`, built with `jq` from the live params: 42 rows before and after.

## Hypotheses tried

1. **Hypothesis:** the key is `id`, and the legibility rows have it under another name. **Test:** read `CandidateRow`. **Verdict:** confirmed. The field is `key`.

## Fix

Not started. Options, not exclusive:

- Let an augmentation declare the key field of its entry collection (for example `entry_key: "key"`), and have `update_entry`, `append_entry` and the hint use it.
- Or give `CandidateRow` an `id` that equals `key`.
- Whatever else is chosen, make the refusal say when no row carries `id`: "rows in `candidates` carry no `id`; found `key`".

## Tests added

N/A — not fixed. `update_entry_rejects_an_unknown_entry_id` (`augmentation.rs:3388`) uses rows that carry `id`, so it cannot see this case.

## Workarounds

Rewrite the whole array through `doc(action="augment", merge=true)` with `params_path`, built from the live params with `jq`. Check the row count before and after.

## Resume

Choose between the declared key and the extra `id`. The hint fix is independent and small.

## References

- `docs/issues/2026-09-21-legibility-auto-close-cannot-tell-repair-from-detector-removal.md`, Update 2026-10-06.
- `get_guide("librarian")` § Augmentation Lifecycle, and the `update_entry` description in the `doc` tool: both prescribe `update_entry` rather than `patch={params:...}`.
- Cluster `IC-6`: the scheme addresses by one field and provides no way to address a row that has another.
