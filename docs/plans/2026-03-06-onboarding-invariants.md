# Onboarding Memory Invariants Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add Invariants + Strong Defaults sections to the architecture memory template in `onboarding_prompt.md` and backfill them into codescout's own architecture memory.

**Architecture:** Two file changes only — no new Rust code. The onboarding prompt template gets new section stubs + instructions; the live architecture memory gets the actual content for codescout.

**Tech Stack:** Markdown, codescout `memory` tool (write action)

---

### Task 1: Update `onboarding_prompt.md` — architecture template

**Files:**
- Modify: `src/prompts/onboarding_prompt.md` (architecture section, lines 75–102)

**Step 1: Read the current architecture section to confirm line range**

```
mcp__codescout__read_file(path="src/prompts/onboarding_prompt.md", heading="### 2. `architecture`")
```

**Step 2: Replace the template block**

Find:
```
## Design Patterns
[Only patterns actually in use: DI, repository, event-driven, etc.]
```

Replace with:
```
## Design Patterns
[Only patterns actually in use: DI, repository, event-driven, etc.]

## Invariants
[Hard rules — for each candidate ask: "what *concretely* breaks if this is ignored?"]
[If the failure mode is vague, it belongs in Strong Defaults, not here]
[Keep to ~5 entries max — if everything is an invariant, nothing is]

| Rule | Why it exists |
|---|---|
| [rule] | [specific failure if broken] |

## Strong Defaults
[Preferred behaviors that CAN be overridden with deliberate reason]

| Default | When it's okay to break it |
|---|---|
| [default behavior] | [specific condition that justifies breaking it] |
```

Use `mcp__codescout__edit_file` with `old_string` / `new_string`.

**Step 3: Update the anti-patterns instruction block for the architecture section**

Find:
```
**Anti-patterns:** Don't repeat what CLAUDE.md's "Project Structure" or "Key Patterns" sections already say
```

Append to the end of the anti-patterns paragraph:
```
For Invariants: don't list every rule from CLAUDE.md — only the ones an agent would realistically
violate. If there's no specific observable failure mode, move it to Strong Defaults.
For Strong Defaults: always include the override condition — a default with no escape hatch is
just an invariant written poorly.
```

**Step 4: Verify the file looks right**

```
mcp__codescout__read_file(path="src/prompts/onboarding_prompt.md", heading="### 2. `architecture`")
```

Confirm both new sections appear in the template block.

**Step 5: Commit**

```bash
git add src/prompts/onboarding_prompt.md
git commit -m "docs: add Invariants + Strong Defaults sections to architecture memory template"
```

---

### Task 2: Backfill codescout's architecture memory

**Files:**
- Modify: codescout architecture memory (via `memory` tool, topic="architecture")

**Step 1: Read current architecture memory**

```
mcp__codescout__memory(action="read", topic="architecture")
```

Confirm it ends after the Embedding Flow section.

**Step 2: Write updated architecture memory**

Call `mcp__codescout__memory(action="write", topic="architecture", content=<full updated content>)`.

The full content = existing content + the following appended:

```markdown
## Invariants

Rules that must never be broken. Each has a specific, observable failure mode.

| Rule | Why it exists |
|---|---|
| `OutputGuard` is the only output limiter (`src/tools/output.rs`) | Per-tool limits create inconsistency; the guard enforces both modes globally |
| Mutation tools return `json!("ok")`, never echo content back | Caller already has what they sent — echoing wastes tokens with zero information gain |
| `RecoverableError` for user-fixable failures; `anyhow::bail!` for real failures | Controls MCP `isError` flag — `bail!` aborts sibling parallel tool calls, `RecoverableError` does not |
| All 3 prompt surfaces updated together on tool changes | `server_instructions.md`, `onboarding_prompt.md`, `build_system_prompt_draft()` in `workflow.rs` — silent staleness corrupts agent guidance |
| New tools must be registered in `CodeScoutServer::new()` | Tools matched by name string in a Vec — unregistered tools silently never run |

## Strong Defaults

Preferred behaviors that can be overridden with deliberate reason.

| Default | When it's okay to break it |
|---|---|
| Exploring mode (compact output) by default | Only after identifying specific targets via overflow hints |
| Lazy LSP startup — servers start on first use | Only when diagnostics are needed before the first file edit |
| `RecoverableError` always includes a `hint` | Only when no corrective action exists for the user |
| Tools live in their category file (`file.rs`, `symbol.rs`, etc.) | Only when a tool genuinely spans multiple categories |
```

**Step 3: Verify**

```
mcp__codescout__memory(action="read", topic="architecture")
```

Confirm the two new sections appear at the end.

**Step 4: Run tests to confirm nothing broke**

```bash
cargo test 2>&1 | grep -E "test result|FAILED"
```

Expected: all passing, 0 failed.

**Step 5: Commit**

```bash
git commit -m "docs: backfill Invariants + Strong Defaults into codescout architecture memory"
```

Note: memory files live in `.codescout/memories/` — verify the file was updated on disk before committing.

```bash
git add .codescout/memories/architecture.md
git commit -m "docs: backfill Invariants + Strong Defaults into codescout architecture memory"
```

---

### Task 3: Final verification

**Step 1: Read both surfaces end-to-end**

```
mcp__codescout__read_file(path="src/prompts/onboarding_prompt.md", heading="### 2. `architecture`")
mcp__codescout__memory(action="read", topic="architecture")
```

Confirm:
- Template has both section stubs with instructions
- Live memory has both sections filled with codescout-specific content
- Anti-patterns block has the new invariant guidance

**Step 2: Check no stale references**

```bash
grep -r "removed\|~~" src/prompts/
```

Expected: no results (we cleaned these in a prior session).
