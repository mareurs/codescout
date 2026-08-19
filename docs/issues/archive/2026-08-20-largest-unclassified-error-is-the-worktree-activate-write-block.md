---
kind: bug
status: fixed
title: 'BUG: 73 of 1139 errors carried no err_family, concentrated in the librarian surface and one worktree write gate'
tags:
- usage-db
- err-family
- taxonomy
closed: 2026-08-20
opened: 2026-08-20
owner: marius
related:
- docs/issues/archive/2026-08-20-friction-target-omits-command-and-file-path.md
severity: low
unverified: 'The backfill''s end-to-end re-classification of historical rows has NOT been observed on a live DB — only the fingerprint change is unit-tested. Re-run scripts/friction-probe.py --null-detail after the next cargo rb + /mcp reconnect to confirm the count drops from 73 toward 4. Also: 4 one-off messages remain unclassified BY DESIGN, not by omission.'
---

# BUG: the largest unclassified error message — the worktree/activate write block — has no err_family, and unclassified is where friction concentrates

## Summary

`normalize_err_family` left 73 of 1,139 errors unclassified, and the population was not a
tail of miscellaneous novelty — it was **two concentrations**: 23 hits from a single
worktree write gate spread across four write tools, and 49% from the artifact/librarian API
surface. Coverage by tool made the same point from the other side: `run_command` at **0.2%**
unclassified and `read_markdown`/`grep`/`references` at **0%**, against `artifact` **37.5%**
and `memory`/`symbols` ~50%. The taxonomy is a map of where someone did the work, not of
which errors are hard.

**This file's original motivation was wrong and is corrected below.** It argued the
unclassified bucket was the *priority queue for gate authoring* because a transcript-joined
pass measured `err_family IS NULL` at a 1.97× friction lift. That did not reproduce: measured
against `usage.db` itself, the bucket's immediate-repeat rate is **2.8%** against a ~4–5%
corpus average — among the **healthiest** errors here. Classification is still worth doing,
for a plainer reason: an unnamed family cannot be counted, trended, or given a
`refusal_predicate`. It is bookkeeping, not pain relief.
## Symptom (Effect)

Measured 2026-08-20, errors with `err_family IS NULL`, grouped by message prefix:

```
n=22  Write blocked: git worktrees detected but workspace(action='activate') has not been
      called. Worktrees: [/home/marius/work/claude/codescout/.claude/wor…
n=6   extra must not contain frontmatter field(s) the schema already models: kind …
n=4   entry_filter set but this artifact is not augmented — declare entry_collection …
n=4   entry_filter set but the augmentation has no entry_collection …
n=3   update_entry: patched entry violates params_schema: /tasks/33/status: "fixed" is not
      one of ["open","in-progress","done","blocked","dropped"]
n=3   at most one of `full`, `heading`, `headings`, `start_line`+`end_line` may be set
n=3   allocate_entry_id: `…/docs/trackers/run-command-pipeline.md` does not declare an
      entry_prefix …
n=2   update_entry: `entry` is append_entry's parameter — this action takes `fields` …
n=2   language server for 'markdown' is still starting and there is no tree-sitter fallback
n=1   update_entry: `fields` is empty — there is nothing to patch …
n=1   unknown topic 'edit_code' …
```

Every one of these has a well-written message with a hint. What they lack is a *family*, so
none of them can be counted, trended, or targeted by `refusal_predicate`.

## Reproduction

```
# HEAD at filing: b4ea12fd989dfc2cbf1604be36090ddd3c99a6a3 (experiments)
sqlite3 -line .codescout/usage.db "
  SELECT substr(error_msg,1,150) msg, COUNT(*) n
  FROM tool_calls WHERE outcome='error' AND err_family IS NULL
  GROUP BY substr(error_msg,1,60) ORDER BY n DESC LIMIT 12;"
```

## Environment

Linux; codescout `experiments` at `b4ea12fd`.

## Root cause

