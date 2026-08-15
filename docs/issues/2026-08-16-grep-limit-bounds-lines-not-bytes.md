---
status: open
opened: 2026-08-16
closed:
severity: medium
owner: marius
related:
  - docs/trackers/2026-08-15-tool-usage-investigation.md
tags:
  - grep
  - progressive-disclosure
  - overflow
  - unbounded-output
kind: bug
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

**Plan.** Add a byte budget alongside the line budget, and truncate on whichever binds first:

- cap each emitted line at N bytes with an explicit elision marker, so a single pathological line
  cannot dominate;
- cap total emitted bytes independently of `limit`, matching the `INLINE_BYTE_BUDGET` /
  `TOOL_OUTPUT_BUFFER_THRESHOLD` constants documented in `get_guide("progressive-disclosure")`;
- surface which cap fired in the response, so the caller can tell "40 matches, all shown" from
  "40 matches, each truncated at 2KB".

The third point matters for the same reason as the filed `read_file` incompleteness bug: a result
that was silently cut reads as complete.

**Consider:** `mode="files"` already exists as the tame path for broad searches. The hint on an
oversized result could name it.

## Tests added

`N/A — not yet fixed.` A regression test is straightforward and should assert on bytes, not lines:
grep a fixture containing one very long matching line with `limit` well above 1, and assert the
emitted output is bounded and marked truncated.

## Workarounds

- Pass `mode="files"` first to see where matches are before pulling content.
- Exclude generated/minified globs (`*.json`, `*.min.js`, `*.csv`) from content greps, or narrow
  `path` to a source subtree.
- After an overflow, query the buffer with a bounded reader rather than re-running the grep.

## Resume

Root cause is now read at the bytes (§ Root cause) — the verification step this section previously
called for is **done**, and the inference held.

Next: add the byte budget. Two call sites need it, not one — `Grep::call`
(`src/tools/grep.rs:207`, `:322`) and `grep_in_buffer` (`src/tools/grep.rs:708`) cap independently
and would otherwise diverge. Suggested order:

1. Per-line byte cap with an explicit elision marker, so one pathological line cannot dominate.
2. Total emitted-byte cap independent of `limit`, against the `INLINE_BYTE_BUDGET` /
   `TOOL_OUTPUT_BUFFER_THRESHOLD` constants in `get_guide("progressive-disclosure")`.
3. Report which cap fired, so "40 matches, all shown" is distinguishable from "40 matches, each
   truncated."

Regression test asserts on **bytes, not lines**: grep a fixture with one very long matching line at
`limit` well above 1, assert output is bounded and marked truncated. Rename `budget`
(`src/tools/grep.rs:322`) while you are there — it is the name that made the confusion natural.
## References

- `docs/trackers/2026-08-15-tool-usage-investigation.md` § History → 2026-08-16, *Overflow*.
- `get_guide("progressive-disclosure")` — the existing budget constants this should honour.
