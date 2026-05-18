---
status: fixed
opened: 2026-05-18
closed: 2026-05-18
severity: low
owner: marius
related: [src/tools/markdown/edit_markdown.rs, src/tools/markdown/frontmatter.rs]
tags: [edit_markdown, frontmatter, non-determinism]
kind: bug
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


Approach: change `apply_ops` to take `&serde_json::Map<String, Value>`
instead of `&HashMap<String, Value>`. serde_json's `preserve_order`
feature is already enabled at `Cargo.toml:17,56`, so `as_object()`
returns an order-preserving map. Zero new dependency.

**Changes:**
- `src/tools/markdown/frontmatter.rs:60` — signature updated.
- `src/tools/markdown/edit_markdown.rs:316-320` — drop the
  HashMap-collect, clone the `serde_json::Map` directly.
- Test sites (9 occurrences in `frontmatter.rs`) — `HashMap::new()`
  → `serde_json::Map::new()`. Same insert API.
- `use std::collections::HashMap;` dropped from `frontmatter.rs:11`
  (no longer referenced).
- Stale comment `// HashMap iteration order is unstable — check
  membership not position` removed from
  `reserved_literal_strings_get_quoted` test.

**Tests added:**
- `bootstrap_emits_keys_in_caller_order` at
  `src/tools/markdown/frontmatter.rs:253` — asserts the same keys in
  reversed insertion order produce reversed output, locking in the
  order-preservation contract.

**Verification:**
- 36 frontmatter tests pass (previously 35 + the new regression).
- Full `cargo test --lib` — 2387 passed, 0 failed, 7 ignored.
- `cargo clippy --lib --tests` clean.

**Commit:** `6d804455` on `experiments`.

## Tests added


`bootstrap_emits_keys_in_caller_order` at
`src/tools/markdown/frontmatter.rs:253`. Asserts that two
serde_json::Map calls with reversed key insertion order produce
reversed output — locks in the preserve_order contract.

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
