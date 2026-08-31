---
kind: bug
status: fixed
tags:
- librarian
- link-scan
- markdown-parsing
- pulldown-cmark
- cluster/addressing-without-an-escape-hatch
closed: 2026-07-05
opened: 2026-07-05
owner: marius
related: []
severity: high
---

# BUG: link_scan's `extract()` treats any pair of bare `---` lines as YAML frontmatter, silently swallowing headings between them

## Summary
`src/librarian/tools/link_scan/extract.rs::extract()` enabled pulldown_cmark's
`Options::ENABLE_YAML_STYLE_METADATA_BLOCKS`, intending only to skip each
artifact's leading frontmatter block. That option instead pairs up **any** two
bare `---` lines anywhere in the document as a `MetadataBlock` — not just a
leading one. Session-log trackers (`docs/trackers/*session-log.md`) separate
every entry with a lone `---`, so roughly every other entry got silently
classified as "metadata" and never scanned: its heading was never recorded as
a `Definition`, and its body was never scanned for citations. Tokens that
genuinely have a heading definition therefore surfaced as `dangling` (0
definers found) purely as a parsing artifact, inflating the dangling/ambiguous
counts reported by `librarian(action="link_scan")`.

## Symptom (Effect)
Discovered while triaging `link_scan`'s dangling-findings report
(2026-07-05, post tracker-cross-linking stream). `docs/trackers/bug-fix-session-log.md`
(catalog id `2dd9d90bc83f9f49`) defines `## W-19 — …` at line 1678 (added
this session) and also cites `W-19` in its own Wins Index table at line 99.
The dangling report listed:
```json
{ "src_id": "2dd9d90bc83f9f49", "raw": "W-19", "kind": "EntryToken", "line": 99 }
```
i.e. zero definers found for `W-19` anywhere in the corpus, despite the
heading existing in the very same file. Pre-existing entries `F-20`, `F-21`,
`F-22` (also real headings in the same file) showed the identical symptom.

## Reproduction
`git rev-parse HEAD` → `23ea7e9c` (pre-fix). In a `cargo test` harness, call
`extract()` directly on the real file:
```rust
let text = std::fs::read_to_string("docs/trackers/bug-fix-session-log.md").unwrap();
let ex = extract(&text);
assert!(ex.definitions.iter().any(|d| d.token == "W-19")); // FAILED pre-fix
```
Minimal repro (no file needed): a doc with a leading real frontmatter block,
then two headings each followed by `**Status:** …\n\n---\n\n` before the next
heading — the *second* heading disappears from `ex.definitions`.

## Environment
codescout `experiments` @ `23ea7e9c` (pre-fix), Rust, `pulldown-cmark`
(workspace-pinned version), `Options::ENABLE_TABLES | ENABLE_YAML_STYLE_METADATA_BLOCKS`.

## Root cause
Confirmed via direct pulldown_cmark event-stream inspection (not assumed):
dumping `Event::Start(Tag::MetadataBlock(YamlStyle))` / `End` events on the
real file showed a metadata block opening at the `---` immediately before
`## W-19` and closing at the *next* `---` separator (before the following
entry) — i.e. pulldown_cmark's YAML-metadata-block detection with this option
enabled pairs up **any** two bare `---` lines as an opening/closing pair,
regardless of document position, not just a leading frontmatter block.
`extract()`'s `in_metadata` guard then correctly (per its own logic) skipped
every `Event::Text`/`Event::Code`/`Event::Start(Tag::Link)` inside that
span — including the swallowed heading — so no `Definition` was ever pushed
for it.

## Evidence
Live session 2026-07-05, ephemeral diagnostic tests in
`src/librarian/tools/link_scan/extract.rs` (removed after root-cause
confirmation, not committed):
- A synthetic doc with **one** bare `---` (no matching close) extracted
  correctly — ruling out "any bare `---` breaks headings."
- The real file, sliced to just `**Status:** validated\n\n---\n\n## W-19 …`
  (4 lines, real bytes) reproduced `DEFS: []`.
