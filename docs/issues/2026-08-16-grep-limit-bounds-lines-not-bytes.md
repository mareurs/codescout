---
kind: bug
status: fixed
tags:
- grep
- progressive-disclosure
- overflow
- unbounded-output
closed: null
opened: 2026-08-16
owner: marius
related:
- docs/trackers/2026-08-15-tool-usage-investigation.md
severity: medium
---

# BUG: grep's `limit` bounds matching lines, not output size — one call buffered 4.4M tokens under `limit: 40`

## Summary

`grep`'s `limit` parameter caps the number of matching **lines**, which is only a proxy for output
size and fails completely on files with very long lines. A single call with `limit: 40` over a
`*.json` glob buffered **4,427,639 tokens** — minified JSON is one line per file, so a 40-line cap
admitted megabytes. `grep` accounts for **68% of all overflow tokens** in the live corpus on a
merely 3.0% overflow rate: the rate is low, the blast radius per incident is not.

## Symptom (Effect)

No error. The call succeeds and the result is buffered, so the only visible sign is an
`overflow_tokens` value four orders of magnitude above the inline budget. From
`.codescout/usage.db` (live DBs, measured 2026-08-16):

```
overflow_tokens | input_json
4427639 | {"pattern":"61\\.1|\"137\"|: 137|32 ?KB|per_call","path":"/home/marius/.local/share/provenance-probe","glob":["*.py","*.json"],"limit":40}
1091528 | {"pattern":"shell_output_limit|SHELL_OUTPUT_LIMIT","include_hidden":true}
  10185 | {"pattern":"R-(5[4-9]|6[0-9]|7[0-9])","glob":"docs/trackers/reconnaissance-patterns.md","mode":"content"}
```

The drop from 4.4M to 10K by the third row shows how concentrated this is — it is not a broad
distribution, it is a small number of catastrophic calls.

## Reproduction

```bash
# Any directory containing a minified .json file (one long line).
grep(pattern="<something that matches>", glob="*.json", limit=40)
```

The call returns an overflow envelope; `overflow_tokens` on the resulting `tool_calls` row is
bounded only by the byte length of the matched lines, not by `limit`.

## Environment

Linux, codescout `v0.15.0`, branch `experiments`, MCP stdio. Measured over the merged live-DB
corpus (21,638 calls, `user_version >= 2`).

## Root cause

