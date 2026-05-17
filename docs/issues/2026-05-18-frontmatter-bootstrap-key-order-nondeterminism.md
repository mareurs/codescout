---
status: open
opened: 2026-05-18
closed:
severity: low
owner: marius
related: [src/tools/markdown/edit_markdown.rs, src/tools/markdown/frontmatter.rs]
tags: [edit_markdown, frontmatter, non-determinism]
---

# BUG: `edit_markdown(frontmatter:{set})` bootstrap path emits keys in HashMap iteration order

## Summary

When `apply_frontmatter_mutation` synthesizes a new frontmatter block on a
file that has none, the keys are emitted in `std::collections::HashMap`
iteration order, which is randomized per-process. Two consecutive calls
with the same `set:` payload can produce identical-keys-but-different-order
output. Cosmetic for now; reproducibility-affecting for tooling that diffs
generated frontmatter.

## Symptom (Effect)

Live probe in session 2026-05-18 against the freshly-built MCP at commit
`4011ed2a`:

```
edit_markdown(path="/tmp/probe.md",
              frontmatter={"set": {"kind":"tracker", "status":"active", "title":"X"}})
```

Output 1: `---\ntitle: X\nkind: tracker\nstatus: active\n---\n\n...`

A second run in a different process could equally produce:
`---\nstatus: active\nkind: tracker\ntitle: X\n---\n\n...`

The 4-tests-pass unit suite was already written to tolerate either
ordering (`assert!(out == a || out == b, ...)`), masking the issue.

## Reproduction

Branch `experiments` at HEAD ≥ `4011ed2a`:

```
edit_markdown(path="/tmp/repro.md",
              frontmatter={"set": {"a":"1", "b":"2", "c":"3"}})
# repeat from a fresh `cargo run --release` — order may vary
```

## Environment

- codescout v0.12.1 release build.
- Linux 7.0.0-15-generic.

## Root cause

`src/tools/markdown/edit_markdown.rs:316-321` collects the `set:` JSON
object into a `std::collections::HashMap<String, Value>`:

```rust
let set: std::collections::HashMap<String, Value> = obj
    .get("set")
    .and_then(|v| v.as_object())
    .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
    .unwrap_or_default();
```

Downstream, `frontmatter::apply_ops(block: &[String], set: &HashMap<...>,
delete: &[String])` iterates `set` directly to append new keys when the
block is empty (bootstrap path) or to discover new keys (in-place edit
path).

For the in-place path, `set` is applied via `line_key` matching against
existing lines first — order of existing keys is preserved, new keys are
appended in HashMap order at the end. The non-determinism is only
visually-visible when ≥2 keys are appended, but it's there.

For the bootstrap path, the existing block is `&[]`, so every key is
"appended" — non-determinism is fully exposed.

## Evidence

- Code: `edit_markdown.rs:316-321` (HashMap construction),
  `frontmatter.rs:60-98` (apply_ops body).
- Live probe output above (single sample — order non-deterministic over
  runs, not within a single call).

## Hypotheses tried

1. **Hypothesis:** `serde_json::Map` (used as the JSON object backing)
   preserves insertion order; only the conversion to `HashMap` loses it.
   **Test:** `obj.get("set").and_then(|v| v.as_object())` returns
   `&serde_json::Map<String, Value>` which DOES preserve insertion order
   when the `preserve_order` feature is enabled (default for serde_json).
   **Verdict:** confirmed — the JSON object iteration order is already
   stable; only our HashMap step shuffles it.
   **Evidence link:** serde_json docs + Cargo.toml feature audit.

## Fix

Replace `std::collections::HashMap<String, Value>` with
`indexmap::IndexMap<String, Value>` (insertion-ordered) in
`apply_frontmatter_mutation` AND in `apply_ops`. Update the signature of
`apply_ops` to accept `&IndexMap<String, Value>` (or `&dyn AsRef<...>`
for less coupling). The conversion at the obj-parse site preserves
serde_json's insertion order, so the bootstrap and in-place paths both
emit keys in the order the caller wrote them.

Alternative (smaller diff): keep `HashMap` but sort keys alphabetically
before emitting in `apply_ops`. Sacrifices caller-intended order for
determinism. Less ergonomic for human readers.

Recommendation: `IndexMap` — `indexmap` is already a likely transitive
dep (used by `serde_json` preserve_order). Confirm via `cargo tree`
before adding to Cargo.toml.

## Tests added

`N/A — bug only filed.` Future fix should add:

- `frontmatter_bootstrap_emits_keys_in_caller_order` — call with `set:
  {a, b, c}` and `set: {c, b, a}` separately, assert each preserves the
  written-in order. Will fail with the current HashMap implementation
  on at least some HashMap seeds.

## Workarounds

Pass `set:` keys in the order you want them emitted — a fix to IndexMap
will make this respected; for now it's a coin flip per process.

## Resume

Run `cargo tree -p indexmap` to confirm transitive availability. If
present, edit `src/tools/markdown/edit_markdown.rs:316` to use
`IndexMap`; cascade the type change into `frontmatter::apply_ops` at
`src/tools/markdown/frontmatter.rs:60`. Add the regression test above.
Verify the existing 9 frontmatter tests still pass, and verify the
live MCP probe with `set: {a, b, c}` produces `a, b, c` order on
repeated runs.

## References

- Surfaced during live verification of fix commit `4011ed2a` for
  `2026-05-18-edit-markdown-frontmatter-no-bootstrap.md`.
