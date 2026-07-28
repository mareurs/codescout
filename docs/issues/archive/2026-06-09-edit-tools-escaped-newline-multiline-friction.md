---
status: fixed
opened: 2026-06-09
closed: 2026-06-09
severity: medium
owner: marius
related:
  - docs/trackers/reconnaissance-patterns.md
  - docs/trackers/bug-fix-session-log.md
tags:
  - edit_file
  - edit_code
  - tooling
  - newline-escaping
  - structural-edit-guard
kind: bug
---

# BUG: edit_file / edit_code mishandle escaped `\n` in multi-line edits — escaped newlines slip past the structural-edit guard and are written to disk literally, corrupting source

## Summary
Multi-line code edits are fragile and recur often. Two distinct failure modes,
both rooted in how `\n` is handled: (1) an `edit_file` `new_string` whose
newlines are written as the two-character escape `\n` (rather than real line
breaks) is treated as a **single physical line** — it bypasses the
structural-edit guard *and* is written verbatim, collapsing a whole function
onto one line of literal `\n` (syntax error on disk); (2) `edit_code`'s `body`
with the same escaped `\n` is rejected as "would introduce syntax errors". The
**same** `\n`-escaped `new_string` decoded correctly in one call earlier in the
same session and incorrectly in another — so the behaviour is inconsistent, not
deterministic. Net effect this session: ~8 failed edit calls and one
written-to-disk corruption (recovered via `git checkout`) to land a single
multi-line test function.

## Symptom (Effect)
Observed while inserting a regression test into `src/tools/memory/tests.rs`
(2026-06-09 onboarding `project_id` fix). In order:

1. `edit_file` multi-line `old_string` → repeatedly:
   ```
   old_string not found in src/tools/memory/tests.rs
   No exact or whitespace-normalized match.
   ```
   …even though `read_file(force=true)` showed the exact bytes of the anchor.
2. A **single-line** `old_string` containing escaped quotes
   (`"    assert!(!project_mem_path.exists(), \"memory should be deleted\");"`)
   → same "old_string not found".
3. A **single-line, quote-free, brace-free, newline-free** `old_string`
   (`"    std::fs::create_dir_all(&mcp).unwrap();"`) → **succeeded**. This is the
   isolating probe: same file, editable; the failing variants all carried `\"`
   and/or `\n` and/or `{`.
4. `edit_file` `new_string` with escaped `\n` → returned `{"status":"ok"}` **but**
   wrote the entire function onto one physical line containing literal `\n`:
   ```
   1114: async fn …() {\n    use crate::agent::Agent;\n    use std::sync::Arc;\n…
   ```
   and emitted `warning: syntax error detected after edit — file may be malformed`.
5. `edit_file` `new_string` with **real** newlines → blocked:
   ```
   edit_file is blocked for structural edits on source code files
   (debug_enforce_symbol_tools is enabled)
   ```
6. `edit_code(action=insert)` `body` with escaped `\n` (even a trivial 3-line
   stub `#[tokio::test]\nasync fn s() {\n    assert!(true);\n}`) →
   `would introduce syntax errors — not written`.
7. `edit_code(action=insert)` `body` with **real** newlines → succeeded
   (`inserted_at_line: 1112`).

## Reproduction
Not a clean unit repro yet (the `\n`-decoding inconsistency in step 4 vs. a
working `edit_file` `new_string` earlier in the session is the unexplained part).
Best lead — reproduce the *guard* path deterministically:

1. `edit_file(path=<any .rs>, old_string=<unique single line>, new_string=<that
   line>"\n"<a second line>)` where `\n` is sent as the escape sequence.
2. Observe: the edit is accepted as a single-line edit and writes literal `\n`
   to disk (rather than being blocked as structural or decoded).
3. Contrast: the same edit with a **real** newline in `new_string` is blocked by
   `debug_enforce_symbol_tools` and routed to `edit_code`.

Observed against codescout MCP, master as of 2026-06-09, `~/.claude-sdd` profile,
project `/home/marius/work/claude/codescout`.

