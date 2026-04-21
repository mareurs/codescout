# Bash language support
## What it does

Bash and shell scripts (`.sh`, `.bash`) now receive full language support:
symbol extraction via tree-sitter and LSP navigation via `bash-language-server`.

Previously Bash was detection-only — file contents were indexed for semantic
search but no symbols were extracted and no LSP server was started.

## Capabilities

| Capability | Available |
|---|---|
| Semantic search indexing | ✅ |
| `list_symbols` / `find_symbol` | ✅ (function definitions) |
| `goto_definition` / `find_references` | ✅ (requires LSP) |
| `hover` | ✅ (requires LSP) |

## Setup

Install `bash-language-server`:

```bash
npm install -g bash-language-server
```

No project configuration needed — codescout detects `.sh` / `.bash` files and
starts the server automatically.

## Known limits

- `bash-language-server` is invoked with `start` (positional argument), not
  `--stdio` — the invocation differs from most other LSP servers.
- Symbol extraction covers `function_definition` nodes only. Variable
  declarations and sourced scripts are not indexed as symbols.
- Large generated shell scripts may produce many small symbol chunks in
  semantic search results.
