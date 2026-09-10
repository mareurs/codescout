---
kind: plan
status: draft
title: Parameter alias collapse — implementation plan
owners:
- marius
tags:
- tool-surface
- schema
topic: tool parameter surface
---

# Parameter Alias Collapse Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Advertise exactly one parameter name per concept across the tool surface, accept the non-canonical names at runtime, and tell the caller every time one was corrected.

**Architecture:** Each tool declares its non-canonical names via a new defaulted `Tool::param_aliases()`. `Tool::call_content` rewrites them in the input **before** anything reads it, then threads a correction notice through all three response render paths exactly as `workspace_notice` already does. Schema properties for the collapsed names are deleted; the four gates that policed alias *honesty* are replaced by five that police alias *absence and announcement*.

**Tech Stack:** Rust, `serde_json`, `async_trait`, tokio test harness.

**Spec:** `docs/superpowers/specs/2026-09-10-parameter-alias-collapse-design.md`

## Global Constraints

- **Never clone `input`.** `create_file`/`edit_file` carry whole file bodies in it; `call_content` warns about this three times. Normalization mutates in place and returns a small `Vec`.
- **Tolerance is permanent.** An alias is always accepted. No task may make one an error.
- **Register is `warning`**, never `hint` — per `Guidance`'s own doc comment in `src/tools/core/types.rs`.
- **Cadence is every affected call, stateless.** No per-session suppression.
- **Canonical wins on conflict**, and the ignored key is named.
- **Gate order for every commit:** `./scripts/fmt-mine.sh`, then `cargo clippy --workspace --all-targets --features local-embed -- -D warnings`, then `cargo test --workspace --no-default-features`, then `cargo test --workspace`. Chain the two test lanes with `;`, never `&&`.
- **Every task that adds a guard must observe a RED by mutating the production path**, and record what it mutated in the commit message. An assertion's existence is not evidence.
- `TOOL_SURFACE_CHAR_BUDGET` is ratcheted **once**, in Task 8, to the exact measured total.

---

## File Structure

| file | responsibility | task |
|---|---|---|
| `src/tools/core/param_alias.rs` | **new** — the alias map type, `normalize_params`, `Correction`, and the notice string builder. One purpose, unit-testable without a server. | 1 |
| `src/tools/core/types.rs` | `Tool::param_aliases()` declaration; the three-site threading inside `call_content` | 2, 3 |
| `src/fs/mod.rs` | `PATH_PARAM_ALIAS_MAP` beside the existing `PATH_PARAM_ALIASES` | 1 |
| `src/tools/{read_file,create_file,grep}.rs`, `src/tools/edit_file/mod.rs`, `src/tools/symbol/{edit_code,references,symbol_at,call_graph/mod}.rs` | `param_aliases()` impl + schema property deletion | 4, 5 |
| `src/tools/symbol/symbols.rs`, `src/librarian/tools/librarian.rs` | the two non-path collapses | 6 |
| `src/server.rs` | replace 4 gates with 5; ratchet the budget | 7, 8 |
| `src/prompts/README.md`, `src/prompts/guides/symbol-navigation.md`, `docs/manual/src/**` | coupled doc surfaces | 6, 9 |

---

## Task 1: The normalizer, standalone

**Files:**
- Create: `src/tools/core/param_alias.rs`
- Modify: `src/tools/core/mod.rs` (add `pub mod param_alias;`)
- Modify: `src/fs/mod.rs:230` (add `PATH_PARAM_ALIAS_MAP` next to `PATH_PARAM_ALIASES`)
- Test: inline `#[cfg(test)]` in `src/tools/core/param_alias.rs`

