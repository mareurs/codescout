# Open Source Release Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Prepare code-explorer for public release on GitHub as `mareurs/code-explorer`

**Architecture:** Four layers shipped as separate commits: foundation (README, LICENSE, cleanup), CI (GitHub Actions), contributor experience (CONTRIBUTING, templates), docs polish (update stale docs). Each layer is independently useful.

**Tech Stack:** Markdown, YAML (GitHub Actions), Bash (CI), Rust (MSRV verification)

---

### Task 1: Delete serena-as-reference and clean .gitignore

**Files:**
- Delete: `serena-as-reference/` (entire directory, 539MB)
- Modify: `.gitignore`

**Step 1: Remove serena-as-reference directory**

Run: `rm -rf serena-as-reference/`

**Step 2: Update .gitignore**

Current `.gitignore`:
```
/target
/serena-as-reference
/.mcp.json
/.code-explorer/
/CHANGELOG.md
/docs/research/
```

Replace with:
```
/target
/.mcp.json
/.code-explorer/
/CHANGELOG.md
/docs/research/
/docs/observations.md
```

Changes: removed `/serena-as-reference` line (directory gone), added `/docs/observations.md` (private).

**Step 3: Remove serena references from CLAUDE.md and ARCHITECTURE.md**

In `CLAUDE.md`, remove the "Reference" section at the bottom that mentions `serena-as-reference/`.
Update the tagline on line 3 to remove the `[Serena](./serena-as-reference/)` link — keep the inspiration mention but link to the actual Serena GitHub repo instead: `[Serena](https://github.com/oraios/serena)`.

In `docs/ARCHITECTURE.md`, remove lines 133-136 (the "Reference Projects" section that mentions `serena-as-reference/`).

**Step 4: Verify build still works**

Run: `cargo build 2>&1 | tail -3`
Expected: `Finished` with no errors

**Step 5: Commit**

```bash
git add -A
git commit -m "chore: remove serena-as-reference, clean gitignore for OS release

Remove the 539MB reference directory (no longer needed, all patterns
extracted). Update gitignore: remove stale entry, add docs/observations.md
to private exclusions. Update references in CLAUDE.md and ARCHITECTURE.md."
```

---

### Task 2: Create LICENSE file

**Files:**
- Create: `LICENSE`

**Step 1: Create MIT LICENSE**

```
MIT License

Copyright (c) 2026 Marius Reimer

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

**Step 2: Add rust-version to Cargo.toml**

Add after the `license = "MIT"` line:
```toml
rust-version = "1.75"
```

This is the MSRV — edition 2021 with async trait support. Will be validated by CI.

**Step 3: Commit**

```bash
git add LICENSE Cargo.toml
git commit -m "chore: add MIT LICENSE file and set rust-version in Cargo.toml"
```

---

### Task 3: Create README.md

**Files:**
- Create: `README.md`

**Step 1: Write README.md**

Structure (problem-centric, as designed):

```markdown
# code-explorer

Rust MCP server giving LLMs IDE-grade code intelligence.

## The Problem

LLMs waste most of their context window on code navigation. `grep` returns
walls of text. `cat` dumps entire files when you need one function. There's no
way to ask "who calls this?" or "what changed here last?" — the tools are
blind to code structure.

The result: shallow understanding, hallucinated edits, constant human
course-correction.

## The Solution

code-explorer is an MCP server that gives your AI coding agent the same
navigation tools a human developer uses in an IDE — but optimized for token
efficiency.

**Four pillars:**

| Pillar | What it does | Tools |
|---|---|---|
| LSP Navigation | Go-to-definition, find references, rename — via real language servers | 7 tools, 9 languages |
| Semantic Search | Find code by concept, not just text match — via embeddings | 3 tools |
| Git Integration | Blame, history, diffs — context no other tool provides | 3 tools |
| Persistent Memory | Remember project knowledge across sessions | 4 tools |

Plus file operations (6 tools), AST analysis (2 tools), workflow (3 tools),
and config (2 tools) — **31 tools total**.

## Quick Start

### Install

​```bash
cargo install code-explorer
​```

### Configure with Claude Code

​```bash
claude mcp add code-explorer -- code-explorer start --project /path/to/your/project
​```

### Verify

​```bash
claude mcp list
# Should show: code-explorer with 31 tools
​```

## Tools

### Symbol Navigation (LSP)

