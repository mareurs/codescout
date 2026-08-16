---
kind: bug
status: fixed
title: 'BUG: edit_markdown frontmatter set/delete is line-based, so mutating a key whose value is a YAML block sequence orphans the `- item` lines — set produces invalid YAML, delete silently reparents them'
tags:
- edit_markdown
- frontmatter
- yaml
- silent-corruption
closed: 2026-08-14
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

Fixed 2026-08-14 on `experiments`, in `src/tools/markdown/frontmatter.rs`. Both
candidates landed, because they cover different things:

**Candidate 1 — consume the continuation lines.** `apply_ops` iterates by index now,
and after replacing or deleting a matched key it drops that key's continuation lines.
Correct for *every* multi-line shape, not just block sequences, because both `set` and
`delete` replace the value wholesale — a nested map, a block scalar (`|`, `>`) and a
block sequence are all discarded the same way.

**Candidate 2 — refuse rather than corrupt.** `orphaned_sequence_items` counts
`- item` lines at column 0 whose nearest preceding key already carries a value.
`apply_ops` refuses when a write would *increase* that count. Compared against the
input rather than checked absolutely, so a block that arrived already broken can still
be repaired — `orphan_backstop_allows_repairing_an_already_broken_block` pins that.

No YAML round-trip validation: there is **no YAML crate in the dependency tree**
(checked `Cargo.toml`). That absence is deliberate — the module is hand-rolled and
flat-only — and pulling a parser in for one validation pass is the worse trade. The
orphan count is a structural approximation and is documented as one.

### `line_key` was the wrong predicate, exactly as the Resume warned

The Resume said to confirm `line_key` is a reliable "this line continues the previous
value" signal before relying on it, because "it was written for a different question."
It is not reliable. For a sequence-of-maps entry:

```yaml
steps:
- name: build
```

`line_key("- name: build")` finds a colon and returns `Some("- name")` — a fabricated
key. A scan terminating on `line_key(..).is_some()` would stop mid-value and leave the
item orphaned. So `is_continuation` is its own predicate: indented, or `-` at column 0.
Pinned by `set_consumes_a_sequence_of_maps_whose_items_contain_colons`.

### Correction: `delete` has two failure modes, not one

This file said the `delete` path leaves items that "reattach to the **preceding** key.
That parses fine and means something different." Measured 2026-08-14, that is true only
when the preceding key is *itself* a block sequence. Both were reproduced live:

| Preceding key | After deleting `related` | Result |
|---|---|---|
| `owner: marius` (scalar) | `owner: marius` / `- a.md` | **Invalid YAML** — `yaml.safe_load` raises `ParserError` |
| `tags:` + `- alpha` (sequence) | `tags:` / `- alpha` / `- a.md` | **Valid YAML, wrong data** — parses as `{'tags': ['alpha', 'a.md']}` |

So which failure you get depends on the **neighbouring** key's shape, not on the key
being deleted. The second row is the one that justifies `severity: high`: `a.md`
becomes a member of `tags` with no error at any layer. The first row is loud, if anyone
looks. Both are now covered by their own test.

### Why `artifact(action="update")` was never affected

There are two frontmatter implementations and only this one is line-based.
`src/librarian/frontmatter.rs` parses into a typed `Frontmatter` (named fields plus
`extra: BTreeMap`) and reserialises the whole block, so a multi-line value round-trips
structurally. That is why `artifact update` rewrote `tags: [docs, retrieval, config]`
into a block sequence earlier in this session without incident, while `edit_markdown`
corrupted the same shape. It also makes the new error hint correct advice rather than a
guess: for a librarian-managed artifact, `artifact(action="update", patch={…})` is the
safe path.

The tool layer needed no change — `apply_frontmatter_mutation`
(`src/tools/markdown/edit_markdown.rs:852-861`) delegates to `apply_ops` for both the
has-frontmatter and bootstrap branches, so this is a single chokepoint and the fix
covers the whole surface. Verified rather than assumed.
## Tests added

Seven, in `src/tools/markdown/frontmatter.rs`:

| Test | Covers |
|---|---|
| `set_on_a_block_sequence_consumes_its_items` | the filed case |
| `delete_of_a_block_sequence_after_a_scalar_key_consumes_its_items` | the invalid-YAML `delete` mode |
| `delete_does_not_reparent_items_onto_a_preceding_sequence` | **the silent mode** — the reason this is high severity |
| `set_consumes_indented_continuations` | nested maps and block scalars |
| `set_consumes_a_sequence_of_maps_whose_items_contain_colons` | the `line_key` trap |
| `orphan_backstop_refuses_a_write_it_cannot_make_safe` | comment-interleaved sequence → refuse |
| `orphan_backstop_allows_repairing_an_already_broken_block` | the backstop is relative, not absolute |

All three corruption cases were **reproduced live through the MCP surface before any
code changed**, each with a `python3 -c "yaml.safe_load(...)"` verdict on the written
file rather than an eyeball on the diff. That is what turned the `delete` correction
above from a guess into a measurement.

Two of the new tests failed on first run — my expected strings assumed array elements
serialise bare (`related: [c.md]`) when this module always double-quotes them
(`related: ["c.md"]`). The assertion that mattered, that the orphan lines were gone,
passed in both. Expectations corrected; no code change.

Gate: **3721 passed / 0 failed / 44 ignored** (3714 + these 7, reconciling exactly),
`clippy --all-targets -D warnings` clean.

### Live-surface verification, 2026-08-14 (post `cargo rb` + `/mcp` reconnect)

The silent mode — the one that justified `severity: high` — re-run against the running
server on the same fixture used to reproduce it:

```
frontmatter={"delete": ["related"]}   over   tags:/- alpha/related:/- a.md

  before  {'tags': ['alpha', 'a.md']}   valid YAML, wrong data, no error
  after   {'tags': ['alpha']}           a.md consumed with its key
```

Verdict taken from `yaml.safe_load` on the written file, not from reading the diff — the
whole point of this bug is that the corrupt output *parses*.
## Workarounds

- For librarian-managed artifacts, use `artifact(action="update", patch={tags: [...]})`
  — the librarian's own writer (`src/librarian/frontmatter.rs`) serializes the whole
  block and does not have this failure mode.
- For plain markdown, read the frontmatter first (`sed -n '1,20p'`) and check whether
  the target key's value is a block sequence. If it is, avoid the `frontmatter` param.
- After any `frontmatter` mutation, re-read the block. The call returns `"ok"`
  regardless.

## Resume

N/A — fixed and verified.

The Resume's second instruction (audit existing files for silent pre-existing
corruption) is now cheap and safe to run at any time, and no longer urgent: the
backstop refuses new occurrences, and `orphaned_sequence_items` is the exact predicate
such an audit would want. A file corrupted by the *silent* mode would show up as a
librarian misclassification or a wrong `tags`/`related` list rather than a YAML error —
searching for a stray entry in those lists is the tell, not `grep '^- '`.
## References

- `src/tools/markdown/frontmatter.rs:59-101` — `apply_ops`, the defect
- `src/tools/markdown/edit_markdown.rs:821-875` — `apply_frontmatter_mutation`, the caller
- `src/librarian/frontmatter.rs` — the writer that produces the block sequences this breaks on
- Occurrence: relocating `researcher/docs/issues/2026-07-27-researcher-rerank-score-scale-mismatch.md`
