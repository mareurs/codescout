---
id: '98dc447e9c72eacc'
kind: bug
status: fixed
title: 'BUG: artifact(get, full=true) → read_file($.body) silently truncates large bodies with no overflow marker'
owners:
- marius
tags:
- librarian
- artifact
- silent-failure
- read_file
- json_path
topic: null
time_scope: null
closed: '2026-07-10'
opened: '2026-07-09'
owner: marius
related:
- '2026-07-09-artifact-get-heading-exact-match-only'
- '2026-07-09-artifact-get-line-slice-blank-separator-offset'
severity: high
---

## Summary
`artifact(action="get", full=true)` on a large artifact returns a buffered envelope;
extracting the body via `read_file(json_path="$.body")` on that buffer silently caps
output at roughly ~500 lines with **no truncation signal whatsoever** — no
`[N of M lines shown]` marker, no `Next:` hint, nothing distinguishing it from a
genuinely complete body. The real body can be 20-30%+ longer. This is a *different*
code path from the already-fixed `start_line`/`end_line` off-by-one bug
(`d6daa994188c77b5`) and from the already-fixed exact-heading-match bug
(`7f1498e8a2ed5b44`) — both of those bugs' own "Workarounds" sections recommend
`full=true` as the safe alternative, which this bug shows is false.

## Symptom (Effect)
Against backend-kotlin's `docs/trackers/solver-invariants.md` (id
`690f130645515497`, real on-disk body 602 raw lines pre-edit):
```
artifact(get, id="690f130645515497", full=true)
  → { output_id: "@tool_46132616", buffered_bytes: 164509 }
read_file(json_path="$.body", path="@tool_46132616")
  → "500 lines" + a fresh @file_461332ca buffer — no overflow marker at all
```
A same-session heading-scoped `get` on the SAME artifact reported
`body_meta.source_line_count: 602` (later 653 after an edit) — the true body length.
The `full=true` extraction silently returned ~100+ fewer lines with zero indication
anything was cut. A second `full=true` fetch after the body had *grown* to 653 real
lines produced an even smaller extraction ("499 lines") — ruling out a fixed
line-count cap and pointing at a byte-budget cap that happens to land near the same
ballpark regardless of true size.

## Reproduction
1. Create/use an artifact whose body exceeds roughly 10-15KB (large tracker).
2. `artifact(action="get", id=<id>, full=true)` → buffered envelope.
3. `read_file(path=<output_id>, json_path="$.body")` → returns a `@file_*` buffer with
   a stated line count well below the artifact's real `source_line_count` (visible via
   any heading-scoped `get` on the same artifact), and no truncation marker.
4. Contrast with `read_file` on a *smaller* JSON array field via the same `json_path`
   mechanism (e.g. an Index-table body_edits fetch in the same session) — those DID
   show `[N of M lines shown]` + `Next: read_file(...)` hints when truncated. The
   `$.body` string-field extraction path does not.

## Environment
codescout MCP server, Rust. Reproduced against backend-kotlin's
`docs/trackers/solver-invariants.md` (`690f130645515497`), branch `experiments`,
2026-07-09, same investigation session as `7f1498e8a2ed5b44` / `d6daa994188c77b5`.

## Root cause

Confirmed — and it is **not** a `read_file`/json_path truncation. `read_from_buffer`'s
json_path branch (`src/tools/read_file.rs:175-313`) and `extract_json_path`
(`src/tools/file_summary/file_summary.rs:450`) both return the **full** string value
uncapped. The ~500-line result was the body being *already capped* upstream by
`apply_soft_cap` (`src/librarian/tools/get.rs:40`, `SOFT_CAP_LINES = 500`) in the
`artifact(get, full=true)` response. That cap is signalled by a sibling `overflow`
object + `body_meta.source_line_count` — but the whole `get` response overflows the
10 KB inline budget (guaranteed for any 500+ line body), so `Tool::call_content`
(`src/tools/core/types.rs:618-621`) buffered it and emitted a generic `"Result stored
in @tool_X"` summary, dropping the warning. `LibrarianAdapter` never overrode
`format_compact`, so nothing promoted `overflow` into the surviving summary. Buried
signal, not missing and not a second cap. Same root cause as `b0e3905454edcba7`.
## Evidence
- `artifact(get, id="690f130645515497", full=true)` → `buffered_bytes: 164509`,
  `read_file(json_path="$.body")` → labeled "500 lines", no marker.
