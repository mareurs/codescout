---
id: '697ee9464df1d7bf'
kind: bug
status: fixed
title: 'BUG: omnibus — survey/query tools return limit-capped results with no "more exists" signal (silent-cap family)'
owners:
- marius
tags:
- silent-cap
- overflow
- librarian
- find
- gather
- read_file
- audit
- omnibus
topic: null
time_scope: null
fixed: '2026-07-10'
opened: '2026-07-10'
related:
- docs/issues/2026-07-10-toml-yaml-key-false-not-found-past-summary-cap.md
- docs/issues/2026-07-10-read-file-buffer-refs-silently-drop-navigation-params.md
- docs/issues/2026-07-10-subagent-bughunt-omnibus-medium-low-findings.md
severity: high
---

## Summary
Roll-up of the "capped collection/string returned with no overflow/truncation signal"
family, found in this session's (`fc0e9019`) 5-agent read-only sweep of the tool surface
and verified first-hand for the high-severity rows. Companion to the already-fixed members
of this family (`2026-07-07`/`2026-07-09` artifact-get truncation, committed `97a36905`;
`2026-07-10-preview-headings-silent-cap-20.md`, committed `3bccb234`) and to session
`5efbda5f`'s error-handling/Windows omnibus
(`2026-07-10-subagent-bughunt-omnibus-medium-low-findings.md`).

**One anti-pattern, ~15 sites.** The codebase already has the cure — the
`overflow{shown,total,hint}` convention (grep/tree/symbols/references/list_overview) and
`more_in_scope` (workspace_state_at). These are the stragglers that return a capped result
without applying it. Two even carry a field *named* like a signal (`count`/`total`) that
only ever equals the shown count — worse than silence, because it reads as authoritative.

## Symptom (Effect)
Survey/query tools cap results at a `limit`/budget and return them with no indication more
exist. An agent surveying state (triage, ID allocation, tracker refresh) treats a partial
result as complete → wrong triage, missed entries, duplicate IDs, incomplete tracker bodies.
This is the same damage class as the already-fixed artifact-get truncation (which caused 7
duplicate sections to be written from a short-read count).

## Findings

### HIGH (verified first-hand against the code)
| site | capped field | signal? | note |
|---|---|---|---|
| `src/librarian/tools/find.rs` (`call`, resp ~:526) | `items` capped at `limit` (default 50) | ✅ FIXED 46d6781b — `more_in_scope` emitted on capped pages | `build_hints` computes the unbounded `here` scope-total then discards it (only cross-scope deltas surfaced). Most-used query tool. Template: `workspace_state_at.rs`. |
| `src/librarian/tools/gather.rs` `gather_git_log`/`gather_artifacts`/`gather_observations`/`gather_grep` | bare `json!(array)` capped at `limit` | ✅ FIXED 7cb23abc — per-source `truncated` warning via `gather_all` | Feeds `artifact_refresh(gather)`; kept the bare array (consumer contract — see Resume). |
| `src/librarian/tools/timeline.rs` (`call` ~:87) | `Ok(Value::Array(out))` capped at `limit` | ✅ FIXED 33f2f229 — wrapped as `{items, count, truncated}`; infer_shape updated | Worst shape — no object to carry a signal. |
| `src/librarian/tools/context.rs` (~:346) | candidate list truncated to a char budget | ✅ FIXED 1d787c8e — `overflow{candidates,included,omitted,candidates_capped}` + line-truncation marker | Dropped `sorted_ids` beyond `char_cap`; no `total_candidates`/`omitted`. |
| `src/tools/file_summary/file_summary.rs:96` (`summarize_markdown`) | `headings` → 30 | ✅ FIXED e85faba2 — `total_headings`/`total_keys`/`total_sections` + `*_truncated` | Same shape as the fixed preview-headings bug; reached via `read_file` oversized-file summary. |

### MEDIUM
| site | capped field | note |
|---|---|---|
| `src/librarian/tools/context.rs:294` | per-artifact body preview `lines().take(30)` | no "N of M lines" marker distinguishing a 25-line body from a cut 500-line one |
| `src/tools/memory/mod.rs` (`recall` arm) | `results` capped at `limit` (default 5) | no `has_more`/`total`; fix: overfetch `limit+1` |
| `src/librarian/tools/refresh_stale.rs` | `count` = shown | `list_stale` never issues `COUNT(*)`; no true total exists |
| `src/tools/file_summary/file_summary.rs:378,731` | JSON keys→30, flat TOML keys→20 (`.take`) | no `total_keys` |
| `src/librarian/tools/legibility_scan/mod.rs` (`build_dry_run`) | `n` = capped head len | never sees pre-cap `grouped.len()` |
| `src/tools/symbol/edit_code.rs` (`do_rename`) + `src/symbol/edit.rs` (`text_sweep`) | `textual_matches` → 20 files | `textual_match_count`/`_shown` derived from the already-truncated vec; `text_sweep` discards pre-cap total. (Agent-A rogue-fixed then reverted — fix deliberately here.) |

