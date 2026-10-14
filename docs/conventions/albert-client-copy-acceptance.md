---
id: eec665e0494885d0
kind: guide
status: active
title: Albert native client acceptance on an isolated CodeScout copy
owners:
- marius
tags:
- albert-r02
- client-acceptance
- codex
- claude
- pi
source_commit: c773a2ca5b5225010c06b5784a13470319bc4617
---

# Albert: client acceptance on an isolated CodeScout copy

The supported recovery command is rebuild-memory-vectors, with an explicit existing SQLite database, local-dir snapshot, target dimensions and fresh backup. Albert's parallel candidate uses a different migrate-memories flag and is a separate implementation. Use the current [SQLite recovery procedure](sqlite-memory-vector-rebuild.md).

All MCP launchers and companion hook commands must resolve to the same verified binary. Keep the existing pilot wrapper that isolates filesystem, model, data and network access; do not replace it with an unrestricted launch. These examples describe the synthetic project and trial storage. Client settings and tool approval do not themselves sandbox the external MCP server.

## Verified components and limits

Albert reported Codex 0.162.0-alpha.17.2 and Claude Code 2.1.193. Local checks here used Codex 0.160.0, Claude 2.1.299, Pi 1.1.0 and pi-mcp-adapter 5.2.0. Check the exact installed versions on his laptop before applying a recipe; local help/schema/manifest validation does not prove behavior on his versions.

The companion manifest remains 1.20.17. The shared repaired Pi guard is claude-plugins:f17afad96f0585e0cb80a40c413d6a0b4e2d5101 (stable patch-id a84e2afad3c1448893261965966c8d0ee5e5225e), while the installed cached copy with that same version still requires a retired Markdown tool. Record actual source hashes; a version string alone does not identify the repair. The supplied repository contrib Pi guard hash was b1da15411a4704fce7497095f7db27e27834ca79bdf5f867e59f46800d57719e.

No native Claude/Codex/Pi model session on Albert's laptop has been accepted by these checks. His report proves copy-only recall with his candidate and a synthetic MCP client; native hook execution and approval behavior remain separate.

## Codex: transport and approvals

For a directly configured server, merge the actual existing wrapper command and arguments into a pilot-only config:

~~~toml
approval_policy = "on-request"
sandbox_mode = "read-only"

[mcp_servers.codescout]
command = "/absolute/path/to/verified-isolated-wrapper"
args = []
enabled_tools = [
  "workspace", "read_file", "symbols", "grep", "references",
  "semantic_search", "memory"
]
default_tools_approval_mode = "writes"

[mcp_servers.codescout.tools.memory]
approval_mode = "prompt"
~~~

The command and args must match the verified pilot wrapper. Do not add another transport over a plugin-provided server: in that case its policies belong under plugins."<actual-plugin-id>".mcp_servers."<actual-server>", and the plugin owns the launcher.

Memory is a multiplexed tool: list/read/recall and write/forget share its name. Prompting every memory call avoids approving its write actions implicitly. A required approval with effective policy never is denied; that denial does not diagnose a SQLite/vector failure. The writes policy uses the server's readOnlyHint metadata, so it is not action-level filtering for a multiplexed tool. See [official MCP settings](https://learn.chatgpt.com/docs/extend/mcp?surface=cli), [configuration reference](https://learn.chatgpt.com/docs/config-file/config-reference) and [permissions](https://learn.chatgpt.com/docs/permissions).

The original policy fragment and the complete placeholder transport example above both passed local AJV validation against the 0.160.0 schema (SHA256 e3dd84ba44b6fd99f3bb5365862417b533c80da75d1380ffe4157738bca99953), with format validation disabled. The operator's actual wrapper paths and Albert's client still need local acceptance.

From the synthetic project, use an interactive client:

~~~sh
codex -a on-request -s read-only -C "/absolute/path/to/pilot-project"
~~~

Check /mcp and effective permissions. Request memory(list), memory(read) and memory(recall) separately, approve each individual call and verify actual results with the selected 384-dimensional trial profile. Test read_file and symbols on synthetic files. Index, file editing and MCP shell are absent from this direct enabled_tools list; retain wrapper restrictions for any tool admitted through a different server/plugin configuration.

For multiple Codex profiles, verify each profile's own cache/install paths before any future persistent update. Do not copy an installPath from another profile.

