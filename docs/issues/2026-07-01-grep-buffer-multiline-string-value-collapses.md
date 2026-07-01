---
status: fixed
opened: 2026-07-01
closed: 2026-07-01
severity: medium
owner: marius
related: ["docs/issues/archive/2026-05-09-grep-buffer-false-negatives.md"]
tags: ["grep", "buffer", "json", "escaped-newline", "artifact", "false-negative"]
kind: bug
---

# BUG: `grep(pattern, path="@tool_*")` collapses on multi-line JSON string values (e.g. artifact `body`) — matches the whole field as one line

## Summary

`grep` against an `@tool_*` buffer whose JSON contains a large multi-line
string field (an artifact `body`, a file blob, any `"key": "…\n…\n…"`)
returns a single match: the entire field as one physical line. Grepping for
`## F-N —` section headings in a session-log tracker body reports `total: 1`
and overflows to a truncated buffer instead of listing each heading. Affects
anyone using `grep @tool_*` to enumerate lines inside a text-heavy tool
result. It is a variant of the fixed 2026-05-09 buffer-grep bug that the
pretty-print fix did not cover.

## Symptom (Effect)

Observed in `~/work/mirela/backend-kotlin/` while finding the max
`## F-N / ## W-N` heading in a session-log tracker (an `artifact(get)`
result buffered as `@tool_1bb7af81`):

```
grep(path="@tool_1bb7af81", pattern="## (F|W)-[0-9]+ —")
→ { "output_id": "@tool_1bba5944",
    "summary": "@tool_1bb7af81 (1)\n… (truncated)",
    "buffered_bytes": 41570 }
```

`total` = 1, and that one match is the whole ~40 KB body on a single line —
"collapsed on the escaped-JSON body." The per-heading enumeration the caller
wanted is impossible from this output.

## Reproduction

Mechanism reproduction (faithful to `serde_json::to_string_pretty`, which —
unlike Python's default `json.dumps` — does NOT escape non-ASCII, so the
em-dash stays literal and the pattern matches once):

```python
import json, re
body = "\n".join(f"## {p}-{n} — entry {n}" for p in ("F","W") for n in range(1,6))
obj  = {"id":"abc","title":"session-log","body":body}
pretty = json.dumps(obj, indent=2, ensure_ascii=False)   # ≈ to_string_pretty
pat = re.compile(r"## [FW]-[0-9]+ —")
print(len([l for l in pretty.splitlines() if pat.search(l)]))  # -> 1 (want 10)
```

In-tree: seed an `@tool_*` buffer with a JSON object whose `body` field holds
`## F-1 —\n## F-2 —\n…`, then `grep(pattern="## F-", path="@tool_id")`.
Pre-fix: `total == 1`. Want: one match per heading line.

- Commit: `a751457b84040423efe5d3377592a53cea0efbf1` (branch `experiments`)
- Invoke: live MCP `grep` on any `@tool_*` buffer holding a multi-line string field.

## Environment

- OS: Linux; codescout MCP over the live release build.
- Observed project: `mirela/backend-kotlin`; root cause in codescout itself.
- Branch: `experiments`.

## Root cause

`grep_in_buffer` (`src/tools/grep.rs:456`) special-cases `@tool_*` buffers by
`serde_json::to_string_pretty`-ing the JSON before the line-oriented match
(`src/tools/grep.rs:478-486`). Pretty-printing inserts physical newlines only
*between JSON tokens* (object keys, array elements). It does **not** decode
`\n` escape sequences *inside* a string value — a literal newline inside a
JSON string is invalid JSON, so an artifact `body` remains one physical line
after pretty-printing. The subsequent `for (i, line) in text.lines()`
(`src/tools/grep.rs:493`) therefore sees the whole body as a single line and
the regex matches it at most once.

The 2026-05-09 fix added the pretty-print step for the
*identifier-in-a-separate-field* shape (`{"name":"foo_bar_baz"}` → own line)
and its regression test `grep_buffer_ref_matches_content_in_tool_buffer`
(`src/tools/grep.rs:734`) only asserts `total > 0` for that shape. The
multi-line-string-value shape was never exercised, so the gap shipped.