- Direct event dump on the real file's tail: `METADATA START YamlStyle at
  byte 7747` / `METADATA END YamlStyle at byte 10662` — spanning exactly the
  `---` before `## W-19` through the next entry's `---` separator.

## Hypotheses tried
1. **Table-cell citation is malformed / entry_re doesn't match "W-19".**
   Test: isolated `extract()` call on a synthetic table+heading doc.
   **Verdict: rejected** — worked correctly in isolation (`DEFS` found `W-19`).
2. **Unbalanced code fence (```) somewhere earlier in the file poisons
   `in_code_block`.** Test: counted backtick-fence markers in the whole file
   (12, even) and dumped `CodeBlock` Start/End events around the swallow
   point. **Verdict: rejected** — no `CodeBlock` event appeared near the
   swallowed heading; the swallow was a single giant `Event::Text`, not code.
3. **The specific heading text (em-dashes, embedded straight quotes,
   `context()` parens) breaks `def_re` or pulldown_cmark's inline tokenizer.**
   Test: extracted the exact real heading text in isolation.
   **Verdict: rejected** — correctly defined `W-19` on its own.
4. **`---` immediately followed by a heading with no blank line breaks ATX
   heading recognition.** Test: minimal repro with that exact shape.
   **Verdict: rejected** — correctly defined `W-19`.
5. **`ENABLE_YAML_STYLE_METADATA_BLOCKS` pairs bare `---` separators as
   metadata blocks anywhere in the doc, not just leading frontmatter.** Test:
   dumped `Tag::MetadataBlock` Start/End events on the real file.
   **Verdict: confirmed** — exact byte range matched the swallowed span.

## Fix
Implemented in `src/librarian/tools/link_scan/extract.rs::extract()`
(uncommitted on `experiments` as of this bug's `closed` date — see `git
status`). Removed `Options::ENABLE_YAML_STYLE_METADATA_BLOCKS` entirely.
Frontmatter is now skipped via an explicit byte-offset guard
(`frontmatter_end`) computed by calling the already-existing, already-tested
`crate::librarian::frontmatter::parse(text)` (the same delimiter-finding
logic every artifact's frontmatter parsing already relies on) and comparing
each pulldown_cmark event's `span.start` against it — sidestepping
pulldown_cmark's own YAML-metadata heuristic for the body entirely. Preserves
exact line-number semantics for all `Definition`/`Citation` entries (no text
slicing, just an offset comparison).

## Tests added
`tests/bare_dash_separator_between_entries_is_not_mistaken_for_yaml_metadata`
(`src/librarian/tools/link_scan/extract.rs`) — 3-entry doc (`W-1`/`W-2`/`W-3`)
separated by bare `---`, behind a real leading frontmatter block; asserts all
3 headings are extracted as `Definition`s. Full `librarian::tools::link_scan::`
module suite (27 tests) and the `tests/link_scan.rs` corpus integration suite
(6 tests) re-verified green after the fix. Full gate: `cargo fmt && cargo
clippy --all-targets -- -D warnings && cargo test` → 3034 passed, 0 failed,
43 ignored.

## Workarounds
None needed post-fix. Pre-fix: none available short of avoiding bare `---`
separators in tracker prose (not practical — it's the established session-log
convention).

## Resume
N/A — fixed and verified this session. If a live-MCP re-run of
`librarian(action="link_scan")` (after `cargo rb` + `/mcp` reconnect) shows a
meaningfully reduced dangling/ambiguous count vs. the pre-fix baseline (366
dangling / 232 ambiguous), note the corrected numbers here for the record;
this section can then move to N/A permanently.

## References
`src/librarian/tools/link_scan/extract.rs::extract` (fix); `src/librarian/frontmatter.rs::parse`
(reused delimiter-finding logic); `docs/trackers/bug-fix-session-log.md`
(the corpus file that surfaced the symptom); tracker cross-linking stream
handoff commit `23ea7e9c`.