## Claude Code: one verified companion

The existing setup package's CLAUDE-PLUGINS.md contains the one-session and local-marketplace procedures. Retain the verified MCP wrapper and merge this only in the synthetic project's .claude/codescout-companion.json:

~~~json
{
  "server_name": "codescout",
  "auto_index": false,
  "drift_warnings": false,
  "goal_stop_hook": false
}
~~~

Check node:sqlite, jq, sqlite3 and Git on Albert's machine. The local companion manifest validation passed with 20 warnings for unquoted plugin-root paths in hooks; use a path without spaces for that copy.

~~~sh
claude plugin validate "/absolute/path/without-spaces/to/codescout-companion"
claude --plugin-dir "/absolute/path/without-spaces/to/codescout-companion"
~~~

Inside the fresh session, check /plugin and /mcp, the effective companion source/version and one connected server. Check Markdown routing to read_file, source navigation through symbols and semantic search on synthetic data. Confirm no retired tool invocation occurs. Verify list/read/recall with the trial model and data; auto_index=false does not prevent manual index calls.

Manifest validation proves structure. The active hook copy and native behavior require this actual session. Keep other workflow plugins separate until this acceptance succeeds. [Claude plugin loading](https://code.claude.com/docs/en/plugins/loading).

## Pi: choose the adapter naming contract

Current Pi native MCP exposes mcp__<server>__<tool>. The contributed guard expects codescout_<tool>, so choose the inspected adapter integration for this pilot. With only native names, its conditional replacement guards remain inactive. The guard's edit pair is codescout_edit_code plus codescout_edit_file, write replacement is codescout_create_file, and read/bash replacement is codescout_read_file.

For the inspected adapter 5.2.0, prepare a pilot-only JSON:

~~~json
{
  "settings": {
    "toolPrefix": "server",
    "allowInstall": false,
    "hostConfigDiscovery": "off",
    "approveTools": true
  },
  "mcpServers": {
    "codescout": {
      "command": "/absolute/path/to/verified-isolated-wrapper",
      "args": [],
      "lifecycle": "lazy",
      "directTools": [
        "workspace", "memory", "grep", "symbols", "symbol_at",
        "tree", "semantic_search", "references", "read_file",
        "edit_code", "edit_file", "create_file"
      ]
    }
  }
}
~~~

DirectTools controls direct exposure, not authorization; other server tools remain reachable through the adapter proxy. Approval and the isolated wrapper must constrain shell, file writes and index actions separately. The contributed bash patterns do not constrain external MCP run_command.

With an existing verified adapter and guard copy, from the synthetic project:

~~~sh
PI_CODING_AGENT_DIR="/absolute/path/to/pilot-agent" \
PI_MCP_CONFIG_MODE=exclusive \
pi --no-extensions --no-mcp \
  -e "/absolute/path/to/pi-mcp-adapter/index.ts" \
  -e "/absolute/path/to/verified/contrib/pi/codescout-mode.ts" \
  --mcp-config "/absolute/path/to/pilot-mcp-adapter.json"
~~~

Exclusive mode uses the selected adapter configuration and excludes ambient package/plugin/host MCP aggregation. Explicit mcp-config avoids the adapter's automatic user-settings update. No-extensions keeps only explicit extensions; no-mcp additionally makes the native-MCP choice explicit. These flags were parsed locally without starting a session.

Check /mcp, one codescout server and actual codescout_* replacement names. Reconnect to warm metadata if necessary and reload/start a new session before testing the active native-tool set. On synthetic files, check Markdown/source reads and native read/edit/write refusal when replacements are active. Request memory list/read/recall and approve calls individually. Load only one contributed/companion guard.

The repaired companion Pi Node harness passed one top-level test. This does not prove a native Pi session, adapter approvals or filesystem isolation on Albert's laptop.

## Required result before any daily switch

Record binary/source/model/companion/adapter hashes and effective transport. Check every active SQLite memory database and namespace, other code/catalog/Qdrant indexes, and every writer. Use a fresh coherent trial, compare exact raw records/topic files, test meaningful recall after a restart, and rehearse restoration.

Report actual native-client approvals, hook behavior and recall independently. Retain the daily installation until those results and the concrete promotion decision are available.