## Evidence

### Faithful mechanism reproduction

```
pretty physical lines            : 5
lines matching (== grep 'total') : 1
bytes in that single match line  : 210
```

The body's 10 heading lines collapse to a single matching physical line —
`grep`'s `total` counts that as 1, exactly matching the `"(1)"` seen live.

### Code path

`src/tools/grep.rs:478-486`:
```rust
let text = if raw_path.starts_with("@tool_") {
    serde_json::from_str::<serde_json::Value>(&raw)
        .ok()
        .and_then(|v| serde_json::to_string_pretty(&v).ok())
        .unwrap_or(raw)
} else { raw };
```
No decode of embedded `\n`; `text.lines()` at `src/tools/grep.rs:493` then
under-splits multi-line string values.

## Hypotheses tried

1. **Hypothesis:** Pretty-print of `@tool_*` JSON keeps multi-line string
   values on one physical line, so line-oriented grep under-splits.
   **Test:** Reproduced with `to_string_pretty`-equivalent (`ensure_ascii=False`);
   10 heading lines → 1 matching physical line. **Verdict:** confirmed.
   **Evidence:** § Faithful mechanism reproduction.

## Fix

Implemented in the working tree on `experiments` (not yet committed / cherry-picked to `master`, so no master SHA yet). In the `@tool_*` branch of `grep_in_buffer`, after `to_string_pretty`, materialize escaped newlines so multi-line string values become grep-able lines:

`src/tools/grep.rs:486`:
```rust
.and_then(|v| serde_json::to_string_pretty(&v).ok())
// … comment …
.map(|pretty| pretty.replace("\\n", "\n"))
.unwrap_or(raw)
```

Search-only text, so the rare literal `\n`-in-data (serialized `\\n` → backslash+newline) is a cosmetically acceptable trade; documented in an inline comment. The `else` branch (`@file_*` / `@cmd_*`, already real-newline text) is untouched.

Verified: `cargo fmt` clean, `cargo clippy --all-targets -- -D warnings` clean, `cargo test` = 2959 passed / 0 failed / 43 ignored. Live `/mcp` verify against a real `artifact(get)` buffer still pending (`cargo rb`).
## Tests added

`tools::grep::tests::grep_buffer_ref_matches_multiline_string_value` (`src/tools/grep.rs:764`) — seeds an `@tool_*` buffer with a `body` field holding `## F-1 —\n…\n## F-10 —`, asserts `grep("## F-", @tool_id)` returns `total >= 10` (one match per heading). Pre-fix: `total == 1` (whole body on one line). Post-fix: `total == 10`. The prior-shape regression test `grep_buffer_ref_matches_content_in_tool_buffer` (`src/tools/grep.rs:734`) still passes (its seed has no embedded newline).
## Workarounds

Extract the field first, then grep the extracted `@file_*` (field extraction
decodes `\n` into real newlines):

```
read_file("@tool_xxx", json_path="$.body")   # -> @file_yyy (real newlines)
grep("## F-", "@file_yyy")                    # per-heading matches
```

Or for a librarian artifact specifically, read the section directly via
`artifact(get, id=…, headings=[…])` / `full=true` rather than grepping the
raw get envelope.

## Resume

Fix + regression test landed and verified in the working tree. Next: commit on `experiments`, then cherry-pick to `master` per the Standard Ship Sequence (`docs/RELEASE.md`); after cherry-pick run `git rev-parse HEAD` on master and record the SHA in § Fix. `cargo rb` + `/mcp`, then live-verify `grep("## F-", @tool_<artifact-get>)` returns per-heading matches. Archive this file to `docs/issues/archive/` only after the fix is on `master` (`git branch --contains <sha>`).
## References

- Prior variant (fixed, archived): `docs/issues/archive/2026-05-09-grep-buffer-false-negatives.md`
- Code: `src/tools/grep.rs:456` (`grep_in_buffer`), regression test at `src/tools/grep.rs:734`
- Progressive-disclosure buffer model: `get_guide("progressive-disclosure")`
