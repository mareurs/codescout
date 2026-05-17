# `json_path` negative index + slice support

**Status:** design • **Date:** 2026-05-18 • **Bug:** [`docs/issues/2026-05-17-read-file-jsonpath-negative-slice.md`](../../issues/2026-05-17-read-file-jsonpath-negative-slice.md)

## Summary

Extend `read_file`'s `json_path` parameter to accept negative-index (`[-N]`) and negative-start open-end slice (`[-N:]`) syntax. Currently both forms are silently parsed as failed `usize` lookups and returned to the caller as misleading `"path segment '[-1]' not found"` errors. This spec adds the two forms, distinguishes unsupported-syntax errors from out-of-bounds errors from not-found errors, and restructures the segment representation from `Vec<String>` to a typed enum so future grammar growth is additive.

## Goals

1. `$.symbols[-1]` returns the last element of `symbols`.
2. `$.symbols[-3:]` returns the last three elements as a JSON array.
3. Out-of-range negative access (`[-100]` or `[-100:]` on a 3-element array) returns a `RecoverableError` with a clear "out of bounds" message — NOT the current "not found" message.
4. Unsupported syntax (`[1:3]`, `[::2]`, `[-0]`, `[abc]`) is rejected at parse time with an "unsupported json_path segment" error that lists the supported forms.
5. Every path that worked before still works (positive index, key access, chained access).

## Non-goals

- Positive slice forms (`[N:]`, `[:M]`, `[N:M]`). Future PRs may add them; the typed grammar makes that additive.
- Slice with step (`[a:b:c]`, including reverse `[::-1]`).
- JSONPath-RFC-9535 features (`[?(…)]` filters, `..` descendants, `[*]` wildcards). If those are ever wanted, swap in a real jsonpath crate; do not hand-roll on top of this.
- Negative-start positive-end slices (`[-3:5]`) or open-start negative-end (`[:-1]`).

## Architecture

Two functions plus one enum, all in `src/tools/file_summary/file_summary.rs`:

```rust
enum Segment {
    Key(String),         // .field
    Index(usize),        // [N]   (non-negative)
    NegIndex(usize),     // [-N]  (positive magnitude, sign implicit)
    NegSliceFrom(usize), // [-N:] (last N elements)
}

fn parse_json_path_segments(path: &str) -> Result<Vec<Segment>, RecoverableError>;
fn resolve_json_segment<'a>(value: &'a Value, seg: &Segment)
    -> Result<Cow<'a, Value>, RecoverableError>;
```

`extract_json_path` walks the `Vec<Segment>` threading `Cow<'a, Value>` through the loop. Before the first slice segment every step is `Cow::Borrowed` (zero clones). The first slice flips the chain to `Cow::Owned`; subsequent steps stay owned via `into_owned()` re-borrow. Caller-visible signature is unchanged.

### Why typed enum (not stringly-typed segments)

- **Parse once.** Today, brackets are parsed in `parse_json_path_segments` and re-inspected inside `resolve_json_segment` (`segment.starts_with('[') && …`). The enum collapses this.
- **Invalid syntax fails at parse time.** Out-of-grammar input cannot reach the resolver; misleading "not found" errors for syntax errors disappear.
- **Future grammar growth is additive.** New form = new variant + parse arm + resolve arm. No flag-day signature change.

### Why `Cow<'a, Value>` (not always-borrowed or always-owned)

- Always-borrowed is impossible — slice produces an owned sub-array.
- Always-owned clones every step, including key access and non-neg-index walks that currently borrow for free.
- `Cow` is uniform across all segment kinds, slight enum-match overhead, no lifetime gymnastics outside the one match in `extract_json_path`.

## Parser grammar

| Input form | Produces | Notes |
|---|---|---|
| `.key` / bare `key` after `$` | `Key("key")` | unchanged |
| `[N]` where N ≥ 0 | `Index(N)` | unchanged |
| `[-N]` where N ≥ 1 | `NegIndex(N)` | new |
| `[-N:]` where N ≥ 1 | `NegSliceFrom(N)` | new |

