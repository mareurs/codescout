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

Implemented 2026-09-02.

1. **Hoisted** `normalize_line_nav_aliases` from `read_file.rs` (private) into
   `src/tools/core/params.rs` as `pub fn`, beside `optional_u64_param`, which it calls.
   `read_file.rs` now imports it; behaviour there is unchanged.
2. **Called** at the top of `ReadMarkdown::call`, before `require_str_param_or_hint` and
   before the heading/headings fork — the same position `ReadFile::call` uses, so the two
   tools normalise at the same point rather than at two points that happen to agree today.

**Step 2's open question is decided: honoured, NOT advertised.** Three reasons, in order of
weight:

- The surface budget has **1 char of headroom**, so advertising (~290 chars) must be funded
  by a cut — and the cut would land in `src/prompts/` and `src/server.rs`, which a peer
  session held at the time (`63083c9e`). Advertising would have converted a four-file,
  contention-free fix into one that collides with an active prompt-surface review.
- `edit_code`'s `file_path` is the standing precedent for unadvertised-but-honoured, and
  this bug's own Fix section names it.
- The defect is the **silent drop**, not the absence of documentation. Honouring closes it;
  advertising is a discoverability improvement that can be funded separately when the budget
  review lands.

The cost is real and worth stating: a caller cannot learn `offset`/`limit` work here by
reading the schema. What they can no longer do is pass them and receive a confident wrong
answer.

`src/server.rs` is untouched — the schema already omitted these params, and it still does.
## Tests added

`src/tools/markdown/tests.rs`, two cases. RED observed first, then green; the full
`tools::markdown` module is 220 passing.

- `read_markdown_honours_offset_and_limit_like_read_file` — `offset=4, limit=2` must return
  lines 4..=5.
- `read_markdown_explicit_start_line_wins_over_the_aliases` — precedence matches `read_file`.

**Only ONE of the two was RED, and the pair inside the first is what caught it.** The
containment assertion (`contains("bravo")`) **passed against the unfixed code**, because a
dropped alias returns the whole file and the whole file contains the slice — monotone under
widening, exactly the shape `CLAUDE.md` § *Testing Discipline* names. The failure came from
the paired exclusion assertion (`!contains("alpha") && !contains("delta")`), which is the
discriminating half. Written that way deliberately; had it been containment-only it would
have shipped green and guarded nothing.

**The precedence test passed BEFORE the fix and is honest about why.** With the aliases
dropped entirely, `start_line`/`end_line` win by default, so the expected outcome occurred
for the wrong reason. It becomes load-bearing only *after* the normaliser exists — at which
point it guards the branch where aliases could override an explicit value. Recorded rather
than quietly counted as coverage: it was inert at the moment it was written.

The five existing `normalize_line_nav_aliases_*` unit tests stay in `read_file.rs` and now
exercise the hoisted `pub fn`. They were **not** moved to `src/tools/core/tests.rs`, which a
peer held at the time; keeping them where they are costs nothing and avoided the collision.
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