**Interfaces:**
- Produces: `pub struct Correction { pub received: String, pub canonical: &'static str, pub conflicted: bool }`; `pub type AliasMap = &'static [(&'static str, &'static str)]`; `pub fn normalize_params(input: &mut Value, aliases: AliasMap) -> Vec<Correction>`; `pub fn correction_notice(tool: &str, corrections: &[Correction]) -> Option<String>`.
- Consumes: nothing.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const MAP: AliasMap = &[("file_path", "path"), ("relative_path", "path")];

    #[test]
    fn renames_an_alias_to_its_canonical_key() {
        let mut input = json!({ "file_path": "src/x.rs", "limit": 5 });
        let got = normalize_params(&mut input, MAP);
        assert_eq!(input["path"], json!("src/x.rs"));
        assert!(input.get("file_path").is_none(), "alias key must be removed");
        assert_eq!(input["limit"], json!(5), "unrelated keys untouched");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].received, "file_path");
        assert_eq!(got[0].canonical, "path");
        assert!(!got[0].conflicted);
    }

    #[test]
    fn canonical_wins_and_the_conflict_is_recorded() {
        let mut input = json!({ "path": "canon.rs", "file_path": "alias.rs" });
        let got = normalize_params(&mut input, MAP);
        assert_eq!(input["path"], json!("canon.rs"), "canonical value survives");
        assert!(input.get("file_path").is_none());
        assert_eq!(got.len(), 1);
        assert!(got[0].conflicted, "must record that a value was discarded");
    }

    #[test]
    fn no_aliases_present_is_a_no_op_and_allocates_no_corrections() {
        let mut input = json!({ "path": "src/x.rs" });
        let got = normalize_params(&mut input, MAP);
        assert_eq!(input, json!({ "path": "src/x.rs" }));
        assert!(got.is_empty());
    }

    #[test]
    fn a_non_object_input_is_left_alone() {
        let mut input = json!("not an object");
        let got = normalize_params(&mut input, MAP);
        assert_eq!(input, json!("not an object"));
        assert!(got.is_empty());
    }

    #[test]
    fn notice_names_the_tool_the_key_and_the_canonical_name() {
        let c = vec![Correction {
            received: "file_path".into(),
            canonical: "path",
            conflicted: false,
        }];
        let n = correction_notice("read_file", &c).expect("a correction must yield a notice");
        assert!(n.contains("file_path"), "must name what was received: {n}");
        assert!(n.contains("read_file"), "must name the tool: {n}");
        assert!(n.contains("path"), "must name the canonical key: {n}");
    }

    #[test]
    fn notice_for_a_conflict_says_the_value_was_ignored() {
        let c = vec![Correction {
            received: "file_path".into(),
            canonical: "path",
            conflicted: true,
        }];
        let n = correction_notice("read_file", &c).unwrap();
        assert!(
            n.contains("ignored"),
            "a discarded value must be stated, not implied: {n}"
        );
    }

    #[test]
    fn no_corrections_yields_no_notice() {
        assert!(correction_notice("read_file", &[]).is_none());
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --lib param_alias 2>&1`
Expected: FAIL to compile — `param_alias` module does not exist.

- [ ] **Step 3: Write the implementation**

Create `src/tools/core/param_alias.rs`:

```rust
//! Parameter alias normalization.
//!
//! The tool surface advertises exactly ONE name per concept. Callers habitually
//! send others — `file_path` above all, because Claude Code's native `Read` uses
//! it. Those are accepted, rewritten to the canonical key before any consumer
//! reads the input, and announced on the response.
//!
//! Why the rewrite happens at the dispatch boundary rather than in each tool:
//! `call_content` reads `input` three times before `call()` runs (`selector_key`,
//! `is_write`, `write_path`), and a tool that normalized internally would leave
//! those three looking at the un-normalized shape. See the spec,
//! `docs/superpowers/specs/2026-09-10-parameter-alias-collapse-design.md`.

use serde_json::Value;

/// A non-canonical parameter name a caller sent, and what it was rewritten to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Correction {
    /// The key as received.
    pub received: String,
    /// The advertised key it was rewritten to.
    pub canonical: &'static str,
    /// True when the canonical key was ALSO supplied, so the aliased value was
    /// discarded rather than used. Reported separately because a silently
    /// dropped value is a different event from a rename.
    pub conflicted: bool,
}

/// `(received, canonical)` pairs. Declared per tool via `Tool::param_aliases`.
pub type AliasMap = &'static [(&'static str, &'static str)];

/// Rewrite every alias key in `input` to its canonical name, in place.
///
/// MUTATES rather than returning a new value: `create_file`/`edit_file` carry
/// whole file bodies in `input`, so a clone would be paid on every call.
///
/// The canonical key wins a conflict, matching `require_path_param`'s
/// long-standing preference — the alias entry is removed either way, so a
/// downstream reader can never see two spellings of one parameter.
pub fn normalize_params(input: &mut Value, aliases: AliasMap) -> Vec<Correction> {
    let Some(obj) = input.as_object_mut() else {
        return Vec::new();
    };
    let mut corrections = Vec::new();
    for (received, canonical) in aliases {
        let Some(value) = obj.remove(*received) else {
            continue;
        };
        let conflicted = obj.contains_key(*canonical);
        if !conflicted {
            obj.insert((*canonical).to_string(), value);
        }
        corrections.push(Correction {
            received: (*received).to_string(),
            canonical,
            conflicted,
        });
    }
    corrections
}

/// One-line notice for the response, or `None` when nothing was corrected.
///
/// Names the tool as well as the keys: the notice is read out of context, in a
/// response the caller may be scanning among several.
pub fn correction_notice(tool: &str, corrections: &[Correction]) -> Option<String> {
    if corrections.is_empty() {
        return None;
    }
    let parts: Vec<String> = corrections
        .iter()
        .map(|c| {
            if c.conflicted {
                format!(
                    "'{}' is not a parameter of {tool} — '{}' was also supplied and won; \
                     the '{}' value was ignored.",
                    c.received, c.canonical, c.received
                )
            } else {
                format!(
                    "'{}' is not a parameter of {tool} — corrected to '{}'. \
                     Use '{}' next time.",
                    c.received, c.canonical, c.canonical
                )
            }
        })
        .collect();
    Some(parts.join(" "))
}
```

Add to `src/tools/core/mod.rs`:

```rust
pub mod param_alias;
```

Add to `src/fs/mod.rs`, directly below the existing `PATH_PARAM_ALIASES` (line ~230):

```rust
/// The same accept-set as [`PATH_PARAM_ALIASES`], as `(received, canonical)`
/// pairs for `Tool::param_aliases`. Derived from one list by hand rather than
/// generated, because `param_aliases` must be `&'static` and a const fn cannot
/// build it; `path_aliases_and_alias_map_agree` (below) pins the two together.
pub(crate) const PATH_PARAM_ALIAS_MAP: crate::tools::core::param_alias::AliasMap = &[
    ("file_path", "path"),
    ("relative_path", "path"),
    ("file", "path"),
];
```

- [ ] **Step 4: Add the agreement test that stops the two lists drifting**

In `src/fs/mod.rs`'s existing `#[cfg(test)] mod tests`:

```rust
/// The alias accept-set exists twice — as names (`PATH_PARAM_ALIASES`, consumed
/// by the per-call helpers) and as pairs (`PATH_PARAM_ALIAS_MAP`, consumed by the
/// dispatch normalizer). Adding to one and not the other is silent: the helper
/// would accept a name the normalizer never renames, so `call()` would see the
/// alias key and no correction would be announced.
#[test]
fn path_aliases_and_alias_map_agree() {
    let from_map: Vec<&str> = PATH_PARAM_ALIAS_MAP.iter().map(|(a, _)| *a).collect();
    assert_eq!(
        from_map, PATH_PARAM_ALIASES,
        "PATH_PARAM_ALIAS_MAP and PATH_PARAM_ALIASES must list the same aliases \
         in the same order"
    );
    assert!(
        PATH_PARAM_ALIAS_MAP.iter().all(|(_, c)| *c == "path"),
        "every path alias must canonicalise to \"path\""
    );
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test --lib param_alias path_aliases_and_alias_map_agree 2>&1`
Expected: PASS, 8 tests.

- [ ] **Step 6: Observe a RED by mutating the production path**

Change `normalize_params` so a conflict still overwrites the canonical value (replace the `if !conflicted` guard with an unconditional `obj.insert`).

Run: `cargo test --lib param_alias 2>&1`
Expected: FAIL at `canonical_wins_and_the_conflict_is_recorded`. **Revert the mutation.**

Then delete `("file", "path")` from `PATH_PARAM_ALIAS_MAP`.

Run: `cargo test --lib path_aliases_and_alias_map_agree 2>&1`
Expected: FAIL naming the list mismatch. **Revert the mutation.**

- [ ] **Step 7: Run the full gate**

```bash
./scripts/fmt-mine.sh
cargo clippy --workspace --all-targets --features local-embed -- -D warnings
cargo test --workspace --no-default-features ; cargo test --workspace
```
Read both lanes' exit codes independently.

- [ ] **Step 8: Commit**

```bash
git add src/tools/core/param_alias.rs src/tools/core/mod.rs src/fs/mod.rs
git commit -F <message file>
```
Message must name the two mutations observed red in Step 6.

---

## Task 2: `Tool::param_aliases`, declared and defaulted

**Files:**
- Modify: `src/tools/core/types.rs:778` (trait body — add the method near `selector_key`)
- Test: `src/tools/core/tests.rs`

**Interfaces:**
- Consumes: `AliasMap` from Task 1.
- Produces: `fn param_aliases(&self) -> AliasMap` on `Tool`, defaulting to `&[]`.

- [ ] **Step 1: Write the failing test**

In `src/tools/core/tests.rs`:

```rust
#[test]
fn param_aliases_defaults_to_empty_so_a_tool_opts_in() {
    // A tool that says nothing declares no aliases: the normalizer is a no-op
    // for it, which is what makes this safe to add to the trait rather than to
    // each implementor.
    struct Bare;
    #[async_trait::async_trait]
    impl crate::tools::Tool for Bare {
        fn name(&self) -> &str { "bare" }
        fn description(&self) -> &str { "d" }
        fn input_schema(&self) -> serde_json::Value { serde_json::json!({"type":"object"}) }
        async fn call(
            &self,
            _input: serde_json::Value,
            _ctx: &crate::tools::ToolContext,
        ) -> anyhow::Result<serde_json::Value> {
            Ok(serde_json::json!({"status":"ok"}))
        }
    }
    assert!(Bare.param_aliases().is_empty());
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test --lib param_aliases_defaults_to_empty 2>&1`
Expected: FAIL to compile — no method `param_aliases`.

- [ ] **Step 3: Add the trait method**

In `src/tools/core/types.rs`, inside `pub trait Tool`, immediately above `fn selector_key`:

```rust
    /// Non-canonical parameter names this tool accepts, as `(received, canonical)`.
    ///
    /// Advertise ONLY the canonical name in `input_schema`. `call_content` rewrites
    /// these before anything reads the input and announces the correction on the
    /// response — so a tool declaring an alias here must NOT also declare it as a
    /// property, and `every_declared_alias_is_absent_from_the_schema` (`src/server.rs`)
    /// enforces that.
    ///
    /// Defaults to empty: a tool opts in.
    fn param_aliases(&self) -> crate::tools::core::param_alias::AliasMap {
        &[]
    }
```

- [ ] **Step 4: Run it to verify it passes**

Run: `cargo test --lib param_aliases_defaults_to_empty 2>&1`
Expected: PASS.

- [ ] **Step 5: Run the full gate, then commit**

```bash
git add src/tools/core/types.rs src/tools/core/tests.rs
git commit -F <message file>
```

---

## Task 3: Thread it through all three render paths

This is the task most likely to ship a silent hole. `workspace_notice` is the working
precedent in the same function — follow it site for site.

**Files:**
- Modify: `src/tools/core/types.rs:979-1185` (`Tool::call_content`)
- Test: `src/tools/core/tests.rs`

**Interfaces:**
- Consumes: `normalize_params`, `correction_notice` (Task 1); `param_aliases` (Task 2).
- Produces: no new public API. Behaviour: `call()` never sees an alias key; the response carries the notice on all three paths.

- [ ] **Step 1: Write the three failing render-path tests plus the ordering test**

In `src/tools/core/tests.rs`. These drive `call_content`, **not** `call` — a direct
`call()` test proves nothing about this mechanism.