| Tool | Purpose |
|---|---|
| `find_symbol` | Find symbols by name (supports glob patterns) |
| `get_symbols_overview` | Symbol tree for a file, directory, or glob |
| `find_referencing_symbols` | Find all callers/usages across the codebase |
| `replace_symbol_body` | Replace a function/method body by name |
| `insert_before_symbol` | Insert code before a symbol |
| `insert_after_symbol` | Insert code after a symbol |
| `rename_symbol` | Rename across the codebase (LSP-powered) |

### File Operations

| Tool | Purpose |
|---|---|
| `read_file` | Read file content (with optional line ranges) |
| `list_dir` | Directory listing (shallow by default) |
| `search_for_pattern` | Regex search across project files |
| `find_file` | Find files by glob pattern |
| `create_text_file` | Create or overwrite a file |
| `replace_content` | Find-and-replace text in a file |

### Git

| Tool | Purpose |
|---|---|
| `git_blame` | Line-by-line authorship with commit info |
| `git_log` | Commit history (filterable by path) |
| `git_diff` | Uncommitted changes or diff against a commit |

### Semantic Search

| Tool | Purpose |
|---|---|
| `semantic_search` | Find code by natural language description |
| `index_project` | Build/rebuild the embedding index |
| `index_status` | Check index health and statistics |

### AST Analysis (tree-sitter)

| Tool | Purpose |
|---|---|
| `list_functions` | Quick function signatures (offline, instant) |
| `extract_docstrings` | Extract doc comments (offline, instant) |

### Memory

| Tool | Purpose |
|---|---|
| `write_memory` | Store project knowledge |
| `read_memory` | Retrieve stored knowledge |
| `list_memories` | List all memory topics |
| `delete_memory` | Remove a memory topic |

### Workflow

| Tool | Purpose |
|---|---|
| `onboarding` | First-time project discovery |
| `check_onboarding_performed` | Check if onboarding is done |
| `execute_shell_command` | Run shell commands in project root |

### Config

| Tool | Purpose |
|---|---|
| `activate_project` | Switch active project |
| `get_current_config` | Show project configuration |

## How It Works

### Progressive Disclosure

Every tool defaults to compact output — names and locations, not full source
code. Request details with `detail_level: "full"` only when you need them.
This keeps the context window useful throughout long sessions.

- **Exploring mode** (default): compact summaries, capped at 200 items
- **Focused mode**: full detail with pagination via `offset`/`limit`
- **Overflow hints**: "showing 47 of 312 — narrow with a file path" instead
  of silent truncation

### Architecture

​```
MCP Layer (rmcp) → Tool trait → dispatch
    ↓
Agent (project state, config, memory)
    ↓
┌──────────┬──────────┬──────────┬──────────┐
│ LSP      │ AST      │ Git      │ Embedding│
│ (9 langs)│ (t-sitter)│ (git2)  │ (SQLite) │
└──────────┴──────────┴──────────┴──────────┘
​```

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for details.

## Configuration

code-explorer stores per-project config in `.code-explorer/project.toml`:

​```toml
[embeddings]
model = "ollama:nomic-embed-text"   # or "openai:text-embedding-3-small"
chunk_size = 1500
chunk_overlap = 200
​```

### Embedding backends

- **Remote** (default feature): Any OpenAI-compatible API — Ollama, OpenAI,
  custom endpoints
- **Local**: CPU-based via fastembed-rs — `cargo install code-explorer --features local-embed`

## Supported Languages

### LSP (full navigation)
Rust, Python, TypeScript/JavaScript, Go, Java, Kotlin, C/C++, C#, Ruby

### Tree-sitter (AST analysis)
Rust, Python, TypeScript, Go, Java, Kotlin

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for how to get started. PRs from Claude
Code are welcome!

## License