## Environment
- OS: Linux. codescout MCP server, `experiments` working tree, master-equivalent edit tools.
- Driven from a Claude Code session doing the `project_id` onboarding fix.
- Tools: `mcp__codescout__edit_file`, `mcp__codescout__edit_code`. The
  structural-edit guard is gated by `debug_enforce_symbol_tools`.

## Root cause
Unconfirmed at the mechanism level; two interacting defects are *observable*:

1. **Escaped `\n` is not consistently decoded** in `edit_file`
   `new_string`/`old_string` (and `edit_code` `body`). In step 4 the two-char
   `\n` reached disk literally; an identical `\n`-escaped `new_string` decoded
   to real newlines in an earlier `src/tools/run_command/tests.rs` edit in the
   same session. The decode appears to depend on something per-call
   (serialization/transport), not on the tool — hence "inconsistent".
2. **The structural-edit guard keys on physical-line shape.** A payload whose
   newlines are literal `\n` looks like one physical line, so
   `debug_enforce_symbol_tools` does not classify it as structural and lets it
   through — exactly the payload that then corrupts the file. With real
   newlines the guard fires correctly and redirects to `edit_code`.

Net: the *guard* and the *decoder* disagree about what a newline is, and the
gap is a write-malformed-source-to-disk path.

The `old_string` "not found" failures (steps 1–2) are a separate facet of the
same `\n`/`\"`-handling fragility: anchors carrying escaped quotes/newlines did
not match bytes that `read_file(force=true)` confirmed present.

## Evidence
### One-line-collapse write (step 4)
`grep "async fn memory_write_accepts" src/tools/memory/tests.rs` after the edit
returned a **single** match whose line body was the entire ~70-line function
with embedded literal `\n` (no real line breaks), terminating in
`…assert_eq!(read_result[\"content\"], \"Use camelCase\");\n}\n\n#[tokio::test]\nasync fn memory_read_sections_filter_integration() {`.

### Isolating probe (step 3)
`edit_file(old_string="    std::fs::create_dir_all(&mcp).unwrap();",
new_string="    std::fs::create_dir_all(&mcp).unwrap(); // probe")` → `"ok"`.
The same file rejected every `old_string` that contained `\"` or `\n`.

### Guard vs. decoder (steps 5–7)
Real-newline `edit_file` → blocked by `debug_enforce_symbol_tools`.
Real-newline `edit_code` body → accepted. Escaped-`\n` `edit_code` body →
"would introduce syntax errors". Escaped-`\n` `edit_file` → accepted + corrupt.

## Hypotheses tried
1. **Hypothesis:** the giant `new_string` had a JSON-escaping bug, breaking
   `old_string` parsing. **Test:** retried the same multi-line `old_string` with
   a *tiny* `new_string`. **Verdict:** rejected — still "old_string not found",
   so the failure is in `old_string` matching, not `new_string`.
2. **Hypothesis:** the file is not editable / wrong bytes. **Test:** quote/brace/
   newline-free single-line edit. **Verdict:** rejected — that edit succeeded.
3. **Hypothesis:** escaped `\n` is the corrupting factor. **Test:** real-newline
   `edit_file` (blocked) and real-newline `edit_code` (succeeded) vs escaped-`\n`
   variants (corrupt / syntax-error). **Verdict:** confirmed — real newlines
   behave correctly; escaped `\n` is the trap.

## Fix

**Implemented for `edit_file` on `experiments`** (cite the master SHA after cherry-pick).

Root defect: `edit_file`'s exact-match path wrote the candidate to disk *before* the syntax check and only warned (`commit_edit` then warn). The fix adds frictionless decode-and-repair on both the apply and match sides, chosen with the user as **both match + apply, with a transparency note**:

