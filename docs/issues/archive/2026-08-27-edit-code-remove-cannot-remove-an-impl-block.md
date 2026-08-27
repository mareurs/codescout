---
id: e3407869eeb7337e
kind: bug
status: fixed
title: 'BUG: edit_code(remove) can never remove an impl block — its own methods trip the sibling guard, and both escapes the hint offers are closed'
tags:
- edit_code
- tooling
- guard
- false-positive
closed: 2026-08-27
opened: 2026-08-27
owner: marius
related:
- docs/issues/archive/2026-08-27-unregistered-memory-tool-structs-read-as-the-live-tool.md
severity: low
---

## Summary

`edit_code(action="remove", symbol="impl Trait for Type")` **can never succeed**. The
guard that protects against an overshooting LSP range treats the impl block's own methods
as "sibling symbols" that would be dropped — but an impl block's methods are *always*
inside its range, by definition. So the check fires on every well-formed removal.

The error text then misdiagnoses it as a stale range, and its hint sends the caller down
a route that also cannot work.

## Symptom

```
edit_code(action="remove", path="src/tools/memory/mod.rs",
          symbol="impl Tool for DeleteMemory")

→ edit_code remove('impl Tool for DeleteMemory') would have dropped sibling symbols:
    DeleteMemory/call, DeleteMemory/description, DeleteMemory/input_schema,
    DeleteMemory/name.
  The range overshot into adjacent code (likely a stale LSP range). File restored.
  hint: Try symbols(path) to refresh, then retry; or narrow the edit via edit_file
        with unique anchors.
```

Every name in that list is a **child** of the symbol being removed, not a sibling.

## Reproduction

Measured 2026-08-27 on `src/tools/memory/mod.rs`:

1. `edit_code(remove, symbol="impl Tool for WriteMemory")` → fails as above.
2. `symbols(path="src/tools/memory/mod.rs")` — ranges confirmed fresh and correct
   (`Object 214-263 impl Tool for DeleteMemory`, methods `215-217`, `218-220`, `221-233`,
   `234-262`, all strictly inside).
3. `edit_code(remove, symbol="impl Tool for DeleteMemory")` → **fails identically.**

Two different impl blocks, once before and once after an explicit refresh. Not a stale
range.

## Root cause

Not read. The behaviour is consistent with the removal guard collecting every symbol
whose span intersects the target's span, then rejecting if any of them is not the target
itself — correct for a genuinely overshooting range, wrong for any container symbol
(`impl` blocks, and presumably `mod` blocks and nested types too).

The discriminator the guard is missing is *containment*: a symbol strictly inside the
target's span is a child being removed on purpose; one that merely overlaps is evidence
of an overshoot.

## Both suggested escapes are closed

The hint offers two routes, and in this repo neither is open:

1. **"Try `symbols(path)` to refresh, then retry."** Step 2 of the reproduction did
   exactly this. The ranges were already correct, so the refresh changes nothing and the
   retry fails the same way. The hint describes a cause that is not the cause, so
   following it costs a round trip and teaches the wrong lesson.
2. **"Narrow the edit via `edit_file` with unique anchors."** `edit_file` is refused by
   the IL-2 gate whenever the edit spans a symbol definition — which removing an `impl`
   block necessarily does. So the fallback is blocked by a different guard.

The only route through was a raw `python3` script rewriting the file by verified line
range. That works, but it is precisely the unstructured edit `edit_code` exists to
replace, and it carries none of the range validation.

## Impact

Low frequency, total when it fires. Deleting a trait implementation is not a common edit,
but when it is needed there is no supported path — and the guidance actively points away
from the workaround. It also means `edit_code`'s `remove` action is unusable for the one
class of symbol where the blast radius is largest and its validation would be worth most.

## Fix

**`a1a56305651554cfd95b04e745d5781efa0ac0e3`** (`experiments`)
patch-id **`5b04e1d8892268f5c841e1e8244664e38f37ede3`**

### Root cause, read rather than inferred

The descendant split this class needs **already existed** — `split_target_subtree`, added for
non-empty modules (`docs/issues/archive/2026-08-11-edit-code-cannot-remove-nonempty-module.md`
gap 2). It keys on the target's **AST** `name_path`, obtained via `find_ast_name_path`.

For an impl block there is none to find, and that is deliberate. `src/ast/parser.rs`:

```rust
// Don't create a symbol for impl blocks; merge methods at the
// current level, so a method reads as `Type/method`.
```