[MIT](LICENSE)
```

**Step 2: Commit**

```bash
git add README.md
git commit -m "docs: add README with problem-centric framing and tool reference"
```

---

### Task 4: Create GitHub Actions CI workflow

**Files:**
- Create: `.github/workflows/ci.yml`

**Step 1: Create directory**

Run: `mkdir -p .github/workflows`

**Step 2: Write ci.yml**

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:

env:
  CARGO_TERM_COLOR: always

jobs:
  fmt:
    name: Format
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt
      - run: cargo fmt --check

  clippy:
    name: Clippy
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy
      - uses: Swatinem/rust-cache@v2
      - run: cargo clippy -- -D warnings

  test:
    name: Test (${{ matrix.name }})
    runs-on: ubuntu-latest
    strategy:
      matrix:
        include:
          - name: default
            flags: ""
          - name: local-embed
            flags: "--features local-embed --no-default-features"
          - name: no-features
            flags: "--no-default-features"
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
        with:
          key: ${{ matrix.name }}
      - run: cargo test ${{ matrix.flags }}

  msrv:
    name: MSRV (1.75)
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@master
        with:
          toolchain: "1.75"
      - uses: Swatinem/rust-cache@v2
        with:
          key: msrv
      - run: cargo check
```

**Step 3: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: add GitHub Actions workflow with feature matrix and MSRV check"
```

---

### Task 5: Create CONTRIBUTING.md

**Files:**
- Create: `CONTRIBUTING.md`

**Step 1: Write CONTRIBUTING.md**

```markdown
# Contributing to code-explorer

We welcome contributions! Whether it's a bug fix, new language support, or
documentation improvement — we're happy to review it.

## Getting Started

​```bash
git clone https://github.com/mareurs/code-explorer.git
cd code-explorer
cargo build
cargo test
​```

## Before Submitting a PR

Run the same checks CI will run:

​```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
​```

## What to Contribute

**Good first contributions:**
- Add a tree-sitter grammar for a new language (see `src/ast/`)
- Add an LSP server config for a new language (see `src/lsp/servers/`)
- Fix a bug
- Improve documentation

**Please open an issue first for:**
- Large architectural changes
- New tool categories
- Changes to the progressive disclosure design

## Using Claude Code?

PRs generated with Claude Code are welcome. Just mention it in the PR
description. If you're using code-explorer itself as an MCP server while
contributing to code-explorer — that's the dream. Let us know how it went.

## Project Structure

See [CLAUDE.md](CLAUDE.md) for the full developer guide, including project
structure, design principles, and key patterns. That file is also what Claude
Code reads when working on this project.
​```
```

**Step 2: Commit**

```bash
git add CONTRIBUTING.md
git commit -m "docs: add CONTRIBUTING.md with low-barrier contributor guide"
```

---

### Task 6: Create PR and issue templates

**Files:**
- Create: `.github/pull_request_template.md`
- Create: `.github/ISSUE_TEMPLATE/bug_report.md`
- Create: `.github/ISSUE_TEMPLATE/feature_request.md`

**Step 1: Create PR template**

`.github/pull_request_template.md`:
```markdown
## What

<!-- Brief description of the change -->

## Why

<!-- Motivation, issue link, or context -->

## Testing

<!-- What you tested and how -->
```

**Step 2: Create issue template directory**

Run: `mkdir -p .github/ISSUE_TEMPLATE`

**Step 3: Create bug report template**

`.github/ISSUE_TEMPLATE/bug_report.md`:
```markdown
---
name: Bug Report
about: Something isn't working as expected
labels: bug
---

## What happened

<!-- Description of the bug -->

## Expected behavior

<!-- What you expected to happen -->

## Steps to reproduce

1.
2.
3.

## Environment

- OS:
- Rust version:
- code-explorer version:
- MCP client (e.g., Claude Code):
```

**Step 4: Create feature request template**

`.github/ISSUE_TEMPLATE/feature_request.md`:
```markdown
---
name: Feature Request
about: Suggest a new feature or improvement
labels: enhancement
---

## What

<!-- What feature or improvement would you like? -->

## Why

<!-- What problem does this solve? What use case does it enable? -->
```

**Step 5: Commit**

```bash
git add .github/pull_request_template.md .github/ISSUE_TEMPLATE/
git commit -m "docs: add PR and issue templates"
```

---

### Task 7: Update ARCHITECTURE.md

**Files:**
- Modify: `docs/ARCHITECTURE.md`

**Step 1: Update stale information**

The current ARCHITECTURE.md has several inaccuracies:
- Says "27 tools" in the ASCII diagram — should be 31
- Tool status table says most tools are "Stubs" — all are working now
- Says "1/3 working" for workflow — all 3 work
- References `serena-as-reference/` and `../cocoindex-code/`
- LSP client described as "stub" — it's implemented

Update the ASCII diagram tool count to 31.

Update the tool status table:

