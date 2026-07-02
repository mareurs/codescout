---
status: open
opened: 2026-07-02
closed:
severity: high
owner: marius
related: []
tags: [benchmark, mcp-tools, progressive-disclosure, read_file]
kind: bug
---

# BUG: `scripts/run-tc-benchmark.py` silently scores buffered semantic_search results as zero

## Summary
The Task-10 lite-vs-hybrid retrieval benchmark harness (`scripts/run-tc-benchmark.py`)
assumes `read_file` returns a JSON-encoded pagination envelope when resolving a
buffered (`@tool_*`) `semantic_search` result, but `read_file` actually returns a
human-readable **text** rendering (`"{N} lines\n\n{content}"`). Every test case whose
`semantic_search` response crosses the ~10 KB buffering threshold silently scores
`0` with `top10_files: []` — with zero warnings on stdout or stderr — regardless of
whether the underlying retrieval actually found relevant results. This confounds any
benchmark run that uses `mode=full` (the harness default) and inflates the apparent
quality gap between backends whose responses buffer at different rates.

## Symptom (Effect)
During a Task-10 benchmark run (hybrid arm), 17 of 25 test cases recorded
`"score": 0, "top10_files": []` in the emitted JSON, with matching stderr progress
lines showing no exception (`TC-07 score=0/3   1842ms`) — i.e. the harness believed
these were genuine zero-result queries. No `[WARN] ... failed: ...` line was ever
printed for any of them (that path is guarded by a different try/except — see Root
cause).

Manually replaying one of these "empty" queries (TC-07:
`"parse_all_headings compute_section_end heading boundary markdown"`) against the
same project via a fresh MCP session returned a real, relevant, non-empty result set
that included **both** of TC-07's expected files
(`src/tools/markdown/edit_markdown.rs`, `src/tools/file_summary/file_summary.rs`) —
proving the recorded emptiness was a measurement artifact, not a genuine retrieval
miss.

## Reproduction
Commit: `cb8e77c6e38e20bd9303bc329f5bd696f03edd85` (branch `experiments`).

1. Build: `cargo build --release` (produces `target/release/codescout`).
2. Run any TC whose `semantic_search` result exceeds ~10 KB buffered
   (`mode=full`, `limit=10` — the harness defaults — reliably crosses this for
   TCs with multi-line code/doc chunks):
   ```
   ./scripts/run-tc-benchmark.sh > /tmp/out.json
   ```
   Inspect `/tmp/out.json` — most `results[].top10_files: []` entries correspond to
   responses that were large enough to buffer.
3. Isolate with a raw MCP probe: spawn `codescout start`, `workspace(activate)` the
   same project, call `semantic_search` with the same query/limit/mode, observe the
   response contains `"output_id": "@tool_xxxx"` (buffered) rather than inline
   `"results"`. Then call `read_file(path="@tool_xxxx", start_line=1, end_line=100)`
   and observe the returned `content[0].text` is:
   ```
   75 lines

   {
     "results": [
       ...
   ```
   i.e. a human-readable header line (`"{N} lines\n\n"`) followed by the sliced
   content — not a JSON object. `json.loads()` on this text raises
   `json.decoder.JSONDecodeError: Extra data: line 1 column 4 (char 3)` (the parser
   accepts `75` as a bare JSON number, then chokes on the trailing ` lines\n\n{...`).

## Environment
- OS: Linux (dev workstation), Rust MCP server over stdio JSON-RPC transport
  (`codescout start`).
- Project under benchmark: `.worktrees/bench` (project id `code-explorer`).
- Harness: `scripts/run-tc-benchmark.py` (Python 3, stdlib `json`/`subprocess` only).
- Reproduced against both the Qdrant-backed hybrid arm and the no-sparse arm; the
  sqlite-vec ("lite") arm is far less affected because its responses less often
  cross the buffering threshold (see Evidence).

## Root cause
Two independent, compounding mismatches between what
`scripts/run-tc-benchmark.py`'s `McpClient.semantic_search()` expects and what
`read_file` actually emits over the wire:

1. **Format mismatch.** `scripts/run-tc-benchmark.py:301-336` treats
   `content[0]["text"]` from every tool response — including the follow-up
   `read_file` pagination calls — as a raw JSON string:
   ```python
   text = content[0].get("text", "{}")
   try:
       data = json.loads(text)
   except json.JSONDecodeError:
       return []                              # line 301-305
   if "output_id" in data:
       ...
       envelope = json.loads(buf_content[0].get("text", "{}"))   # line 321
       ...
   ```
   For the *first* `semantic_search` call this happens to work, because
   codescout's tool-output envelope (`{"output_id": ..., "summary": ..., "hint": ...}`)
   really is JSON. But the harness's own follow-up `read_file(path=output_id,
   start_line=N, end_line=M)` call does **not** get JSON back: `read_file`'s
   human-facing text renderer, `format_read_file()` in `src/tools/read_file.rs:773-802`,
   turns the internal structured value (`content`, `total_lines`, `shown_lines`,
   `complete`, `next`) into a formatted string:
   ```rust
   // src/tools/read_file.rs:789
   let mut out = format!("{total} lines\n\n");
   out.push_str(content);
   ```
   i.e. `"{N} lines\n\n{sliced_content}"`. That string is what lands in
   `content[0]["text"]` of the MCP response — not the structured JSON the harness
   assumes.

2. **Silent swallowing.** Both parse attempts are wrapped in bare
   `except json.JSONDecodeError` blocks that discard the error instead of
   surfacing it:
   - `scripts/run-tc-benchmark.py:320-323` — `break`s the pagination loop on
     failure, leaving `raw_parts` empty.
   - `scripts/run-tc-benchmark.py:332-335` — `json.loads("".join(raw_parts))` on an
     empty list is `json.loads("")`, which also raises `JSONDecodeError`, caught and
     turned into `return []`.

   Because the exception never escapes `semantic_search()`, the outer
   `try/except Exception` in `main()` (`scripts/run-tc-benchmark.py:436-440`, which
   *would* print `[WARN] {tc['id']} failed: {exc}`) never fires. The TC is scored
   exactly like a genuine zero-hit query: `score=0`, `top10_files=[]`, no diagnostic
   output anywhere.

The harness's assumed field names (`content`, `shown_lines`, `complete`) do exist in
codescout's *internal* `Value` representation for a paginated read (visible in
`src/tools/read_file.rs:783-786` and exercised by
`read_file_buffer_ref_range_auto_chunks` in `src/tools/edit_file/tests.rs:526`) — but
that structured value is rendered to human-readable text before being placed in the
MCP `content[0].text` field for the default (non-`json_path`) response mode. The
harness needed the structured form and never asked for it.

**This is not a new regression** — checked via `git log -S'shown_lines' --
src/tools/read_file.rs`, which shows the buffered/paginated `shown_lines` shape (and,
per the commit message of `40c4b828` on 2026-05-21, its rendering "on the buffered
(large-byte) axis" through the same compact-text path) has been in place since commit
`b2ddef88` (2026-04-24, `refactor(tools/read_file): extract ReadFile tool`) —
predating this repo's own `docs/research/2026-05-06-retrieval-stack-benchmark.md`,
whose "Known Issues" §3 claims *"Benchmark `semantic_search` now paginates `read_file`
to reconstruct the full result"* as an already-applied fix. `scripts/run-tc-benchmark.py`
was last touched on 2026-05-16 (`bc0e7fe8`), five days *before* `40c4b828`, so that
later commit is not the trigger either — the mismatch on the buffered-pagination path
appears to have been present (unverified) since the reconstruction loop was first
written. Left unresolved: whether the 2026-05-06 report's claimed fix was ever
byte-for-byte verified against a real multi-page buffered response, or whether that
run's actual payloads happened to stay small enough to avoid exercising this exact
path. Flagging as an open discrepancy rather than asserting which.

## Evidence

### E1 — hybrid arm (Arm A) emitted JSON
`/tmp/claude-1000/.../scratchpad/hybrid.json` (Task-10 run, this session): 17 of 25
TCs have `"top10_files": []`, `"score": 0`. Aggregate `{total: 9, max: 75,
p50_latency_ms: 1863.6, p95_latency_ms: 2629.7}`. Matching stderr log
(`hybrid.stderr.log`) contains only `TC-NN score=X/3  YYYYms` lines for all 25 TCs —
zero `WARN`/`Error` lines.

### E2 — no-sparse arm (Arm B) emitted JSON
`/tmp/claude-1000/.../scratchpad/no-sparse.json`: 8 of 25 TCs empty. Aggregate
`{total: 26, max: 75, p50_latency_ms: 1021.7, p95_latency_ms: 1455.1}`.

### E3 — lite arm (Arm C) emitted JSON
`/tmp/claude-1000/.../scratchpad/lite.json`: only 1 of 25 TCs (TC-06) empty.
Aggregate `{total: 31, max: 75, p50_latency_ms: 72.3, p95_latency_ms: 116.6}`. The
sqlite-vec backend skips reranking and returns smaller merged result payloads, so
far fewer of its responses cross the 10 KB buffering threshold — consistent with
this bug affecting arms in proportion to how often their responses buffer, not in
proportion to actual retrieval quality.

### E4 — direct raw-protocol reproduction (TC-07)
Standalone probe script spawning `codescout start` directly (bypassing the harness),
`workspace(activate)` on `.worktrees/bench`, then `semantic_search` with TC-07's
exact query. Raw response:
```
{output_id: "@tool_22d6d93f", summary: "10 results\n\n  docs/archive/bug-reports/...tool-misbehaviors.md:19-24 ...\n  src/tools/markdown/edit_markdown.rs:226-254 ...\n  ...\n  src/tools/file_summary/file_summary.rs:286-319 ...", buffered_bytes: 13152, ...}
```
Both of TC-07's expected files appear in the top handful of hits — this query would
have scored non-zero had the harness parsed the buffered result correctly. Yet
`hybrid.json` records `score: 0, top10_files: []` for TC-07.

### E5 — reproduced the exact parse failure (TC-08 query)
Second probe replicated the harness's pagination loop verbatim against a live
`@tool_*` buffer (query: `"embedding dimension mismatch vec0 schema migration"`):
```
--- read_file page 0: 1 content block(s) ---
raw text (first 1000 chars): 75 lines

