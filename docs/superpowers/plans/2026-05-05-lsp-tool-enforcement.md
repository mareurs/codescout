# LSP Tool Enforcement Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enforce LSP tool usage (symbol_at, references, call_graph) via prompt surface improvements and a light hint in edit_code for rename/replace actions.

**Architecture:** Three independent change areas — (1) server_instructions.md text additions, (2) builders.rs nav strategy additions + ONBOARDING_VERSION bump, (3) edit_code.rs call() mutation to inject a hint field on rename/replace success.

**Tech Stack:** Rust, MCP server prompt surfaces, tokio async tests

---

## File Map

| File | Change |
|------|--------|
| `src/prompts/server_instructions.md` | Iron Law #8, symbol_at decision tree bullet, LSP Workflow section, 1 new anti-pattern row |
| `src/prompts/builders.rs` | `symbol_at` + `references` steps in single-project and multi-project nav strategy |
| `src/tools/onboarding.rs` | Bump `ONBOARDING_VERSION` 22 → 23 |
| `src/tools/symbol/edit_code.rs` | Mutate `call()` result for rename/replace to add `"hint"` field |
| `src/tools/symbol/tests.rs` | New integration test: `edit_code_replace_appends_caller_hint` |

---

## Task 1: server_instructions.md — Iron Laws and anti-patterns

**Files:**
- Modify: `src/prompts/server_instructions.md`