1. `decode_literal_escapes` helper (`src/tools/edit_file/mod.rs`) — single-pass decode of literal newline / tab / carriage-return escapes. Returns None when there is nothing to decode; leaves escaped quotes, backslashes, and regex escapes untouched.
2. `finalize_edit_content` helper — before writing, if the edit *introduces* a parse error the original file did not have, it retries with the inserted fragment's escapes decoded and keeps the result only if it parses (returning a repair note). If the error is unrepairable it returns the candidate unchanged, so the project's deliberate **non-fatal warn** path still fires. No new reject is introduced, so a legitimately-temporarily-broken edit is never blocked.
3. Match-side fallback: when `old_string` yields zero matches (after exact + whitespace-normalized matching), it retries with decoded escapes; a unique decoded match applies the decoded pair and reports `applied_via = "escape-decoded match"`.
4. A repaired edit returns `status: ok` plus a `note`, so the correction is visible, not silent.

Verified: `cargo fmt` + `cargo clippy --lib --tests` clean; `cargo test --lib edit_file` = 213 passed.

**Unified across both tools (2026-06-09).** The repair logic now lives in a shared module `src/tools/edit_repair.rs` (`decode_literal_escapes`, `RepairResult`, `finalize_edit_content`, `REPAIR_NOTE`, + 7 unit tests). Both `edit_file` (single-edit: match-side + apply-side) and `edit_code`'s `do_insert` route through `finalize_edit_content`. They share the *repair*; the *fallback policy* differs by design — `edit_file` is non-fatal (writes + warns), while `edit_code`'s insert *rejects* an unrepairable introduced error without writing (it has no LSP round-trip to self-heal, and a 2026-06-05 regression guard depends on that). Remaining minor gap: `edit_file`'s batch / prepend paths still call `atomic_write` directly without the safety net.

Note: the underlying `\n` / `\"` escape-decoding inconsistency observed across tool calls this session appears to live at the MCP transport / serialization layer (codescout writes what it receives); the codescout-side fix is the decode-repair safety net, which recovers regardless of why an un-decoded escape arrives.
## Tests added

In `src/tools/edit_file/tests.rs`:

- `edit_file_auto_repairs_literal_newline_in_new_string` — a `new_string` whose line breaks arrived as literal backslash-n escapes is auto-decoded to valid multi-line code; the result carries a repair note.
- `edit_file_matches_old_string_with_literal_newline_escapes` — an `old_string` carrying literal escapes matches a real-newline region via the decode fallback (`applied_via = "escape-decoded match"`).
- `edit_file_preserves_legitimate_backslash_n_in_valid_edit` — a legitimate newline escape inside a Rust string literal (valid as-is) is NOT altered; the repair does not over-trigger.
- Existing `edit_file_warns_on_syntax_error_after_edit` still passes — an unrepairable introduced error stays non-fatal (write + warning), confirming no regression to the deliberate design.
## Workarounds
- **Use real newline characters, never the `\n` escape**, in any multi-line
  `edit_file`/`edit_code` payload.
- **Use `edit_code` (`action=insert`/`replace`) for adding or replacing whole
  symbols**, not `edit_file` — `edit_file` is for in-line text/literal/comment
  edits. (This is Iron Law 2; the episode is a concrete datapoint for it.)
- For `edit_file` text anchors, prefer **quote-free, brace-free, single-line**
  `old_string`s; verify exact bytes with `read_file(path, start_line, end_line,
  force=true)` first (`force=true` bypasses the symbol-overview redirect).
- If a corrupt write lands, `git -C <root> checkout -- <file>` to revert (works
  when the file has no other intended changes).

## Resume
Locate the `edit_file` argument-decode path and the `debug_enforce_symbol_tools`
structural-edit guard (grep `debug_enforce_symbol_tools` across `src/tools/`),
and the `edit_code` `body` ingest. Add a literal-`\n` detector: either decode or
reject, and make the guard count literal `\n`. Confirm the inconsistency in
step 4 (same escaping decoded in one call, not another) by diffing the two
call payloads in the session JSONL. Add the three tests in "Tests added".

## References
- Observed during the 2026-06-09 `project_id` onboarding fix; sibling bug
  `docs/issues/2026-06-09-onboarding-prompt-uses-project-not-project-id.md`.
- Recon ledger: `docs/trackers/reconnaissance-patterns.md` (multi-line
  edit-anchor friction is a recurring R-N theme).
- Guard symbol: `debug_enforce_symbol_tools`.
