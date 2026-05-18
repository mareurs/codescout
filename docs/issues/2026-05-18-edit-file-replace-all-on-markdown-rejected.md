---
status: fixed
opened: 2026-05-18
closed: 2026-05-18
severity: medium
owner: marius
related: [src/tools/edit_file/mod.rs]
tags: [edit_file, edit_markdown, gate, replace_all]
kind: bug
---

# BUG: `edit_file(path="*.md", replace_all=true)` hard-rejected even for legitimate global string swaps

## Summary

The `.md`-routing gate in `edit_file::call` blocks every call whose path
ends in `.md` unless the call uses `insert="prepend"` or `insert="append"`.
This catches the legitimate "global string swap across a markdown file"
use case (e.g., rename an ID, swap a date, replace a brand mention in
all occurrences), even when `replace_all: true` is set. The user is
forced to either (a) locate every heading containing the string and
call `edit_markdown(action="edit", heading=..., replace_all=true)` per
heading, or (b) use `read_markdown` + manual reconstruction.

Surfaced 2026-05-18 during this session — `edit_file(path="CLAUDE.md",
old_string="abc513d3ee0f0b50", new_string="b3fa993849ac83ab",
replace_all=true)` was hard-rejected with the standard gate message
even though both occurrences live in the same section and the operation
is the textbook find-and-replace case `edit_file` was designed for.

## Symptom (Effect)

```
{
  "ok": false,
  "error": "Use edit_markdown for markdown files",
  "hint": "edit_markdown provides heading-based editing for .md files. edit_file with insert='prepend'/'append' is still allowed."
}
```

The hint is misleading — it suggests `edit_markdown` is the right
substitute, but `edit_markdown` does not have a file-scoped global
replace mode. Its `action="edit"` requires a `heading` argument.

## Reproduction

Branch `experiments` at HEAD ≥ `4923f62`. Any path ending `.md`:

```python
edit_file(path="CLAUDE.md",
          old_string="<old-id>",
          new_string="<new-id>",
          replace_all=True)
```

→ rejected with the message above. Even with `replace_all=true`.

## Environment

- codescout v0.12.1 release build.
- Branch `experiments`.
- Linux 7.0.0-15-generic.

## Root cause

`src/tools/edit_file/mod.rs:177-186`:

```rust
if path.ends_with(".md") || path.ends_with(".markdown") {
    let insert_mode = input["insert"].as_str();
    if insert_mode != Some("prepend") && insert_mode != Some("append") {
        return Err(super::RecoverableError::with_hint(
            "Use edit_markdown for markdown files",
            "edit_markdown provides heading-based editing for .md files. edit_file with insert='prepend'/'append' is still allowed.",
        ).into());
    }
}
```

The gate intentionally short-circuits all `.md` writes except the two
boundary inserts. It does not consider `replace_all`, which is the one
input shape where `edit_file`'s file-wide find/replace gives strictly
better ergonomics than `edit_markdown`'s heading-scoped editor.

## Evidence

- Live probe in session 2026-05-18, top of conversation: the rename of
  CLAUDE.md's `abc513d3ee0f0b50` → `b3fa993849ac83ab` was forced through
  `edit_markdown(heading=..., replace_all=true)` after the natural
  `edit_file(replace_all=true)` was rejected.
- The gate at `src/tools/edit_file/mod.rs:177-186` ignores both top-level
  `replace_all` and batch-mode `edits[*].replace_all`.

## Hypotheses tried

1. **Hypothesis:** `edit_markdown` already supports file-wide `replace_all`
   without a heading.
   **Test:** Check `edit_markdown` schema and behavior.
   **Verdict:** rejected — `edit_markdown(action="edit", ...)` requires
   `heading` (or `edits[*].heading` in batch mode). There is no
   file-scoped global-replace mode.

## Fix


Approach (1) shipped — loosened the gate in `edit_file::call`.

**Change at `src/tools/edit_file/mod.rs:177-204`:**

```rust
if path.ends_with(".md") || path.ends_with(".markdown") {
    let insert_mode = input["insert"].as_str();
    let single_replace_all = input["replace_all"].as_bool().unwrap_or(false);
    let batch_all_replace_all = input["edits"].as_array().and_then(|edits| {
        if edits.is_empty() { None }
        else { Some(edits.iter().all(|e| e["replace_all"].as_bool().unwrap_or(false))) }
    });
    let allowed = matches!(insert_mode, Some("prepend") | Some("append"))
        || single_replace_all
        || batch_all_replace_all.unwrap_or(false);
    if !allowed {
        return Err(super::RecoverableError::with_hint(
            "Use edit_markdown for markdown files",
            "edit_markdown provides heading-based editing for .md files. \
             edit_file is still allowed with insert='prepend'/'append' or \
             replace_all=true (file-wide find/replace).",
        ).into());
    }
}
```

The hint message also updated to mention the `replace_all` exception.

**Verification:**
- 31 `edit_file_*` tests pass, including:
  - 3 pre-existing `md_gate_*` tests (blocks_non_insert, allows_prepend,
    allows_append) — unchanged behavior on those paths.
  - 4 new regression tests:
    - `edit_file_replace_all_on_markdown_passes_through`
    - `edit_file_single_replace_on_markdown_still_gated`
    - `edit_file_batch_all_replace_all_on_markdown_passes_through`
    - `edit_file_batch_mixed_replace_all_on_markdown_still_gated`
- `cargo clippy --lib --tests` clean.
- `cargo fmt` clean.

**Commit:** `<tba>` on `experiments`.

## Tests added


Four new regression tests at `src/tools/edit_file/tests.rs`:

1. `edit_file_replace_all_on_markdown_passes_through` — positive case
   for single-edit `replace_all=true` on `.md`.
2. `edit_file_single_replace_on_markdown_still_gated` — negative: gate
   still fires on non-replace_all single edits.
3. `edit_file_batch_all_replace_all_on_markdown_passes_through` — positive
   case for batch mode where every entry sets `replace_all=true`.
4. `edit_file_batch_mixed_replace_all_on_markdown_still_gated` — negative:
   batch with any entry omitting `replace_all=true` is still gated.

## Workarounds

Use `edit_markdown(action="edit", heading=<section>, old_string=...,
new_string=..., replace_all=true)` — the heading scope means you must
find the section first.

## Resume

Apply fix (1) in `src/tools/edit_file/mod.rs:177-186`. Add the two
tests. Run `cargo test --lib edit_file`. Commit on `experiments`,
ship later.

## References

- Surfaced during the librarian-misclassification + tracker-archive
  cleanup session, 2026-05-18.
- Workaround used immediately: `edit_markdown(action="edit",
  heading="### Tool Usage Patterns ...", replace_all=true)` —
  successfully fixed the stale ID in CLAUDE.md.