- [ ] **Step 1: Add Iron Law #8**

  Append after the last line of the `## Iron Laws` section (after the closing paragraph of law #7). Use `edit_markdown`:

  ```
  edit_markdown("src/prompts/server_instructions.md",
    action="edit",
    heading="## Iron Laws",
    old_string="   `grep` on code gives raw text you must interpret; `symbols` gives structured\n   output (signature, body, line range) in fewer tokens with zero ambiguity.",
    new_string="   `grep` on code gives raw text you must interpret; `symbols` gives structured\n   output (signature, body, line range) in fewer tokens with zero ambiguity.\n\n8. **REFERENCES BEFORE EDITING.** Before `edit_code(action=\"rename\"|\"replace\")`,\n   run `references(symbol, path)` to get the concrete call-site list.\n   `call_graph` gives transitive reach; `references` gives the actual locations.\n   Skip only when you already ran references for this symbol in this session."
  )
  ```

- [ ] **Step 2: Add `symbol_at` bullet to Iron Law #7 decision tree**

  In the `## Iron Laws` section, add one line to the decision tree after `"What does symbol X look like?" → symbols(name=X, include_body=true)`:

  ```
  edit_markdown("src/prompts/server_instructions.md",
    action="edit",
    heading="## Iron Laws",
    old_string='   - "What does symbol X look like?" → `symbols(name=X, include_body=true)`\n   - "What\'s in this file/dir?" → `symbols(path=...)`',
    new_string='   - "What does symbol X look like?" → `symbols(name=X, include_body=true)`\n   - "I have a path + line number from tool output" → `symbol_at(path, line)` — type sig + hover docs, no re-search needed\n   - "What\'s in this file/dir?" → `symbols(path=...)`'
  )
  ```

- [ ] **Step 3: Add `### LSP Workflow` section after `### Symbol Navigation Patterns`**

  ```
  edit_markdown("src/prompts/server_instructions.md",
    action="insert_after",
    heading="### Symbol Navigation Patterns",
    content="### LSP Workflow — Standard Sequence\n\nFor any symbol change, in order:\n1. `symbols(name=X)` — locate the symbol, get its defining file + line\n2. `symbol_at(path, line)` — inspect type signature + docs (when you need to understand what it IS)\n3. `references(symbol, path)` — enumerate all call sites before touching anything\n4. `call_graph(symbol, path, direction=\"callers\", max_depth=3)` — transitive blast radius for renames/structural changes\n5. `edit_code(...)` — make the change\n"
  )
  ```

- [ ] **Step 4: Add one new anti-pattern row**

  In `## Anti-Patterns`, append one row to the table:

  ```
  edit_markdown("src/prompts/server_instructions.md",
    action="edit",
    heading="## Anti-Patterns — STOP if you catch yourself doing these",
    old_string="| `symbols(query=\"foo\\|bar\")` | `grep(pattern=\"foo\\|bar\")` or separate `symbols` calls | `symbols` rejects regex-like patterns |",
    new_string="| `symbols(query=\"foo\\|bar\")` | `grep(pattern=\"foo\\|bar\")` or separate `symbols` calls | `symbols` rejects regex-like patterns |\n| `semantic_search(\"X\")` when you already have path+line for X | `symbol_at(path, line)` | Re-searching wastes tokens; you already have the location |"
  )
  ```

- [ ] **Step 5: Verify prompt surfaces test passes**

  ```bash
  cargo test prompt_surfaces_reference_only_real_tools
  ```

  Expected: `test server::tests::prompt_surfaces_reference_only_real_tools ... ok`

- [ ] **Step 6: Commit**

  ```bash
  git add src/prompts/server_instructions.md
  git commit -m "docs(prompts): add LSP workflow section, Iron Law #8, symbol_at to decision tree"
  ```

---

## Task 2: builders.rs nav strategy + ONBOARDING_VERSION bump

**Files:**
- Modify: `src/prompts/builders.rs`
- Modify: `src/tools/onboarding.rs`

- [ ] **Step 1: Add symbol_at + references steps to single-project nav block**

  In `build_system_prompt_draft`, find the single-project nav section (the `else` branch that builds steps 1–7). Insert after the `symbols(name=..., include_body=true)` push and before the `call_graph` push:

  ```rust
  // After this line:
  draft.push_str("4. `symbols(name=\\\"Name\\\", include_body=true)` — read implementation\\n");
  draft.push_str("   - regex-like patterns belong in `grep`, not `symbols`\\n");

  // Insert:
  draft.push_str("4b. `symbol_at(path, line)` — hover + type sig when you have an exact location from prior tool output; skip re-searching\\n");
  draft.push_str("4c. `references(symbol, path)` — all call sites before any edit\\n");

  // Existing continues:
  draft.push_str("5. `call_graph(symbol=\\\"Name\\\", direction=\\\"callers\\\")` — transitive blast radius; ...");
  ```

  Use `edit_code(action="replace", symbol="build_system_prompt_draft", ...)` with the full updated function body — or use `edit_file` for this literal string section since it's push_str content, not a structural change.

  Exact edit with `edit_file`:
  ```
  edit_file("src/prompts/builders.rs",
    old_string="        draft.push_str(\"   - regex-like patterns belong in `grep`, not `symbols`\\n\");\n        draft.push_str(\n            \"5. `call_graph",
    new_string="        draft.push_str(\"   - regex-like patterns belong in `grep`, not `symbols`\\n\");\n        draft.push_str(\"4b. `symbol_at(path, line)` — hover + type sig when you have an exact location from prior tool output; skip re-searching\\n\");\n        draft.push_str(\"4c. `references(symbol, path)` — all call sites before any edit\\n\");\n        draft.push_str(\n            \"5. `call_graph"
  )
  ```

- [ ] **Step 2: Add symbol_at + references steps to multi-project per-project nav block**

  In the multi-project loop block, after the `symbols("<root>")` step push, insert:

  ```
  edit_file("src/prompts/builders.rs",
    old_string="            draft.push_str(&format!(\n                \"1. `symbols(\\\"{}\\\")` — [fill in entry point during onboarding]\\n\",\n                p.relative_root.display()\n            ));\n            draft.push_str(&format!(\n                \"2. `semantic_search",
    new_string="            draft.push_str(&format!(\n                \"1. `symbols(\\\"{}\\\")` — [fill in entry point during onboarding]\\n\",\n                p.relative_root.display()\n            ));\n            draft.push_str(\"1b. `symbol_at(path, line)` — hover + type sig when you have an exact location\\n\");\n            draft.push_str(\"1c. `references(symbol, path)` — all call sites before any edit\\n\");\n            draft.push_str(&format!(\n                \"2. `semantic_search"
  )
  ```

- [ ] **Step 3: Bump ONBOARDING_VERSION**

  In `src/tools/onboarding.rs` line 21:

  ```
  edit_file("src/tools/onboarding.rs",
    old_string="pub(crate) const ONBOARDING_VERSION: u32 = 22;",
    new_string="pub(crate) const ONBOARDING_VERSION: u32 = 23;"
  )
  ```

- [ ] **Step 4: Run tests**

  ```bash
  cargo test
  ```

  Expected: all tests pass (the `onboarding_version_stale` tests use `ONBOARDING_VERSION - 1` and `+ 1` so they stay valid after the bump).

- [ ] **Step 5: Commit**

  ```bash
  git add src/prompts/builders.rs src/tools/onboarding.rs
  git commit -m "feat(prompts): add symbol_at + references to nav strategy, bump ONBOARDING_VERSION to 23"
  ```

---

## Task 3: edit_code hint for rename and replace

**Files:**
- Modify: `src/tools/symbol/edit_code.rs`
- Modify: `src/tools/symbol/tests.rs`

- [ ] **Step 1: Write the failing test**

  Add to `src/tools/symbol/tests.rs` (inside `#[cfg(test)]` mod, after the last test):

  ```rust
  #[tokio::test]
  async fn edit_code_replace_appends_caller_hint() {
      if !std::process::Command::new("rust-analyzer")
          .arg("--version")
          .output()
          .map(|o| o.status.success())
          .unwrap_or(false)
      {
          eprintln!("Skipping: rust-analyzer not installed");
          return;
      }

      let dir = tempdir().unwrap();
      std::fs::write(
          dir.path().join("Cargo.toml"),
          "[package]\nname = \"test-hint\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
      )
      .unwrap();
      std::fs::create_dir_all(dir.path().join("src")).unwrap();
      std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
      std::fs::write(
          dir.path().join("src/lib.rs"),
          "pub fn greet(name: &str) -> String {\n    format!(\"Hello, {}!\", name)\n}\n",
      )
      .unwrap();

      let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
      let ctx = ToolContext {
          agent,
          lsp: lsp(),
          output_buffer: buf(),
          progress: None,
          peer: None,
          section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
              crate::tools::section_coverage::SectionCoverage::new(),
          )),
      };

      // Retry loop: rust-analyzer needs time to index
      let mut outcome: Option<Value> = None;
      for attempt in 0..5 {
          if attempt > 0 {
              tokio::time::sleep(std::time::Duration::from_millis(600 * attempt)).await;
          }
          let result = EditCode
              .call(
                  json!({
                      "symbol": "greet",
                      "path": "src/lib.rs",
                      "action": "replace",
                      "body": "pub fn greet(name: &str) -> String {\n    format!(\"Hi, {}!\", name)\n}\n"
                  }),
                  &ctx,
              )
              .await;
          match result {
              Ok(v) => {
                  outcome = Some(v);
                  break;
              }
              Err(e) => {
                  eprintln!("Attempt {}: {}", attempt + 1, e);
              }
          }
      }

      let result = match outcome {
          Some(v) => v,
          None => {
              eprintln!("Skipping: LSP did not respond in time");
              return;
          }
      };

      let hint = result["hint"]
          .as_str()
          .expect("replace result must contain 'hint' field");
      assert!(
          hint.contains("references("),
          "hint should mention references: {hint}"
      );
      assert!(
          hint.contains("greet"),
          "hint should include symbol name: {hint}"
      );
  }
  ```

- [ ] **Step 2: Run the test to verify it fails**

  ```bash
  cargo test edit_code_replace_appends_caller_hint
  ```

  Expected: FAIL — `replace result must contain 'hint' field` (or skipped if rust-analyzer unavailable)

- [ ] **Step 3: Implement the hint in edit_code.rs**

  In `src/tools/symbol/edit_code.rs`, modify the `call()` method. Replace the `"rename"` and `"replace"` match arms:

  **Before:**
  ```rust
  "rename" => {
      let Some(new_name) = input["new_name"].as_str() else {
          return Err(RecoverableError::new("action 'rename' requires 'new_name'").into());
      };
      self.do_rename(ctx, name_path, rel_path, new_name).await
  }
  // ...
  "replace" => {
      let Some(body) = input["body"].as_str() else {
          return Err(RecoverableError::new("action 'replace' requires 'body'").into());
      };
      self.do_replace(ctx, name_path, rel_path, body).await
  }
  ```

  **After:**
  ```rust
  "rename" => {
      let Some(new_name) = input["new_name"].as_str() else {
          return Err(RecoverableError::new("action 'rename' requires 'new_name'").into());
      };
      let mut result = self.do_rename(ctx, name_path, rel_path, new_name).await?;
      result["hint"] = json!(format!(
          "verify callers: references(\"{}\", \"{}\")",
          name_path, rel_path
      ));
      Ok(result)
  }
  // ...
  "replace" => {
      let Some(body) = input["body"].as_str() else {
          return Err(RecoverableError::new("action 'replace' requires 'body'").into());
      };
      let mut result = self.do_replace(ctx, name_path, rel_path, body).await?;
      result["hint"] = json!(format!(
          "verify callers: references(\"{}\", \"{}\")",
          name_path, rel_path
      ));
      Ok(result)
  }
  ```

  Use `edit_code(action="replace", symbol="impl Tool for EditCode/call", path="src/tools/symbol/edit_code.rs", body=<full updated body>)`.

- [ ] **Step 4: Run the test to verify it passes**

  ```bash
  cargo test edit_code_replace_appends_caller_hint
  ```

  Expected: PASS (or skipped — both are acceptable)

- [ ] **Step 5: cargo fmt + clippy + full test suite**

  ```bash
  cargo fmt && cargo clippy -- -D warnings && cargo test
  ```

  Expected: no warnings, all tests pass

- [ ] **Step 6: Commit**

  ```bash
  git add src/tools/symbol/edit_code.rs src/tools/symbol/tests.rs
  git commit -m "feat(edit_code): append caller-check hint on rename/replace success"
  ```

---

## Spec Coverage Check

| Spec requirement | Task |
|---|---|
| Iron Law #8 (references before editing) | Task 1, Step 1 |
| symbol_at in decision tree | Task 1, Step 2 |
| LSP Workflow section | Task 1, Step 3 |
| New anti-pattern (semantic_search when have path+line) | Task 1, Step 4 |
| builders.rs single-project nav | Task 2, Step 1 |
| builders.rs multi-project nav | Task 2, Step 2 |
| ONBOARDING_VERSION bump | Task 2, Step 3 |
| edit_code hint on rename+replace | Task 3, Step 3 |
| No hint on insert/remove | By design — only rename/replace arms mutated |
