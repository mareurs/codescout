---
status: open
opened: 2026-09-02
closed:
severity: medium
owner: marius
related:
  - docs/issues/archive/2026-06-14-read-file-offset-limit-silently-ignored-on-buffers.md
tags:
  - cluster/accepted-parameter-silently-dropped
kind: bug
---

# BUG: `read_markdown` silently ignores `offset` / `limit`, the aliases `read_file` learned to honour

## Summary

`read_file` accepts native-`Read`-style `offset`/`limit` as line-range aliases and
normalises them to `start_line`/`end_line` before doing anything else. `read_markdown` —
the tool `read_file` itself redirects every `.md` read to — does not. A call carrying
`offset`/`limit` returns the whole heading map with no error and no `corrections` note, so
the caller receives a plausible answer to a question they did not ask. Same defect, same
fix shape, one tool over; the 2026-06-14 fix stopped at `read_file`.

## Symptom (Effect)

Live, 2026-09-02, against the running MCP server:

```
read_markdown(path="docs/PROBES.md", offset=5, limit=3)
→ 213 lines  @file_5f916f80
  # PROBES — the measurement instruments in this repo  L8
    ## What counts as a probe  L18
    …   (full heading map; no error, no corrections field)
```

Expected either lines 5–7 (what `read_file` returns for the same params) or a refusal that
names `start_line`/`end_line`.

Field instances, `.codescout/usage.db`, 30-day window ending 2026-09-02:

```
sqlite3 .codescout/usage.db "SELECT called_at, input_json, error_msg FROM tool_calls
  WHERE tool_name='read_markdown' AND (input_json LIKE '%\"offset\"%' OR input_json LIKE '%\"limit\"%')"
2026-08-25 | {"path":"docs/trackers/bug-fix-session-log.md","offset":"1","limit":"15"}   | (no error)
2026-08-25 | {"path":"docs/trackers/bug-fix-session-log.md","offset":"100","limit":"16"} | (no error)
```

Two calls, both `success`, both returned the heading map of a 4,000-line ledger instead of
the 15-line slice asked for. The count is a floor: usage.db is retention-swept at 30 days.

## Reproduction

`git rev-parse HEAD` → `4dc0daa2` (`experiments`). Any MCP client:

```
read_markdown(path="docs/PROBES.md", offset=5, limit=3)
```

Observe the heading map. Compare `read_file(path="docs/PROBES.md", offset=5, limit=3)`
if you first bypass the `.md` redirect — `read_file` slices correctly.

## Environment

Linux, codescout `experiments` @ `4dc0daa2`, Claude Code over stdio. Not environment-dependent.

## Root cause

`src/tools/markdown/read_markdown.rs:537-538` reads only `start_line` / `end_line`:

```rust
let start_line = optional_u64_param(&input, "start_line");
let end_line = optional_u64_param(&input, "end_line");
```

The alias normaliser is `normalize_line_nav_aliases` at `src/tools/read_file.rs:466`,
called at `src/tools/read_file.rs:55` as the first thing `ReadFile::call` does. It is a
private `fn` of that module; `read_markdown.rs` neither calls it nor checks for unknown
params, so `offset`/`limit` fall through `serde_json::Value` lookups untouched.

Measured 2026-09-02: the live call above, plus `grep "offset"|"limit"|normalize_line_nav_aliases
src/tools/markdown/read_markdown.rs` → 0 matches.

Why the two tools diverge: the 2026-06-14 fix
(`docs/issues/archive/2026-06-14-read-file-offset-limit-silently-ignored-on-buffers.md`)
measured 191 `read_file` calls with native-Read intent and chose *make it work* over
*reject*. Its scope was `read_file`; `read_markdown` was not examined, and the Iron Law 4
redirect (`read_file` → `read_markdown` for `.md`) means an agent arriving with native-Read
habits lands on exactly the tool that never learned the aliases.

## Evidence

### Live call (2026-09-02)
Recorded verbatim under *Symptom*. Session scratchpad
`/tmp/claude-1000/-home-marius-work-claude-codescout/2cb44cd3-8673-4604-a8ac-5adea75ca54b/`.

### usage.db (2026-09-02)
Query and two rows under *Symptom*. Both rows have `error_msg` NULL.

### Source
`read_markdown.rs:537-538` (reads start/end only); `read_file.rs:55` and `:466`
(normaliser, module-private); `read_file.rs:2099` `normalize_line_nav_aliases_maps_offset_and_limit`
(the existing test, `read_file`-only).

## Hypotheses tried

1. **Hypothesis:** `read_markdown` rejects unknown params. **Test:** live call above.
   **Verdict:** rejected — no error, heading map returned.
2. **Hypothesis:** the aliases are normalised centrally in `call_content` before dispatch.
   **Test:** grep for `normalize_line_nav_aliases` callers. **Verdict:** rejected — one
   caller, `ReadFile::call`.

## Fix

Plan, not implemented:

1. Hoist `normalize_line_nav_aliases` out of `read_file.rs` into `src/tools/core/params.rs`
   (where the alias-aware helper family already lives) and call it at the top of
   `ReadMarkdown::call`, before the heading/headings fork.
2. Decide whether to **advertise** `offset`/`limit` on `read_markdown` as `read_file` does
   (~290 chars; `TOOL_SURFACE_CHAR_BUDGET` has 1 char of headroom, so this must be funded by
   a cut — see the 2026-09-02 review in `docs/trackers/prompt-surface-compaction-session-log.md`)
   or leave them unadvertised-but-honoured, as `file_path` is on `edit_code`. Either is
   defensible; silently dropping them is not.

## Tests added

None yet. Owed: a `read_markdown` twin of `normalize_line_nav_aliases_maps_offset_and_limit`
asserting that `offset=5, limit=3` returns lines 5–7 of a fixture, and — because the
positive test is monotone under widening — a second asserting the response carries **no**
heading map when a line range was requested. Mutation: delete the new `normalize_…` call and
both must fail.

## Workarounds

Pass `start_line` / `end_line`. They work today.

## Resume

Implement step 1 of *Fix*; run `cargo test --lib read_markdown` and the two new tests; then
decide step 2 against the surface budget.

## References

- `docs/issues/archive/2026-06-14-read-file-offset-limit-silently-ignored-on-buffers.md` — the
  same defect on `read_file`, and the telemetry that chose *make it work*.
- `docs/trackers/prompt-surface-compaction-session-log.md`, 2026-09-02 review section — where
  this was found, with the full-surface alias inventory.
- `docs/trackers/issue-clusters.md` `IC-15`.