`normalize_err_family` (`src/usage/db.rs:225-330`) is an ordered chain of
`msg.contains(...)` probes, extended each time someone measures the unclassified
population. The worktree write-block message was never added. `ERR_FAMILIES`
(`src/usage/db.rs:444-483`) is the companion list whose FNV-1a fingerprint occupies
`PRAGMA user_version` and drives the re-classification backfill, so adding a family
automatically re-classifies history on next open — the mechanism is already correct and
just has not been pointed at these messages.

measured 2026-08-20: the query above, plus
`grep(pattern="il3_pipe_to_trimmer|normalize_err_family", path="src/usage/db.rs")` read
directly.

## Evidence

### FALSIFIED 2026-08-20 — the table below is retained as the claim that failed

Re-measured against `usage.db` with `scripts/friction-probe.py` (calibrated against TU-7
first: ratios 0.9 / 0.82 / 0.63, ordering preserved). The lift below does **not** reproduce
from the database:

| Predicate (usage.db only) | NULL | classified |
|---|---:|---:|
| immediate repeat (TU-7's discriminator) | **2.8%** (2/71) | ~4–5% avg |
| same tool succeeds later | 15.5% | 11.2% |
| `calls_to_recovery` | mean **1.13**, 0% unrecovered | mean 1.0–1.67 |

The NULL bucket is at or below corpus average on every predicate computable from the store
a detector would query. Full reasoning, and the invalid test run while producing this
refutation, in `capability-proposals:CAP-9` § *Correction 2026-08-20*.

### Why NULL looked like the interesting bucket (superseded)

From a 2026-08-19/20 detector-validation pass over 840 matched errors (74.5% of the 30-day
error corpus), friction rate by family:

| err_family | n | friction rate | lift |
|---|---|---|---|
| `il2_structural_edit` | 47 | 4.3% | 0.16× |
| `il3_shell_on_source` | 119 | 13.4% | 0.50× |
| `il3_pipe_to_trimmer` | 188 | 19.1% | 0.71× |
| **`err_family IS NULL`** | **62** | **53.2%** | **1.97×** |
| `read_markdown_overflow_threshold` | 12 | 58.3% | 2.16× |

The named Iron-Law families are the *healthiest* errors in the corpus (median 1 call to
recover) and also the highest-volume. Unclassified errors are where recovery actually
costs something. Reported by that pass; the friction label is its own construct (retry of
the same mistake, or ≥3 calls to recover, or no recovery within 25 calls) and the absolute
rates move with that threshold — the ordering does not.

### A claim that did NOT reproduce

A separate analysis pass reported an uncatalogued family
`"another codescout instance is writing to this project"` attributed to concurrent-write
contention under parallel subagent dispatch. **Not reproducible:**
`SELECT COUNT(*) FROM tool_calls WHERE error_msg LIKE '%another codescout instance%'` → `0`,
and a wider `LIKE '%instance%' OR '%locked%' OR '%concurrent%'` sweep returns only the
IL-3 families plus the worktree block above. Recorded here so the next session does not
hunt for it: either it was paraphrased from a different message, or it was a client-side
hook result that never became a `tool_calls` row.

## Hypotheses tried

1. **Hypothesis:** the unclassified head is dominated by a concurrency/write-lock family.
   **Test:** the two `LIKE` queries above. **Verdict:** rejected — zero hits; the head is
   the worktree/activate write block plus librarian API-shape errors.
   **Evidence:** *A claim that did NOT reproduce*.

## Fix

**FIXED 2026-08-20 on `experiments` in `4c7608ee`**
(patch-id `ec94d2846dc69de4f1db40928ba995d53ddcbb42`), at `src/usage/db.rs`.

Thirteen new arms plus three extensions to existing families. **Simulated against the live
NULL rows before committing: 69 of 73 classify (95%).**

| n | family | n | family |
|---:|---|---:|---|
| 23 | `worktree_activate_required` | 3 | `entry_patch_param_misuse` |
| 9 | `entry_collection_missing` | 2 | `json_path_shape_mismatch` |
| 6 | `extra_models_reserved_field` | 2 | `json_path_wrong_buffer_kind` |
| 5 | `params_schema_violation` | 2 | `lsp_still_starting` |
| 5 | `ledger_not_declared` | 1 | `artifact_not_augmented` |
| 4 | `memory_target_not_found` | 1 | `cwd_escapes_root` |
| 3 | `mutually_exclusive_params` | | |

Extensions, not new families: `unknown_enum_value` += `unknown topic `, `invalid at=` (2);
`path_not_found` += `no file to edit at` (1).

**Split where the repair diverges, merged where it does not** — the convention the
2026-08-15 block set. `entry_collection_missing` is *one* family reached from `entry_filter`,
from `append_entry`, and from a retrofit hint, because "declare `entry_collection`" fixes all
three. Against that, the two new `json_path` families stay separate from
`json_path_unsupported` and `json_path_key_miss`: syntax-rejected, key-absent,
wrong-buffer-kind and wrong-value-shape are four different repairs.

**One ordering hazard, pinned rather than left to luck.** `allocate_entry_id: artifact X has
no augmentation` also contains the substring `artifact_not_augmented` matches. The correct
family is `ledger_not_declared` (the repair is to declare the ledger, not merely to augment),
so that arm is placed first and a case in the test fails if the order is ever reversed. It
claims 5 rows; `artifact_not_augmented` correctly keeps 1.

Adding to `ERR_FAMILIES` moves the FNV-1a fingerprint in `PRAGMA user_version`, so the
existing backfill re-classifies history on next `open_db` at no extra cost. That mechanism
was already correct and had simply never been pointed at these messages.

**Residue — 4 rows stay NULL by design**, each a genuine one-off with no second instance to
generalise from: `append_entry: cite \`WIN-27\` did not resolve`; `LSP returned suspicious
range for …`; `leaf must have exactly one field, got 0`; `edits[0]: no headings found in
file`. A residual tail is the healthy state — classifying at n=1 invents families nobody
queries.
## Tests added

`normalize_err_family_maps_the_2026_08_20_unclassified_head` (`src/usage/db.rs`), written
**before** the arms and failing red on the first case. 28 cases: every new family, both
extensions, the ordering guard described under *Fix*, and four **regression guards** proving
the new arms do not steal existing families (`read_markdown_overflow_threshold`,
`json_path_key_miss`, `edit_stale_match`, `il3_pipe_to_trimmer`).

Its doc comment carries the falsification — that this cohort is *not* a friction ranking —
so the next reader cannot infer from "we classified the head" that the head was where the
pain was.

Two pre-existing tests did the real safety work and both pass unchanged:
`err_families_lists_exactly_what_the_classifier_can_emit` pins `ERR_FAMILIES` to the
classifier **in both directions** by reading this file's own source (a new arm cannot ship
without being listed), and `extending_the_taxonomy_moves_the_backfill_fingerprint` confirms
the `user_version` marker moved so history re-classifies.

