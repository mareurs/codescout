---
status: open
opened: 2026-08-20
closed:
severity: low
owner: marius
related:
  - docs/issues/2026-08-20-friction-target-omits-command-and-file-path.md
tags:
  - usage-db
  - err-family
  - taxonomy
kind: bug
---

# BUG: the largest unclassified error message — the worktree/activate write block — has no err_family, and unclassified is where friction concentrates

## Summary

`normalize_err_family` leaves 36 recent errors unclassified, and the single largest
unclassified message (22 occurrences) is
`Write blocked: git worktrees detected but workspace(action='activate') has not been called`.
This matters more than a missing label usually would: a detector-validation pass measured
`err_family IS NULL` as the **highest-friction** signal in the corpus — 53.2% friction rate
against a 27.0% base rate, a 1.97× lift — because a named family is evidence someone
already wrote a teaching hint for that error, and NULL is the complement of the teaching
effort. The unclassified head is therefore the priority queue for gate authoring, and its
largest member is currently invisible to any family-grouped query.

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

### Why NULL is the interesting bucket

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

Not yet implemented. Add family probes to `normalize_err_family` for at least the top
entries, in volume order. Candidate names, matching the existing convention:

- `worktree_activate_required` (22) — the write guard demanding `workspace(activate)`
- `extra_models_reserved_field` (6)
- `entry_filter_without_collection` (4 + 4 — two distinct messages, same cause; decide
  whether they collapse into one family)
- `entry_violates_params_schema` (3 + 1)
- `read_markdown_exclusive_params` (3)
- `ledger_missing_entry_prefix` (3)
- `update_entry_wrong_param` (2 + 1)
- `lsp_still_starting` (2)
- `unknown_guide_topic` (1)

Adding to `ERR_FAMILIES` changes the fingerprint, so the existing backfill re-classifies
history on next open at no extra cost. The highest-volume ones are also candidates for
`refusal_predicate` entries (`src/prompts/mod.rs:487-519`) if they turn out to recur — but
check first: that surface is for gates an agent cannot predict, and several of these are
one-shot API-shape mistakes whose own message already says the right thing.

Record the fix SHA **and** its patch-id (`git show <sha> | git patch-id --stable`).

## Tests added

None yet. `normalize_err_family_maps_the_unclassified_head` (`src/usage/db.rs:1865`)
already exists as the home for exactly this kind of addition — extend its `cases` array
rather than adding a new test.

## Workarounds

Group the unclassified population by message prefix, as in *Reproduction*. It is a stable
enough key for triage even though it is not a family.

## Resume

Add probes to `normalize_err_family` (`src/usage/db.rs:225`) starting with
`worktree_activate_required`, add the same names to `ERR_FAMILIES`, and extend
`normalize_err_family_maps_the_unclassified_head` (`src/usage/db.rs:1865`). Then re-run the
*Reproduction* query to confirm the head has moved, and re-check the NULL friction rate —
if it stays near 2× lift after classification, the signal is about *newness* rather than
about these specific messages, which is worth knowing before wiring it to anything.

## References

- `src/usage/db.rs:225-330` — `normalize_err_family`
- `src/usage/db.rs:444-483` — `ERR_FAMILIES`; `:485-525` — the fingerprint and backfill
- `src/usage/db.rs:1865` — `normalize_err_family_maps_the_unclassified_head`
- `src/prompts/mod.rs:487-519` — `refusal_predicate`
