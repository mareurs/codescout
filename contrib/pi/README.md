# codescout <-> Pi integration

Makes codescout's code-intelligence tools the primary read/search/edit path
inside the Pi coding agent (pi.dev). Design rationale:
`docs/superpowers/specs/2026-06-19-codescout-pi-integration-design.md`.
Reconnaissance findings (grep collision, PATH, setActiveTools) live as F-1/F-2
in `docs/trackers/pi-integration-session-log.md`.

## How it works

pi-mcp-adapter connects codescout (`codescout start`, stdio, lazy) and promotes
a hot-set of codescout tools to first-class `codescout_*` Pi tools (adapter
default `toolPrefix: "server"` — see F-3). The codescout-mode extension drops
Pi's native `edit`/`write` on session start (guarded so it no-ops when
codescout isn't loaded), and hard-blocks native `read` (except images) and
`bash` (except commands outside codescout's redundant read/search set, or
carrying an explicit `# codescout-override` marker) via the `tool_call` hook.
AGENTS.md documents the tool-map for the model.

## Prerequisites
- Node >= 23.6 (`node -v`).
- A codescout release binary. `mcp.json` points at an ABSOLUTE path
  (`/home/marius/.cargo/bin/codescout`) because `~/.cargo/bin` is not on PATH
  on this machine (F-2). Adjust the `command` in `mcp.json` to your binary
  location if different.
- `mcp.json` is gitignored (it holds API keys for the `researcher` server) —
  copy it from `mcp.json.example` before installing:

      cp contrib/pi/mcp.json.example contrib/pi/mcp.json

  then fill in your keys and adjust the `command` paths.

## Install

    npm install -g --ignore-scripts @earendil-works/pi-coding-agent
    pi install npm:pi-mcp-adapter@2.10.0
    bash contrib/pi/install.sh

Then launch pi in a repo and run once: `/mcp reconnect codescout` (warms the
directTools cache; on the first session directTools fall back to the proxy
until the cache is populated).

## Files
- `mcp.json` (gitignored, personal — create from `mcp.json.example`) -> `~/.pi/agent/mcp.json` — codescout server (absolute command) + directTools hot-set.
- `mcp.json.example` — tracked template with placeholder API keys.
- `codescout-mode.ts` -> `~/.pi/agent/extensions/` — drops native edit/write, hard-blocks native read/bash via `tool_call`.
- `AGENTS.md` -> `~/.pi/agent/AGENTS.md` — tool-map guidance.
- `install.sh` — idempotent symlink installer (backs up any existing real AGENTS.md).

## Contingency: grep name collision

Resolved by F-3: pi-mcp-adapter's default `toolPrefix: "server"` means every
codescout direct tool (including `grep`) registers as `codescout_<name>`, so
there's no collision with Pi's built-in `grep`/`read`/`write`/`edit`/`bash`.
`grep` is back in `directTools` and reachable directly as `codescout_grep`.
Set `"settings": { "toolPrefix": "none" }` in `mcp.json` only if bare names are
ever preferred — doing so would reintroduce the collision this section used to
work around, and `codescout-mode.ts`'s tool-name constants would need updating
to match.
