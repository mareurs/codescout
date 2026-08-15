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

`inferred from the parameter's own contract — not yet traced to the truncation site.` `limit` is
documented as "Max matching lines" and is applied as a row count. Nothing in the path bounds the
*bytes* of each emitted line, so total output is `limit × (unbounded line length)`. For source code
the two are correlated closely enough that the proxy holds; for minified JSON, bundled JS, CSV, or
any generated single-line file, the correlation collapses entirely.

`measured 2026-08-16: SELECT overflow_tokens, input_json FROM tool_calls WHERE tool_name='grep'
ORDER BY overflow_tokens DESC` → top row 4,427,639 tokens under `limit: 40`.

Note this is *not* the same failure as an unbounded pipe (IL3): the guard for that is about shell
composition, and this call was a well-formed `grep` tool invocation with an explicit limit set. The
agent did the documented thing and still got 4.4M tokens.

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

Locate grep's result-assembly site and confirm where line collection happens relative to any byte
accounting (the root cause above is inferred from the parameter contract, not yet read at the
bytes — verify before designing the cap). Then add the per-line and total-byte budgets.

## References

- `docs/trackers/2026-08-15-tool-usage-investigation.md` § History → 2026-08-16, *Overflow*.
- `get_guide("progressive-disclosure")` — the existing budget constants this should honour.
