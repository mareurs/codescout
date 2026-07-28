---
status: fixed
opened: 2026-07-01
closed: 2026-07-01
severity: high
owner: marius
related: ["docs/issues/2026-05-09-read-file-json-path-array-elements.md", "docs/issues/2026-05-17-read-file-jsonpath-negative-slice.md"]
tags: ["read_file", "json_path", "file_summary", "footgun", "friction"]
kind: bug
---

# BUG: `read_file` json_path cannot address object keys containing `.` (or any key needing bracket-quoting) — quoted `["key"]` / `['key']` are rejected outright

## Summary

`read_file(path, json_path=…)` has no syntax that reaches an object key
containing a dot (e.g. `"1.1"`, `"2.1.5"`). `$.2.1.5` splits into segments
`2/1/5`; `$["2.1.5"]` and `$['2.1.5']` are rejected as "unsupported json_path
segment". The parser's bracket form only accepts numeric array indices, so a
dotted (or otherwise quote-needing) key is unaddressable — and the error hint
never says so, listing only forms that structurally cannot express the key.
Surfaced by usage analysis: one file (`benchmarks/section-audit/uat_criteria_by_section.json`,
top-level keys `1.1`…`2.1.5`) drove **278 read_file errors across 461 calls
(60% failure)** in the MRV-poc trace — ~23% of that project's total errors.

## Symptom (Effect)

Against a JSON object whose top-level keys are `"1.1"`, `"1.2"`, …, `"2.1.5"`:

```
read_file(path=".../uat_criteria_by_section.json", json_path="$.2.1.5")
→ path segment '2' not found — hint: Available keys: 1.1, 1.2, 1.3, 1.4, …

read_file(path=".../uat_criteria_by_section.json", json_path="$[\"2.1.5\"]")
→ unsupported json_path segment near '["2' — hint: Supported forms: '.key', '[N]', '[-N]', '[-N:]'.

read_file(path=".../uat_criteria_by_section.json", json_path="$['2.1.5']")
→ unsupported json_path segment near '['2' — hint: Supported forms: …
```

Every documented and undocumented form fails. There is no json_path that
returns the value at key `"2.1.5"`.

## Reproduction

```bash
echo '{"1.1":{"a":1},"2.1.5":{"b":2}}' > /tmp/dotkeys.json
# via codescout read_file:
#   read_file("/tmp/dotkeys.json", json_path="$.1.1")      -> segment '1' not found
#   read_file("/tmp/dotkeys.json", json_path="$[\"1.1\"]") -> unsupported json_path segment near '["1'
#   read_file("/tmp/dotkeys.json", json_path="$['1.1']")   -> unsupported json_path segment near '['1'
```

- Commit: `a751457b84040423efe5d3377592a53cea0efbf1` (branch `experiments`)
- Evidence DB: `~/work/stefanini/southpole/MRV-poc/.codescout/usage.db`

## Environment

- codescout MCP; observed in project `stefanini/southpole/MRV-poc`.
- Parser: `src/tools/file_summary/file_summary.rs`.

## Root cause

Two layered gaps in `parse_json_path_segments` (`src/tools/file_summary/file_summary.rs:485`):

1. **Naive `.` tokenization.** After stripping `$`/`$.`, the path is
   `path.split('.')` (`file_summary.rs:494`). This splits *inside* bracket
   segments too: `$["2.1.5"]` becomes parts `["2`, `1`, `5"]`, so the first
   part `["2` has an unterminated `[` and errors at
   `file_summary.rs` (`rest.find(']')` → `unsupported_bracket`). Bracket
   contents are never parsed as a unit.

2. **`parse_bracket` accepts only integers** (`src/tools/file_summary/file_summary.rs:520`):
   all-ASCII-digit → `Segment::Index`, `-N`/`-N:` → neg index/slice, else
   error. A quoted string key like `"1.1"` (or even `"1"`) hits the final
   `Err(unsupported json_path segment)`. So `["key"]` / `['key']` string-key
   access is unsupported even for dot-free keys.

Net: object keys that are not valid `.`-delimited bare identifiers are
unreachable. The hint (`file_summary.rs:521`) compounds it by advertising
`.key` without noting that a `.` inside a key can't be escaped.

## Evidence

### Usage-analysis blast radius (MRV-poc usage.db)

```
errors tied to uat_criteria_by_section.json:  461 calls / 278 errors (60%)
top failing json_paths on that file:
  $["2.1.3"] 15, $["2.1.2"] 13, $.1 12, $["1.5"] 11, $["1.12"] 11,
  $["1.4"] 10, $["2.1.1"] 8, $["1.8"] 8, $.2 7, $.1.13 7, …
```

