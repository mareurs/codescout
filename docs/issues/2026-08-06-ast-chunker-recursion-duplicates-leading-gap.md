---
status: open
opened: 2026-08-06
closed:
severity: medium
owner: marius
related: []
tags: [embed, chunking, retrieval]
kind: bug
---

# BUG: container decomposition emits a gap chunk duplicating everything before the container

## Summary

When `nodes_to_chunks` decomposes a container (impl / class) into its inner
declarations, it recurses into itself with the **unchanged whole-file `source`**
and `prev_end` reset to `0`. The recursion therefore treats every line before the
first inner declaration as a "gap" and emits it as a chunk — duplicating content
that the outer call already chunked. Every decomposed container in every indexed
file adds one redundant chunk carrying a copy of all preceding file text.

## Symptom (Effect)

For a 12-line Rust file with one `impl` block, `split_file` returns a chunk
spanning lines 1–8 whose content repeats the three chunks already emitted before
it:

```
--- chunk 0: lines 2-4 meta=Some("src/mystore.rs :: pub fn top_level()")
pub fn top_level() {
    println!("hi");
}
--- chunk 1: lines 6-6 meta=Some("src/mystore.rs :: pub struct MyStore;")
pub struct MyStore;
--- chunk 2: lines 8-8 meta=Some("src/mystore.rs :: impl MyStore")
impl MyStore {
--- chunk 3: lines 1-8 meta=Some("src/mystore.rs")     <-- duplicate of 0+1+2
```

Reproduced again on a second fixture (`impl Tiny`), where the recursion emitted
`"\nimpl Tiny {"` at lines 1–2 alongside the header chunk `"impl Tiny {"` at
line 2.

## Reproduction

At `ca442498` on `experiments`:

1. Add a temporary test to the `tests` module of `src/embed/ast_chunker.rs`:

```rust
#[test]
fn dump() {
    use std::path::Path;
    let src = "\npub fn top_level() {\n    println!(\"hi\");\n}\n\npub struct MyStore;\n\nimpl MyStore {\n    pub fn build(&self) {\n        // body\n    }\n}\n";
    for (i, c) in split_file(src, "rust", Path::new("src/mystore.rs"), 4000).iter().enumerate() {
        println!("--- chunk {i}: lines {}-{} meta={:?}\n{}", c.start_line, c.end_line, c.metadata, c.content);
    }
    panic!("diagnostic");
}
```

2. `cargo test --lib embed::ast_chunker::tests::dump -- --nocapture`

## Environment

Linux, Rust (workspace MSRV 1.82), branch `experiments` at `ca442498`.
Language-agnostic — any language with non-empty `inner_node_types`
(Rust, Python, TypeScript, Java, Kotlin, Go) hits it.

## Root cause

`src/embed/ast_chunker.rs` — the inner-node recursion passes `source` whole:

```rust
let inner_chunks = nodes_to_chunks(
    source,          // <-- whole file, not the container's slice
    &inner,
    ...
);
```

Inside that call, `let lines: Vec<&str> = source.lines().collect();` and
`let mut prev_end: usize = 0;`. The first inner node's `expanded_start` is its
absolute file line, so the gap branch `if expanded_start > prev_end` is true with
`prev_end == 0`, and `lines[0..expanded_start]` is emitted as a gap chunk. That
range covers every line of the file preceding the container, all of which the
outer call has already chunked as its own nodes.

The trailing-gap branch has the mirror-image effect but is harmless in size — it
emits the container's closing brace as its own chunk.

## Evidence

### Diagnostic dump, `ca442498`

See *Symptom* above — chunk 3's content is byte-identical to the concatenation of
chunks 0, 1 and 2 plus the blank separator lines.

### The `AST_CHUNK_MIN` interaction that surfaced it

The duplicate gap was latent (an extra chunk with bare-file-path metadata) until
`coalesce_small_chunks` was added in the same session. Coalescing merged the
spurious leading gap, the real method chunk, and the trailing `}` into ONE
whole-file chunk labelled `src/mystore.rs :: impl MyStore` — which dropped
`fn build` from the metadata header entirely and broke
`split_file_rust_populates_metadata_headers` and
`inner_method_signature_skips_doc_comments`.

That coupling is now closed from the coalescing side: gap chunks are excluded
from merge runs (`ca442498`). The duplication itself is untouched.

## Hypotheses tried

1. **Hypothesis** — the duplication is an artifact of the new coalescing code.
   **Test** — added the gap-exclusion fix and re-ran the diagnostic.
   **Verdict** — rejected. The lines-1–8 duplicate chunk is still emitted after
   the fix; coalescing only ever consumed it.
   **Evidence link** — *Diagnostic dump, `ca442498`*.

## Fix

Not implemented. Plan: give the recursion an explicit starting `prev_end` (the
container's body start line) so it stops re-deriving gaps it does not own, or
give it the container's line slice rather than the whole file.

Blast radius is real and argues for doing this deliberately rather than as
merge-prep: it changes the emitted chunk set for every decomposed container in
every language, so the guards worth checking first are
`chunk_overlap_removed_from_ast_paths`, `ast_split_captures_gap_text`,
`ast_split_trailing_gap_captured`, and `sub_split_covers_all_body_lines`. Any
existing semantic index is invalidated by the change and needs a rebuild.

## Tests added

None yet — the bug is not fixed. The nearest existing coverage,
`chunk_overlap_removed_from_ast_paths`, does not catch it: it asserts on
overlap between *adjacent* chunk line ranges, and this duplicate is a
non-adjacent superset range.

## Workarounds

None needed for correctness — retrieval still returns valid results, and the
duplicate carries file-level metadata so it ranks weakly. The cost is index
size and embed spend: one extra chunk per decomposed container, sized by that
container's offset into the file, so it is worst in long files whose containers
sit near the bottom.

## Resume

Add `body_start: usize` as a parameter to `nodes_to_chunks` (default `0` for the
top-level call, `expanded_start` for the recursive one) and initialise
`prev_end = body_start`. Then re-run the *Reproduction* diagnostic and confirm
chunk 3 disappears while chunks 0–2 and the `fn build` chunk are unchanged. Run
`cargo test --lib embed::ast_chunker` — the four guards named under *Fix* are the
ones expected to move if the change is wrong.

## References

- `src/embed/ast_chunker.rs` — `nodes_to_chunks` (gap branches and the inner-node
  recursion)
- `ca442498` (`experiments`) — the coalescing commit that surfaced this, and
  which closed the metadata-loss half by excluding gap chunks from merge runs