Rejected at parse time with `RecoverableError`:

| Input | Reason |
|---|---|
| `[1:]`, `[:3]`, `[1:3]`, `[a:b:c]` | positive slice — out of scope |
| `[-1:3]`, `[:-1]`, `[::-1]` | mixed / step slice — out of scope |
| `[abc]`, `[]`, `[+1]` | malformed |
| `[-0]`, `[-0:]` | `-0` not a meaningful negative index — use `[0]` |
| `[?(…)]`, `..foo`, `[*]` | JSONPath-RFC extensions — out of scope |

Rejection message: `"unsupported json_path segment '[{inner}]'"` with hint `"Supported forms: '.key', '[N]' (non-negative integer), '[-N]' (negative integer), '[-N:]' (last N elements). Other slice/filter forms not supported."`

Parse algorithm:

```
for part in path.strip_prefix("$.").or("$").unwrap_or(path).split('.'):
  if empty: continue
  if part contains '[':
    split into <key_prefix> + zero-or-more <[inner]> brackets
    push Key(key_prefix) if non-empty
    for each [inner]:
      all-digits          -> Index(parse::<usize>)
      "-" + digits        -> NegIndex(parse) ; reject if magnitude == 0
      "-" + digits + ":"  -> NegSliceFrom(parse) ; reject if magnitude == 0
      else                -> Err(unsupported segment) with hint
  else:
    push Key(part)
```

Chained brackets like `$.items[0][-1]` parse to `[Key("items"), Index(0), NegIndex(1)]` — mirrors the existing positive-chain behavior.

## Resolver semantics

| Segment | On Object | On Array | On Other (string/number/null) |
|---|---|---|---|
| `Key(k)` | `Cow::Borrowed(obj.get(k))` or `Err(not-found)` | `Err(type-mismatch)` | `Err(type-mismatch)` |
| `Index(n)` | `Err(type-mismatch)` | `Cow::Borrowed(arr.get(n))` or `Err(oob)` | `Err(type-mismatch)` |
| `NegIndex(n)` | `Err(type-mismatch)` | if `n ≤ arr.len()` → `Cow::Borrowed(&arr[arr.len()-n])`; else `Err(oob)` | `Err(type-mismatch)` |
| `NegSliceFrom(n)` | `Err(type-mismatch)` | if `n ≤ arr.len()` → `Cow::Owned(Value::Array(arr[arr.len()-n..].to_vec()))`; else `Err(oob)` | `Err(type-mismatch)` |

Walk:

```rust
let mut current: Cow<'_, Value> = Cow::Borrowed(&parsed);
for seg in &segments {
    current = match current {
        Cow::Borrowed(v) => resolve_json_segment(v, seg)?,
        Cow::Owned(v)    => resolve_json_segment(&v, seg)?.into_owned().into(),
    };
}
```

