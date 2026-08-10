---
kind: bug
status: open
title: 'BUG: edit_markdown frontmatter set/delete is line-based, so mutating a key whose value is a YAML block sequence orphans the `- item` lines — set produces invalid YAML, delete silently reparents them'
tags:
- edit_markdown
- frontmatter
- yaml
- silent-corruption
closed: null
opened: 2026-08-08
owner: marius
related:
- docs/issues/2026-08-08-artifact-extra-key-collision-unclassifies-silently.md
severity: high
---

# BUG: edit_markdown's frontmatter mutation orphans block-sequence continuation lines

## Summary

`edit_markdown(frontmatter={set: {...}})` rewrites the matched `key:` **line** and
leaves every continuation line of a multi-line YAML value in place. For a block
sequence this yields a bare `- item` line following a complete key/value pair —
invalid YAML. The call returns `"ok"`; nothing warns, and the file is written.

The `delete` path is worse: it removes the `key:` line and leaves the `- item`
lines, which then reattach to the **preceding** key. That parses fine and means
something different — silent semantic corruption with no error at any layer.

## Symptom (Effect)

Observed live 2026-08-08 while relocating a bug file into the `researcher` repo.

Before (valid):

```yaml
owner: marius
related:
- docs/issues/archive/2026-07-27-reranker-gpu-tei-cuda-oom.md
severity: medium
```

Call:

```
edit_markdown(path=..., frontmatter={"set": {"related": ["codescout:docs/issues/archive/2026-07-27-reranker-gpu-tei-cuda-oom.md"]}}, edits=[...])
```

Returned `"ok"`. After (invalid YAML):

```yaml
owner: marius
related: ["codescout:docs/issues/archive/2026-07-27-reranker-gpu-tei-cuda-oom.md"]
- docs/issues/archive/2026-07-27-reranker-gpu-tei-cuda-oom.md
severity: medium
```

The orphaned `- …` line is a block-sequence entry with no parent key.

## Reproduction

```
1. Create a .md file whose frontmatter has a key with a block-sequence value:
     ---
     owner: marius
     related:
     - a.md
     severity: medium
     ---
2. edit_markdown(path=..., frontmatter={"set": {"related": ["b.md"]}})
3. Read the file — the `- a.md` line survives beneath the new inline `related: ["b.md"]`.
```

Commit: `1a1f2c82` (experiments). Also reproduces conceptually for
`frontmatter={"delete": ["related"]}`, which leaves `- a.md` attached to `owner`.

## Environment

codescout `experiments` @ `1a1f2c82`, Linux, MCP stdio transport. Bug files and
librarian artifacts are the affected population — `tags:`, `owners:`, and
`related:` are all written as block sequences by the librarian's own writer
(`src/librarian/frontmatter.rs`), so any artifact is a candidate.

## Root cause

`apply_ops` at `src/tools/markdown/frontmatter.rs:59-101` iterates the frontmatter
block **one line at a time** and dispatches on `line_key(line)`:

- `Some(key) if set.contains_key(key)` → pushes `format!("{key}: {}", serialize_value(v)?)`
  and advances a single line.
- The following `- item` line has no key, so `line_key` returns `None` and it hits
  the catch-all `_ => out.push(line.clone())`, which preserves it verbatim.

Nothing in the function models a key's value as spanning more than its own line, so
the continuation lines are invisible to both `set` and `delete`. The doc comment
declares the module "flat-only" and rejects *object* values on the input side
(`serialize_value` errors on `Object`), but there is no corresponding check on the
**existing** value's shape — flat-only is enforced against what the caller passes
in, never against what is already in the file.

*measured 2026-08-08: ran the call above against a real bug file, read the result
with `sed -n '1,17p'` and saw the orphaned line. Mechanism then read directly at
`src/tools/markdown/frontmatter.rs:59-101` and `src/tools/markdown/edit_markdown.rs:821-875`.*

