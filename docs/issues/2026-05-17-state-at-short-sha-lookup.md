---
status: fixed
opened: 2026-05-17
closed: 2026-05-17
severity: medium
owner: marius
related: []
tags: ["librarian", "state_at", "short-sha", "lookup", "ambiguity-guard"]
---

# BUG: `state_at(commit=<short-sha>)` failed: lookup used exact match not prefix

## Summary

`artifact(action="state_at", commit="d482ca8a")` (any short SHA) returned `commit d482ca8a not indexed; run librarian_reindex`. The `commits` table IS populated (2931 rows); the full 40-char SHA worked. Root cause: `resolve_cutoff_ts` used exact match, not prefix match. The error message implied the table was empty when really the lookup mode was wrong. Fixed by commit `2f085f45` (2026-05-17): switched to `LIKE ?1 || '%'` with a `LIMIT 2` ambiguity guard.

## Symptom (Effect)

```
artifact(action="state_at", commit="d482ca8a", artifact_id="<any id>")
→ Err: "commit d482ca8a not indexed; run librarian_reindex"

artifact(action="state_at", commit="d482ca8ac91241a7a96a487e46ca394095019912", artifact_id="<same id>")
→ Ok: { "as_of": 1779038809000, ... }
```

## Reproduction

Call `state_at` with any short SHA on this project's catalog post-`librarian(reindex, scope=project)`.

## Environment

- Date observed: 2026-05-17
- Tool: `mcp__codescout__artifact(action="state_at")`
- Component: `src/librarian/tools/state_at.rs::resolve_cutoff_ts`
- Catalog state: 2931 commits indexed (verified via `sqlite3 catalog.db "SELECT COUNT(*) FROM commits"`)

## Root cause

`state_at.rs::resolve_cutoff_ts` used `SELECT authored_at FROM commits WHERE hash = ?1` — exact match. Stored hashes are full 40-char; callers (humans + LLMs) pass short SHAs. The match failed → the misleading `commit not indexed` error fired even though the table was populated.

Note: original session-log diagnosis (F-5 first version) thought the `commits` table was empty. Post-rebuild verification corrected this — table had 2931 rows; the lookup mode was wrong.

## Evidence

- `sqlite3 catalog.db "SELECT COUNT(*) FROM commits"` → 2931 rows.
- `state_at(commit="d482ca8a")` → "not indexed" error.
- `state_at(commit="d482ca8ac91241a7a96a487e46ca394095019912")` → success.
- Session log: `docs/trackers/archive/artifact-code-linkage-session-log.md` F-5 (with corrected diagnosis).

## Hypotheses tried

1. **Hypothesis (original):** `commits` table is empty because `backfill_commits` silently swallows errors. **Test:** Direct SQL count. **Verdict:** Rejected — table had 2931 rows.
2. **Hypothesis (corrected):** Lookup uses exact match on a column where callers pass short SHAs. **Test:** Tried full SHA. **Verdict:** Confirmed — full SHA worked, short SHA failed.
3. **Hypothesis:** Switch to `LIKE ?1 || '%'` for prefix matching. **Verdict:** Confirmed — adopted as the fix. With a `LIMIT 2` ambiguity guard so ambiguous prefixes return an actionable error naming the conflicts. **Evidence link:** see Fix.

## Fix

Fixed by commit `2f085f45` (2026-05-17). `resolve_cutoff_ts` SQL changed from `WHERE hash = ?1` to `WHERE hash LIKE ?1 || '%' LIMIT 2` with three-branch match:

- 0 matches → `not indexed` error
- 1 match → use it
- ≥ 2 matches → ambiguous error naming the conflicting full SHAs

Verified post-rebuild:

- `state_at(commit="d482ca8a", artifact_id=...)` → `as_of: 1779038809000` (success).
- `state_at(commit="d", artifact_id=...)` → `"ambiguous (matches at least dd43... and d9e2...); use a longer prefix or the full 40-char SHA"`.

## Tests added

*Test names not enumerated in commit description; recommend `resolve_cutoff_ts_resolves_short_sha`, `resolve_cutoff_ts_reports_ambiguous_prefix`, `resolve_cutoff_ts_not_indexed_when_no_match`.*

## Workarounds

Pre-fix: pass the full 40-char SHA, or use `timestamp=<unix-ms>` instead.

## Resume

N/A — fixed. After commit `2f085f45` lands on master, move this file to `docs/issues/archive/`.

## References

- Originally tracked as **#8** in `docs/issues/bug-tracker.md` (retired after migration to per-file system).
- Session log: `docs/trackers/archive/artifact-code-linkage-session-log.md` F-5 (with verification trail post-rebuild).
- Fix commit: `2f085f45` on `experiments`.
