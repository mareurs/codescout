# MCP Prompt Channel Redesign — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Shrink codescout's MCP system prompt to fit Claude Code's ~2 KB cap, move deep content into an on-demand `get_guide(topic)` tool, and auto-suggest the guide on first relevant tool call.

**Architecture:** Four surfaces. (A) `source.md` rewritten to ≤1,800 chars with paired do-instead Iron Laws. (B) New `get_guide(topic)` tool returning full guide text as tool result. (C) First-call hint in `Tool::call_content` wrapper injects `_guide_hint` once per topic per session. (D) Tracker for future topic candidates.

**Tech Stack:** Rust 1.94, async-trait, rmcp, parking_lot, anyhow. Existing codescout patterns: `Tool` trait, `OutputBuffer`, `RecoverableError`.

**Spec:** `docs/superpowers/specs/2026-05-19-mcp-prompt-channel-redesign-design.md`
**Evidence base:** `docs/architecture/mcp-channel-caps.md`

---

## File Structure

**New files:**
- `src/tools/guide.rs` — `GetGuide` tool implementation
- `src/prompts/guides/librarian.md` — extracted from existing `librarian-guide.md`
- `src/prompts/guides/tracker-conventions.md` — extracted from `CLAUDE.md` + `docs/issues/_TEMPLATE.md`
- `src/prompts/guides/progressive-disclosure.md` — extracted from `docs/PROGRESSIVE_DISCOVERABILITY.md`
- `src/prompts/guides/error-handling.md` — extracted from `src/tools/core/types.rs` doc comments + `CLAUDE.md`
- `docs/trackers/get-guide-topics.md` — future topic tracker

**Modified files:**
- `src/prompts/source.md` — rewrite to ≤1,800 chars
- `src/tools/mod.rs` — `pub mod guide;`
- `src/tools/core/types.rs` — add `relevant_guide_topic`; modify `call_content` to inject hint; extend `ToolContext` with `guide_hints_emitted`
- `src/server.rs` — register `GetGuide`; remove `librarian::INSTRUCTIONS` concat at lines 91, 379; add `guide_hints_emitted` field; pass into `ToolContext` at dispatch
- `src/tools/config/mod.rs` — reset `guide_hints_emitted` on activate
- `src/tools/run_command/run_command.rs` — override `relevant_guide_topic`
- `src/tools/symbol/symbols.rs` — override `relevant_guide_topic`
- `src/librarian/adapter.rs` — override `relevant_guide_topic` on librarian-tool adapters
- `src/librarian/server.rs` — delete `INSTRUCTIONS` const (line 38)
- `src/librarian/mod.rs` — drop re-export of `INSTRUCTIONS` if any
- `build.rs` — size assert on rendered `server_instructions.md`

