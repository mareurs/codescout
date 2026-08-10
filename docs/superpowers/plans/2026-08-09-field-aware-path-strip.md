---
id: '8ac062511ae99092'
kind: plan
status: done
title: Field-aware path stripping — implementation plan
owners:
- marius
tags:
- post-process
- path-display
- output-fidelity
- call_content
- plan
topic: path-display-and-output-fidelity
---

# Field-Aware Path Stripping Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Strip the project-root prefix from tool output by JSON **key** on the typed `serde_json::Value`, instead of by textual lookbehind on rendered text — eliminating both the corruption of path literals inside file content and the collapse of root-valued fields to `""`.

**Architecture:** A pure walker (`src/tools/core/path_strip.rs`) relativizes values under an **allowlist** of path keys, leaving everything else — content, prose, unknown keys — byte-identical. It is invoked once, in the `Tool::call_content` default implementation immediately after `self.call(...)`, so every downstream consumer (`exceeds_inline_limit`, the `@tool_*` buffer payload, `format_compact`, the pretty-printer) sees already-relative values. `server::post_process` loses its strip entirely and keeps only the once-per-activation banner.

**Tech Stack:** Rust 2021, `serde_json`, `async_trait`, `tokio` + `tempfile` for tests.

## Global Constraints

- Spec: `docs/superpowers/specs/2026-08-09-field-aware-path-strip-design.md`. Bug: `docs/issues/2026-08-09-path-strip-corrupts-file-content-and-root-fields.md`.
- **Allowlist only.** A key absent from `PATH_KEYS` keeps its absolute path. Never invert into a denylist of content keys — an unknown key would then be stripped by default, which is the defect being removed.
- **Never emit `""` for a path.** Any relativization that would produce an empty string leaves the value unchanged.
- **Roots stay absolute.** `project_root`, `git_root`, `root`, `old_root`, `new_root`, `cwd`, `repo_root` are anchors, not paths.
- **Errors are never stripped.** `route_tool_error` (`src/server.rs:1129`, invoked at `src/server.rs:794`) produces no `Value` and must stay byte-faithful — `not_found_msg` (`src/tools/edit_file/mod.rs:196`) embeds raw file bytes by design.
- **Ordering invariant:** strip → `format_compact` → buffer. Reversed, the buffered-summary leak returns.
- Pre-commit gate (CLAUDE.md): `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test` must all pass before each task's commit.
- Branch `experiments`. Never commit to `master`.
- Windows carry-over: values built with `.display().to_string()` keep native separators while the prefix is forward-slash. That mismatch exists today and is **not** addressed here; do not "fix" it in this plan.

---

## File Structure

| File | Responsibility |
|---|---|
| `src/tools/core/path_strip.rs` **(create)** | The allowlists and the pure `Value` walker. No I/O, no async, no ctx. |
| `src/tools/core/mod.rs` **(modify)** | Register the new module. |
| `src/tools/core/types.rs:546-679` **(modify)** | Call the walker once in `Tool::call_content`, immediately after `self.call(...)`. |
| `src/server.rs:527-579` **(modify)** | `post_process` keeps the banner, loses the strip and the `run_command` special case. |
| `src/server.rs:1662-1744` **(delete)** | `strip_project_root_from_result` and `strip_prefix_from_text`, including the bare-root branch. |
| `src/server.rs:1746+` **(modify)** | Repair the four affected tests; add the corpus gate. |
| `src/prompts/source.md` + guide body **(modify)** | Correct the `progressive-disclosure` guide text. |

---

## Task 1: The path-strip walker

**Files:**
- Create: `src/tools/core/path_strip.rs`
- Modify: `src/tools/core/mod.rs`

**Interfaces:**
- Consumes: nothing (leaf module).
- Produces: `pub(crate) fn strip_paths_in_value(val: &mut serde_json::Value, root_prefix: &str)` — `root_prefix` MUST end with `/`; an empty prefix is a no-op. Also `pub(crate) const PATH_KEYS: &[&str]` and `pub(crate) const ROOT_KEYS: &[&str]`.

- [ ] **Step 1: Write the failing tests**