- Same artifact, heading-scoped `get(heading="SI-23")` in the same session →
  `body_meta.source_line_count: 602` (pre-edit).
- Post-edit: `full=true` again → `buffered_bytes: 163994` → `$.body` → "499 lines";
  `get(heading="SI-40")` on the same post-edit artifact → `body_meta.source_line_count:
  653`. Real on-disk `wc -l` after the edit: 672 lines (includes the just-added Index
  rows + a pre-existing unrelated anomaly, see References).
- **Consequence:** this silent truncation caused a real downstream mistake this
  session — a `grep '^## SI-'` run against the truncated `$.body` extraction reported
  only 36 of 43 invariants' body sections existed, when the true count (confirmed via
  `wc -l` + `grep` directly on the on-disk file) was 41 (5 of which were themselves a
  *separate*, pre-existing duplicate-heading problem the truncated read never revealed).
  Acting on the wrong 36-count, 7 new `## SI-N` sections were written — 4 of which
  duplicated pre-existing sections the truncated read had hidden. Caught and reverted
  in the same session via direct on-disk `grep`/`wc -l`, before it reached a commit —
  see `docs/trackers/solver-invariants-tracker-session-log.md` (backend-kotlin) F-2 for
  the full incident writeup.

## Hypotheses tried
1. **Hypothesis:** the `full=true` get response itself was silently capping `body`
   server-side (not a `read_file`/json_path issue).
   **Test:** compared `buffered_bytes` (164509, 163994 — both large, consistent with a
   full response) against the `read_file` extraction's reported line count (500, 499 —
   suspiciously stable regardless of underlying growth).
   **Verdict:** inconclusive without reading `get.rs`'s `full=true` path directly —
   deferred to whoever picks this up; flagging both the `get` response and the
   `read_file(json_path)` extraction as candidate locations rather than asserting one.

## Fix

Fixed at the summary layer (`src/librarian/adapter.rs`): `LibrarianAdapter::format_compact`
→ `librarian_compact_summary(inner_name, result)`. When an `artifact` response carries an
`overflow` object, the buffered summary now announces
`"artifact body TRUNCATED — only N of M lines are in $.body …"` with the narrower-selector
guidance, instead of the bland `"Result stored in …"`. `read_file`/json_path needed no
change — it was never the culprit. Shipped on `experiments`.
## Tests added

`src/librarian/adapter.rs` new tests: `compact_summary_surfaces_artifact_get_body_truncation`,
`compact_summary_none_without_overflow`, `compact_summary_none_for_non_artifact_tools`.
`cargo test --lib` → 2969 passed / 0 failed. Live-verified post-reconnect against
`2dd9d90bc83f9f49`: summary flipped to the TRUNCATED warning at the same
`buffered_bytes: 47588`.
## Workarounds
Do not trust `full=true` as a "safe, complete" read for large artifacts. Cross-check
any line/heading count derived from a `full=true` → `$.body` extraction against a
heading-scoped `get`'s `body_meta.source_line_count`, or against a direct on-disk
`grep`/`wc -l` when the artifact is large and the read result will drive a write.

## Resume

Fixed 2026-07-10. The trace hypothesis ("cap in `read_file`'s json_path handler") was
**wrong** — json_path extraction returns full strings; the cap was upstream in
`get.rs::apply_soft_cap` and the loss was the summary layer dropping the `overflow`
signal on buffering. Same fix closes sibling `b0e3905454edcba7`.
## References
- `docs/issues/2026-07-09-artifact-get-heading-exact-match-only.md` — sibling bug,
  same investigation, same artifact.
- `docs/issues/2026-07-09-artifact-get-line-slice-blank-separator-offset.md` — sibling
  bug whose own "Workarounds" section recommends `full=true`, which this bug shows is
  not reliably safe for large artifacts.
- `backend-kotlin:docs/trackers/solver-invariants-tracker-session-log.md` F-2 — the
  incident this bug caused, including the on-disk repair.
- `backend-kotlin:docs/issues/2026-07-09-solver-invariants-orphaned-index-entries.md`
  — the original (now-superseded-in-part) triage that this truncation initially
  misinformed.
