---
id: '78a926d62d663912'
kind: bug
status: open
title: edit_code replace does not refuse a rename or a several-symbols body for a method inside a Rust impl block
owners:
- marius
tags:
- cluster/guard-narrower-than-its-name
- edit-code
opened: 2026-11-12
severity: medium
unverified: The cause is read from the source and agrees with one probe and five usage rows; no unit test with a mock LSP path `impl Agent/old` has run it. Whether callers broke after the five rows, and whether each was an intended rename, was not checked. The probe crate had no caller. Only Rust was examined; other languages' LSP paths were not compared with the AST's. Usage figures are from one checkout's usage.db and cover every agent that ran against it.
---

# BUG: edit_code replace does not refuse a rename or a several-symbols body for a method inside a Rust impl block

## Summary

`edit_code(action="replace")` refuses a body that declares a different symbol than its target ("`replace` cannot rename"). For a method inside a Rust `impl` block the refusal does not fire. The file is written and the call returns `status: ok`. The same shape on a free function, or on a function inside a `mod`, is refused. The refusal keeps the target-gone checks (`TargetRenamed`, `TargetReplacedBySeveral`, `TargetDropped`) behind a name comparison that is false for every impl method.

## Class fit

Guard narrower than its name. The guard is described as "refuse a replace that would delete the target and add another symbol", and it reads as covering every symbol. It covers only symbols whose LSP name path equals their AST name path, and Rust methods inside an `impl` block are not among them. It differs from the class's other members in that the gap is a whole language construct, and nothing reports it: the answer is a normal `ok` with an `also_defined` list.

## Symptom (Effect)

An agent replaces `Type/method` with a declaration under another name, or with several declarations. The call returns `ok`. The old method is gone, its callers are not updated, and the response lists the new name under `also_defined` but carries no refusal and no rename hint.

## Reproduction

Scratch crate `probe_rename` (rust-analyzer), session `5566e8a8`, with `lib.rs`:

```rust
pub struct Agent;

impl Agent {
    pub fn old(&self) -> u32 {
        1
    }
}

pub fn same_file_caller() -> u32 {
    2
}

pub fn keeper() -> u32 {
    3
}
```

| # | Call | Result |
|---|---|---|
| 1 | `edit_code(replace, symbol="same_file_caller", path="src/lib.rs", body="pub fn renamed_caller(extra: u32) -> u32 {\n    2 + extra\n}")` | refused: `...was given a complete declaration, but it declares `renamed_caller` instead of `same_file_caller`. `replace` cannot rename... File restored.` |
| 2 | `edit_code(replace, symbol="Agent/old", path="src/lib.rs", body="pub fn fresh(&self) -> u32 {\n    5\n}")` | `status: ok`, `replaced_lines: "4-6"`, `also_defined: ["Agent/fresh"]`. No refusal. |
| 3 | `symbols(path="src/lib.rs", depth=2)` | `impl Agent` L3-7 with the child `fresh` L4-6. `old` is gone. |

Call 1 is the control: same file, same kind of body, only the nesting differs. Call 2 addressed the method as `Agent/old`. Not tried: `symbol="impl Agent/old"`, which is the form `symbols` prints. The crate had no caller of `old`, so call 2 did not show a broken caller.

## Environment

codescout built after `9c96ac3e1`, Linux, rust-analyzer. Usage figures are from this checkout's `.codescout/usage.db` and cover every agent that ran against it.

## Root cause

Read at the bytes, not run in a test:

- `src/tools/symbol/edit_code.rs`, `do_replace`: `pre_count` is `count_symbols_by_name_path(pre_ast, &sym.name_path)`. `sym.name_path` is the LSP's path.
- `src/symbol/edit.rs`, `find_ast_name_path`, doc comment: "LSP and AST name_paths diverge on Rust impl blocks (LSP: `impl Type/m`, AST: `Type/m`)". The `gotchas` memory says the AST extractor emits no impl symbol.
- `src/symbol/query.rs`, `count_symbols_by_name_path`: exact equality on `name_path`. For an impl method it counts 0 in the AST.
- `src/symbol/edit.rs`, `corruption_verdict`: the block that returns `TargetRenamed`, `TargetReplacedBySeveral` and `TargetDropped` is inside `pre_count > 0 && ...`. With 0, none of them can return.
- `do_replace` already computes `target_ast_name_path` with `find_ast_name_path` (name and start line), which handles the divergence. It reaches `corruption_verdict` only for the "lost others" exemption, not for the count.