Create `src/tools/core/path_strip.rs` containing only the test module for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const ROOT: &str = "/home/u/proj/";

    #[test]
    fn relativizes_a_path_key() {
        let mut v = json!({ "file": "/home/u/proj/src/lib.rs" });
        strip_paths_in_value(&mut v, ROOT);
        assert_eq!(v["file"], "src/lib.rs");
    }

    #[test]
    fn leaves_file_content_untouched() {
        // The reported bug: a quoted path literal inside content must survive.
        let src = "REPO = \"/home/u/proj/.worktrees/single-stage\"";
        let mut v = json!({ "content": src, "stdout": src, "body": src, "text": src });
        strip_paths_in_value(&mut v, ROOT);
        assert_eq!(v["content"], src);
        assert_eq!(v["stdout"], src);
        assert_eq!(v["body"], src);
        assert_eq!(v["text"], src);
    }

    #[test]
    fn never_produces_an_empty_string() {
        // A bare root under a path key must NOT collapse to "" — it does not
        // match a slash-terminated prefix, and the guard is belt-and-braces.
        let mut v = json!({ "path": "/home/u/proj", "abs_path": "/home/u/proj/" });
        strip_paths_in_value(&mut v, ROOT);
        assert_eq!(v["path"], "/home/u/proj");
        assert_eq!(v["abs_path"], "/home/u/proj/");
    }

    #[test]
    fn root_keys_stay_absolute() {
        let mut v = json!({
            "project_root": "/home/u/proj",
            "git_root": "/home/u/proj",
            "root": "/home/u/proj",
            "cwd": "/home/u/proj"
        });
        strip_paths_in_value(&mut v, ROOT);
        assert_eq!(v["project_root"], "/home/u/proj");
        assert_eq!(v["git_root"], "/home/u/proj");
        assert_eq!(v["root"], "/home/u/proj");
        assert_eq!(v["cwd"], "/home/u/proj");
    }

    #[test]
    fn scope_block_abs_path_survives_while_item_abs_path_relativizes() {
        // Same key, two meanings. The trailing slash is what discriminates:
        // the scope root is stored bare and cannot match ROOT.
        let mut v = json!({
            "items": [{ "abs_path": "/home/u/proj/docs/t.md" }],
            "scope":  { "abs_path": "/home/u/proj", "git_root": "/home/u/proj" }
        });
        strip_paths_in_value(&mut v, ROOT);
        assert_eq!(v["items"][0]["abs_path"], "docs/t.md");
        assert_eq!(v["scope"]["abs_path"], "/home/u/proj");
    }

    #[test]
    fn relativizes_an_array_of_paths() {
        // tree's `entries` is an array of bare strings, not objects.
        let mut v = json!({ "entries": ["/home/u/proj/src/", "/home/u/proj/docs/"] });
        strip_paths_in_value(&mut v, ROOT);
        assert_eq!(v["entries"], json!(["src/", "docs/"]));
    }

    #[test]
    fn recurses_into_nested_objects_and_arrays() {
        let mut v = json!({
            "file_groups": [{ "file": "/home/u/proj/a.rs", "matches": [
                { "file": "/home/u/proj/b.rs", "line_text": "x = \"/home/u/proj/c\"" }
            ]}]
        });
        strip_paths_in_value(&mut v, ROOT);
        assert_eq!(v["file_groups"][0]["file"], "a.rs");
        assert_eq!(v["file_groups"][0]["matches"][0]["file"], "b.rs");
        assert_eq!(
            v["file_groups"][0]["matches"][0]["line_text"],
            "x = \"/home/u/proj/c\""
        );
    }

    #[test]
    fn unknown_key_keeps_its_absolute_path() {
        // Fail-verbose, never fail-corrupt.
        let mut v = json!({ "some_new_key": "/home/u/proj/src/lib.rs" });
        strip_paths_in_value(&mut v, ROOT);
        assert_eq!(v["some_new_key"], "/home/u/proj/src/lib.rs");
    }

    #[test]
    fn empty_prefix_is_a_no_op() {
        let mut v = json!({ "file": "/home/u/proj/src/lib.rs" });
        strip_paths_in_value(&mut v, "");
        assert_eq!(v["file"], "/home/u/proj/src/lib.rs");
    }

    #[test]
    fn a_path_outside_the_root_is_untouched() {
        let mut v = json!({ "file": "/etc/hosts" });
        strip_paths_in_value(&mut v, ROOT);
        assert_eq!(v["file"], "/etc/hosts");
    }
}
```

Register the module — add to `src/tools/core/mod.rs`, next to the existing `mod` declarations:

```rust
pub(crate) mod path_strip;
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib path_strip`
Expected: FAIL to compile — `cannot find function 'strip_paths_in_value' in this scope`.

- [ ] **Step 3: Write the implementation**

Prepend to `src/tools/core/path_strip.rs`, above the test module:

```rust
//! Field-aware project-root stripping.
//!
//! Renders absolute project paths relative **by JSON key**, on the typed
//! `Value`, before it is rendered to text. This replaces the text-level
//! transform that lived in `server::post_process`: that one ran after the
//! `Value` had been flattened to a string, so it could only guess from a
//! one-character lookbehind — which stripped path literals inside file
//! content and collapsed root-valued fields to the empty string.
//!
//! See `docs/issues/2026-08-09-path-strip-corrupts-file-content-and-root-fields.md`
//! and `docs/superpowers/specs/2026-08-09-field-aware-path-strip-design.md`.

use serde_json::Value;

/// Keys whose values are paths to be rendered relative to the project root.
///
/// **This is an ALLOWLIST and must stay one.** A key that is absent keeps its
/// absolute path — that costs tokens but never corrupts. Inverting this into a
/// denylist of content keys would strip unknown keys by default, which is
/// precisely the defect this module exists to remove. When a new tool's paths
/// come back absolute, add its key here; the corpus gate in `src/server.rs`
/// (`no_absolute_project_paths_in_rendered_output`) is what makes the omission
/// visible.
pub(crate) const PATH_KEYS: &[&str] = &[
    "abs_path",
    "deleted_abs_path",
    "directory",
    "entries",
    "file",
    "file_path",
    "main_path",
    "new_abs_path",
    "new_path",
    "old_abs_path",
    "path",
    "prompt_path",
    "rel_path",
    "synthesis_prompt_path",
    "targets",
];

