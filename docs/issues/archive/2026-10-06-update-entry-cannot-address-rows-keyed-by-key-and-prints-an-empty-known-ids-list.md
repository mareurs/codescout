---
id: d04d30314e7a7472
kind: bug
status: fixed
title: 'BUG: doc update_entry cannot address rows keyed by key, and its Known ids hint prints an empty list for them'
tags:
- librarian
- update_entry
- augmentation
- cluster/addressing-without-an-escape-hatch
closed: 2026-10-10
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

## Fix provenance (2026-10-10)

This section supersedes `## Fix`, `## Tests added` and `## Resume` above, which describe the state before the fix.

- **SHA:** `f0e0e030` (`experiments`)
- **patch-id:** `1e631131889dbc383e450a89a59cab1ddd4b39ae`
- **SHA:** `a3a317c0` (`experiments`)
- **patch-id:** `47bd6bdc828e47c06279ba7edcddb89db54ca34a`

`update_entry` now addresses a row by its `id`, or by its `key` when the row has no `id`. The rules, in `locate_entry` (`src/librarian/catalog/augmentation.rs`):

- A row that has an `id` is addressed by that `id` only, and its `key` is an ordinary field. A duplicate `id` still patches the first match, as before.
- A `key` match is taken only on a row with no `id`, and only when it is unique.
- A value that is one row's `id` and also another row's `key`, or the `key` of two rows, is refused by name as ambiguous. Nothing is written.
- A field patch may not rewrite the `key` that is a row's only address.
- The unknown-entry hint lists what IS addressable (`Known ids`, `Known keys`, or `Known ids and keys`), capped at twelve with the remainder counted, and counts rows that carry neither handle.
- `append_entry` id allocation is untouched, and a `key` never becomes citable: the snapshot and heading notes, which reason about citable `PREFIX-N` ids, say nothing for a `key`-addressed row.

Tests, all in `augmentation.rs`: `update_entry_addresses_a_row_that_has_a_key_and_no_id`, `..._both_by_its_id_only`, `..._refuses_to_guess_between_an_id_and_a_key_or_between_two_keys`, `..._unknown_key_lists_the_addressable_keys_and_not_an_empty_list`, `..._keeps_the_id_only_behaviour_byte_for_byte`, `..._refuses_to_rewrite_the_key_that_is_a_rows_only_address` and `..._says_nothing_about_citable_headings_for_a_row_addressed_by_key`. Mutation checks, each applied and restored byte-identical: disabling `key` addressing and making an ambiguous match pick the `id` row, applied together, fail four of them, among them the first and the ambiguity test. The `entry_id` description in the `doc` schema names the `key` form (`a3a317c0` trims it to +28 characters). The full gate ran green on the merged tree (lean 4471, default 6744, Python 1010 passed, 0 failed).

Not done: the live legibility tracker was retagged on 2026-10-06 with a whole-array merge, because this action could not address its rows. Nothing was changed there; a future retag can use this action.

## References

- `docs/issues/2026-09-21-legibility-auto-close-cannot-tell-repair-from-detector-removal.md`, Update 2026-10-06.
- `get_guide("librarian")` § Augmentation Lifecycle, and the `update_entry` description in the `doc` tool: both prescribe `update_entry` rather than `patch={params:...}`.
- Cluster `IC-6`: the scheme addresses by one field and provides no way to address a row that has another.