| Category | File | Tools | Status |
|----------|------|-------|--------|
| File | `file.rs` | read_file, list_dir, search_for_pattern, find_file, create_text_file, replace_content | Working |
| Workflow | `workflow.rs` | onboarding, check_onboarding_performed, execute_shell_command | Working |
| Symbol | `symbol.rs` | find_symbol, get_symbols_overview, find_referencing_symbols, replace_symbol_body, insert_before_symbol, insert_after_symbol, rename_symbol | Working (LSP) |
| AST | `ast.rs` | list_functions, extract_docstrings | Working (tree-sitter) |
| Git | `git.rs` | git_blame, git_log, git_diff | Working |
| Semantic | `semantic.rs` | semantic_search, index_project, index_status | Working |
| Memory | `memory.rs` | write_memory, read_memory, list_memories, delete_memory | Working |
| Config | `config.rs` | activate_project, get_current_config | Working |

Update LSP client description: remove "stub" wording, describe actual implementation.

Update AST engine description: remove "stub" wording, mention tree-sitter grammars for Rust, Python, TypeScript, Go, Java, Kotlin.

Remove "Reference Projects" section at the bottom.

**Step 2: Commit**

```bash
git add docs/ARCHITECTURE.md
git commit -m "docs: update ARCHITECTURE.md to reflect completed implementation"
```

---

### Task 8: Update ROADMAP.md

**Files:**
- Modify: `docs/ROADMAP.md`

**Step 1: Update content**

```markdown
# Roadmap

See the detailed implementation plan: [`plans/2026-02-25-v1-implementation-plan.md`](plans/2026-02-25-v1-implementation-plan.md)

## Quick Status

| Phase | Description | Sprints | Status |
|-------|-------------|---------|--------|
| 0 | Architecture Foundation (ToolContext) | 0.1 | **Done** |
| 1 | Wire Existing Backends | 1.1-1.4 | **Done** |
| 2 | Complete File Tools | 2.1 | **Done** |
| 3 | LSP Client | 3.1-3.5 | **Done** |
| 4 | Tree-sitter AST Engine | 4.1-4.2 | **Done** |
| 5 | Polish & v1.0 | 5.1-5.3 | **In progress** |

## What's Built

- 31 tools across 8 categories (file, workflow, symbol, AST, git, semantic, memory, config)
- LSP client with transport, lifecycle, document symbols, references, definition, rename
- Tree-sitter symbol extraction + docstrings for Rust, Python, TypeScript, Go, Java, Kotlin
- Embedding pipeline: chunker, SQLite index, remote + local embedders
- Git integration: blame, log, diff via git2
- Persistent memory store with markdown-based topics
- Progressive disclosure output (exploring/focused modes via OutputGuard)
- MCP server over stdio (rmcp)
- 232 tests (227 passing, 5 ignored)

## What's Next

- HTTP/SSE transport (in addition to stdio)
- Additional tree-sitter grammars
- Additional LSP server configurations
- sqlite-vec integration for vector similarity (currently pure-Rust cosine)
- Companion Claude Code plugin: `code-explorer-routing`
```

**Step 2: Commit**

```bash
git add docs/ROADMAP.md
git commit -m "docs: update ROADMAP.md with final build state and next steps"
```

---

### Task 9: Update CLAUDE.md references

**Files:**
- Modify: `CLAUDE.md`

**Step 1: Update stale references**

- Update test count from "186 passing" to "232 tests (227 passing, 5 ignored)"
- Update the tagline's Serena link from `./serena-as-reference/` to `https://github.com/oraios/serena`
- Remove the Reference section at the bottom that mentions `serena-as-reference/`
- Update cocoindex reference to link to GitHub if available, or remove

**Step 2: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: update CLAUDE.md with current test counts and external links"
```

---

### Task 10: Final verification

**Step 1: Run full CI equivalent locally**

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
cargo test --features local-embed --no-default-features
cargo test --no-default-features
```

All should pass.

**Step 2: Verify no private files are tracked**

```bash
git status
```

Confirm: no `docs/research/`, no `docs/observations.md`, no `.code-explorer/`, no `.mcp.json` in tracked files.

**Step 3: Review the full diff**

```bash
git log --oneline -10
```

Should show ~9 clean commits from this plan.

**Step 4: Verify README renders**

```bash
cat README.md | head -20
```

Quick sanity check that the markdown isn't broken.