/// Keys whose value IS a root rather than a path under one. These stay
/// ABSOLUTE: a root is the anchor every other path is relative to, and
/// relativizing one yields the empty string — measured 136 times across 12
/// sessions before this change.
pub(crate) const ROOT_KEYS: &[&str] = &[
    "cwd",
    "git_root",
    "new_root",
    "old_root",
    "project_root",
    "repo_root",
    "root",
];

/// Relativize every allowlisted path value in `val`, in place.
///
/// `root_prefix` must end with `/`; pass `""` when no project is active and
/// the call becomes a no-op. The trailing slash is load-bearing: it is what
/// stops a value that *equals* the root (stored bare) from matching, which is
/// how the same key name can mean "file path" on one node and "root" on
/// another without the walker needing path context.
pub(crate) fn strip_paths_in_value(val: &mut Value, root_prefix: &str) {
    if root_prefix.is_empty() {
        return;
    }
    match val {
        Value::Object(map) => {
            for (key, child) in map.iter_mut() {
                if ROOT_KEYS.contains(&key.as_str()) {
                    continue;
                }
                if PATH_KEYS.contains(&key.as_str()) {
                    relativize(child, root_prefix);
                }
                strip_paths_in_value(child, root_prefix);
            }
        }
        Value::Array(items) => {
            for item in items {
                strip_paths_in_value(item, root_prefix);
            }
        }
        _ => {}
    }
}

