# Librarian Companion Hint

> ⚠ Experimental — may change without notice.

`librarian-mcp` ships a short prompt block describing itself for sibling
companion plugins to inject at session start. The aim is to make Librarian
discoverable to LLMs that don't auto-enumerate every MCP server, without
hardcoding tool names in any plugin.

## How it works

1. The prompt source lives in
   `crates/librarian-mcp/src/prompts/companion_hint.md`. It evolves with
   librarian's tool surface and is guarded by build-time tests
   (`tests/companion_hint.rs`) that check every `artifact_*` / `librarian_*`
   token against the real tool registry.
2. A new clap subcommand prints the prompt verbatim to stdout:

   ```bash
   librarian-mcp print-companion-hint
   ```

   No side effects, no MCP handshake, safe to call from shell hooks.
3. The codescout-companion plugin's `SessionStart` hook detects librarian
   filesystem-side and pipes the output into the additional-context block
   alongside codescout's own guidance.

## Detection rules

The companion hook emits the hint when **all** are true:

- `LIBRARIAN_COMPANION_HINT` is unset or non-zero (suppression escape hatch).
- `librarian-mcp` (or `librarian-mcp-wrapper.sh`) is on `PATH`.
- Either `$LIBRARIAN_DB` / `$XDG_DATA_HOME/librarian/catalog.db` exists, or
  `$LIBRARIAN_WORKSPACE` / `$XDG_CONFIG_HOME/librarian/workspace.toml` exists.

Filesystem-based detection is deliberate: scanning `~/.claude/.claude.json` is
unreliable across multi-instance setups (e.g. `~/.claude` and
`~/.claude-sdd`). Librarian's user-global state is the same regardless of
which Claude Code instance is active.

## Why a CLI subcommand instead of an MCP resource

A shell hook calling a CLI is the simplest possible transport. No MCP client,
no roundtrip, no client-name coupling. The trade-off is that other MCP clients
(Cursor, Gemini CLI) won't see the hint until they grow analogous
session-start injection hooks.