The `Cow::Owned` arm forces an owned re-borrow because the resolver returns a `Cow<'a, Value>` tied to the input's lifetime; once we own the chain, each subsequent step takes `&v` (where `v: Value`) and returns `Cow<'a_local, Value>`, which we `into_owned()` to extend the lifetime. One clone per `Owned` step after the first slice; before the first slice, zero clones.

## Error catalog (caller-visible)

| Trigger | New message | Old behavior |
|---|---|---|
| `$.symbols[-1]` on present array | success: last element | `"path segment '[-1]' not found"` (the bug) |
| `$.symbols[-3:]` on len ≥ 3 array | success: array of last 3 | same `"not found"` |
| `$.symbols[-100]` on 3-element array | `"index -100 out of bounds for array of length 3"` + hint `"Use a non-negative index in 0..3 or a negative index in -3..-1"` | `"not found"` (wrong semantic) |
| `$.symbols[-100:]` on 3-element | `"index -100 out of bounds for array of length 3"` + hint `"For slice '[-N:]', N must be in 1..=3"` | `"not found"` |
| `$.symbols[abc]` | `"unsupported json_path segment '[abc]'"` + hint listing supported forms | `"not found"` (misleading) |
| `$.symbols[1:3]` | `"unsupported json_path segment '[1:3]'"` + hint | `"not found"` (misleading) |
| `$.symbols[-0]` / `$.symbols[-0:]` | `"unsupported json_path segment '[-0]'"` + hint `"Use [0] for the first element"` | parses as `Index(0)` (silent accidental match) |
| `$.items.name` where `items` is array | `"cannot apply key 'name' to array (expected object)"` + hint `"Use [N] to index into an array."` | `"not found"` |
| `$.obj[0]` where `obj` is object | `"cannot apply index '[0]' to object (expected array)"` + hint `"Use .key to access an object field."` | `"not found"` |
| `$.s[0]` where `s` is a string/number/null | `"cannot apply index '[0]' to string (expected array)"` + hint `"Segment requires array or object."` | `"not found"` |

**Hint consistency:** OOB hints state the valid range in *both directions* for single-index, and the valid N range for slice. Callers reading the hint should be able to self-correct without re-parsing the array elsewhere.
## Public API impact

- `read_file` tool — no schema change. Same `json_path` parameter, same response shape.
- `extract_json_path` — same return type `Result<(String, String, Option<usize>), RecoverableError>`.
- `parse_json_path_segments` — return type changes from `Vec<String>` to `Vec<Segment>`. Internal helper; not exported outside the module per `pub` audit (sub-section below).
- `resolve_json_segment` — signature changes from `fn(&'a Value, &str) -> Option<&'a Value>` to `fn(&'a Value, &Segment) -> Result<Cow<'a, Value>, RecoverableError>`. Internal helper.

### `pub` audit before merge

Verified at design time (2026-05-18): both `parse_json_path_segments` and `resolve_json_segment` are private (no `pub` qualifier in `src/tools/file_summary/file_summary.rs:470, 497`) and each has exactly one caller — `extract_json_path` itself at lines 430 and 434. No external module imports either symbol. Signature changes in this spec are internal-only; no breaking external surface.
## Dependencies

None. `Cow` from `std::borrow`. `serde_json::Value` already in use.

## Tests

All in `src/tools/file_summary/file_summary.rs` `mod tests`, co-located with `extract_json_path_array_index` and the regression-pin from the related wontfix bug (`read_file_buffer_json_path_array_element_returns_value`).

### Parser tests (10)

```
parse_empty_path_returns_empty_segments  ""            => []
parse_root_only                          "$"           => []
parse_negative_single_index              "$.a[-1]"     => [Key("a"), NegIndex(1)]
parse_negative_slice_from                "$.a[-3:]"    => [Key("a"), NegSliceFrom(3)]
parse_chained_negative_after_positive    "$.a[0][-1]"  => [Key("a"), Index(0), NegIndex(1)]
parse_top_level_negative_index           "$[-1]"       => [NegIndex(1)]
parse_rejects_positive_slice             "$.a[1:3]"    => Err(unsupported) + hint
parse_rejects_slice_with_step            "$.a[::2]"    => Err(unsupported)
parse_rejects_open_end_positive          "$.a[1:]"     => Err(unsupported)
parse_rejects_negative_zero              "$.a[-0]"     => Err(unsupported) + hint
parse_rejects_non_integer_bracket        "$.a[abc]"    => Err(unsupported)
```
### Resolver tests (8)

```
extract_root_returns_parsed
  input: {"a":1}, path: "$"
  expect: returns the whole object, type "object", count Some(1)

extract_top_level_negative_index
  input: ["a","b","c"], path: "$[-1]"
  expect: ("c", "string", None)

extract_negative_index_returns_last_element
  input: {"items":["a","b","c"]}, path: "$.items[-1]"
  expect: ("c", "string", None)

extract_negative_slice_returns_tail
  input: {"items":["a","b","c","d"]}, path: "$.items[-2:]"
  expect: pretty array of [\"c\", \"d\"], type "array", count Some(2)

extract_negative_index_oob_returns_clear_error
  input: {"items":["a"]}, path: "$.items[-5]"
  expect: Err message contains "out of bounds" and "length 1"

extract_negative_slice_oob_returns_clear_error
  input: {"items":["a"]}, path: "$.items[-5:]"
  expect: Err message contains "out of bounds" and "length 1"

extract_mid_path_slice_then_index
  input: {"items":[{"v":1},{"v":2},{"v":3}]}, path: "$.items[-2:][0].v"
  expect: ("1", "number", None)   — exercises Cow-Owned re-borrow

extract_unsupported_syntax_distinguished_from_not_found
  input: {"items":["a"]}, path: "$.items[1:3]"
  expect: Err message contains "unsupported json_path segment", NOT "not found"
```
### Regression

Existing tests stay green:

- `extract_json_path_array_index` (positive index) — in `src/tools/file_summary/file_summary.rs` `mod tests`
- `read_file_buffer_json_path_array_element_returns_value` (positive index + property, regression pin from 2026-05-09 wontfix) — at `src/tools/read_file.rs:1003`, **different file** from where the new parser/resolver tests live; both must pass.
- All key-access and chained-positive paths

Total new tests: **18** (10 parser + 8 resolver). Each maps to a single grammar arm, a single resolver behavior, or a contract corner (empty path, root, top-level array, mid-path slice) — no test does double duty.
## Implementation order (for the plan)

1. Define `Segment` enum (no behavior change yet, no tests — enum is consumed in step 2).
2. Rewrite `parse_json_path_segments` to return `Vec<Segment>` + all 8 parser tests. Old single-caller `extract_json_path` continues to call the new parser; the old `resolve_json_segment(&str)` is temporarily fed the `Debug` rendering of segments OR the parser is wrapped to also emit a parallel `Vec<String>` for the unchanged resolver. **Picking the latter is too fragile** — instead, merge steps 2 and 3 (see step 3 note).
3. Rewrite `resolve_json_segment` to take `&Segment` + return `Result<Cow<'a, Value>, RecoverableError>` + rewrite the loop in `extract_json_path` to thread `Cow<'a, Value>` — **in the same commit** as the parser rewrite. The signature change in step 3 makes the old loop in `extract_json_path` non-compiling; tests cannot run between parser rewrite and loop rewrite. Land both together as one logical commit. Add all 6 resolver tests in this commit.
4. `cargo fmt && cargo clippy -- -D warnings && cargo test`. Regression pin `tools::read_file::tests::read_file_buffer_json_path_array_element_returns_value` at `src/tools/read_file.rs:1003` must remain green.
5. Live MCP verification: `cargo build --release` + `/mcp` restart + manual `read_file(json_path="$.symbols[-1]")` and `read_file(json_path="$.symbols[-3:]")` against a populated buffer.

**Why steps 2+3 merge.** Yak's "tests after each step" rule cannot hold across a signature-change boundary. The parser-returning-`Vec<Segment>` + resolver-taking-`&str` combination is non-compiling; no intermediate commit can be green there. Merge into one commit so the green-bar discipline survives.
## Open questions

None at design time. All grammar / lifetime / error / test decisions are locked above.

## References

- Bug: `docs/issues/2026-05-17-read-file-jsonpath-negative-slice.md`
- Related closed bug: `docs/issues/2026-05-09-read-file-json-path-array-elements.md`
- Brainstorm session: Pika scan in `cc_session_id=42874b1a-1ef5-44ce-ad64-4eb5b84cf93f`, surfaced as `pika_observations.id=58` with `bug_id` set.
- Existing code: `src/tools/file_summary/file_summary.rs` — `extract_json_path` (line 419), `parse_json_path_segments` (line 470), `resolve_json_segment` (line 497).