So the three target-gone verdicts are unreachable for impl methods. The probe and the five rows below agree with this reading. No unit test with a mock LSP path `impl Agent/old` has confirmed it.

## Evidence

`.codescout/usage.db`, `tool_calls`, window 2026-10-13 to 2026-11-11. Filter: `edit_code` with `action=replace`, a `.rs` path, and no `fn` in the body under the target's last segment.

Five calls on a method inside an impl block, all `success` with no refusal:

| Row | Symbol | Body declares |
|---|---|---|
| 195606 | `impl DoctorScope/below_floor` | `below_floor_in_scope`, `below_floor_out_of_scope`, `would_admit` |
| 280062 | `MockLspClient/with_server_busy` | `with_cold_start_window` |
| 280063 | `MockLspClient/server_busy` | `cold_start_window_open` |
| 281762 | `CodeScoutServer/replay_pin_ack` | `replay_ack` |
| 314470 | `LspClient/resync_if_drifted` | `resync_all_drifted` |

Rows 280062, 280063, 281762 and 314470 carry `also_defined` with the new name in AST form (`MockLspClient/with_cold_start_window`), the same signature as call 2 above. The output of row 195606 was cut at 230 bytes and not read to the end.

In the same window, 34 calls of the same shape on other symbols were refused as `replace_would_rename`: 15 on top-level symbols and 19 on nested ones such as `tests/...`. Eight more successes with an `impl ...` symbol replaced a whole impl block (the body is its methods), so they are not counted.

Script: `replace-differently-named-fn-v2.py` in the session scratchpad (not in the repo).

## Hypotheses tried

- A stale LSP range: **not supported**. The range 4-6 in call 2 is the method's own range, and the AST re-extraction found the new name.
- The caller's name form (`Agent/old` against `impl Agent/old`) is the cause: **not tested**. By the code read above `sym.name_path` comes from the LSP either way.

## Fix

Pending, for the owner. A candidate, none tried: count the target in the AST frame, that is `count_symbols_by_name_path(pre_ast, target_ast_name_path.as_deref().unwrap_or(&sym.name_path))`, and pass that same path to `corruption_verdict` as `counted_name_path`, so the pre and post counts use one dialect. A cost: the `corruption_verdict` tests pin its arguments, and `do_remove` passes `pre_count = 0` on purpose.

## Tests added

None. A red test would use `MockLspClient` with the name path `impl Agent/old` over a real file, in the shape of `a_replace_whose_body_renames_is_told_to_use_rename` in `src/tools/symbol/tests.rs`.

## Workarounds

For a method in an impl block, send `rename` first, then `replace` on the new name, and check callers with `references`. Do not rely on the refusal for a Rust method.

## Resume

Open, medium severity. Next action: write the red test described under Tests added, run it on HEAD and watch it go red for the stated reason, then try the candidate in Fix. Not measured: whether callers broke after the five rows, whether each was an intended rename, and whether Kotlin, Java or Python methods show the same gap (their LSP paths were not compared with the AST's). Related: `docs/issues/2026-11-11-edit-code-replace-refuses-a-declaration-with-a-new-name-so-a-rename-with-a-body-change-takes-two-calls.md` (the refusal that this one fails to raise), and the archived bug of 2026-09-13 that added `TargetRenamed`.

## References

- `src/tools/symbol/edit_code.rs`: `do_replace`, `pre_count`, `target_ast_name_path`
- `src/symbol/edit.rs`: `corruption_verdict`, `find_ast_name_path`
- `src/symbol/query.rs`: `count_symbols_by_name_path`
