# Manual Design

## Summary

Create a proper user manual for code-explorer using mdBook, deployed to GitHub Pages via GitHub Actions.

## Audience

Three tiers, weighted by priority:
1. **LLM/AI agent operators** — people setting up code-explorer as an MCP server for Claude Code
2. **End-user developers** — people using Claude Code with code-explorer already installed
3. **Contributors/extenders** — people adding languages, tools, or embedding backends

## Directory Structure

```
docs/manual/
├── book.toml
├── src/
│   ├── SUMMARY.md
│   ├── introduction.md
│   ├── getting-started/
│   │   ├── installation.md
│   │   ├── first-project.md
│   │   └── routing-plugin.md
│   ├── concepts/
│   │   ├── progressive-disclosure.md
│   │   ├── tool-selection.md
│   │   └── output-modes.md
│   ├── tools/
│   │   ├── overview.md
│   │   ├── symbol-navigation.md
│   │   ├── file-operations.md
│   │   ├── semantic-search.md
│   │   ├── git.md
│   │   ├── ast.md
│   │   ├── memory.md
│   │   ├── editing.md
│   │   └── workflow-and-config.md
│   ├── configuration/
│   │   ├── project-toml.md
│   │   └── embedding-backends.md
│   ├── semantic-search-guide.md
│   ├── language-support.md
│   ├── extending/
│   │   ├── adding-languages.md
│   │   ├── writing-tools.md
│   │   └── tool-trait.md
│   ├── architecture.md
│   └── troubleshooting.md
```

## book.toml

```toml
[book]
title = "code-explorer Manual"
authors = ["Marius"]
language = "en"
src = "src"

[build]
build-dir = "../../target/manual"

[output.html]
git-repository-url = "https://github.com/mareurs/code-explorer"
edit-url-template = "https://github.com/mareurs/code-explorer/edit/master/docs/manual/{path}"
no-section-label = false
```

Build output goes to `target/manual/` (already gitignored).

## Tool Reference Format

Each tool reference page groups related tools. Each tool follows this format:

```markdown
## `tool_name`

**Purpose:** One-line description.

**Parameters:**
| Name | Type | Required | Default | Description |
|------|------|----------|---------|-------------|

**Example:**
\```json
{ "tool": "tool_name", "arguments": { ... } }
\```

**Output (exploring mode):**
\```json
...
\```

**Output (focused mode):**
\```json
...
\```

**Tips:** When to use this vs alternatives.
```

## CI: GitHub Actions

### Manual build and deploy (`.github/workflows/manual.yml`)

```yaml
name: Manual

on:
  push:
    branches: [master]
    paths: [docs/manual/**]
  pull_request:
    paths: [docs/manual/**]

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: taiki-e/install-action@mdbook
      - run: mdbook build docs/manual

  deploy:
    if: github.ref == 'refs/heads/master'
    needs: build
    runs-on: ubuntu-latest
    permissions:
      pages: write
      id-token: write
    environment:
      name: github-pages
      url: ${{ steps.deploy.outputs.page_url }}
    steps:
      - uses: actions/checkout@v4
      - uses: taiki-e/install-action@mdbook
      - run: mdbook build docs/manual
      - uses: actions/upload-pages-artifact@v3
        with:
          path: target/manual
      - id: deploy
        uses: actions/deploy-pages@v4
```

### Tool docs sync check (addition to existing CI)

```yaml
  tool-docs-sync:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo build
      - name: Check all tools are documented
        run: |
          grep -r 'fn name(&self)' src/tools/ | sed 's/.*"\(.*\)"/\1/' | sort > /tmp/code-tools.txt
          grep -roh '## `[a-z_]*`' docs/manual/src/tools/ | sed 's/## `\(.*\)`/\1/' | sort > /tmp/doc-tools.txt
          diff /tmp/code-tools.txt /tmp/doc-tools.txt || (echo "Tool docs out of sync!" && exit 1)
```

## Content Strategy

| Chapter | Source | Notes |
|---|---|---|
| Introduction | Fresh | Expand README's problem/solution with philosophy |
| Getting Started | Adapted from README | Add first-project walkthrough, verification |
| Core Concepts | Fresh | Progressive disclosure, tool selection — user-facing prose |
| Tool Reference | Fresh | Per-tool with examples and sample output (bulk of work) |
| Configuration | Partially fresh | Document project.toml fields, embedding backends |
| Semantic Search Guide | Fresh | End-to-end: install Ollama, configure, index, search |
| Language Support | Fresh | Per-language: LSP binary, install, tree-sitter, quirks |
| Extending | Adapted from ARCHITECTURE.md + CONTRIBUTING.md | Concrete examples |
| Architecture | Adapted from docs/ARCHITECTURE.md | Lighter, link to full doc |
| Troubleshooting | Fresh | Common issues from experience |

## Tone

Practical, second-person ("you"), generous with examples. Closer to ripgrep's GUIDE than academic docs. Each tool reference shows real input/output.

## README Impact

After the manual exists, slim down README.md:
- Keep the problem/solution pitch and installation
- Replace full tool tables with a summary + "Read the manual" link
- Remove architecture diagram (lives in manual now)

## Design Decisions

1. **Hand-written tool reference with CI sync check** — editorial control over examples and prose, with automated guard against drift.
2. **mdBook default theme** — no custom CSS to start. Add later if needed.
3. **Build to target/manual/** — keeps build artifacts in the existing gitignored target/ directory.
4. **Path-filtered CI** — only triggers on `docs/manual/` changes, doesn't slow down code PRs.