```rust
/// A tool that declares an alias, echoes what `call()` actually received, and can
/// be switched between the two OutputForms and a large payload. One fixture for
/// all three render paths so the paths are the only variable.
struct AliasEcho {
    form: crate::tools::core::types::OutputForm,
    big: bool,
}

#[async_trait::async_trait]
impl crate::tools::Tool for AliasEcho {
    fn name(&self) -> &str { "alias_echo" }
    fn description(&self) -> &str { "d" }
    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({"type":"object","properties":{"path":{"type":"string","description":"p"}}})
    }
    fn param_aliases(&self) -> crate::tools::core::param_alias::AliasMap {
        &[("file_path", "path")]
    }
    fn output_form(&self) -> crate::tools::core::types::OutputForm { self.form }
    // FIXTURE NOTE: the compact form deliberately renders ONLY `seen` — it models a
    // real tool that selects the fields it knows about. If this ever echoes the whole
    // value, the compact-text test stops discriminating and would pass on a framework
    // key it never re-attached.
    fn format_compact(&self, result: &serde_json::Value) -> Option<String> {
        Some(format!("seen={}", result["seen"]))
    }
    async fn call(
        &self,
        input: serde_json::Value,
        _ctx: &crate::tools::ToolContext,
    ) -> anyhow::Result<serde_json::Value> {
        let seen: Vec<String> = input.as_object().unwrap().keys().cloned().collect();
        let mut out = serde_json::json!({ "seen": seen.join(","), "path": input["path"] });
        if self.big {
            out["filler"] = serde_json::json!("x".repeat(15_000));
        }
        Ok(out)
    }
}

fn text_of(blocks: &[rmcp::model::Content]) -> String {
    blocks
        .iter()
        .filter_map(|b| b.as_text().map(|t| t.text.clone()))
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::test]
async fn call_sees_the_canonical_key_never_the_alias() {
    let (_dir, ctx) = make_ctx().await;
    let tool = AliasEcho { form: crate::tools::core::types::OutputForm::Json, big: false };
    let out = tool
        .call_content(serde_json::json!({ "file_path": "src/x.rs" }), &ctx)
        .await
        .unwrap();
    let t = text_of(&out);
    assert!(t.contains("seen=\"path\"") || t.contains("\"seen\": \"path\""),
        "call() must receive `path` and no `file_path`: {t}");
}

#[tokio::test]
async fn correction_reaches_the_caller_on_the_json_path() {
    let (_dir, ctx) = make_ctx().await;
    let tool = AliasEcho { form: crate::tools::core::types::OutputForm::Json, big: false };
    let out = tool
        .call_content(serde_json::json!({ "file_path": "src/x.rs" }), &ctx)
        .await
        .unwrap();
    let t = text_of(&out);
    assert!(t.contains("file_path") && t.contains("alias_echo"),
        "json path must carry the correction: {t}");
}

#[tokio::test]
async fn correction_reaches_the_caller_on_the_compact_text_path() {
    // THE REGRESSION THAT ALREADY HAPPENED ONCE, to `workspace_notice`:
    // docs/issues/2026-09-02-the-worktree-notice-is-injected-then-discarded-by-every-compact-renderer.md
    // `format_compact` renders only the fields the tool knows about, so a key the
    // framework added after `call()` returned is dropped unless re-attached HERE.
    let (_dir, ctx) = make_ctx().await;
    let tool = AliasEcho { form: crate::tools::core::types::OutputForm::Text, big: false };
    let out = tool
        .call_content(serde_json::json!({ "file_path": "src/x.rs" }), &ctx)
        .await
        .unwrap();
    let t = text_of(&out);
    assert!(t.contains("seen=") , "the compact render must still be present: {t}");
    assert!(t.contains("file_path"),
        "compact-text path dropped the correction — this is the 2026-09-02 bug: {t}");
}

#[tokio::test]
async fn correction_reaches_the_caller_on_the_buffered_path() {
    let (_dir, ctx) = make_ctx().await;
    let tool = AliasEcho { form: crate::tools::core::types::OutputForm::Json, big: true };
    let out = tool
        .call_content(serde_json::json!({ "file_path": "src/x.rs" }), &ctx)
        .await
        .unwrap();
    let t = text_of(&out);
    assert!(t.contains("output_id"), "payload should have buffered: {t}");
    assert!(t.contains("file_path"),
        "the returned envelope must carry the correction, not just the buffer: {t}");
}

#[tokio::test]
async fn no_alias_means_no_notice_anywhere() {
    let (_dir, ctx) = make_ctx().await;
    let tool = AliasEcho { form: crate::tools::core::types::OutputForm::Json, big: false };
    let out = tool
        .call_content(serde_json::json!({ "path": "src/x.rs" }), &ctx)
        .await
        .unwrap();
    let t = text_of(&out);
    assert!(!t.contains("not a parameter"),
        "a clean call must not be decorated: {t}");
}
```