/// Relativize a value sitting under a path key: a string, or an array of
/// strings (`tree`'s `entries`, `reindex`'s `targets`).
fn relativize(val: &mut Value, root_prefix: &str) {
    match val {
        Value::String(s) => {
            if let Some(rest) = s.strip_prefix(root_prefix) {
                // Never emit "" for a path — an empty string reads as a valid
                // value and is worse than a long one.
                if !rest.is_empty() {
                    *s = rest.to_string();
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                relativize(item, root_prefix);
            }
        }
        _ => {}
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib path_strip`
Expected: PASS, 10 tests.

- [ ] **Step 5: Gate and commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test --lib path_strip
git add src/tools/core/path_strip.rs src/tools/core/mod.rs
git commit -m "feat(tools): add a field-aware, allowlist-driven project-root stripper

Walks the typed Value and relativizes only allowlisted path keys, leaving
content, prose and unknown keys byte-identical. An unknown key keeps its
absolute path (verbose) rather than being stripped (corrupt) — that inversion
is the point. Not wired in yet."
```

---

### Task 1 amendment — what actually shipped (2026-08-09)

The Opus task review found that three of the tests authored above **cannot fail**. The
human ruled that the findings govern over this plan text. Commits `1a30e91e..91757be4`.
The shipped `src/tools/core/path_strip.rs` is the source of truth; do not re-derive the
tests from the block above.

1. `leaves_file_content_untouched` embedded the root **mid-string**, but `relativize`
   uses `strip_prefix`, which matches at offset 0 only — so it passed even with the
   `PATH_KEYS` gate deleted. Fixed by adding a value that *begins* with the root
   (`"stdout": "/home/u/proj/src/lib.rs\n"`).
2. `root_keys_stay_absolute` never exercised `ROOT_KEYS`: those keys are absent from
   `PATH_KEYS`, so deleting the `continue` guard left it green. Added
   `a_root_key_prunes_recursion_beneath_it`, which pins the guard's only live effect —
   skipping the whole subtree under a root key.
3. The trailing-slash contract was asserted only where the empty-string guard also held,
   so neither single mutation was detectable. Added
   `a_sibling_directory_sharing_the_root_as_a_prefix_is_untouched`
   (`/home/u/projX/file.rs` against root `/home/u/proj/`) — the `<root>-backup/foo` hazard
   the replaced code named.
4. Restored `debug_assert!(root_prefix.ends_with('/'), …)`, which the code being replaced
   carried and this plan dropped. Callers MUST pass a slash-terminated prefix.
5. `#[allow(dead_code)]` → `#[expect(dead_code, reason = "wired in by Task 2…")]` on all
   four items, below each doc comment. `#[expect]` errors once the item is used, so Task 2
   cannot forget to remove them.

Each of 1–3 was mutation-verified: the mutation applied, the named test observed failing,
the mutation reverted. Known deferred minor: four `unfulfilled_lint_expectation` warnings
under `cargo test` (the `#[cfg(test)]` module uses the items). `cargo clippy -- -D warnings`
is clean; Task 2 deletes the attributes, so the noise lasts exactly one task.
## Task 2: Wire the walker into `Tool::call_content`

**Files:**
- Modify: `src/tools/core/types.rs:546-679`
- Test: `src/tools/core/tests.rs`

**Interfaces:**
- Consumes: `strip_paths_in_value(&mut Value, &str)` from Task 1; `Agent::project_root_for(Option<&Path>) -> Option<PathBuf>` (async, `src/agent/mod.rs:632`); `crate::util::fs::to_forward_slash(&Path) -> String` (`src/util/fs.rs:106`).
- Produces: every tool's `Value` is relative-by-key before `exceeds_inline_limit`, the buffer payload, and `format_compact` observe it.

**The one override needs no change.** `Onboarding::call_content` (`src/tools/onboarding.rs:294`) bypasses this default implementation and therefore never strips — which is correct: every path it emits is a hardcoded relative string (`".codescout/tmp/onboarding-prompt.md"`, `format!(".codescout/tmp/{}", file_name)`). Do not add a strip call there; there is nothing absolute to relativize.

- [ ] **Step 1: Write the failing tests**

Append to `src/tools/core/tests.rs`. `rooted_ctx` is new here — copy it verbatim (it mirrors `src/tools/grep.rs:964`):

```rust
async fn rooted_ctx(root: &std::path::Path) -> ToolContext {
    std::fs::create_dir_all(root.join(".codescout")).unwrap();
    ToolContext {
        agent: crate::agent::Agent::new(Some(root.to_path_buf())).await.unwrap(),
        lsp: crate::lsp::LspManager::new_arc(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    }
}

/// Like `EchoTool`, but its compact summary is DERIVED from the result it is
/// handed. `EchoTool::format_compact` ignores its `_result` argument and
/// returns a stored string, so it cannot detect whether the value was stripped
/// before or after the summary was built — which is exactly the ordering this
/// test exists to pin.
struct SummarizingEchoTool {
    result: serde_json::Value,
}

#[async_trait::async_trait]
impl Tool for SummarizingEchoTool {
    fn name(&self) -> &str {
        "summarizing_echo_tool"
    }
    fn description(&self) -> &str {
        "test"
    }
    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({})
    }
    async fn call(
        &self,
        _input: serde_json::Value,
        _ctx: &ToolContext,
    ) -> anyhow::Result<serde_json::Value> {
        Ok(self.result.clone())
    }
    fn format_compact(&self, result: &serde_json::Value) -> Option<String> {
        let first = result["matches"][0]["file"].as_str().unwrap_or("?");
        Some(format!("200 matches\n\nfirst: {first}"))
    }
}

#[tokio::test]
async fn call_content_relativizes_path_keys_but_not_content() {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let ctx = rooted_ctx(&root).await;
    let root_fwd = crate::util::fs::to_forward_slash(&root);

    let literal = format!("REPO = \"{root_fwd}/.worktrees/single-stage\"");
    let tool = EchoTool {
        result: serde_json::json!({
            "file": format!("{root_fwd}/src/lib.rs"),
            "content": literal,
            "project_root": root_fwd,
        }),
        user_summary: None,
    };

    let content = tool.call_content(serde_json::json!({}), &ctx).await.unwrap();
    let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");

    assert!(text.contains("\"src/lib.rs\""), "path key must relativize: {text}");
    assert!(
        text.contains(&literal),
        "file CONTENT must survive byte-identical: {text}"
    );
    assert!(
        text.contains(&format!("\"project_root\": \"{root_fwd}\"")),
        "root-valued field must stay absolute, never \"\": {text}"
    );
}

#[tokio::test]
async fn call_content_buffered_summary_is_built_from_the_stripped_value() {
    // Pins the ordering invariant: strip runs BEFORE format_compact builds the
    // buffer summary. Reversed, absolute paths escape through the serialized
    // envelope — 85% of the leaks measured before this change.
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let ctx = rooted_ctx(&root).await;
    let root_fwd = crate::util::fs::to_forward_slash(&root);

    let items: Vec<serde_json::Value> = (0..200)
        .map(|i| serde_json::json!({ "file": format!("{root_fwd}/src/f_{i}.rs"), "line": i }))
        .collect();
    let tool = SummarizingEchoTool {
        result: serde_json::json!({ "matches": items }),
    };

    let content = tool.call_content(serde_json::json!({}), &ctx).await.unwrap();
    let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");

    assert!(text.contains("@tool_"), "test data must exceed the inline budget: {text}");
    assert!(
        text.contains("first: src/f_0.rs"),
        "format_compact must have observed the STRIPPED value: {text}"
    );
    assert!(
        !text.contains(&format!("{root_fwd}/src/")),
        "no absolute project path may survive into the buffered envelope: {text}"
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib call_content_relativizes call_content_buffered_summary`
Expected: FAIL — `call_content` does not yet strip, so `"src/lib.rs"` is absent and the summary reads `first: <abs>/src/f_0.rs`.

- [ ] **Step 3: Write the implementation**

In `src/tools/core/types.rs`, replace the first line of `call_content` (currently `let val = self.call(input, ctx).await?;` at line 547) with:

```rust
        let mut val = self.call(input, ctx).await?;

        // Field-aware project-root stripping. Runs HERE, on the typed Value,
        // and therefore BEFORE `exceeds_inline_limit`, the `@tool_*` buffer
        // payload, and `format_compact` — the ordering is load-bearing: a
        // summary built from an unstripped Value leaks absolute paths through
        // the serialized envelope, which is how 85% of the pre-fix leaks
        // escaped. The root resolves from the same `ctx` the tool body used,
        // so a `workspace=` pin cannot be mismatched here the way it could
        // when `post_process` had to be handed the pin separately
        // (docs/issues/archive/2026-07-09-residual-workspace-pin-gaps-post-edit-code-fix.md).
        let root_prefix = ctx
            .agent
            .project_root_for(ctx.workspace_override.as_deref())
            .await
            .map(|p| format!("{}/", crate::util::fs::to_forward_slash(&p)))
            .unwrap_or_default();
        crate::tools::core::path_strip::strip_paths_in_value(&mut val, &root_prefix);
        let val = val;
```

Everything below is unchanged — `let json = serde_json::to_string(&val)` and all later reads now observe the stripped value.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib call_content`
Expected: PASS — the two new tests plus the eight pre-existing `call_content_*` buffering tests.

- [ ] **Step 5: Gate and commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test --lib
git add src/tools/core/types.rs src/tools/core/tests.rs
git commit -m "feat(tools): strip project-root paths on the Value inside call_content

Runs before exceeds_inline_limit, the buffer payload and format_compact, so
buffered summaries render relative instead of leaking absolute paths through
the serialized envelope. Root resolves from the same ctx the tool body used,
so a workspace= pin cannot be mismatched at this layer."
```
---

## Task 3: Retire the text strip in `post_process`

**Files:**
- Modify: `src/server.rs:527-579` (`post_process`)
- Delete: `src/server.rs:1662-1684` (`strip_project_root_from_result`), `src/server.rs:1702-1744` (`strip_prefix_from_text`)
- Modify: `src/server.rs` tests — `post_process_strips_and_annotates_against_the_pinned_root` (2750), `stripped_responses_emit_paths_relative_annotation_once_per_activation` (3004), and the doc comment on `call_tool_strips_bare_project_root_from_list_dir_output` (2668)

**Interfaces:**
- Consumes: Task 2's guarantee that tool output arrives already relative.
- Produces: `post_process(&self, CallToolResult, &str, Option<&Path>) -> CallToolResult` — banner only, no text mutation.

**Three decisions this task locks in:**

1. **The banner loses its trigger.** It currently fires on the boolean returned by `strip_project_root_from_result`. With no strip there is no such signal, so the banner now fires on the **first non-`run_command` response after activation for which a project root resolves**. Same once-per-activation cadence, no coupling to a transform that no longer exists.
2. **The `run_command` exemption is deleted, not preserved.** `run_command` returns `{"exit_code", "stdout", …}`; `stdout` is not a path key, so the allowlist leaves it verbatim with no tool-name branch. `run_command_output_keeps_absolute_project_paths` (`src/server.rs:3101`) stays as the guard and must keep passing.
3. **Both `tree` tests keep passing unchanged — do not edit their bodies.** `call_tool_strips_project_root_from_output` (2634) and `call_tool_strips_bare_project_root_from_list_dir_output` (2668) assert `!text.contains(&root)`. Under Task 1, `entries` is allowlisted, so `format_list_dir` computes `common_path_prefix` over already-relative names and renders `dir_display` as `.` — the bare root never forms. Only the second test's doc comment, which explains the behaviour in terms of the deleted bare-root branch, becomes false and must be rewritten.

- [ ] **Step 1: Rewrite the two affected tests**

Replace the body of `post_process_strips_and_annotates_against_the_pinned_root` (`src/server.rs:2750`) and rename it:

```rust
    #[tokio::test]
    async fn post_process_annotates_against_the_pinned_root_without_mutating_text() {
        // post_process no longer strips — stripping moved to Tool::call_content.
        // What survives here is the banner, which must still name the PINNED
        // root rather than the session default.
        let dir_a = tempdir().unwrap();
        let (_dir_b, server) = make_server().await;
        std::fs::create_dir_all(dir_a.path().join(".codescout")).unwrap();
        let root_a = std::fs::canonicalize(dir_a.path()).unwrap();
        let root_a_fwd = to_forward_slash(&root_a);

        let literal = format!("REPO = \"{root_a_fwd}/.worktrees/x\"");
        let payload = CallToolResult::success(vec![Content::text(literal.clone())]);

        let processed = server.post_process(payload, "read_file", Some(&root_a)).await;
        let joined: String = processed
            .content
            .iter()
            .filter_map(|c| c.as_text().map(|t| t.text.clone()))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(
            joined.contains(&literal),
            "post_process must not mutate result text at all; got: {joined}"
        );
        assert!(
            joined.contains(&format!("paths are relative to {root_a_fwd}")),
            "the banner must name the PINNED root A; got: {joined}"
        );
    }
```

Rename `stripped_responses_emit_paths_relative_annotation_once_per_activation` (`src/server.rs:3004`) to `responses_emit_paths_relative_annotation_once_per_activation`. Its `make_payload()` no longer needs to contain an absolute path for the banner to fire; leave every assertion as-is — the once-per-activation cadence, the `run_command` exclusion, and the reset-after-activation behaviour are all unchanged.

Replace the doc comment above `call_tool_strips_bare_project_root_from_list_dir_output` (`src/server.rs:2660-2668`) with:

```rust
    /// Regression (2026-07-18-tree-strip-bare-root-not-stripped): when a
    /// listing's sole common prefix IS the project root, `format_list_dir`
    /// renders that prefix WITHOUT a trailing slash. The old text strip needed
    /// a dedicated bare-root branch to catch that shape; field-aware stripping
    /// does not, because `entries` is allowlisted, so `common_path_prefix` runs
    /// over already-relative names and `dir_display` collapses to ".". This
    /// test guards the rendered outcome, which is unchanged — `make_server`'s
    /// tempdir needs a visible top-level entry or the listing short-circuits to
    /// "(empty directory)" and exercises nothing.
```

- [ ] **Step 2: Add the byte-faithfulness guard for errors**

```rust
    #[tokio::test]
    async fn edit_failure_hint_reproduces_the_files_real_bytes() {
        // The "Nearest content" hint (src/tools/edit_file/mod.rs:196) embeds RAW
        // file bytes. Before this change it returned through the same strip as
        // read_file, so a path literal was rewritten identically in both and the
        // mismatch could not be falsified from inside the session.
        let (dir, server) = make_server().await;
        let root_fwd = to_forward_slash(dir.path());
        let literal = format!("REPO = \"{root_fwd}/.worktrees/single-stage\"");
        std::fs::write(dir.path().join("probe.txt"), format!("{literal}\n")).unwrap();

        let req = CallToolRequestParams::new("edit_file").with_arguments(
            serde_json::from_value(serde_json::json!({
                "path": "probe.txt",
                "old_string": "REPO = \".worktrees/single-stage\"",
                "new_string": "x",
            }))
            .unwrap(),
        );
        let result = server
            .call_tool_inner(req, None, None, tokio_util::sync::CancellationToken::new())
            .await
            .unwrap();
        let text = result
            .content
            .iter()
            .find_map(|c| c.as_text().map(|t| t.text.as_str()))
            .unwrap_or("");

        assert!(
            text.contains(&literal),
            "the failure must quote the file's REAL bytes so the caller can see \
             why the match failed; got: {text}"
        );
    }
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test --lib post_process_annotates edit_failure_hint_reproduces`
Expected: FAIL — `post_process` still mutates the text, so both the `joined.contains(&literal)` and the error-bytes assertions fail.

- [ ] **Step 4: Write the implementation**

Replace `post_process` (`src/server.rs:527-579`) in full:

```rust
    /// Append the once-per-activation `[codescout] paths are relative to <root>`
    /// banner.
    ///
    /// This method no longer transforms result text. Project-root stripping is
    /// field-aware and happens upstream, on the typed `Value`, inside
    /// `Tool::call_content` — see `src/tools/core/path_strip.rs`. Doing it here,
    /// on rendered text, meant guessing from a one-character lookbehind: it
    /// stripped path literals out of file content and collapsed root-valued
    /// fields to `""`
    /// (docs/issues/2026-08-09-path-strip-corrupts-file-content-and-root-fields.md).
    ///
    /// `run_command` needs no special case any more. Its payload is
    /// `{"exit_code", "stdout", ...}` and `stdout` is not an allowlisted path
    /// key, so raw shell bytes are left verbatim by the allowlist itself
    /// rather than by a tool-name branch. It is excluded here only from the
    /// banner, which would be noise on raw shell output.
    async fn post_process(
        &self,
        mut call_result: CallToolResult,
        tool_name: &str,
        workspace_override: Option<&std::path::Path>,
    ) -> CallToolResult {
        if tool_name == "run_command" {
            return call_result;
        }
        let Some(root) = self.agent.project_root_for(workspace_override).await else {
            return call_result;
        };

        // Novelty-gated: emit only the FIRST eligible response since server
        // start or the last `activate_project`. `call_tool` resets the flag in
        // its `is_activate` branch so the next activation re-announces the new
        // root. See [`path_note_emitted_since_activation`].
        let already_emitted = self
            .path_note_emitted_since_activation
            .swap(true, std::sync::atomic::Ordering::Relaxed);
        if !already_emitted {
            let root = to_forward_slash(&root);
            call_result.content.push(Content::text(format!(
                "\n[codescout] paths are relative to {root}"
            )));
        }
        call_result
    }
```

Then delete `strip_project_root_from_result` (`src/server.rs:1662-1684`) and `strip_prefix_from_text` (`src/server.rs:1702-1744`) entirely, plus any imports the compiler then reports unused.

- [ ] **Step 5: Run the full suite**

Run: `cargo test`
Expected: PASS. Any test that called the two deleted functions directly should be deleted — their behaviour is now covered by `src/tools/core/path_strip.rs`'s unit tests. Do **not** delete a test that exercises rendered output; that is coverage, not a helper test.

- [ ] **Step 6: Gate and commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
git add src/server.rs
git commit -m "refactor(server): post_process keeps the banner and stops rewriting text

Stripping is field-aware and now happens in Tool::call_content, so the text
transform and both its helpers are deleted along with the bare-root branch
that collapsed root-valued fields to \"\". The banner fires on the first
eligible response after activation rather than on a strip signal that no
longer exists. run_command needs no strip exemption: stdout is not a path
key, so the allowlist leaves it verbatim without a tool-name branch.

The edit-failure hint now quotes the file's real bytes, so a failed match is
diagnosable from inside the session instead of rendering identically to the
read_file output it contradicts."
```
---

## Task 4: The corpus gate

**Files:**
- Modify: `src/server.rs` (tests module, `src/server.rs:1746+`)

**Interfaces:**
- Consumes: `make_server()`, and `call_tool_inner(req, None, None, CancellationToken::new())` where `req` is built exactly as in `call_tool_strips_project_root_from_output` (`src/server.rs:2634`).
- Produces: `no_absolute_project_paths_in_rendered_output` — the mechanism that makes a missing `PATH_KEYS` entry fail CI.

**Predicate:** rendered output must not contain `<root>/`. A **bare** `<root>` is permitted — that is a legitimate anchor (`project_root`, `git_root`). The trailing slash is exactly what separates "path under the root" from "the root itself", which is also what lets the same key name mean both.

- [ ] **Step 1: Write the gate**

```rust
    #[tokio::test]
    async fn no_absolute_project_paths_in_rendered_output() {
        // The PATH_KEYS allowlist in src/tools/core/path_strip.rs is a co-change
        // contract. This gate is what enforces it: a tool emitting paths under a
        // key nobody added fails here instead of silently costing tokens forever.
        let (dir, server) = make_server().await;
        let root_fwd = to_forward_slash(dir.path());
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/lib.rs"), "pub fn a() {}\n").unwrap();
        std::fs::write(dir.path().join("notes.md"), "# Notes\n\nbody\n").unwrap();

        let cases: Vec<(&str, serde_json::Value)> = vec![
            ("tree", serde_json::json!({ "path": "." })),
            ("grep", serde_json::json!({ "pattern": "pub fn" })),
            ("read_file", serde_json::json!({ "path": "src/lib.rs" })),
            ("read_markdown", serde_json::json!({ "path": "notes.md" })),
            ("symbols", serde_json::json!({ "path": "src/lib.rs" })),
        ];

        let needle = format!("{root_fwd}/");
        for (tool, input) in cases {
            let req = CallToolRequestParams::new(tool)
                .with_arguments(serde_json::from_value(input).unwrap());
            let result = server
                .call_tool_inner(req, None, None, tokio_util::sync::CancellationToken::new())
                .await
                .unwrap();
            let joined: String = result
                .content
                .iter()
                .filter_map(|c| c.as_text().map(|t| t.text.clone()))
                .collect::<Vec<_>>()
                .join("\n");
            assert!(
                !joined.contains(&needle),
                "tool `{tool}` leaked an absolute project path. Either its path key \
                 is missing from PATH_KEYS in src/tools/core/path_strip.rs, or it \
                 introduced a new one. Output:\n{joined}"
            );
        }
    }
```

- [ ] **Step 2: Add the end-to-end content-survival case**

The gate proves paths are relative; this proves content was not collateral damage — the originally reported symptom, through the real `read_file`/`grep` stack rather than a mock tool.

```rust
    #[tokio::test]
    async fn read_file_and_grep_show_a_path_literal_in_content_verbatim() {
        let (dir, server) = make_server().await;
        let root_fwd = to_forward_slash(dir.path());
        let literal = format!("REPO = \"{root_fwd}/.worktrees/single-stage\"");
        std::fs::write(dir.path().join("probe.txt"), format!("{literal}\n")).unwrap();

        for (tool, input) in [
            ("read_file", serde_json::json!({ "path": "probe.txt" })),
            ("grep", serde_json::json!({ "pattern": "REPO", "path": "probe.txt" })),
        ] {
            let req = CallToolRequestParams::new(tool)
                .with_arguments(serde_json::from_value(input).unwrap());
            let result = server
                .call_tool_inner(req, None, None, tokio_util::sync::CancellationToken::new())
                .await
                .unwrap();
            let joined: String = result
                .content
                .iter()
                .filter_map(|c| c.as_text().map(|t| t.text.clone()))
                .collect::<Vec<_>>()
                .join("\n");
            assert!(
                joined.contains(&literal),
                "`{tool}` must show the file's path literal verbatim — an edit keyed \
                 on this text has to match the bytes on disk. Got:\n{joined}"
            );
        }
    }
```

> This test and `no_absolute_project_paths_in_rendered_output` are deliberately in tension: one forbids `<root>/` in output, the other requires it inside `content`. That tension IS the specification — a change that satisfies only one of them has broken the other.

- [ ] **Step 3: Run both**

Run: `cargo test --lib no_absolute_project_paths_in_rendered_output read_file_and_grep_show_a_path_literal`
Expected: the content test PASSES (Tasks 1–3 did the work); the gate either passes or FAILS naming a tool whose key is missing.

- [ ] **Step 4: Close any gap the gate reports**

For each tool named in a failure, find the key it emits its path under and add that key to `PATH_KEYS` in `src/tools/core/path_strip.rs`, keeping the list alphabetical. Add a unit test there for the newly-covered key, mirroring `relativizes_a_path_key`. Do **not** widen the walker's matching rules to make a case pass — adding a key is the only sanctioned fix, because widening the rules is how the old heuristic became unsafe.

- [ ] **Step 5: Re-run until green, then gate and commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
git add src/server.rs src/tools/core/path_strip.rs
git commit -m "test(server): gate that no absolute project path reaches rendered output

The PATH_KEYS allowlist is a co-change contract; prose does not hold one. A
tool emitting paths under an unlisted key now fails CI instead of silently
forfeiting the savings. A bare root is permitted — it is an anchor, and the
trailing slash is what separates it from a path beneath it. The paired
content test requires the same absolute prefix INSIDE file content, so the
two together pin both directions of the transform."
```
---

## Task 5: Correct the three documentation surfaces

**Files:**
- Modify: `src/prompts/source.md` (or wherever `topic_body("progressive-disclosure")` resolves — locate with `grep "Path-relative annotation" src/`)
- Modify: `docs/issues/2026-08-09-path-strip-corrupts-file-content-and-root-fields.md` (status → `fixed`)
- Modify: `docs/superpowers/specs/2026-08-09-field-aware-path-strip-design.md` (status → `active`)

**Interfaces:**
- Consumes: the shipped behaviour from Tasks 1–4.
- Produces: no code interface. The two stale `src/server.rs` doc comments are already gone with their functions (Task 3).

- [ ] **Step 1: Locate the guide text**

Run: `grep -rn "Path-relative annotation" src/`
Expected: one hit in the prompts source that backs `get_guide("progressive-disclosure")`.

- [ ] **Step 2: Replace the section**

Replace the whole `## Path-relative annotation` section with:

```markdown
## Path-relative annotation

Paths in tool responses are project-relative. On the first response after
each `activate_project`, codescout appends `[codescout] paths are relative
to <root>` naming the root they resolve against; later responses omit it,
because the same fact lives in the `Active project` line of
`server_instructions`.

Relativization is **field-aware**: it applies to path-valued JSON fields
only, never to file content, shell output, prose, or error text — all of
which are byte-faithful. Root-valued fields (`project_root`, `git_root`,
`cwd`) stay absolute: they are the anchor the rest resolve against.

To check a path against catalog state, read the value straight from the
response — path fields are relative, root fields absolute, and content is
verbatim. `run_command` output is raw shell bytes and is never rewritten.
```

This removes both defects: the claim that *every* response carries the banner (it is novelty-gated), and the recommendation to verify via `read_file(@tool_xxx, json_path=…)`, which was itself stripped.

- [ ] **Step 3: Check whether `ONBOARDING_VERSION` must bump**

Read `src/prompts/README.md` § version rules and the 2200-byte slice cap. If the edited text is inside a `server_instructions` or `onboarding_prompt` slice, bump `ONBOARDING_VERSION` and re-check the slice size. If it is only a `get_guide` topic body, no bump is needed.

Run: `cargo test --lib prompt_surfaces_reference_only_real_tools`
Expected: PASS.

- [ ] **Step 4: Flip the artifact statuses**

Both files are librarian artifacts — use the catalog, never a bare frontmatter edit:

```
artifact(action="update", id="ece908f37854e557", status="fixed",
         extra={"closed": "2026-08-09"})
artifact(action="update", id="464dce7fe5cd6a3c", status="active")
```

Then fill the bug's `## Fix` and `## Tests added` sections with the real commit SHAs and test names from Tasks 1–4, using `patch={body_edits: [...]}`.

**Do not archive the bug file in this task.** Archiving requires the gate green on `experiments` plus the regression tests, which the final verification step confirms. Archive via `artifact(action="move", …)` only after that, and — since `git rev-list --left-right --count master...experiments` reports `0` on the left, meaning fast-forward — do **not** add a pending-master-SHA line.

- [ ] **Step 5: Gate and commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
git add src/prompts/ docs/issues/ docs/superpowers/specs/
git commit -m "docs(prompts): describe path relativization as field-aware

The progressive-disclosure guide claimed every response carries the
paths-are-relative banner (it is gated to once per activation) and told
readers to verify absolute values by reading the @tool_* buffer — a path
that was itself stripped. Both are corrected, and the bug/spec statuses
move to fixed/active."
```

---

## Final verification

- [ ] `cargo fmt --check && cargo clippy -- -D warnings && cargo test` — all green.
- [ ] `cargo rb` (release build; **not** `cargo build --release` on this stack), then `/mcp` to reconnect.
- [ ] Live check against the recorded symptoms:
  - `workspace(action="activate", path=<this repo>)` → `project_root` is an **absolute** path, not `""`.
  - `artifact(action="find", kind="spec", limit=1)` → `scope.abs_path` and `scope.git_root` are absolute, not `""`.
  - `memory(action="read", topic="claude-code-mcp-env")` → the `CLAUDE_PROJECT_DIR` row shows `/home/marius/work/claude/codescout`, not an empty cell.
  - Write a probe containing the repo root as a quoted literal, `read_file` it, and confirm the displayed bytes match `run_command`'s.
  - `symbols(name="post_process", include_body=true)` on a large result → the buffered `summary` carries relative paths.
- [ ] Archive the bug file: `artifact(action="move", id="ece908f37854e557", new_rel_path="docs/issues/archive/2026-08-09-path-strip-corrupts-file-content-and-root-fields.md")`.
- [ ] `librarian(action="link_scan", write=true)` to reconcile cites edges after the move.