### LOW
| site | note |
|---|---|
| `src/tools/semantic/semantic_search.rs` | `total` = `result_items.len()` — misleading signal-named field, never reveals more |
| `src/librarian/tools/link_scan/resolve.rs` (`Ambiguous`) | candidates capped at 5, no "and N more" (unlike `audit_doc_refs` which does it right) |
| `src/librarian/tools/link_scan/mod.rs` | `artifacts_scanned` bounded by limit, no total (no `count_matching`) |
| `src/librarian/tools/schema_validate.rs:10` | `.iter_errors().take(3)`, no "+K more" |
| `src/librarian/tools/context.rs:179,217` | candidate-discovery caps (50/10) feeding the truncation above |

## Root cause
No missing capability — a missing *convention application*. `OverflowInfo{shown,total,hint}`
(`src/tools/output.rs`) and `more_in_scope` (`workspace_state_at.rs`) are the established
signals; grep/tree/symbols/references/list_overview/audit_doc_refs all use them correctly
(confirmed in the sweep's ruled-out lists). The sites above predate or forgot the convention.

## Fix
Apply the existing convention per site:
- **Result-set limit tools** (find/gather/timeline/memory/refresh_stale/legibility/link_scan):
  compute the unbounded match total (`count_matching`-style) and emit `total` + a
  `more`/`has_more`/`more_in_scope` hint when it exceeds the returned count; for bare-array
  returns, wrap in `{items, total, truncated}`. Rename misleading `count`/`total` fields or
  make them true totals.
  - ⚠ **Check the consumer before wrapping.** `gather` (fixed 7cb23abc) could NOT wrap: `refresh.rs` treats each source's `data` as a `Value::Array` in three places (the `source_key` merge, `commits_since_last`, the `hints` loop), so wrapping would silently drop merged same-key sources. It kept the bare array and emitted a `truncated` warning via the existing `warnings` channel instead. `timeline` (bare `Value::Array`, no wrapper) and `context` need the same consumer check before any shape change.
- **file_summary caps**: emit `total_headings`/`total_keys` + `*_truncated` (mirror the
  `2026-07-10-preview-headings` fix that added `headings::cap`/`stamp_truncation`).
- **context body previews**: append a "… N of M lines" marker.
- **edit_code rename**: thread pre-cap file count out of `text_sweep`; add
  `textual_files_total`/`textual_files_truncated`.
- **Prevent regressions**: consider a test/lint asserting that any response field capped by
  a `limit`/`MAX_*` carries a sibling total-or-`truncated` signal.

## Tests added

**find** ✅ 46d6781b — `more_in_scope_signals_capped_page`, `no_more_in_scope_when_page_holds_everything`.

**gather** ✅ 7cb23abc — `gather_all_warns_when_source_truncated`, `gather_all_no_truncation_warning_under_limit`; updated `gather_grep_limits_results` to assert `truncated`.

Remaining sites: each fix gets a regression test asserting the signal fires exactly when the cap bites (pattern established in `preview/headings.rs` tests).
## Workarounds
Don't trust a survey tool's returned count as the true total for large sets; cross-check
with a scoped/unbounded count or a raw `grep` when the result drives a write.

## Resume

✅ **COMPLETE (2026-07-10, branch `experiments`).** All HIGH/MED/LOW sites fixed, each with a regression test asserting the signal fires exactly when the cap bites. Awaiting `master` merge before archiving (per bug-tracking convention).

Commits: find `46d6781b` · gather `7cb23abc` · timeline `33f2f229` · context `1d787c8e` · file_summary `e85faba2` · memory recall `0d62b2ec` · edit_code/text_sweep `b2345fd1` · MED/LOW tail (schema_validate, semantic_search, refresh_stale, link_scan ×2, legibility) `6f3dfacc`.

⚠ **Consumer-contract lesson (load-bearing):** the **Fix** section's generic "wrap bare-array returns in `{items, total, truncated}`" is NOT safe blindly. `gather` could not wrap (`refresh.rs` merges each source's `data` as a `Value::Array`); it rode a `truncated` warning on the existing warnings channel instead. `timeline` COULD wrap, but only after confirming the CLI formatter's defensive `items` path and fixing `infer_shape` to classify the new shape. Always verify the consumer before changing a response shape.

Signal conventions used, by consumer shape: **hints object** (find `more_in_scope`); **warnings channel** (gather); **wrapped `{items,…,truncated}`** (timeline); **sibling response fields** `overflow`/`has_more`/`*_truncated`/`candidates_total`/`scan_truncated` (context, memory, refresh_stale, link_scan, file_summary, edit_code); **inline message suffix** (schema_validate `(+K more)`).
## References
- `docs/issues/2026-07-10-toml-yaml-key-false-not-found-past-summary-cap.md` — correctness
  standout from this sweep (wrong output, not just incomplete).
- `docs/issues/2026-07-10-read-file-buffer-refs-silently-drop-navigation-params.md` (session
  5efbda5f) — a sibling silent-drop in the same read_file surface.
- `docs/issues/2026-07-10-subagent-bughunt-omnibus-medium-low-findings.md` (session 5efbda5f)
  — parallel omnibus (error-handling + Windows paths); F19/F20 touch the same read_file area.
- Already-fixed family members: `2026-07-07`/`2026-07-09` artifact-get truncation (`97a36905`),
  `2026-07-10-preview-headings-silent-cap-20.md` (`3bccb234`).