**Confirmed at the bytes 2026-08-16** (the entry below previously read *inferred from the
parameter's own contract*; it is now read, and the mechanism is exactly as inferred).

`limit` is a **row count** from schema to enforcement, and no stage measures the size of what a
row contains:

- `src/tools/grep.rs:36` — schema: `"limit": { "type": "integer", "default": 50, "description":
  "Max matching lines" }`
- `src/tools/grep.rs:81` — `let max = optional_u64_param(&input, "limit").unwrap_or(50) as usize;`
- `src/tools/grep.rs:207` — `if matches.len() >= max` — the cap, on the **count of matches**
- `src/tools/grep.rs:322-323` — `let budget = max;` then `cap_grouped(matches, budget)`

That penultimate line is the defect in miniature: **the variable is named `budget` while holding a
row count.** A budget bounds a resource; this bounds cardinality. Total emitted output is therefore
`max × (unbounded line length)`.

The only size-like limits anywhere in the file bound something else entirely:

- `src/tools/grep.rs:590-591` — `.size_limit(1 << 20)` / `.dfa_size_limit(1 << 20)` bound the
  **compiled regex**, not the output;
- `src/tools/grep.rs:147` — `bytes.iter().take(8192)` is the binary-detection peek, not a read cap;
  `src/tools/grep.rs:143` reads the whole file (`std::fs::read`) regardless.

`grep_in_buffer` carries the identical shape independently — `src/tools/grep.rs:652` (`max` from
`limit`) and `:708` (`if matches.len() >= max`) — so the buffer-search path needs the same fix, not
just the filesystem path.

`measured 2026-08-16: SELECT overflow_tokens, input_json FROM tool_calls WHERE tool_name='grep'
ORDER BY overflow_tokens DESC` → top row 4,427,639 tokens under `limit: 40`.

Note this is *not* the IL3 unbounded-pipe failure: that guard is about shell composition, and this
was a well-formed `grep` tool invocation with an explicit limit set. The agent did the documented
thing and still got 4.4M tokens.
## Evidence

### Overflow tokens by tool (live DBs, 2026-08-16)

```
tool           calls   ovf   pct    tokens      max_single
grep            2285    68   3.0%   5,775,117   4,427,639
symbols         2933   228   7.8%   1,018,637      27,167
artifact        1309   104   7.9%     603,556      26,127
librarian        146    58  39.7%     537,349      45,088
run_command     7238   714   9.9%     183,259      44,600
```

`grep` is fourth by overflow *count* and first by overflow *tokens* by a factor of 5.7×. Ranking
overflow by call count — as TU-10 did — hides this completely.

## Hypotheses tried

1. **Hypothesis:** the 4.4M call was an unbounded search with no limit set.
   **Test:** read `input_json` for the row.
   **Verdict:** **rejected.** `limit: 40` was explicitly set. The parameter was used correctly and
   did not bound the output.
   **Evidence:** § Symptom, first row.

## Fix

**Implemented 2026-08-16.** Two bounds, because the per-match clamp and the aggregate cap catch
different shapes:

| Bound | Value | Catches |
|---|---|---|
| `MAX_MATCH_BYTES` | 2,000 B per emitted match | one pathological line (the minified-JSON case) |
| `MAX_TOTAL_MATCH_BYTES` | 60,000 B summed | many individually-reasonable matches |

`clamp_match()` walks back to a UTF-8 char boundary before cutting and appends
`… [truncated: N of M bytes shown]`. **The marker is the fix, not decoration** — a silently
truncated result reads as complete, which is the same defect class as the buffered-summary bug, so a
caller who needs the rest has to be able to see that there is a rest.

**Applied at all six emission sites**, three per call site — simple mode, context-mode mid-block, and
context-mode final block — in both `Grep::call` and `grep_in_buffer`. The bug predicted two call
sites; each turned out to have three emission points. A fix at the obvious one would have left the
context-mode paths unbounded.

**Renamed `budget` → `max_matches`.** The variable held a row count while being named for a size
bound, which is exactly the confusion that let a caller set `limit: 40` and receive 4.4M tokens.
Naming a proxy after the thing it proxies is how the proxy stops being questioned.

**The overflow envelope now says which cap fired** — `reason: "byte budget"` plus
`truncated_bytes: true` when the aggregate cap stopped collection. "40 of 900 matches" and "stopped
at 60KB" call for different next moves.

One deliberate asymmetry: the aggregate cap is enforced in `Grep::call` but not at
`grep_in_buffer`'s final in-flight block, where nothing reads the running total afterwards — the
compiler flagged the dead accumulation. The per-match clamp still applies there, which is the part
that bounds the payload.
## Tests added

In `src/tools/grep.rs`:

| Test | Mutation it catches |
|---|---|
| `grep_bounds_output_bytes_not_only_matching_lines` | removing either budget — restores unbounded output under a correctly-set `limit` |
| `grep_marks_a_clamped_line_instead_of_silently_cutting` | dropping the marker — a cut result then reads as complete |

The first reproduces production shape directly: five 200KB single-line `*.json` files under
`limit: 40`. **Pre-fix it emitted 1,000,527 bytes**; post-fix it is bounded well under 64KB. That is
the 4.4M-token incident in miniature, now a regression test.

Gate: **3836 passed, 0 failed**, `cargo clippy --all-targets -- -D warnings` clean.
## Workarounds

- Pass `mode="files"` first to see where matches are before pulling content.
- Exclude generated/minified globs (`*.json`, `*.min.js`, `*.csv`) from content greps, or narrow
  `path` to a source subtree.
- After an overflow, query the buffer with a bounded reader rather than re-running the grep.

## Resume

Fixed, tested, gate green on `experiments`. **Two things before archiving:**

1. **Verify live after the next `cargo rb` + `/mcp`** — this session's pattern, and it has earned its
   keep twice: a green suite and a working tool are different claims. Re-run the original shape
   (`grep` over a `*.json` glob with a small `limit`) against the running server and confirm the
   emitted payload is bounded and marked.
2. **Record the fix SHA and archive** via `artifact(action="move", …)` — never a bare `git mv`.
   Check the promotion path first (`git rev-list --left-right --count master...experiments`): a `0`
   on the left means fast-forward, in which case the `experiments` SHA already IS the master SHA and
   **no pending-master-SHA line should be written**.

Acceptance signal for a later corpus pass: `grep`'s share of buffered tokens should fall sharply
from 68% (5,775,117 of 8,451,310), and no single call should exceed ~60KB. Date-bound any
re-measurement to after the rebuild.
## References

- `docs/trackers/2026-08-15-tool-usage-investigation.md` § History → 2026-08-16, *Overflow*.
- `get_guide("progressive-disclosure")` — the existing budget constants this should honour.