So `find_ast_name_path` returns `None` — necessarily, not by weak matching — the `match` falls to
the arm that skips the split entirely, and the block's own methods land in the sibling set. That
is also why the error printed `Thing/hello` rather than `impl Greet for Thing/hello`: the reported
names come from the AST namespace, where the impl block does not exist.

This corrects the file's own `unverified:` note — the guard's source has now been read, and the
inferred cause ("treats children as siblings") was right about the symptom and silent about the
mechanism that matters for the fix.

### The proposed fix would have been worse than the bug — measured

The *Fix* section originally proposed span containment: "a symbol whose span lies entirely within
the target's span is a child." Implemented literally, that is a **regression**, because the
target's span comes from the LSP and a stale LSP range is precisely what this guard exists to
doubt. When the range overshoots, the swallowed neighbour lies *inside* the reported span, is
reclassified as a descendant, and is deleted silently.

Run as mutation B against the new overshoot test, span containment returned:

```json
{"status": "ok", "removed_lines": "7-15",
 "removed_descendants": ["Thing/hello", "keep_me"]}
```

`keep_me` — a true sibling — deleted under `status: "ok"`. **An over-refusal traded for a silent
over-delete**, which is the strictly worse direction: today's bug at least fails loudly.

### What shipped instead

`descendant_ast_paths` (`src/symbol/edit.rs`) keys the descendant set on the target's **LSP child
list**, mapping each child into the AST namespace with the existing `find_ast_name_path`. It names
the same methods without consulting the suspect range, and it cannot grow under an overshoot — an
adjacent symbol is not a child however far the range runs, so the guard's real job survives
intact. A child that does not resolve into the AST namespace stays in the sibling set:
unresolvable means unproven.

The new `match` arm is generic over *"target has no AST name_path"*, not special-cased to Rust
impl blocks, so any future extractor that hoists a container's members is covered by the same
path.

### Tests

Both in `src/tools/symbol/tests.rs`, sharing one fixture whose trait method and impl method
**deliberately share a name** — the realistic shape, and it exercises the start-line
disambiguation inside `find_ast_name_path`.

| test | mutation that breaks it, and only it |
|---|---|
| `edit_code_remove_deletes_an_impl_block_and_names_its_methods` | remove the new `match` arm |
| `edit_code_remove_still_refuses_when_an_overshooting_range_swallows_a_sibling` | swap the child list for span containment |

Both mutations were run with the blast radius predicted in advance and matched exactly (1 failure
each). Mutation A reproduced the filed error message verbatim, confirming the test covers the real
defect rather than a lookalike.

Gate: `cargo fmt`, `cargo clippy --workspace --all-targets --features local-embed -- -D warnings`,
`cargo test` — **4732 passed, 0 failed**.
## Live verification (2026-08-27, post-`cargo rb` + `/mcp`)

Freshness green on all three axes first: binary `22:40:44` > last commit `22:26:50`; serving pid
`2885849` parented to this session, started `22:40:56` > build.

The call that could never succeed, run against the rebuilt server on a scratch fixture with a
trait, an impl block, and a true sibling below it:

```
edit_code(action="remove", symbol="impl Greet for Thing", path="recon-impl-live.rs")
→ {"status": "ok", "removed_lines": "8-15", "line_count": 8,
   "removed_descendants": ["Thing/bye", "Thing/hello"]}
```

Reading the file back: the impl block is gone, `trait Greet` above it survives, `fn keep_me()` below
it survives. The descendants are reported under their **AST** name paths (`Thing/*`), which is the
naming divergence that caused the bug — now surfaced as information instead of as a false sibling-drop
refusal.

Before this fix the identical call rolled back every time.
## Not established

Answered.

- **`mod` blocks**: already work, and are guarded by
  `edit_code_remove_deletes_a_non_empty_module_and_names_its_children`. `mod_item` *does* get an
  AST symbol, so `find_ast_name_path` resolves it and the existing prefix split runs.
- **Other Rust containers**: `trait_item` and `struct_item` also emit symbols. `impl_item` is the
  **only** Rust container the extractor deliberately omits, which is why it was the only one that
  failed.
- **Other languages**: not enumerated, and deliberately not needed — the new arm triggers on the
  general condition (*no AST name_path for the target*) rather than on a Rust impl block, so any
  extractor that hoists a container's members takes the same path.
- **The guard's source**: read. See § *Fix*.