{
  "results": [
    ...
JSONDecodeError parsing envelope: Extra data: line 1 column 4 (char 3) -> harness would break here
...
FINAL JSONDecodeError: Expecting value: line 1 column 1 (char 0) -> this is why top10_files ends up []
```
This is a direct, line-for-line reproduction of the harness's own failure path.

## Hypotheses tried
1. **Hypothesis:** Qdrant `code_chunks` collection staleness (orphaned points from
   the pre-rename `.worktrees/bench` content) is depressing hybrid scores.
   **Test:** `codescout index --project .worktrees/bench --force`; observed
   `added=24923 deleted=16180`, confirming genuine staleness; re-ran Arm A.
   **Verdict:** rejected as the sole/primary cause — score got *worse* after the
   reindex (11→9/75) and latency increased, so staleness cleanup did not explain the
   pattern. **Evidence link:** run logs from this session (not preserved as a
   separate artifact).
2. **Hypothesis:** `CODESCOUT_RERANKER_PROTOCOL` mismatch (ambient env `infinity` vs
   `.env.amd`'s `llama-server`) breaks reranking.
   **Test:** read `src/retrieval/reranker.rs`'s `Protocol::from_env()` match arm.
   **Verdict:** rejected — both strings map to the identical `Protocol::Infinity`
   variant; no behavioral difference. **Evidence link:** `src/retrieval/reranker.rs`
   (grep for `from_env`).
3. **Hypothesis:** the empty `top10_files` entries are genuine zero-hit results
   (i.e. hybrid retrieval really found nothing relevant for those queries).
   **Test:** manually replayed TC-07's exact query via a fresh raw MCP probe
   (bypassing the harness entirely).
   **Verdict:** rejected — the probe returned both of TC-07's expected files in the
   top results. **Evidence link:** Evidence E4 above.
4. **Hypothesis:** the harness's `read_file` pagination-reconstruction path silently
   fails whenever `semantic_search`'s response is buffered, independent of query
   content.
   **Test:** replicated the harness's exact `read_file` pagination loop
   line-for-line against a live buffer from a different query (TC-08).
   **Verdict:** confirmed — reproduced the identical `JSONDecodeError` path the
   harness's own `except` blocks silently swallow. **Evidence link:** Evidence E5
   above; root cause section.

## Fix
Not implemented (out of scope for Task 10, which is docs-only). Plan for whoever
picks this up:

- In `scripts/run-tc-benchmark.py`, either:
  (a) stop assuming `read_file`'s paginated response is JSON — strip the
      `"{N} lines\n\n"` header line and treat the remainder as raw text to
      accumulate, then `json.loads()` only the final concatenated string; or
  (b) request the structured form directly instead of line-range pagination, e.g.
      `read_file(path=output_id, json_path="$")` (or the narrowest `json_path` that
      covers the `results` array), sidestepping `format_read_file`'s human-text
      renderer entirely.
- Either fix should be paired with restoring the `except json.JSONDecodeError`
  blocks at `scripts/run-tc-benchmark.py:304-305,322-323,334-335` to `print(...,
  file=sys.stderr)` before falling back to `[]`/`break`, so a future regression is
  visible in the run log instead of silently changing scores.
- After the fix, re-run all three Task-10 arms — the current hybrid (9/75) and
  no-sparse (26/75) totals are known undercounts; the lite arm (31/75) is the most
  trustworthy of the three as-is (only 1/25 TCs affected) but should be re-run too
  for a clean apples-to-apples comparison.

## Tests added
`N/A` — no fix implemented yet in this session (Task 10 is a docs-only deliverable;
fixing the benchmark harness is out of scope). Whoever implements the Fix plan above
should add a Python-side regression test (or a `--self-test` mode in the harness)
that feeds a >10 KB buffered response through `semantic_search()` and asserts a
non-empty result, so this doesn't regress silently again.

## Workarounds
- Lower `--limit` and/or use `mode=compact` when invoking the harness, to reduce the
  chance any individual `semantic_search` response crosses the ~10 KB buffering
  threshold (`MAX_INLINE_TOKENS` = 2,500 tokens, `src/tools/core/types.rs:18`). This
  does not fix the harness but reduces how often the buggy path triggers.
- Treat any current or historical run of `scripts/run-tc-benchmark.py` using
  `mode=full` as a **lower bound** on true retrieval quality for backends whose
  responses tend to be large (hybrid/rerank arms especially), not an exact score.

## Resume
Implement Fix option (b) above in `scripts/run-tc-benchmark.py`'s
`McpClient.semantic_search()` (around line 306-335): replace the manual
`read_file` start_line/end_line pagination loop with a single
`read_file(path=ref_id, json_path="$")` call (or, if the buffered payload can itself
exceed a single response, page via `json_path` on the `results` array index rather
than raw line ranges), then re-run all three Task-10 arms and diff against
`docs/research/2026-07-02-lite-vs-hybrid-benchmark.md`'s recorded numbers to quantify
how much of the "lite beats hybrid" delta was harness artifact vs. real.

## References
- `scripts/run-tc-benchmark.py:296-336` (`McpClient.semantic_search`)
- `scripts/run-tc-benchmark.py:423-441` (`main()` — outer exception handling that
  never fires for this bug)
- `src/tools/read_file.rs:773-802` (`format_read_file` — the human-text renderer)
- `src/tools/edit_file/tests.rs:526-586` (`read_file_buffer_ref_range_auto_chunks` —
  documents the actual structured shape the harness should have targeted)
- `src/tools/core/types.rs:18-27` (`MAX_INLINE_TOKENS`, `TOOL_OUTPUT_BUFFER_THRESHOLD`)
- `docs/research/2026-07-02-lite-vs-hybrid-benchmark.md` — the Task-10 deliverable
  this bug was discovered while producing; see its "Known Issues" section for the
  benchmark-level framing of this caveat.