`err_family` rollup: `json_path_unsupported` = 168 (plus uncategorized
`path segment not found` from the `$.N.N` attempts).

### Parser code

`src/tools/file_summary/file_summary.rs:494` — `for part in path.split('.')`.
`src/tools/file_summary/file_summary.rs:520-570` — `parse_bracket` integer-only.

## Hypotheses tried

1. **Hypothesis:** dotted keys unreachable because `.`-split runs before
   bracket parsing AND `parse_bracket` rejects quoted keys. **Test:** read
   both functions; traced `$["2.1.5"]` → split → `["2` → unterminated bracket
   error. **Verdict:** confirmed. **Evidence:** § Parser code.

## Fix

Implemented in the working tree on `experiments` (not yet committed / cherry-picked to `master`, so no master SHA yet). Two coordinated changes in `src/tools/file_summary/file_summary.rs`:

1. **Bracket-aware tokenization.** New helper `split_on_unbracketed_dot` (`file_summary.rs:489`) splits the path on `.` only outside `[...]` (bracket-depth tracked), replacing the naive `path.split('.')` in `parse_json_path_segments`. So `["2.1.5"]` is one token instead of fragmenting into `["2` / `1` / `5"]`.
2. **Quoted-key support.** New helper `strip_matching_quotes` (`file_summary.rs:511`); `parse_bracket` (`file_summary.rs:557`) now returns `Segment::Key` for `["key"]` / `['key']` (matching quotes only), before the integer branch — so a quoted numeric string is a Key, not an array Index. Reuses the existing `Segment::Key` apply arm in `resolve_json_segment:585` (`obj.get(k)`) — no new enum variant needed (per recon W-18).

Hints updated in both `parse_bracket`'s `supported_hint` and `unsupported_bracket` to advertise `["key"]` / `['key']`.

**Preserved invariants (recon W-18):** `$.a[abc]` still errors (unquoted, non-numeric → `parse_rejects_non_integer_bracket` green); `$.a[0][-1]` chained brackets still parse (`parse_chained_negative_after_positive` green).

**Known non-goal:** an object key literally containing `]` (e.g. `["a]b"]`) is still unreachable — the segment loop's `find(']')` matches the first `]`. Extremely rare; out of scope.

Verified: `cargo fmt` clean, `cargo clippy --all-targets -- -D warnings` clean, `cargo test` = 2962 passed / 0 failed / 43 ignored. Live `/mcp` verify pending (`cargo rb`).
## Tests added

In `src/tools/file_summary/tests.rs`:
- `parse_bracket_quoted_key_with_dots` (`tests.rs:737`) — `$["2.1.5"]`, `$['1.1']`, `$["1"]` → single `Segment::Key`; the quoted-`"1"`-is-a-key case guards against index/key confusion.
- `parse_bracket_quoted_key_then_field` (`tests.rs:756`) — `$["2.1.5"].x` → `[Key("2.1.5"), Key("x")]`, pinning the split-dots-outside-brackets tokenizer.
- `extract_json_path_dotted_string_key` (`tests.rs:765`) — end-to-end `extract_json_path` on `{"1.1":…, "2.1.5":{…}}` via `$["2.1.5"]`.

All three failed pre-fix with `unsupported json_path segment near '["2'`; pass post-fix. The two anchor tests `parse_rejects_non_integer_bracket` (`tests.rs:731`) and `parse_chained_negative_after_positive` (`tests.rs:681`) remain green — the regression guard held.
## Workarounds

Until fixed, dotted-key JSON can't be reached with json_path. Instead:
- `run_command("jq '.\"2.1.5\"' path.json")` (jq quotes handle dots), or
- `read_file(path, start_line=N, end_line=M)` after locating the key with
  `grep('"2.1.5"', path)`.

## Resume

Fix + regression tests landed and verified in the working tree. Next: commit on `experiments`, cherry-pick to `master` (Standard Ship Sequence, `docs/RELEASE.md`); record master SHA in § Fix after cherry-pick. `cargo rb` + `/mcp`, then live-verify `read_file(path, json_path="$[\"2.1.5\"]")` against a dotted-key fixture. Archive to `docs/issues/archive/` only after the fix is on `master` (`git branch --contains <sha>`).
## References

- Surfaced by `/analyze-usage` on MRV-poc; see `docs/usage-reports/2026-07-01-usage-analysis.md`.
- Prior json_path bugs (different facets): `docs/issues/2026-05-09-read-file-json-path-array-elements.md`, `docs/issues/2026-05-17-read-file-jsonpath-negative-slice.md`
- Parser: `src/tools/file_summary/file_summary.rs:485` (`parse_json_path_segments`), `:520` (`parse_bracket`)
