---
status: fixed
opened: 2026-07-09
closed: 2026-07-09
severity: medium
owner: marius
related: [2026-07-09-artifact-get-heading-exact-match-only]
tags: [librarian, artifact, silent-failure]
kind: bug
---

# BUG: `artifact(get, start_line, end_line)` line numbers are off by one — line 1 is an invisible separator, not the first content line

## Summary
`artifact(action="get", id=X, start_line=N, end_line=M)` succeeds and returns an empty
(or wrong) body slice because the 1-indexed line numbering counts a blank separator line
that sits between the YAML frontmatter's closing `---` and the visible body content.
`start_line=1` addresses that invisible blank, not the first real content line. No error
is raised; the caller sees a normal-looking `success` envelope with an empty `body`.

## Symptom (Effect)
`artifact(get, id="690f130645515497", start_line=1, end_line=1)` on backend-kotlin's
solver-invariants tracker returned:
```json
{ "body": "", "body_meta": { "line_count": 0, "source_line_count": 573, "start_line": 1, "end_line": 1 } }
```
`start_line=1, end_line=5` on the same artifact returned only 4 lines
(`line_count: 4`), one short of what an inclusive `[1,5]` range should yield — because
line 1 was the blank separator, not real content.

## Reproduction
Git commit: `a7b2f839ec86d71ef31921ed649ce08df8979fb6` (branch `experiments`).

1. `artifact(action="create", kind="scratch", rel_path="scratchpad/probe.md", body="L1\nL2\nL3\nL4\nL5\n")`
2. `artifact(action="get", id=<id>, start_line=1, end_line=1)`
3. Pre-fix: `body: ""`, `body_meta.line_count: 0`. Expected: `body: "L1"`, `line_count: 1`.

Confirmed independently against 6+ historical occurrences in
`/home/marius/work/mirela/backend-kotlin/.codescout/usage.db` (`tool_calls` rows,
2026-07-01 → 2026-07-09) and 1 in codescout's own `usage.db`, all showing the same
`read_file(json_path="$.body") → "0 lines"` signature downstream of an
`artifact(get, start_line/end_line or heading)` call without `full=true`.

## Environment
codescout MCP server, Rust, `src/librarian/tools/get.rs`. Reproduced against both
backend-kotlin (`docs/trackers/solver-invariants.md`, artifact id `690f130645515497`)
and a throwaway scratch artifact in codescout itself. Not transport- or OS-specific.

## Root cause
`frontmatter::parse()` (`src/librarian/frontmatter.rs:31-54`) returns the raw remainder
of the file after the closing `---` line, **verbatim** — including the conventional
blank separator line that `artifact(create)`/`update` write between the frontmatter
block and the body content. `get.rs::call` (`src/librarian/tools/get.rs:407-413`, prior
to fix) assigned this remainder directly to `parsed_body` with no trimming. Every
line-oriented consumer downstream — `slice_lines`, `find_heading_section`,
`apply_soft_cap`, and `source_line_count` — then indexed 1-based against a string whose
line 1 is that invisible blank, silently shifting every human-meant line number by one.
`slice_lines`'s own range math (`lines[start-1..end]`, inclusive `end_line` semantics)
was correct; the bug was entirely in what "line 1" of `body` actually pointed at.

## Evidence
Live repro on a controlled 5-line scratch artifact (`scratchpad/pika-probe.md`),
`start_line=2, end_line=5` returned `"L1\nL2\nL3\nL4"` (body_meta echoed
`start_line:2, end_line:5`) — proving line 2, not line 1, was "L1". `full=true` on the
same artifact returned body `"\nL1\nL2\nL3\nL4\nL5\n\n"` (leading AND trailing blank),
`body_meta.line_count: 7` for 5 real content lines — confirming the blank separator is
counted as a real line throughout.

## Hypotheses tried
1. **Hypothesis:** `slice_lines`'s `end_line` is treated as exclusive (off-by-one in the
   range math itself), contradicting the documented "1-indexed inclusive" contract.
   **Test:** hand-traced `slice_lines(body, 1, 1)` and `slice_lines(body, 2, 5)` against
   the function as written.
   **Verdict:** rejected — `lines[start-1..end]` is mathematically inclusive-correct
   (`[2,5]` → 4 elements = `5-2+1`, matching the existing passing test
   `line_slice_returns_requested_range`, which deliberately crafted a fixture WITHOUT
   the leading blank to dodge this exact issue — see its code comment).
   **Evidence link:** see Evidence section; controlled scratch-artifact repro.
2. **Hypothesis:** the deployed MCP binary was stale relative to source (the
   `~/.cargo/bin/codescout` symlink gotcha).
   **Test:** checked `git log`/`git status` on `get.rs` (clean, unchanged since
   `0de733aa`) against the release binary's build timestamp.
   **Verdict:** rejected — binary and source were in sync; the bug is real, not staleness.
3. **Hypothesis (confirmed):** the body fed to all line-oriented operations includes an
   unaccounted-for leading blank separator line from the frontmatter/body boundary.
   **Test:** `read_markdown` on the raw on-disk file showed the literal blank line
   between the frontmatter's closing `---` and `L1`; `full=true`'s returned body string
   literally starts with `"\n"`.
   **Verdict:** confirmed. **Evidence link:** Evidence section above.

## Fix
Strip exactly one leading blank-line separator (`\r\n` or `\n`) from the frontmatter-
parsed remainder before assigning it to `parsed_body`, so `start_line=1` means the first
visible content line for every consumer (full, slice, heading) in one place.
`src/librarian/tools/get.rs:407-417`. Committed on `experiments` as `21662112`
(not yet on `master`). Deliberately scoped to `get.rs` only; `frontmatter::parse`
itself is untouched to avoid blast radius on `update.rs`/`create.rs`/`indexer.rs`/
`context.rs`/`link_scan/extract.rs`, which also consume it.

## Tests added
- `librarian::tools::get::tests::line_slice_start_line_1_returns_first_visible_content_line`
  (`src/librarian/tools/get.rs`) — asserts `start_line=1, end_line=1` on a normally-shaped
  file (frontmatter + blank separator + content) returns `"L1"`, not empty.
- Full existing suite re-run clean: `cargo test --lib` → 2959 passed, 0 failed, 6 ignored
  (unrelated). `cargo clippy --lib -- -D warnings` clean.
- Live-verified post-`cargo rb` + `/mcp` reconnect against the original failing case
  (backend-kotlin's `690f130645515497`, `start_line=1, end_line=1` on a live scratch
  artifact) — now returns `body: "L1"`, `line_count: 1`.

## Workarounds
Use `full=true` (soft-capped, but not shifted) or a `heading=` selector instead of
`start_line`/`end_line` when precise line addressing matters and the fix isn't deployed yet.

## Resume
N/A — fixed and verified this session. If reopened: re-check
`src/librarian/tools/get.rs` around the `parsed_body` derivation for regressions after
any refactor of `frontmatter::parse` call sites.

## References
- Sibling bug: `docs/issues/2026-07-09-artifact-get-heading-exact-match-only.md` (found
  in the same investigation, same file).
- `docs/trackers/tool-usage-patterns.md` (T-N entry pending) — cross-repo usage.db
  evidence (backend-kotlin + codescout `.codescout/usage.db`).
- Existing regression test that unknowingly worked around this bug:
  `librarian::tools::get::tests::line_slice_returns_requested_range`.
