# codescout <-> Pi integration

Makes codescout's code-intelligence tools the primary read/search/edit path
inside the Pi coding agent (pi.dev). Design rationale:
`docs/superpowers/specs/2026-06-19-codescout-pi-integration-design.md`.
Reconnaissance findings (grep collision, PATH, setActiveTools) live as F-1/F-2
in `docs/trackers/pi-integration-session-log.md`.

## How it works
pi-mcp-adapter connects codescout (`codescout start`, stdio, lazy) and promotes
a hot-set of codescout tools to first-class Pi tools. The codescout-mode
extension drops Pi's native `edit` and activates the codescout hot-set on every
session (guarded so it no-ops when codescout isn't loaded), and appends a
one-time hint when bash is used to search source. AGENTS.md documents the
tool-map for the model.

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
- `codescout-mode.ts` -> `~/.pi/agent/extensions/` — curation + bash nudge.
- `AGENTS.md` -> `~/.pi/agent/AGENTS.md` — tool-map guidance.
- `install.sh` — idempotent symlink installer (backs up any existing real AGENTS.md).

## Contingency: grep name collision
Pi ships built-in tools (read, write, edit, bash, grep, find, ls), so codescout's
`grep` is NOT in directTools (F-1) — reach it via the `mcp` proxy. To make it
first-class instead, add `"settings": { "toolPrefix": "cs" }` to `mcp.json`
(renames ALL codescout tools `cs_*`) and update the hot-set names in
`codescout-mode.ts` accordingly.