**Deleted files:**
- `src/librarian/prompts/server_instructions.md` (the librarian's bundled prompt) — after `get_guide("librarian")` verified working.

**Cleanup (final commit):**
- `/home/marius/agents/llm-proxy/.env` — remove `LOG_FULL_TOOLS=1` and `LOG_TOOL_DIGEST=1`
- `~/.claude-kat/.claude.json` — remove `CODESCOUT_PROBE=1` from codescout env

Note: probe code in `src/tools/probe.rs` and `src/mcp_resources/probe.rs` STAYS in the tree, gated on `CODESCOUT_PROBE=1`. Reusable for future cap re-measurement.

---

## Commit Sequence

Three commits, in order. Each commit is independently shippable on `experiments` branch. Cherry-pick to `master` after the three land + smoke test.

1. **Commit A:** Surface B + guide content (GetGuide tool, 4 content files). `librarian::INSTRUCTIONS` still concatenated. Tasks 1-9.
2. **Commit B:** Surface A rewrite + sever librarian concat + delete INSTRUCTIONS + Surface D tracker. Tasks 10-18.
3. **Commit C:** Surface C first-call hint mechanism. Tasks 19-29.
4. **Cleanup commit:** revert proxy + .claude.json debug flags. Task 30.

---

# Commit A — Surface B (`get_guide` tool + content)

### Task 1: Stage guide content directory

**Files:**
- Create: `src/prompts/guides/.gitkeep` (placeholder for the directory)

- [ ] **Step 1: Create directory**

```bash
mkdir -p src/prompts/guides
touch src/prompts/guides/.gitkeep
```

- [ ] **Step 2: Verify**

Run: `ls src/prompts/guides/`
Expected: `.gitkeep`

---

### Task 2: Extract `librarian` guide content

**Files:**
- Create: `src/prompts/guides/librarian.md`
- Reference: `src/prompts/librarian-guide.md` (existing, 9 KB)
- Reference: `src/librarian/prompts/server_instructions.md` (existing, the concat source)

- [ ] **Step 1: Copy existing librarian-guide.md as the base**

```bash
cp src/prompts/librarian-guide.md src/prompts/guides/librarian.md
```

- [ ] **Step 2: Inspect the bundled server_instructions.md (the dead concat source)**

```bash
wc -c src/librarian/prompts/server_instructions.md
head -50 src/librarian/prompts/server_instructions.md
```

- [ ] **Step 3: Merge any unique content from the bundled file into `src/prompts/guides/librarian.md`**

Read both files. If the bundled `server_instructions.md` has paragraphs not in `librarian-guide.md`, append them under a `## Runtime tips` section. If everything overlaps, skip.

Use `read_markdown(path, heading="<section>")` on both files. Use `edit_markdown(path="src/prompts/guides/librarian.md", action="insert_after", heading="<last existing>", content="...")` to append.

- [ ] **Step 4: Verify size**

Run: `wc -c src/prompts/guides/librarian.md`
Expected: 8,000-12,000 bytes (about the same as the original).

- [ ] **Step 5: Commit the staging move**

Defer commit until end of Task 9 — this is one commit.

---

### Task 3: Extract `tracker-conventions` guide content

**Files:**
- Create: `src/prompts/guides/tracker-conventions.md`
- Reference: `CLAUDE.md` heading "## Session Intelligence Trackers"
- Reference: `docs/issues/_TEMPLATE.md` header comment

- [ ] **Step 1: Pull the trackers section from CLAUDE.md**

```bash
# Read the section
```

Use `read_markdown("CLAUDE.md", heading="## Session Intelligence Trackers")`.

- [ ] **Step 2: Pull the bug template header**

Use `read_markdown("docs/issues/_TEMPLATE.md", start_line=1, end_line=60)`.

- [ ] **Step 3: Compose the guide file**

`create_file("src/prompts/guides/tracker-conventions.md", content="""
# Tracker Conventions

(One-paragraph intro: trackers vs bugs vs ADRs, where each lives.)

## Bug files (docs/issues/)

(extracted from _TEMPLATE.md header — status vocab, trigger rules, capture
discipline, archive flow.)

## Tracker artifacts (docs/trackers/)

(extracted from CLAUDE.md — frontmatter shape, status field, archive trigger.)

## Querying with the librarian

(short examples: artifact(action="find", kind="tracker") etc.)
""")`

- [ ] **Step 4: Verify size and shape**

Run: `wc -c src/prompts/guides/tracker-conventions.md`
Expected: 2,500-4,000 bytes.

Verify the file has the three top-level sections above.

---

### Task 4: Extract `progressive-disclosure` guide content

**Files:**
- Create: `src/prompts/guides/progressive-disclosure.md`
- Reference: `docs/PROGRESSIVE_DISCOVERABILITY.md` (existing)
- Reference: `src/tools/core/types.rs:15-51` (the constants block)

- [ ] **Step 1: Read the existing canonical doc**

Use `read_markdown("docs/PROGRESSIVE_DISCOVERABILITY.md")` to get section list. Then pull the model-facing subset (output budgets, @ref buffer mechanics, overflow patterns — skip developer-facing patterns like "how to write a tool").

- [ ] **Step 2: Read the constants for cap math**

Use `symbols(name="MAX_INLINE_TOKENS", path="src/tools/core/types.rs", include_body=true)` and surrounding constants.

- [ ] **Step 3: Compose the guide file**

`create_file("src/prompts/guides/progressive-disclosure.md", content="""
# Progressive Disclosure

How codescout handles results too big to inline, and how the model
should respond to them.

## Output budgets

| Constant | Bytes | Tokens (≈) | Source |
|---|---|---|---|
| MAX_INLINE_TOKENS | 10,000 | 2,500 | src/tools/core/types.rs:18 |
| INLINE_BYTE_BUDGET | 9,000 | 2,250 | src/tools/core/types.rs:27 |
| COMPACT_SUMMARY_MAX_BYTES | 2,000 | 500 | src/tools/core/types.rs:49 |

Above MAX_INLINE_TOKENS, the tool result is stored in the @tool_*
buffer and a compact summary returned to the model.

## The @ref buffer

When a tool returns `{output_id: "@tool_xyz", summary, hint}`:

- `output_id` is a handle pointing to a server-side buffer holding
  the full result.
- `summary` is the compact form (≤ 2 KB).
- `hint` shows the most useful follow-up call for that tool.

Query the buffer instead of re-running the tool:

```
grep PATTERN @tool_xyz                    # search the buffer
read_file("@tool_xyz", json_path="$.foo") # extract a field
read_file("@tool_xyz", start_line=N, end_line=M)  # slice lines
```

## Anti-patterns

- Re-running a tool because the result was "too long". Query the
  buffer instead.
- Asking the user to paste content from a buffered result. The
  buffer is server-side; you can read it.
- Treating `output_id` as a filename. It's a handle; read_file with
  the @ref works, file-system paths don't.
""")`

- [ ] **Step 4: Verify size**

Run: `wc -c src/prompts/guides/progressive-disclosure.md`
Expected: 1,800-3,500 bytes.

---

### Task 5: Extract `error-handling` guide content

**Files:**
- Create: `src/prompts/guides/error-handling.md`
- Reference: `src/tools/core/types.rs` doc comments around `RecoverableError`

- [ ] **Step 1: Read RecoverableError docs**

Use `symbols(name="RecoverableError", path="src/tools/core/types.rs", include_body=true)`.

- [ ] **Step 2: Compose the guide file**

`create_file("src/prompts/guides/error-handling.md", content="""
# Error Handling

Two error paths in codescout. The model behavior differs.

## RecoverableError → isError: false

Input-driven, expected failure. Sibling tool calls in the same turn
SURVIVE. Examples:

- Unknown symbol name passed to `symbols(name=...)`.
- Topic not found in `get_guide(topic)`.
- User declined an elicitation prompt.

The error message contains a `hint` describing how to recover.
Read the hint, adjust the call, retry.

## anyhow::bail! → isError: true

Genuine tool failure: LSP server crashed, file system error, parse
error in a config the tool owns. Fatal — the whole tool call
sequence in the turn is at risk.

The model should not retry without changing inputs. The user may need
to intervene.

## How to tell them apart

The MCP response has an `isError` field. `true` → fatal, stop.
`false` → recoverable, read the message + hint and adapt.

For tool authors: return `RecoverableError::new(message, hint)` when
the error is the model's fault (bad input). Return `anyhow::bail!`
when it's not.
""")`

- [ ] **Step 3: Verify size**

Run: `wc -c src/prompts/guides/error-handling.md`
Expected: 1,200-2,000 bytes.

---

### Task 6: Write failing tests for `GetGuide` tool

**Files:**
- Create: `src/tools/guide.rs` (tests module only at this stage)
- Modify: `src/tools/mod.rs` (add `pub mod guide;`)

- [ ] **Step 1: Add the module declaration**

Use `edit_file("src/tools/mod.rs", insert="append", new_string="\npub mod guide;\n")`.

- [ ] **Step 2: Create the test scaffold**

`create_file("src/tools/guide.rs", content="""
//! `get_guide(topic)` tool — returns deep guidance text as the tool result.
//!
//! Topics are content files embedded at build time. See
//! `docs/superpowers/specs/2026-05-19-mcp-prompt-channel-redesign-design.md`
//! for the design.

use anyhow::Result;
use serde_json::{json, Value};
use std::collections::BTreeMap;

use crate::tools::core::{RecoverableError, Tool, ToolContext};

pub struct GetGuide {
    topics: BTreeMap<&'static str, &'static str>,
}

impl GetGuide {
    pub fn new() -> Self {
        let mut topics: BTreeMap<&'static str, &'static str> = BTreeMap::new();
        topics.insert("librarian", include_str!("../prompts/guides/librarian.md"));
        topics.insert("tracker-conventions", include_str!("../prompts/guides/tracker-conventions.md"));
        topics.insert("progressive-disclosure", include_str!("../prompts/guides/progressive-disclosure.md"));
        topics.insert("error-handling", include_str!("../prompts/guides/error-handling.md"));
        Self { topics }
    }
}

impl Default for GetGuide {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::Agent;
    use std::sync::Arc;

    fn ctx() -> ToolContext {
        // Reuse the existing test helper if one exists; otherwise build a
        // minimal one. Search src/tools/core for `fn test_context`.
        unimplemented!("wire to existing test ToolContext factory");
    }

    #[tokio::test]
    async fn get_guide_lists_topics_with_no_arg() {
        let g = GetGuide::new();
        let result = g.call(json!({}), &ctx()).await.unwrap();
        let topics = result["topics"].as_array().unwrap();
        let names: Vec<&str> = topics.iter().map(|v| v.as_str().unwrap()).collect();
        assert!(names.contains(&"librarian"));
        assert!(names.contains(&"tracker-conventions"));
        assert!(names.contains(&"progressive-disclosure"));
        assert!(names.contains(&"error-handling"));
        assert_eq!(names.len(), 4);
    }

    #[tokio::test]
    async fn get_guide_returns_librarian_body() {
        let g = GetGuide::new();
        let result = g.call(json!({"topic": "librarian"}), &ctx()).await.unwrap();
        assert_eq!(result["topic"].as_str(), Some("librarian"));
        let body = result["body"].as_str().unwrap();
        assert!(body.len() > 2000, "librarian body should be > 2KB, got {} bytes", body.len());
        assert!(body.contains("artifact"), "should mention artifact in librarian guide");
    }

    #[tokio::test]
    async fn get_guide_unknown_topic_is_recoverable() {
        let g = GetGuide::new();
        let err = g.call(json!({"topic": "nope"}), &ctx()).await.unwrap_err();
        let recoverable = err.downcast_ref::<RecoverableError>();
        assert!(recoverable.is_some(), "expected RecoverableError, got {err}");
        let msg = format!("{err}");
        assert!(msg.contains("librarian"), "error message should list available topics");
    }
}
""")`

- [ ] **Step 3: Find the test ToolContext factory and wire it in**

Run: `grep "fn.*ToolContext.*->.*ToolContext\\|fn test_context\\|TestContext" src/tools/core/tests.rs src/tools/core/mod.rs`

Replace the `unimplemented!()` in the `ctx()` helper with the actual factory call. The librarian module has integration tests that build a ToolContext — pattern after those.

- [ ] **Step 4: Run tests — expect compilation failure**

Run: `cargo test --lib --no-run guide::tests`
Expected: build fails because guide content files don't yet exist OR test ctx isn't wired.

If the failure is about missing content files, that's fine — Task 7 fixes it. If it's about ctx wiring, fix Step 3.

---

### Task 7: Implement `Tool` for `GetGuide` and run tests

**Files:**
- Modify: `src/tools/guide.rs`

- [ ] **Step 1: Add the `Tool` impl below the struct definition**

Use `edit_code` to insert after `impl Default for GetGuide`:

```rust
#[async_trait::async_trait]
impl Tool for GetGuide {
    fn name(&self) -> &str { "get_guide" }

    fn description(&self) -> &str {
        "Fetch deep guidance for a topic. Returns the full text as the tool \
         result; large topics overflow to @tool_* buffer. Use when the system \
         prompt points you here (e.g. \"see get_guide('librarian')\"). \
         Topics: librarian | tracker-conventions | progressive-disclosure | \
         error-handling. Call with no args to list topics + 1-line summaries."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "topic": {
                    "type": "string",
                    "description": "Topic to fetch. Omit to list available topics.",
                    "enum": ["librarian", "tracker-conventions",
                             "progressive-disclosure", "error-handling"]
                }
            },
            "additionalProperties": false
        })
    }

    async fn call(&self, input: Value, _ctx: &ToolContext) -> Result<Value> {
        let topic = input.get("topic").and_then(|v| v.as_str());
        match topic {
            None => Ok(json!({
                "topics": self.topics.keys().collect::<Vec<_>>(),
                "summaries": {
                    "librarian": "artifact model, filter syntax, trackers, augmentations",
                    "tracker-conventions": "frontmatter, archive flow, status vocabulary",
                    "progressive-disclosure": "MAX_INLINE_TOKENS, @ref buffer, overflow patterns",
                    "error-handling": "RecoverableError vs anyhow::bail, is_error routing"
                }
            })),
            Some(t) => match self.topics.get(t) {
                Some(body) => Ok(json!({ "topic": t, "body": *body })),
                None => {
                    let available = self.topics.keys().cloned().collect::<Vec<_>>().join(", ");
                    Err(RecoverableError::with_hint(
                        format!("unknown topic '{t}'"),
                        format!("available topics: {available}"),
                    ).into())
                }
            }
        }
    }
}
```

- [ ] **Step 2: Build**

Run: `cargo build --release 2>&1`
Expected: clean build. If `include_str!` complains about a missing file, the corresponding Task 2-5 was incomplete; finish it.

- [ ] **Step 3: Run tests**

Run: `cargo test --lib guide::tests`
Expected: all 3 tests pass.

If `get_guide_returns_librarian_body` fails on the `contains("artifact")` assertion, the content file is missing the word — adjust the assertion to a marker actually present in your librarian guide.

---

### Task 8: Register `GetGuide` in the server

**Files:**
- Modify: `src/server.rs` (the `from_parts` tools vec, around line 120)

- [ ] **Step 1: Add to the tools vec**

Use `edit_file`:

```rust
// old_string:
            // Library tools
            Arc::new(Library),
        ];

// new_string:
            // Library tools
            Arc::new(Library),
            // Deep-guidance tool — see docs/architecture/mcp-channel-caps.md
            Arc::new(crate::tools::guide::GetGuide::new()),
        ];
```

- [ ] **Step 2: Verify `get_guide` is listed**

Run a fresh stdio probe:

```bash
cat > /tmp/get_guide_probe.json <<'EOF'
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"probe","version":"0.0"}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/list"}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"get_guide","arguments":{"topic":"librarian"}}}
EOF
cargo build --release 2>&1
(cat /tmp/get_guide_probe.json; sleep 4) | target/release/codescout start 2>/dev/null > /tmp/get_guide_resp.json &
sleep 5; kill %1 2>/dev/null
python3 -c "
import json
for ln in open('/tmp/get_guide_resp.json'):
    j = json.loads(ln)
    if j.get('id') == 2:
        names = [t['name'] for t in j['result']['tools']]
        assert 'get_guide' in names, f'get_guide missing from {names}'
        print('get_guide in tool list ✓')
    if j.get('id') == 3:
        content = j['result']['content'][0]['text']
        # Result may be buffered if body is large
        print('call result first 200:', content[:200])
        print('call result length:', len(content))
"
```

Expected: `get_guide in tool list ✓` and a call result containing either the librarian body or a buffer envelope with `output_id`.

- [ ] **Step 3: Run the existing prompt-surfaces consistency test**

Run: `cargo test --lib prompt_surfaces`
Expected: pass. If `prompt_surfaces_reference_only_real_tools` fails because some surface still mentions `get_guide` as unknown, that's a false positive — add `get_guide` to the test's known-tools allowlist if needed.

---

### Task 9: Commit A

- [ ] **Step 1: Stage and commit**

```bash
git add src/prompts/guides/ src/tools/guide.rs src/tools/mod.rs src/server.rs
git status
git commit -m "feat: add get_guide(topic) tool for deep guidance content

Introduces an on-demand channel for content that exceeds Claude Code's
~2KB MCP instructions cap. Four topics ship initially: librarian,
tracker-conventions, progressive-disclosure, error-handling.

librarian::INSTRUCTIONS remains concatenated for now; Commit B severs it.

Spec: docs/superpowers/specs/2026-05-19-mcp-prompt-channel-redesign-design.md
Evidence: docs/architecture/mcp-channel-caps.md"
```

- [ ] **Step 2: Verify HEAD**

Run: `git log --oneline -1`
Expected: the new commit.

---

# Commit B — Surface A rewrite + sever librarian concat + Surface D tracker

### Task 10: Write failing size + structure tests for `source.md`

**Files:**
- Modify: `src/prompts/mod.rs` (test module)

- [ ] **Step 1: Add tests**

Locate the existing test module in `src/prompts/mod.rs`. Append:

```rust
#[cfg(test)]
mod redesign_invariants {
    use super::*;

    const MAX_INSTRUCTIONS_CHARS: usize = 1800;

    #[test]
    fn source_md_under_cap() {
        let rendered = build_server_instructions(None);
        assert!(
            rendered.len() <= MAX_INSTRUCTIONS_CHARS,
            "server instructions are {} chars; cap is {}. \
             Cut content or move it to get_guide.",
            rendered.len(),
            MAX_INSTRUCTIONS_CHARS,
        );
    }

    #[test]
    fn every_iron_law_has_do_instead() {
        let rendered = build_server_instructions(None);
        // Iron Laws section uses "NEVER X → Y" format. Each NEVER line must
        // have an arrow on the same line or within the next 2 lines.
        for (i, line) in rendered.lines().enumerate() {
            if line.contains("NEVER ") || line.starts_with(|c: char| c.is_ascii_digit())
                && line.contains("NEVER")
            {
                let next_two: String = rendered
                    .lines()
                    .skip(i)
                    .take(3)
                    .collect::<Vec<_>>()
                    .join(" ");
                assert!(
                    next_two.contains("→") || next_two.contains(" use ") || next_two.contains(" do "),
                    "Iron Law without do-instead clause: '{}'",
                    line
                );
            }
        }
    }

    #[test]
    fn server_instructions_mentions_get_guide() {
        let rendered = build_server_instructions(None);
        assert!(
            rendered.contains("get_guide"),
            "system prompt must mention get_guide for discoverability"
        );
    }

    #[test]
    fn server_instructions_does_not_concat_librarian() {
        // After Task 14 lands, the librarian block must not be appended.
        let rendered = build_server_instructions(None);
        assert!(
            !rendered.contains("artifact_event(action=\"create\")"),
            "librarian guide content should not be in instructions; \
             move it to get_guide(\"librarian\")"
        );
    }
}
```

- [ ] **Step 2: Run — expect 4 failures**

Run: `cargo test --lib prompts::redesign_invariants`
Expected: `source_md_under_cap` FAIL (current ~21 KB), `every_iron_law_has_do_instead` FAIL (current format), `server_instructions_mentions_get_guide` FAIL, `server_instructions_does_not_concat_librarian` FAIL.

---

### Task 11: Rewrite `source.md`

**Files:**
- Modify: `src/prompts/source.md` (full rewrite)

- [ ] **Step 1: Back up the current file**

```bash
cp src/prompts/source.md src/prompts/source.md.bak
```

- [ ] **Step 2: Replace with the new shape**

Use `create_file("src/prompts/source.md", overwrite=true, content="""
codescout MCP — semantic code intelligence.
Subagents inherit these rules. Pass them along.

## Iron Laws (never X, do Y)

1. NEVER read_file source code → symbols(path) for overview,
   symbols(name=..., include_body=true) for bodies.
2. NEVER edit_file structural code → edit_code (LSP-aware).
3. NEVER pipe unbounded run_command output → run bare, query
   the @cmd_* buffer (grep \"ERROR\" @cmd_abc). Bounded LHS
   (ls, cat, awk, sed, find -maxdepth N) is OK.
4. NEVER read_file markdown → read_markdown (heading-addressed).
5. NEVER edit_file markdown → edit_markdown (heading-addressed).

## Search/Edit decision quickref

- Know name → symbols(name=X) | symbol_at(path, line, col)
- Know concept → semantic_search(query)
- Exact string/regex → grep(pattern, path=optional)
- Who calls X → references(symbol, path) — NOT grep
- Structural code edit → edit_code | Text/import edit → edit_file

## Buffered tool results (@ref)

When a tool returns {output_id: \"@tool_xyz\", summary, hint}:
- Result was too big to inline. Stored in the buffer.
- Query it: grep PATTERN @tool_xyz | read_file(@ref, json_path=\"$.foo\")
  | read_file(@ref, start_line=N, end_line=M).
- Don't re-call the tool. Don't ask the user to paste content.

## Workspace gate

After workspace(activate, path=foreign), call workspace(activate, path=home)
before finishing the turn. Foreign-project state otherwise leaks.

## Deeper guidance

Call get_guide(topic) where topic in:
- \"librarian\"               — artifact model, filters, trackers
- \"tracker-conventions\"     — frontmatter, archive flow, status
- \"progressive-disclosure\"  — output budgets, @ref buffer details
- \"error-handling\"          — RecoverableError vs anyhow::bail
""")`

- [ ] **Step 3: Verify size**

Run: `wc -c src/prompts/source.md`
Expected: 1,200-1,800 bytes.

- [ ] **Step 4: Delete the backup once you're confident**

```bash
rm src/prompts/source.md.bak
```

---

### Task 12: Make `build_server_instructions` template substitution match

**Files:**
- Modify: `src/prompts/mod.rs` if there are template tokens in `source.md` that no longer apply

- [ ] **Step 1: Check for template tokens**

Run: `grep -E "\\{\\{|<symbol_navigation_block>|%%[A-Z_]+%%" src/prompts/source.md`
Expected: empty.

If non-empty, look up the substitution code in `src/prompts/mod.rs::build_server_instructions` and remove the now-stale substitution. Update the test `server_instructions_template_has_symbol_nav_token` accordingly — likely needs deletion, since the new prompt has no language-nav token. Remove or update the test.

- [ ] **Step 2: Run the affected test**

Run: `cargo test --lib prompts::tests::server_instructions_template_has_symbol_nav_token`
Expected: either passes (substitution still present) or you've deleted it deliberately.

---

### Task 13: Run the redesign invariant tests

- [ ] **Step 1: Run the four tests from Task 10**

Run: `cargo test --lib prompts::redesign_invariants`
Expected:
- `source_md_under_cap` PASS
- `every_iron_law_has_do_instead` PASS
- `server_instructions_mentions_get_guide` PASS
- `server_instructions_does_not_concat_librarian` — still FAIL (librarian is still concat'd; Task 14 fixes it)

If `source_md_under_cap` still fails, trim more.
If `every_iron_law_has_do_instead` still fails, the regex in the test may be too strict — review which line failed and either tighten `source.md` wording or relax the regex (preferred: tighten wording).

---

### Task 14: Sever the librarian concat and delete `INSTRUCTIONS`

**Files:**
- Modify: `src/server.rs` (delete lines 89-92 and 377-380 — the two concat sites)
- Modify: `src/librarian/server.rs` (delete the `pub const INSTRUCTIONS` at line 38)
- Modify: `src/librarian/mod.rs` (drop any `pub use` of `INSTRUCTIONS`)
- Delete: `src/librarian/prompts/server_instructions.md` (its `include_str!` source)

- [ ] **Step 1: Remove the concat at `from_parts` (line ~89)**

Use `edit_file`:

```rust
// old_string:
        let mut instructions = crate::prompts::build_server_instructions(status.as_ref());
        #[cfg(feature = "librarian")]
        if librarian_enabled_at_runtime(status.as_ref().map(|s| s.path.as_str())) {
            instructions.push_str("\n\n");
            instructions.push_str(crate::librarian::INSTRUCTIONS);
        }

// new_string:
        let instructions = crate::prompts::build_server_instructions(status.as_ref());
```

The `#[cfg_attr(not(feature = "librarian"), allow(unused_mut))]` directly above the original `let mut instructions = ...` is no longer needed; remove it too.

- [ ] **Step 2: Remove the concat at `refresh_instructions` (line ~377)**

Locate the same pattern in `CodeScoutServer::refresh_instructions` (`src/server.rs:361-371`). Same edit shape.

- [ ] **Step 3: Delete the const**

Use `edit_code(action="remove", symbol="INSTRUCTIONS", path="src/librarian/server.rs")`.

If `edit_code` can't find a `const` symbol, fall back to `edit_file` with the literal text:

```rust
// old_string:
pub const INSTRUCTIONS: &str = include_str!("prompts/server_instructions.md");

// new_string:
(empty)
```

- [ ] **Step 4: Delete the source markdown**

```bash
git rm src/librarian/prompts/server_instructions.md
```

- [ ] **Step 5: Drop the re-export if any**

Run: `grep "INSTRUCTIONS" src/librarian/mod.rs`
If any line references `INSTRUCTIONS`, remove it via `edit_file`.

- [ ] **Step 6: Build**

Run: `cargo build --release 2>&1`
Expected: clean. If a downstream test or doc-example references `crate::librarian::INSTRUCTIONS`, follow the compiler error to remove it.

---

### Task 15: Add a `librarian_instructions_removed` regression test

**Files:**
- Modify: `src/prompts/mod.rs::redesign_invariants`

- [ ] **Step 1: Append**

```rust
#[test]
fn librarian_instructions_const_removed() {
    // Sentinel test — fails to compile if the const is reintroduced.
    // Uncomment to verify:
    // let _ = crate::librarian::INSTRUCTIONS;
    //
    // Compile-time enforcement: any reintroduction must remove this test
    // or re-add the const. The presence of this no-op test acts as
    // documentation of intent.
}
```

(This is a documentation marker more than a functional test. The real enforcement is that the const is gone; any code referencing it won't compile.)

- [ ] **Step 2: Re-run the redesign invariant suite**

Run: `cargo test --lib prompts::redesign_invariants`
Expected: all 4 pass now, including `server_instructions_does_not_concat_librarian`.

---

### Task 16: Add the `every_tool_description_under_cap` test

**Files:**
- Modify: `src/server.rs` (test module)

- [ ] **Step 1: Find the existing tools-list test**

Run: `grep -n "fn prompt_surfaces_reference_only_real_tools" src/server.rs`

Add a sibling test next to it:

```rust
#[test]
fn every_tool_description_under_cap() {
    const CAP: usize = 1800;
    let server = test_server();  // existing helper
    let over: Vec<(String, usize)> = server
        .tools
        .iter()
        .map(|t| (t.name().to_string(), t.description().len()))
        .filter(|(_, n)| *n > CAP)
        .collect();
    assert!(
        over.is_empty(),
        "tool descriptions over the {CAP}-char cap: {:?}",
        over
    );
}
```

If `test_server()` doesn't exist as-is, look for the pattern other server tests use (e.g. `prompt_surfaces_reference_only_real_tools` near line 1671) and copy the construction.

- [ ] **Step 2: Run**

Run: `cargo test --lib server::tests::every_tool_description_under_cap`
Expected: pass (current largest description was 729 chars per the 2026-05-19 probe).

---

### Task 17: Create Surface D — `docs/trackers/get-guide-topics.md`

**Files:**
- Create: `docs/trackers/get-guide-topics.md`

- [ ] **Step 1: Write the tracker file**

`create_file("docs/trackers/get-guide-topics.md", content="""
---
kind: tracker
status: active
title: Get-guide candidate topics
owners: []
tags: [prompt-surfaces, guide, channel-caps]
---

# Get-guide candidate topics

Living list of topics for the `get_guide(topic)` tool. Live topics
ship with content. Candidates are watched but not promoted until
the promote-criterion fires (a friction or pattern observation
names a gap that the topic would close).

## Live topics

| Topic | Source file | Last updated |
|---|---|---|
| `librarian` | `src/prompts/guides/librarian.md` | 2026-05-19 |
| `tracker-conventions` | `src/prompts/guides/tracker-conventions.md` | 2026-05-19 |
| `progressive-disclosure` | `src/prompts/guides/progressive-disclosure.md` | 2026-05-19 |
| `error-handling` | `src/prompts/guides/error-handling.md` | 2026-05-19 |

## Candidate topics

| # | Candidate | Source | Promote-when |
|---|---|---|---|
| 1 | `anti-patterns` | extract from `docs/trackers/tool-usage-patterns.md` T-N entries | ≥3 distinct T-N cite a missing rule outside the 4 live topics |
| 2 | `run-command-budget` | `src/tools/run_command/*.rs` + Iron Law #3 extended | Friction shows the bounded-LHS clause was ignored |
| 3 | `symbol-navigation` | `src/prompts/language_nav.rs` (14 KB, exists) | Friction shows model failed at symbol-vs-references choice |
| 4 | `subagent-coordination` | new — handoff patterns, isolation modes | Any subagent-related F-N or T-N |
| 5 | `prompt-surface-consistency` | `CLAUDE.md` section of same name | (low priority — meta-rule for codescout devs, not model usage) |

## Declined topics

(none yet — append entries with `why declined` to avoid re-litigation)

## Promotion procedure

When a candidate's promote-criterion fires:

1. Write `src/prompts/guides/<topic>.md`.
2. Register the topic in `GetGuide::new()` at `src/tools/guide.rs`.
3. Update the `enum` in `GetGuide::input_schema`.
4. Update the topic list in `src/prompts/source.md` (Surface A) if budget allows.
5. Move the row from `## Candidate topics` to `## Live topics` in this file.
6. One commit per promotion.

## References

- Spec: `docs/superpowers/specs/2026-05-19-mcp-prompt-channel-redesign-design.md`
- Evidence: `docs/architecture/mcp-channel-caps.md`
""")`

- [ ] **Step 2: Trigger librarian indexing**

Use `librarian(action="reindex", scope="project")`.

- [ ] **Step 3: Verify discovery**

Use `artifact(action="find", kind="tracker", filter={"title": {"contains": "Get-guide"}})`.
Expected: one row, the new tracker.

---

### Task 18: Commit B

- [ ] **Step 1: Stage and commit**

```bash
git add -A src/prompts/source.md src/prompts/mod.rs src/server.rs src/librarian/server.rs src/librarian/mod.rs docs/trackers/get-guide-topics.md
git rm src/librarian/prompts/server_instructions.md
git status
```

Verify the diff is clean — only the intended files.

```bash
git commit -m "feat(prompts): shrink source.md to fit Claude Code's 2KB cap

- Rewrite src/prompts/source.md from ~21KB to ~1.6KB.
- Pair every Iron Law with a do-instead clause (Hamsa heuristic #1).
- Add search/edit quickref, @ref buffer pattern, workspace gate.
- Sever the runtime librarian::INSTRUCTIONS concat at from_parts and
  refresh_instructions — that content lives in get_guide(\"librarian\").
- Delete librarian::INSTRUCTIONS const and its source markdown.
- Add docs/trackers/get-guide-topics.md for future topic candidates.

Tests: source_md_under_cap, every_iron_law_has_do_instead,
server_instructions_mentions_get_guide,
server_instructions_does_not_concat_librarian,
every_tool_description_under_cap.

Spec: docs/superpowers/specs/2026-05-19-mcp-prompt-channel-redesign-design.md
Evidence: docs/architecture/mcp-channel-caps.md"
```

- [ ] **Step 2: Re-run full test suite**

Run: `cargo test --lib`
Expected: all pass. Investigate any failure before moving on; many tests touch `server_instructions.md` content (e.g. `prompt_surfaces_reference_only_real_tools` may need an allowlist update for tokens no longer present).

---

# Commit C — Surface C first-call hint

### Task 19: Add `relevant_guide_topic` trait method

**Files:**
- Modify: `src/tools/core/types.rs` (the `Tool` trait, around line 331)

- [ ] **Step 1: Append a default-`None` method on `Tool`**

Use `edit_file`. Locate the `Tool` trait body (`src/tools/core/types.rs:331-470`) and add the method just before the closing `}` of the trait:

```rust
    /// Topic name this tool's discipline depends on, for the first-call
    /// hint mechanism. When the model first calls a tool with
    /// `Some("librarian")`, `Tool::call_content` injects a one-shot
    /// `_guide_hint` pointing to `get_guide("librarian")`. Topic dedup
    /// is session-wide; calling a different tool with the same topic in
    /// the same session does NOT re-emit the hint. The set is cleared
    /// on workspace(action="activate").
    ///
    /// Default `None` — most tools' rules fit in their `description`
    /// and don't need the hint.
    fn relevant_guide_topic(&self) -> Option<&str> { None }
```

- [ ] **Step 2: Build**

Run: `cargo build --release 2>&1`
Expected: clean — default impl means no downstream change required.

---

### Task 20: Extend `ToolContext` with `guide_hints_emitted`

**Files:**
- Modify: `src/tools/core/types.rs` (`ToolContext` struct around line 58)
- Modify: every site that constructs a `ToolContext`

- [ ] **Step 1: Add the field**

```rust
// Add to ToolContext struct (around line 58):
    /// Session-scoped set of guide topics already hinted to the model.
    /// Reset on workspace(action="activate").
    pub guide_hints_emitted: Arc<parking_lot::Mutex<std::collections::HashSet<String>>>,
```

- [ ] **Step 2: Find every ToolContext construction site**

Run: `grep -rn "ToolContext {" src/ tests/ 2>/dev/null`

For each, add a new field initializer:

```rust
    guide_hints_emitted: Arc::new(parking_lot::Mutex::new(Default::default())),
```

Tip: the production site is in `src/server.rs::dispatch_tool_call` (or equivalent). Tests construct `ToolContext` ad hoc; each needs the field too.

- [ ] **Step 3: Add a shared field on `CodeScoutServer`**

Modify `src/server.rs` (around line 67 where `resources` field lives):

```rust
    guide_hints_emitted: Arc<parking_lot::Mutex<std::collections::HashSet<String>>>,
```

Initialize in `from_parts`:

```rust
guide_hints_emitted: Arc::new(parking_lot::Mutex::new(Default::default())),
```

Pass through at every tool dispatch site:

```rust
let ctx = ToolContext {
    /* existing fields */,
    guide_hints_emitted: Arc::clone(&self.guide_hints_emitted),
};
```

- [ ] **Step 4: Build**

Run: `cargo build --release 2>&1`
Expected: clean.

---

### Task 21: Override `relevant_guide_topic` on librarian adapters

**Files:**
- Modify: `src/librarian/adapter.rs`

- [ ] **Step 1: Inspect the adapter shape**

Use `symbols(path="src/librarian/adapter.rs")` to find the `impl Tool for ...` blocks.

- [ ] **Step 2: Add the override on each librarian adapter**

For each `impl Tool for <Adapter>` block, add:

```rust
    fn relevant_guide_topic(&self) -> Option<&str> { Some("librarian") }
```

If all adapters share a common newtype + macro, add it once on the macro expansion.

- [ ] **Step 3: Build**

Run: `cargo build --release 2>&1`
Expected: clean.

---

### Task 22: Override `relevant_guide_topic` on `RunCommand`

**Files:**
- Modify: `src/tools/run_command/run_command.rs`

- [ ] **Step 1: Find the existing `impl Tool for RunCommand` block**

Use `symbols(name="impl Tool for RunCommand", path="src/tools/run_command")` or `symbols(path="src/tools/run_command/run_command.rs")`.

- [ ] **Step 2: Add the override**

```rust
    fn relevant_guide_topic(&self) -> Option<&str> { Some("progressive-disclosure") }
```

- [ ] **Step 3: Same for `Symbols` if its result can overflow**

Modify `src/tools/symbol/symbols.rs`:

```rust
    fn relevant_guide_topic(&self) -> Option<&str> { Some("progressive-disclosure") }
```

- [ ] **Step 4: Build**

Run: `cargo build --release 2>&1`
Expected: clean.

---

### Task 23: Write failing tests for hint emission

**Files:**
- Modify: `src/server.rs` (test module)

- [ ] **Step 1: Append tests**

```rust
#[cfg(test)]
mod guide_hint_tests {
    use super::*;
    use serde_json::Value;

    // Helper: extract `_guide_hint` from the first content block of a tool
    // result, if present.
    fn extract_hint(result: &[rmcp::model::Content]) -> Option<String> {
        let text = result.first()?.as_text()?.text.clone();
        let v: Value = serde_json::from_str(&text).ok()?;
        v.get("_guide_hint").and_then(|h| h.as_str()).map(String::from)
    }

    #[tokio::test]
    async fn first_artifact_call_emits_librarian_hint() {
        let srv = test_server_with_active_project().await;
        let result = srv.call_tool("artifact", json!({"action": "find", "kind": "tracker"})).await;
        assert!(extract_hint(&result).unwrap().contains("librarian"));
    }

    #[tokio::test]
    async fn second_artifact_call_no_hint() {
        let srv = test_server_with_active_project().await;
        let _ = srv.call_tool("artifact", json!({"action": "find", "kind": "tracker"})).await;
        let result = srv.call_tool("artifact", json!({"action": "find", "kind": "tracker"})).await;
        assert!(extract_hint(&result).is_none());
    }

    #[tokio::test]
    async fn artifact_event_after_artifact_no_hint() {
        let srv = test_server_with_active_project().await;
        let _ = srv.call_tool("artifact", json!({"action": "find", "kind": "tracker"})).await;
        let result = srv.call_tool("artifact_event", json!({"action": "list", "artifact_id": "nonexistent"})).await;
        assert!(extract_hint(&result).is_none());
    }

    #[tokio::test]
    async fn activate_project_resets_hints() {
        let srv = test_server_with_active_project().await;
        let _ = srv.call_tool("artifact", json!({"action": "find", "kind": "tracker"})).await;
        let _ = srv.call_tool("workspace", json!({"action": "activate", "path": std::env::current_dir().unwrap().to_str().unwrap()})).await;
        let result = srv.call_tool("artifact", json!({"action": "find", "kind": "tracker"})).await;
        assert!(extract_hint(&result).unwrap().contains("librarian"));
    }

    #[tokio::test]
    async fn run_command_without_overflow_no_progressive_hint() {
        let srv = test_server_with_active_project().await;
        let result = srv.call_tool("run_command", json!({"command": "echo small"})).await;
        assert!(extract_hint(&result).is_none());
    }

    #[tokio::test]
    async fn run_command_with_overflow_emits_progressive_hint_once() {
        let srv = test_server_with_active_project().await;
        // Generate >10KB of output reliably.
        let big = srv.call_tool("run_command", json!({"command": "yes filler | head -2000"})).await;
        assert!(extract_hint(&big).unwrap().contains("progressive-disclosure"));
        let second = srv.call_tool("run_command", json!({"command": "yes filler | head -2000"})).await;
        assert!(extract_hint(&second).is_none());
    }
}
```

If `test_server_with_active_project` doesn't exist, locate the closest existing helper (`test_server`, `make_test_server`, etc.) and wrap it with project activation. If `srv.call_tool` doesn't exist, build it from the dispatch surface available in the existing tests.

- [ ] **Step 2: Run — expect all 6 to fail**

Run: `cargo test --lib server::guide_hint_tests`
Expected: all 6 FAIL (hint not yet injected).

---

### Task 24: Inject `_guide_hint` in `Tool::call_content`

**Files:**
- Modify: `src/tools/core/types.rs::Tool::call_content` (the default impl around line 422)

- [ ] **Step 1: Modify `call_content`**

Locate the default impl. Add the hint-injection logic immediately after `let val = self.call(input, ctx).await?;`:

```rust
async fn call_content(&self, input: Value, ctx: &ToolContext) -> Result<Vec<Content>> {
    let val = self.call(input, ctx).await?;
    let form = self.output_form();
    let json = serde_json::to_string(&val).unwrap_or_else(|_| val.to_string());

    // Compute potential hint topic + whether it should fire on this call.
    let hint_topic: Option<String> = if let Some(topic) = self.relevant_guide_topic() {
        let mut emitted = ctx.guide_hints_emitted.lock();
        if emitted.contains(topic) {
            None
        } else {
            let should = match topic {
                "progressive-disclosure" => exceeds_inline_limit(&json),
                _ => true,
            };
            if should {
                emitted.insert(topic.to_string());
                Some(topic.to_string())
            } else {
                None
            }
        }
    } else {
        None
    };

    fn inject_hint(val: &mut Value, topic: &str) {
        if let Some(obj) = val.as_object_mut() {
            obj.insert(
                "_guide_hint".to_string(),
                Value::String(format!(
                    "First call this session for topic '{topic}'. \
                     Run get_guide(\"{topic}\") for full guidance."
                )),
            );
        }
    }

    if exceeds_inline_limit(&json) {
        let json_len = json.len();
        let ref_id = ctx.output_buffer.store_tool(self.name(), json);
        let raw_summary = self
            .format_compact(&val)
            .unwrap_or_else(|| format!("Result stored in {} ({} bytes)", ref_id, json_len));
        let summary = truncate_compact(
            &raw_summary,
            COMPACT_SUMMARY_MAX_BYTES,
            COMPACT_SUMMARY_HARD_MAX_BYTES,
        );

        let jp = self.json_path_hint(&val);
        let hint = format!(
            "read_file(\"{ref_id}\", json_path=\"{jp}\") to extract a specific field, \
             or read_file(\"{ref_id}\", start_line=N, end_line=M) to browse sections"
        );
        let mut buffered = serde_json::json!({
            "output_id": ref_id,
            "summary": summary,
            "hint": hint,
        });
        if let Some(topic) = &hint_topic {
            inject_hint(&mut buffered, topic);
        }
        return Ok(vec![Content::text(
            serde_json::to_string_pretty(&buffered)
                .unwrap_or_else(|_| format!("{{\"output_id\":\"{ref_id}\"}}")),
        )]);
    }

    // Small output path.
    let mut val = val;
    if let Some(topic) = &hint_topic {
        inject_hint(&mut val, topic);
    }
    if form == OutputForm::Text {
        if let Some(text) = self.format_compact(&val) {
            return Ok(vec![Content::text(text)]);
        }
    }
    Ok(vec![Content::text(
        serde_json::to_string_pretty(&val).unwrap_or_else(|_| val.to_string()),
    )])
}
```

- [ ] **Step 2: Build**

Run: `cargo build --release 2>&1`
Expected: clean.

- [ ] **Step 3: Run the hint tests**

Run: `cargo test --lib server::guide_hint_tests`
Expected: 5 of 6 pass. The activate-reset test will fail until Task 25.

---

### Task 25: Reset `guide_hints_emitted` on workspace activate

**Files:**
- Modify: `src/tools/config/mod.rs` (the `ActivateProject::call` method, around line 114)

- [ ] **Step 1: Plumb the set through to the activate code path**

The activate handler runs inside `Tool::call`, which receives `&ToolContext`. The reset is one line at the top of the activate logic:

```rust
ctx.guide_hints_emitted.lock().clear();
```

Add it as the first statement of the activate branch (after argument parsing succeeds, before the project switch).

- [ ] **Step 2: Run the test**

Run: `cargo test --lib server::guide_hint_tests::activate_project_resets_hints`
Expected: PASS.

- [ ] **Step 3: Run the full hint suite**

Run: `cargo test --lib server::guide_hint_tests`
Expected: all 6 PASS.

---

### Task 26: Update Surface D tracker with hint-mechanism state

**Files:**
- Modify: `docs/trackers/get-guide-topics.md`

- [ ] **Step 1: Add a Mechanism note section**

Use `edit_markdown(path="docs/trackers/get-guide-topics.md", action="insert_after", heading="## Live topics", content="""
## Mechanism

The first-call hint is emitted by `Tool::call_content` when a tool whose
`relevant_guide_topic()` returns `Some(t)` is invoked for the first
time in the session AND topic `t` has not been hinted yet. The set is
cleared on `workspace(action="activate")`.

Tools currently tagged:

| Tool | Topic | Fires when |
|---|---|---|
| All `librarian::*` adapters | `librarian` | unconditional first call |
| `run_command` | `progressive-disclosure` | first call whose output overflows |
| `symbols` | `progressive-disclosure` | first call whose output overflows |

To tag a new tool, override `Tool::relevant_guide_topic` returning
`Some("<topic>")`.
""")`

- [ ] **Step 2: Re-trigger librarian reindex**

Use `librarian(action="reindex", scope="project")`.

---

### Task 27: Verify end-to-end via live MCP

**Files:** none (manual smoke test)

- [ ] **Step 1: Restart Claude Code's MCP connection**

Tell the user: "Run `/mcp` to reconnect codescout. Then invoke any
librarian-touching tool (e.g. `artifact(action='find', kind='tracker')`)
in a fresh session."

- [ ] **Step 2: Expected observation**

The first artifact call result includes a `_guide_hint` pointing to
`get_guide("librarian")`. The second does not.

If the live test fails, examine the actual response and reconcile
with the test mocks.

---

### Task 28: Run full test suite

- [ ] **Step 1: Run everything**

Run: `cargo test --lib`
Expected: all pass.

Run: `cargo clippy --release -- -D warnings`
Expected: clean.

Run: `cargo fmt --check`
Expected: clean.

---

### Task 29: Commit C

- [ ] **Step 1: Stage**

```bash
git add -A src/tools/core/types.rs src/server.rs src/librarian/adapter.rs src/tools/run_command/ src/tools/symbol/ src/tools/config/mod.rs docs/trackers/get-guide-topics.md
git status
```

- [ ] **Step 2: Commit**

```bash
git commit -m "feat(prompts): first-call hint for get_guide topics

When a tool whose relevant_guide_topic() returns Some(t) is called
for the first time in the session, Tool::call_content injects a
'_guide_hint' field pointing at get_guide(t). Topic dedup is
session-wide and cleared on workspace(activate). Currently tagged:
all librarian adapters (unconditional), run_command and symbols
(only on buffer overflow).

Tests: guide_hint_tests (6 cases).

Spec: docs/superpowers/specs/2026-05-19-mcp-prompt-channel-redesign-design.md"
```

---

# Cleanup commit

### Task 30: Revert debug flags

**Files:**
- Modify: `/home/marius/agents/llm-proxy/.env`
- Modify: `~/.claude-kat/.claude.json`

- [ ] **Step 1: Remove LOG_FULL_TOOLS / LOG_TOOL_DIGEST from the proxy .env**

```bash
sed -i '/^LOG_FULL_TOOLS=/d; /^LOG_TOOL_DIGEST=/d' /home/marius/agents/llm-proxy/.env
# remove the comment block that introduced them too
```

Verify:

```bash
grep -E "^LOG_" /home/marius/agents/llm-proxy/.env || echo "no LOG_* env left"
```

- [ ] **Step 2: Restart the proxy**

```bash
systemctl --user restart llm-proxy.service
sleep 2
systemctl --user is-active llm-proxy.service
```

- [ ] **Step 3: Remove CODESCOUT_PROBE from `.claude.json`**

```bash
python3 -c "
import json
p = '/home/marius/.claude-kat/.claude.json'
d = json.load(open(p))
env = d['mcpServers']['codescout'].get('env', {})
env.pop('CODESCOUT_PROBE', None)
with open(p, 'w') as f: json.dump(d, f, indent=2)
print('CODESCOUT_PROBE removed:', 'CODESCOUT_PROBE' not in env)
"
```

- [ ] **Step 4: Tell the user to `/mcp` reconnect**

The probe tool and probe resources disappear from the tool list.

- [ ] **Step 5: There is no source-code commit for this task**

These are local debug toggles, not tracked files. The proxy passthrough.rs changes (`tools_full`, `tools_digest`) remain in the proxy repo — they're useful infrastructure. Document in the proxy repo's own changelog if needed; out of scope here.

---

# Post-merge

After all three commits land on `experiments`:

- [ ] Cherry-pick A, B, C to master in order.
- [ ] After cherry-pick, run the experiments rebase per `CLAUDE.md`'s "Standard Ship Sequence".
- [ ] Update `docs/architecture/mcp-channel-caps.md` frontmatter `status: draft → adopted`.

---

## Self-review (against spec sections)

| Spec section | Plan coverage |
|---|---|
| Surface A — source.md ≤1800 chars | Task 11 (rewrite), 10/12/13 (tests + build check) |
| Surface B — get_guide tool | Tasks 6, 7, 8 (tests + impl + registration) |
| Surface B — 4 content files | Tasks 2, 3, 4, 5 |
| Surface C — relevant_guide_topic trait | Task 19 |
| Surface C — session state | Task 20 |
| Surface C — per-tool overrides | Tasks 21, 22 |
| Surface C — hint injection | Task 24 |
| Surface C — activate reset | Task 25 |
| Surface D — tracker file | Task 17 |
| Migration commit sequence | Tasks 9, 18, 29 |
| Librarian concat removed | Task 14 |
| `librarian::INSTRUCTIONS` deleted | Task 14 |
| Build-time `source.md` cap | Task 10 (`source_md_under_cap` test) |
| Per-tool description cap test | Task 16 |
| Hint tests (6 cases) | Tasks 23, 24, 25 |
| Cleanup debug flags | Task 30 |
| Post-merge ADR status flip | Post-merge checklist |