Gate: `cargo fmt` clean, `cargo clippy --all-targets -- -D warnings` clean, `cargo test`
**4,279 passed / 45 ignored / 0 failed**.
## Workarounds

Group the unclassified population by message prefix, as in *Reproduction*. It is a stable
enough key for triage even though it is not a family.

## Resume

N/A — fixed and archived.

One thing to actually observe rather than assume: the backfill re-classifies history on the
next `open_db`, which has **not been watched happen** on a live DB (the fingerprint change is
unit-tested; the end-to-end re-classification is not). After the next `cargo rb` + `/mcp`
reconnect, re-run `python3 scripts/friction-probe.py --null-detail` and confirm the
unclassified count drops from 73 toward 4. If it does not, the gate is the place to look
(`src/usage/db.rs` `backfill_legacy_rows`), not the classifier.

Two of the newly-named families are now candidates for `refusal_predicate`
(`src/prompts/mod.rs`) — `worktree_activate_required` (23 hits, one clear remedy) most
obviously. Check the repeat rate first: that surface is for gates an agent cannot predict,
and a family whose message already teaches on first read does not need one.
## References

- `src/usage/db.rs:225-330` — `normalize_err_family`
- `src/usage/db.rs:444-483` — `ERR_FAMILIES`; `:485-525` — the fingerprint and backfill
- `src/usage/db.rs:1865` — `normalize_err_family_maps_the_unclassified_head`
- `src/prompts/mod.rs:487-519` — `refusal_predicate`
