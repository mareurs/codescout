---
status: open
opened: 2026-07-05
closed:
severity: low
owner: marius
related: []
tags: [prompts, librarian, doc-drift]
kind: bug
---

# BUG: librarian guide says artifact ids are "8-hex"; implementation is 16-hex

## Summary
`get_guide("librarian")` § Artifact Model documents `id` as "string (8-hex)". The
implementation derives ids as `sha256(abs_path)` truncated to **16** hex chars. Any consumer
building a regex or validation from the guide under-matches every real id.

## Symptom (Effect)
Real ids are 16 chars (e.g. `59ebeebb6ed05c89`); a guide-derived `[0-9a-f]{8}` pattern
matches only a prefix. Nearly caused a wrong extraction regex in the link_scan design
(caught by the audit_doc_refs scout, 2026-07-05).

## Reproduction
Compare `get_guide("librarian")` Artifact Model table with `src/librarian/ids.rs`.

## Environment
codescout `experiments`, 2026-07-05.

## Root cause
Doc drift: `src/prompts/guides/librarian.md` (Artifact Model field table) says "8-hex";
`src/librarian/ids.rs:4-23` (`artifact_id`, `artifact_id_from_abs`) uses `hex[..16]`, pinned
by test `sixteen_hex_chars` (`ids.rs:41-45`). Possibly "8 bytes" was mistranscribed as
"8-hex".

## Evidence
Scout report (plan-mode, 2026-07-05): "`artifact_id()` … = `sha256(...)` truncated to 16
(`hex[..16]`). Test `sixteen_hex_chars`."

## Hypotheses tried
N/A — mechanism read directly from source.

## Fix
One-word doc fix in `src/prompts/guides/librarian.md`: "8-hex" → "16-hex". Scheduled with
the W2 conventions edits of the tracker cross-linking plan.

## Tests added
N/A — doc-only; the id width is already pinned by `sixteen_hex_chars`.

## Workarounds
Trust `ids.rs`, not the guide, for id shape.

## Resume
Apply the one-word fix during W2 (same commit as the cross-linking guide section); verify
with `cargo test --lib prompts`.

## References
`src/prompts/guides/librarian.md` (Artifact Model), `src/librarian/ids.rs:4-45`.