**Note for the implementer:** `make_ctx()` is the existing helper in
`src/tools/core/tests.rs` used by the `call_content_*` tests. Read one of them
(e.g. `call_content_buffers_large_output`) and reuse whatever it uses verbatim — do
not invent a new fixture. If the helper has a different name, use the existing one.

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test --lib correction_reaches call_sees_the_canonical 2>&1`
Expected: FAIL — `call_sees_the_canonical_key_never_the_alias` shows `seen=file_path`, and all three `correction_reaches_*` show no correction.

- [ ] **Step 3: Normalize at the top of `call_content`**

In `src/tools/core/types.rs`, make `input` mutable and normalize as the **first**
statement of `call_content`, above the `selector` capture:

```rust
    async fn call_content(&self, mut input: Value, ctx: &ToolContext) -> Result<Vec<Content>> {
        // FIRST, before anything reads `input`. Three consumers below
        // (`selector_key`, `is_write`, `write_path`) inspect it pre-`call()`, and
        // `write_path` reads the literal key "path" — so a `file_path` call loses its
        // write-path annotation entirely if this runs later. Mutates in place: never
        // clone `input`, which for create_file/edit_file holds a whole file body.
        let corrections =
            crate::tools::core::param_alias::normalize_params(&mut input, self.param_aliases());
        let param_notice =
            crate::tools::core::param_alias::correction_notice(self.name(), &corrections);
        let selector = self.selector_key(&input);
```

- [ ] **Step 4: Thread the notice through the three render paths**

Site A — the buffered envelope. After the existing `workspace_notice` injection:

```rust
            if let Some(notice) = &workspace_notice {
                inject_notice(&mut buffered, notice);
            }
            if let Some(n) = &param_notice {
                buffered["warning"] = Value::String(n.clone());
            }
```

Site B — the compact-text branch. Extend the existing prefix so both notices reach
the channel that is actually read:

```rust
                    Content::text({
                        let mut prefix = String::new();
                        if let Some(notice) = &workspace_notice {
                            prefix.push_str(&format!("⚠ {notice}\n\n"));
                        }
                        if let Some(n) = &param_notice {
                            prefix.push_str(&format!("⚠ {n}\n\n"));
                        }
                        format!("{prefix}{text}")
                    })
```

Site C — the small-output value, before the form branch:

```rust
            if let Some(notice) = &workspace_notice {
                inject_notice(&mut val, notice);
            }
            if let Some(n) = &param_notice {
                if let Some(obj) = val.as_object_mut() {
                    obj.insert("warning".to_string(), Value::String(n.clone()));
                }
            }
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test --lib correction_reaches call_sees_the_canonical no_alias_means_no_notice 2>&1`
Expected: PASS, 5 tests.

- [ ] **Step 6: Observe a RED for each of the three sites, separately**

Delete Site A → expect only `correction_reaches_the_caller_on_the_buffered_path` to fail. Revert.
Delete Site B → expect only `..._on_the_compact_text_path` to fail. Revert.
Delete Site C → expect only `..._on_the_json_path` to fail. Revert.

Then move the `normalize_params` call to directly below the `write_path` capture.
Expected: `call_sees_the_canonical_key_never_the_alias` still passes (it inspects
`call()`), which is the point — add this instead, then revert the move:

```rust
#[tokio::test]
async fn normalization_precedes_the_write_path_capture() {
    // `write_path` reads the literal key "path" before `call()`. If normalization runs
    // after it, a `file_path` write silently loses its path annotation — a hole no
    // correction test can see, because the correction is still announced.
    let (_dir, ctx) = make_ctx().await;
    let out = crate::tools::CreateFile
        .call_content(
            serde_json::json!({ "file_path": "notes.txt", "content": "hi" }),
            &ctx,
        )
        .await
        .unwrap();
    let t = text_of(&out);
    assert!(t.contains("notes.txt"),
        "the write-path annotation must name the file even when given as file_path: {t}");
}
```

**If all three site deletions red the same test, the tests are not independent — stop and fix them before proceeding.**

- [ ] **Step 7: Run the full gate, then commit**

Message must list which test each of the four mutations reddened.

---

## Task 4: Collapse the four file tools

**Files:**
- Modify: `src/tools/read_file.rs:37-57`, `src/tools/create_file.rs:36-47`, `src/tools/edit_file/mod.rs:390-401`, `src/tools/grep.rs:39-45`
- Test: `src/server.rs` (the per-alias gate arrives in Task 7; this task is covered by Task 3's mechanism plus the surface report)

**Interfaces:**
- Consumes: `param_aliases` (Task 2), `PATH_PARAM_ALIAS_MAP` (Task 1).
- Produces: nothing new.

- [ ] **Step 1: For each of the four tools, delete the alias properties and add the declaration**

`read_file` — delete the `file_path`, `relative_path`, `file`, `output_id` properties **and
the FIXTURE NOTE comment above them** (it documents a gate that no longer exists after Task
7). Then add:

```rust
    fn param_aliases(&self) -> crate::tools::core::param_alias::AliasMap {
        // `output_id`/`file_id` join the path family: `path` already accepts
        // `@tool_*`/`@cmd_*`/`@file_*` handles via `strip_buffer_ref_quotes`, so these
        // were renames of `path`, never a separate capability.
        &[
            ("file_path", "path"),
            ("relative_path", "path"),
            ("file", "path"),
            ("output_id", "path"),
            ("file_id", "path"),
        ]
    }
```

`create_file`, `edit_file`, `grep` — delete their alias properties and FIXTURE NOTE
comments, then add to each:

```rust
    fn param_aliases(&self) -> crate::tools::core::param_alias::AliasMap {
        crate::fs::PATH_PARAM_ALIAS_MAP
    }
```

- [ ] **Step 2: Simplify `read_file`'s own fallback chain**

`src/tools/read_file.rs:79-96` hand-rolls the alias walk including `output_id` and
`file_id`. Leave the chain in place (Global Constraints: direct-`call()` tests depend on
it) but update its comment to say it is now a redundant second layer, naming
`param_alias.rs` as the first.

- [ ] **Step 3: Run the tests**

Run: `cargo test --workspace 2>&1`
Expected: the four schema gates in `src/server.rs` FAIL — `EXPECTED_ALIAS_COUNTS_BY_TOOL` now finds 0 aliases where it expects 3-4. **This is the expected intermediate state**; Task 7 replaces them. Every other test must pass. If anything else fails, stop.

- [ ] **Step 4: Commit**

Commit with the gates red, stating in the message that Task 7 replaces them and that no other test regressed. This is the one deliberately-red commit in the plan; do not skip the statement.

---

## Task 5: Collapse the four symbol tools

**Files:**
- Modify: `src/tools/symbol/edit_code.rs:116-134`, `src/tools/symbol/references.rs:233-245`, `src/tools/symbol/symbol_at.rs:359-371`, `src/tools/symbol/call_graph/mod.rs:377-387`

**Interfaces:** as Task 4.

- [ ] **Step 1: Delete the alias properties and FIXTURE NOTE comments from all four**

- [ ] **Step 2: Add the declaration to `references`, `symbol_at`, `call_graph`**

```rust
    fn param_aliases(&self) -> crate::tools::core::param_alias::AliasMap {
        crate::fs::PATH_PARAM_ALIAS_MAP
    }
```

- [ ] **Step 3: `edit_code` also folds its two inline-documented aliases**

Delete the trailing `Alias: \`name_path\` … is accepted.` sentence from `symbol`'s
description and the `Alias: \`content\` …` sentence from `body`'s, then:

```rust
    fn param_aliases(&self) -> crate::tools::core::param_alias::AliasMap {
        &[
            ("file_path", "path"),
            ("relative_path", "path"),
            ("file", "path"),
            ("name_path", "symbol"),
            ("content", "body"),
        ]
    }
```

- [ ] **Step 4: Check `edit_code`'s `call()` for its own alias handling**

`edit_code` reads `name_path` and `content` itself. Find those reads
(`grep(pattern="name_path|\"content\"", path="src/tools/symbol/edit_code.rs")`) and leave
them, per Global Constraints. Add a comment naming `param_alias.rs` as the primary layer.

- [ ] **Step 5: Run the tests**

Run: `cargo test --workspace 2>&1`
Expected: same four gates still red from Task 4; nothing else new.

- [ ] **Step 6: Commit**

---

## Task 6: The two non-path collapses, with their coupled docs

`symbols` is the only task in this plan that changes a name our own served guidance
teaches. The doc edits are **not** optional follow-up — shipped separately, every session
following the guide gets warned on every call.

**Files:**
- Modify: `src/tools/symbol/symbols.rs:135-145`
- Modify: `src/librarian/tools/librarian.rs:110`
- Modify: `src/prompts/guides/symbol-navigation.md` (4 sites)
- Modify: `docs/manual/src/concepts/tool-selection.md`, `docs/manual/src/tools/symbol-navigation.md`, `docs/manual/src/tools/tool-workflows.md`

- [ ] **Step 1: Collapse `symbols`**

Delete the `name` and `name_path` properties. Keep `query` and `symbol`. Add:

```rust
    fn param_aliases(&self) -> crate::tools::core::param_alias::AliasMap {
        // `symbol` is the family-wide name for this concept on references/call_graph/
        // edit_code; `docs/manual/src/tools/api-redesign.md` already documents
        // name_path -> symbol as the intended rename. This completes it.
        &[
            ("name", "query"),
            ("name_path", "symbol"),
            ("file_path", "path"),
            ("relative_path", "path"),
            ("file", "path"),
        ]
    }
```

Note `symbols` takes `path` optionally via `get_path_param`, so the path family applies here too.

- [ ] **Step 2: Collapse `librarian`'s `root`**

Delete the `root` property. Rewrite `old_root`'s description to drop "Preferred alias of root — use this name, it's the one the doctor hints and error text surface", since it is now simply the parameter name. Add:

```rust
    fn param_aliases(&self) -> crate::tools::core::param_alias::AliasMap {
        &[("root", "old_root")]
    }
```

**Verify before committing:** `librarian`'s `call()` reads `root` for `merge_worktree` and `doctor` as well as `rehome`. Run `grep(pattern="\"root\"", path="src/librarian/tools/")` and confirm every read is the same parameter. **If `root` means something different for `merge_worktree` than for `rehome`, they are not aliases and this step must be dropped** — report that finding instead of forcing it.

- [ ] **Step 3: Update the served guide**

In `src/prompts/guides/symbol-navigation.md`, replace all four `symbols(name_path=…)` with `symbols(symbol=…)` and `symbols(name=…)` with `symbols(query=…)`.

- [ ] **Step 4: Update `src/prompts/README.md:33`**

The sentence "aliases (`file_path`, `limit`) are discoverable from the tool schema" is now false. Replace with:

```markdown
5. **Don't document every param.** Pagination (`offset`, `limit`, `detail_level`) is
   discoverable from the tool schema. Aliases are NOT in the schema at all — there is
   exactly one advertised name per concept, and a caller who sends another is corrected
   at runtime with a `warning` naming the right one (`src/tools/core/param_alias.rs`).
   Only document params that change behavior in non-obvious ways.
```

- [ ] **Step 5: Update the manual examples**

Replace `relative_path` with `path` and `references(name_path, path)` with `references(symbol, path)` in the four manual files listed above. Leave `docs/manual/src/tools/api-redesign.md` alone — it is a historical rename table.

- [ ] **Step 6: Run the tests, then the doc-ref audit**

```
cargo test --workspace 2>&1
```
Then `librarian(action="audit_doc_refs")` and confirm no new `high` findings.

- [ ] **Step 7: Commit** — code and every doc surface in ONE commit.

---

## Task 7: Replace the four vacuous gates with five

The old gates find offenders by parsing `"Alias for "` descriptions. With the properties
gone they have nothing to scan and **pass by finding nothing** — an absence assertion is
monotone under removal. They must be replaced, not edited.

**Files:**
- Modify: `src/server.rs` — delete `required_names_no_key_that_has_a_declared_alias`, `path_requiring_tools_never_name_path_or_an_alias_in_required`, `alias_offender_detection_catches_a_synthetic_offender`, `parse_declared_aliases`, `find_alias_offenders`, `schema_required_names`, `EXPECTED_ALIAS_COUNTS_BY_TOOL`, `TOOLS_REQUIRING_PATH_VIA_ALIASES`. **Keep `no_tool_schema_declares_a_top_level_combinator` untouched.**

- [ ] **Step 1: Write the five replacement gates**

```rust
    /// No property may describe itself as an alias, because no property IS one any
    /// more — `Tool::param_aliases` holds the accept-set and the schema advertises
    /// exactly one name per concept. Reds if a collapsed alias is reintroduced as a
    /// property, which is the regression this collapse invites.
    ///
    /// Scoped to the two prose forms the corpus actually used (`"Alias for "`,
    /// `"(alias of "`). `read_file`'s `offset`/`limit` say "Native-Read-style alias"
    /// and are deliberately NOT matched: they are a second calling convention, not a
    /// rename, and remain advertised on purpose.
    #[tokio::test]
    async fn no_schema_property_declares_itself_an_alias() {
        let (_dir, server) = make_server().await;
        let mut offenders = Vec::new();
        for t in &server.tools {
            let schema = t.input_schema();
            let Some(props) = schema.get("properties").and_then(|p| p.as_object()) else {
                continue;
            };
            for (name, def) in props {
                let Some(d) = def.get("description").and_then(|v| v.as_str()) else {
                    continue;
                };
                if d.starts_with("Alias for ") || d.contains("(alias of ") {
                    offenders.push(format!(
                        "{}.{name}: description declares an alias — move it to \
                         param_aliases() and delete the property",
                        t.name()
                    ));
                }
            }
        }
        assert!(
            server.tools.len() >= 15,
            "truncated registry: {} tools",
            server.tools.len()
        );
        assert!(offenders.is_empty(), "{}", offenders.join("\n  "));
    }

    /// The honesty gate, inverted. A name in `param_aliases()` must NOT also be a
    /// property: advertising it re-creates the ambiguity the collapse removed, and
    /// makes `required` unstateable again — which is what produced the API-illegal
    /// top-level `anyOf` (see `no_tool_schema_declares_a_top_level_combinator`).
    #[tokio::test]
    async fn every_declared_alias_is_absent_from_the_schema() {
        let (_dir, server) = make_server().await;
        let mut offenders = Vec::new();
        let mut checked = 0usize;
        for t in &server.tools {
            let schema = t.input_schema();
            let props = schema.get("properties").and_then(|p| p.as_object());
            for (received, canonical) in t.param_aliases() {
                checked += 1;
                if props.is_some_and(|p| p.contains_key(*received)) {
                    offenders.push(format!(
                        "{}: {received:?} is both a declared alias and an advertised \
                         property; delete the property",
                        t.name()
                    ));
                }
                if !props.is_some_and(|p| p.contains_key(*canonical)) {
                    offenders.push(format!(
                        "{}: alias {received:?} canonicalises to {canonical:?}, which is \
                         not an advertised property — the rewrite would produce a key no \
                         caller can discover",
                        t.name()
                    ));
                }
            }
        }
        assert!(
            checked >= 20,
            "expected the collapsed alias population, saw {checked} — this sweep is \
             vacuous if the declarations went missing"
        );
        assert!(offenders.is_empty(), "{}", offenders.join("\n  "));
    }

    /// PER ALIAS, not per tool: an aggregate cannot verify a per-member claim. Drives
    /// the real dispatch path, because a direct `call()` bypasses the normalizer and
    /// would prove nothing.
    #[tokio::test]
    async fn every_declared_alias_is_normalized_and_announced() {
        let (_dir, server) = make_server().await;
        let mut checked = 0usize;
        for t in &server.tools {
            for (received, canonical) in t.param_aliases() {
                checked += 1;
                let mut input = serde_json::json!({});
                input[*received] = serde_json::json!("probe-value");
                let corrections = crate::tools::core::param_alias::normalize_params(
                    &mut input,
                    t.param_aliases(),
                );
                assert!(
                    input.get(*received).is_none(),
                    "{}: {received:?} survived normalization",
                    t.name()
                );
                assert_eq!(
                    input[*canonical],
                    serde_json::json!("probe-value"),
                    "{}: {received:?} did not land on {canonical:?}",
                    t.name()
                );
                let notice = crate::tools::core::param_alias::correction_notice(
                    t.name(),
                    &corrections,
                )
                .unwrap_or_else(|| panic!("{}: {received:?} produced no notice", t.name()));
                assert!(
                    notice.contains(received) && notice.contains(t.name()),
                    "{}: notice must name the key and the tool: {notice}",
                    t.name()
                );
            }
        }
        assert!(checked >= 20, "vacuous: only {checked} aliases seen");
    }

    /// `call_content` is where normalization happens, so a tool that OVERRIDES it opts
    /// out of the mechanism entirely — silently. `Onboarding` is the only override in
    /// the tree (`src/tools/onboarding.rs`). This gate is a hand-list because the trait
    /// gives no way to ask "did you override this"; keeping the list short is the point.
    #[tokio::test]
    async fn call_content_overriders_declare_no_aliases() {
        const OVERRIDES_CALL_CONTENT: &[&str] = &["onboarding"];
        let (_dir, server) = make_server().await;
        let mut seen = 0usize;
        for t in &server.tools {
            if !OVERRIDES_CALL_CONTENT.contains(&t.name()) {
                continue;
            }
            seen += 1;
            assert!(
                t.param_aliases().is_empty(),
                "{} overrides call_content AND declares aliases, so its aliases are \
                 never normalized or announced. Either drop the override or normalize \
                 inside it.",
                t.name()
            );
        }
        assert_eq!(
            seen,
            OVERRIDES_CALL_CONTENT.len(),
            "OVERRIDES_CALL_CONTENT names a tool that is not registered; re-derive it \
             with grep(pattern=\"fn call_content\", glob=\"src/**/*.rs\")"
        );
    }
```

- [ ] **Step 2: Delete the four old gates and their now-unused helpers**

Delete the items listed under **Files**. `cargo clippy` will name anything left unused.

- [ ] **Step 3: Run the tests**

Run: `cargo test --workspace 2>&1`
Expected: PASS. The Task 4/5 red is now resolved.

- [ ] **Step 4: Observe a RED for each new gate**

| mutation | must red |
|---|---|
| re-add `"file_path": {"type":"string","description":"Alias for path"}` to `create_file` | `no_schema_property_declares_itself_an_alias` **and** `every_declared_alias_is_absent_from_the_schema` |
| change `edit_code`'s `("content","body")` to `("content","bodyy")` | `every_declared_alias_is_absent_from_the_schema` (canonical not advertised) |
| make `normalize_params` return `Vec::new()` early | `every_declared_alias_is_normalized_and_announced` |
| add `("x","path")` to `Onboarding::param_aliases` | `call_content_overriders_declare_no_aliases` |

Revert each. Record the four in the commit message.

- [ ] **Step 5: Run the full gate, then commit**

---

## Task 8: Ratchet the surface budget

**Files:**
- Modify: `src/server.rs` (`TOOL_SURFACE_CHAR_BUDGET` and its log comment)

- [ ] **Step 1: Measure**

Run: `cargo test --lib tool_surface_report_lengths -- --nocapture 2>&1`
Read the `TOTAL (N tools)` line. **Use that number; do not compute or estimate one.**

- [ ] **Step 2: Set the constant to the exact measured total and add a log entry**

Follow the existing entries' form: state what the bytes bought, or in this case what was
removed and why it was not padding. Note that ~26 duplicate properties went and that the
saving is removed rather than banked, per the constant's own rule.

- [ ] **Step 3: Verify**

Run: `cargo test --lib tool_surface_under_budget tool_surface_report_lengths -- --nocapture 2>&1`
Expected: PASS with `headroom 0`.

- [ ] **Step 4: Commit**

---

## Task 9: Verify the real wire, and record the outcome

A green suite is not evidence that the served surface changed — the gates read
`input_schema()`, not `tools/list`.

**Files:**
- Create: `docs/issues/` entry only if a defect is found. Otherwise no file changes.

- [ ] **Step 1: Build and probe the live surface**

```bash
cargo rb
```
Then run a `tools/list` probe against the built binary with a project active and assert, in
the returned JSON: zero properties named `file_path`/`relative_path`/`file`/`output_id`/`name`/`name_path`/`root`, and zero top-level `anyOf`/`oneOf`/`allOf`.

- [ ] **Step 2: Exercise one corrected call end to end**

Reconnect (`/mcp`) and call `read_file(file_path="Cargo.toml", toml_key="package")`.
Expected: the file content **and** a `warning` naming `file_path` and `path`.

- [ ] **Step 3: Exercise the compact-text path specifically**

Call `symbols(name_path="Tool/call_content", path="src/tools/core/types.rs")`.
Expected: the symbol body **and** a `⚠` line naming `name_path` → `symbol`. This is the
path that regressed for `workspace_notice`; confirm it on the wire, not only in a test.

- [ ] **Step 4: Append the outcome to the session log**

Append a `W-N` win (or `F-N` friction if anything above failed) to the project's
session-log tracker via `doc(action="append_entry")`, citing the measured budget delta and
the three wire probes. Consult `docs/TAXONOMY.md` for the exact call and artifact id.

- [ ] **Step 5: Final full gate, then commit**

---

## Self-Review

**Spec coverage.** Goals: one advertised name → Tasks 4, 5, 6. Always accepted → Task 1 (`normalize_params` never errors) + the retained per-call fallbacks (Task 4 Step 2, Task 5 Step 4). Announced on every shape → Task 3. Structurally impossible to forget → Task 3 (one boundary) + Task 7 gate 3 (per alias) + gate 4 (the override hole). Non-goals: `offset`/`limit` appear in no task; no task makes an alias an error. Design §1 → Task 2; §2 → Task 3 Step 3; §3 → Task 3 Steps 4/6; §4 → Task 4 Step 2, Task 5 Step 4; §5 → Task 5 Step 3, Task 6 Step 2. Inventory rows all mapped (`grep` → Task 4, `librarian` → Task 6). Gate replacement → Task 7. Coupled surfaces → Task 6. Budget → Task 8. Testing → Tasks 1/3/7 plus Task 9 for the wire.

**Placeholder scan.** No TBD/TODO. Every code step carries real code. The two places that say "read the existing helper and reuse it verbatim" (Task 3 Step 1's `make_ctx`, Task 5 Step 4's `grep`) are deliberate: inventing a fixture name that does not exist would be worse than naming the lookup, and both name the exact command to run.

**Type consistency.** `AliasMap`, `Correction { received, canonical, conflicted }`, `normalize_params(&mut Value, AliasMap) -> Vec<Correction>`, `correction_notice(&str, &[Correction]) -> Option<String>` are used identically in Tasks 1, 3 and 7. `param_aliases()` returns `AliasMap` everywhere. `PATH_PARAM_ALIAS_MAP` is `pub(crate)` in `src/fs/mod.rs` and referenced as `crate::fs::PATH_PARAM_ALIAS_MAP` in Tasks 4, 5, 6.

**One risk the plan carries deliberately.** Task 4 commits with four gates red, resolved in Task 7. The alternative — deleting the gates before the properties — leaves the tree with no alias policing at all for two tasks. A stated red is better than a silent gap, and Task 4 Step 4 requires the commit message to say so.