## Evidence

`apply_ops`, the dispatch that drops the continuation line on the floor:

```rust
for line in block {
    match line_key(line) {
        Some(key) if delete_set.contains(key) => {
            // skip — deletes the line
        }
        Some(key) if set.contains_key(key) => {
            let v = &set[key];
            out.push(format!("{key}: {}", serialize_value(v)?));
            applied.insert(key);
        }
        _ => out.push(line.clone()),   // <-- `- item` lands here
    }
}
```

`apply_frontmatter_mutation` (`src/tools/markdown/edit_markdown.rs:821-875`) passes
`fm.lines` straight through and splices the result back, so there is no second
layer that could notice.

## Hypotheses tried

1. **Hypothesis:** the tool refuses multi-line values, and I used it out of contract.
   **Test:** read the tool schema and `apply_ops`' doc comment.
   **Verdict:** rejected as an excuse, confirmed as a doc gap. The schema does say
   "Flat keys only (one-key-per-line; scalar / string / inline-array values)", but
   that describes the *value the caller supplies*, and `serialize_value` enforces
   exactly that. Nothing states or checks that the value **already in the file**
   must be single-line. A tool that corrupts on an unsupported input rather than
   refusing it is a defect regardless of what the docs say.
   **Evidence:** § Root cause.

## Fix

Not yet implemented. Two candidates, and they compose:

1. **Consume the continuation lines.** When a matched key is replaced or deleted,
   also drop the following lines until the next line that has a `line_key` (or the
   end of the block). This is the smallest change that makes both `set` and
   `delete` correct, and it preserves the existing inline-output style.
2. **Refuse instead of corrupting.** If a matched key's value spans multiple lines
   and the change cannot be expressed safely, return `RecoverableError` naming the
   key and pointing at `artifact(action="update", patch={...})` for artifacts. This
   is the honest floor even if candidate 1 lands — it covers block scalars (`|`,
   `>`) and nested maps, which candidate 1 would still mangle.

Candidate 2 alone would have been enough to prevent this incident.

## Tests added

None yet. The regression test this needs is a round-trip: frontmatter with a
block-sequence `tags:`, a `set` on it, then a YAML parse of the result — asserting
the parse succeeds AND that the key holds exactly the new value. A test that only
diffs the text would pass on the `delete` variant, which stays parseable while
meaning something else.

## Workarounds

- For librarian-managed artifacts, use `artifact(action="update", patch={tags: [...]})`
  — the librarian's own writer (`src/librarian/frontmatter.rs`) serializes the whole
  block and does not have this failure mode.
- For plain markdown, read the frontmatter first (`sed -n '1,20p'`) and check whether
  the target key's value is a block sequence. If it is, avoid the `frontmatter` param.
- After any `frontmatter` mutation, re-read the block. The call returns `"ok"`
  regardless.

## Resume

Read `line_key` in `src/tools/markdown/frontmatter.rs` (it is the function that
decides what counts as a key line) and confirm it returns `None` for both `- item`
and indented continuation lines before implementing candidate 1 — the fix depends on
that predicate being a reliable "this line continues the previous value" signal, and
it was written for a different question.

Then check the blast radius: `grep -c '^- ' docs/issues/*.md docs/trackers/*.md` for
files whose frontmatter already carries block sequences, and confirm none were
corrupted by an earlier call. This session's occurrence was caught and repaired by
hand; an earlier silent one would look like a librarian classification failure, not
a YAML bug.

## References

- `src/tools/markdown/frontmatter.rs:59-101` — `apply_ops`, the defect
- `src/tools/markdown/edit_markdown.rs:821-875` — `apply_frontmatter_mutation`, the caller
- `src/librarian/frontmatter.rs` — the writer that produces the block sequences this breaks on
- Occurrence: relocating `researcher/docs/issues/2026-07-27-researcher-rerank-score-scale-mismatch.md`
