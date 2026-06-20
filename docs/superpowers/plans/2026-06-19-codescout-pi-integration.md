# codescout ↔ Pi Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make codescout's code-intelligence tools the primary read/search/edit path inside the Pi coding agent, as a personal daily-driver, with codescout's redundant edit primitive removed and bash steered toward codescout for source search.

**Architecture:** `pi-mcp-adapter` connects codescout (`codescout start`, stdio, lazy) and promotes a hot-set of codescout tools to first-class Pi tools (`directTools`). A small `codescout-mode` extension curates the active tool set on `session_start` (drops Pi's native `edit`, activates the codescout hot-set — guarded so it no-ops when codescout isn't loaded) and appends a one-time, non-blocking hint via `tool_result` when bash is used to search source. A global `AGENTS.md` makes the substitution legible to the model. Source of truth for all config lives in the codescout repo under `contrib/pi/` and is symlinked into `~/.pi/agent/`.

**Tech Stack:** Pi (`@earendil-works/pi-coding-agent`, npm global), `pi-mcp-adapter` (Pi package), TypeScript (extension, run uncompiled via Pi's jiti), codescout (existing Rust MCP server), bash (installer).

## Global Constraints

- Node ≥ 23.6 required by Pi; environment has `node v26.2.0`, `npm 11.16.0` — satisfied.
- Install Pi with `--ignore-scripts` (Pi's own supply-chain guidance; no install scripts needed).
- Pin `pi-mcp-adapter` to an exact version after install (record it in `contrib/pi/README.md`).
- `mcp.json` top-level key is **`"mcpServers"`** (verified — `"servers"` is wrong and silently fails).
- codescout is invoked as bare **`codescout start`** — it auto-detects + canonicalizes the cwd (`src/server.rs:1248`). Do **NOT** pass `--project .` (the code comment warns this causes path-form drift).
- codescout binary resolves via `~/.cargo/bin/codescout` → `target/release/codescout` (symlink); it must exist and be on `PATH` for the Pi-spawned adapter.
- All `~/.pi/agent/` artifacts are symlinks to versioned files in `contrib/pi/`; never hand-edit the copies under `~/.pi/agent/`.
- Work stays on the `experiments` branch (codescout `master` is protected).

---

### Task 1: Preflight — verify toolchain, codescout binary, and PATH

**Files:** none (environment gate; no commit).

- [ ] **Step 1: Verify node/npm versions**

Run: `node -v; npm -v`
Expected: node ≥ v23.6 (have v26.2.0), npm present.

- [ ] **Step 2: Verify the codescout binary is on PATH and fresh**

Run: `command -v codescout && readlink -f "$(command -v codescout)"`
Expected: prints a path, and the resolved target ends in `target/release/codescout`. If `command -v codescout` is empty, add `~/.cargo/bin` to PATH. If the symlink is missing/stale, recreate it from the codescout repo root: `ln -sf "$(pwd)/target/release/codescout" ~/.cargo/bin/codescout`.

- [ ] **Step 3: Verify codescout runs and shows the `start` subcommand**

Run: `codescout --help`
Expected: usage listing including `start` (and `--transport stdio|http`). Confirms the binary is the right one.

- [ ] **Step 4: Confirm no commit needed**

This task changes no files. Proceed to Task 2.

---

### Task 2: Install Pi globally and confirm it launches

**Files:** none (system install; no commit).

- [ ] **Step 1: Install Pi**

Run: `npm install -g --ignore-scripts @earendil-works/pi-coding-agent`
Expected: completes without error; installs the `pi` binary.

- [ ] **Step 2: Verify the binary**

Run: `command -v pi && pi --version`
Expected: a path under the npm global bin, and a version string (e.g. `0.79.x`).

- [ ] **Step 3: Verify help lists modes**

Run: `pi --help`
Expected: usage text mentioning interactive/print modes and flags like `-p`, `--mode`, `--extensions`.

- [ ] **Step 4: Note auth prerequisite**

Behavioral verification (Task 7) needs a model provider. Confirm one is available: either `echo "${ANTHROPIC_API_KEY:+set}"` prints `set`, or plan to run `/login` inside Pi. No commit.

---

### Task 3: Install pi-mcp-adapter

**Files:** none (Pi package install; no commit).

- [ ] **Step 1: Install the adapter**

Run: `pi install npm:pi-mcp-adapter`
Expected: success message; the package lands under `~/.pi/agent/` (packages dir).

- [ ] **Step 2: Verify it installed**

Run: `ls -la ~/.pi/agent/packages 2>/dev/null; ls -la ~/.pi/agent/extensions 2>/dev/null`
Expected: a `pi-mcp-adapter` entry appears under one of these (package layout). If neither exists yet, re-run Step 1 and check its output for the install path.

- [ ] **Step 3: Record the pinned version**

Run: `npm view pi-mcp-adapter version`
Expected: a version string — note it for `contrib/pi/README.md` (Task 7) so the install is reproducible.

---

### Task 4: Configure codescout in Pi (`mcp.json`) and warm the directTools cache

**Files:**
- Create: `contrib/pi/mcp.json` (repo source of truth)
- Symlink target: `~/.pi/agent/mcp.json`

**Interfaces:**
- Produces: a registered MCP server named `codescout` whose hot-set tool names (`symbols`, `symbol_at`, `tree`, `semantic_search`, `references`, `read_file`, `read_markdown`, `edit_code`, `edit_file`, `edit_markdown`) become first-class Pi tools. Task 5's extension consumes these exact names.

- [ ] **Step 1: Create the repo source-of-truth config**

Create `contrib/pi/mcp.json`:

```json
{
  "mcpServers": {
    "codescout": {
      "command": "codescout",
      "args": ["start"],
      "lifecycle": "lazy",
      "directTools": [
        "symbols",
        "symbol_at",
        "tree",
        "semantic_search",
        "references",
        "read_file",
        "read_markdown",
        "edit_code",
        "edit_file",
        "edit_markdown"
      ]
    }
  }
}
```

- [ ] **Step 2: Validate the JSON**

Run: `jq . contrib/pi/mcp.json`
Expected: pretty-prints the object with top-level key `mcpServers`. (No error = valid JSON.)

- [ ] **Step 3: Symlink into the Pi agent dir**

Run: `mkdir -p ~/.pi/agent && ln -sf "$(pwd)/contrib/pi/mcp.json" ~/.pi/agent/mcp.json && readlink -f ~/.pi/agent/mcp.json`
Expected: resolves to `<repo>/contrib/pi/mcp.json`.

- [ ] **Step 4: Warm the directTools cache (interactive)**

Launch `pi` in the codescout repo, then run `/mcp reconnect codescout`, then `/mcp tools`.
Expected: codescout connects; the hot-set tools are listed as direct tools. (On the very first session the cache is cold and tools fall back to proxy-only — `/mcp reconnect codescout` populates `~/.pi/agent/mcp-cache.json`.) Quit pi.

> Name collision (confirmed — see F-1 in `docs/trackers/pi-integration-session-log.md`): Pi's tool registry includes a built-in `grep` (coding-agent CHANGELOG), so codescout's `grep` is **dropped from `directTools` by default** to avoid the clash — reach it via the `mcp` proxy. To keep `grep` first-class instead, add `"settings": { "toolPrefix": "cs" }` to `mcp.json` and prefix the hot-set names in Task 5 (`cs_symbols`, …). Also: `pi.setActiveTools` rejects on unknown/duplicate names (F-1) — the extension wraps it in `await` + try/catch so a stale name degrades to "native tools kept".

- [ ] **Step 5: Commit**

```bash
git add contrib/pi/mcp.json
git commit -m "feat(contrib/pi): codescout MCP server config for Pi (mcpServers + directTools hot-set)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 5: Write the `codescout-mode` extension

**Files:**
- Create: `contrib/pi/codescout-mode.ts` (repo source of truth)
- Symlink target: `~/.pi/agent/extensions/codescout-mode.ts`

**Interfaces:**
- Consumes: the codescout hot-set tool names registered by Task 4 (`symbols`, `edit_code`, etc.).
- Produces: on `session_start`, an active tool set with Pi's native `edit` removed and the codescout hot-set added (guarded by presence of `edit_code` + `symbols`); on `tool_result`, a one-time appended hint when bash searched source.

- [ ] **Step 1: Write the extension**

Create `contrib/pi/codescout-mode.ts`:

```typescript
/**
 * codescout-mode — curate-and-substitute integration for codescout.
 *
 * - session_start: drop Pi's native `edit` and activate codescout's hot-set,
 *   but ONLY if codescout's tools are actually registered (cache warm / code
 *   project). Otherwise no-op — never leave the session with no edit tool.
 * - tool_result: append a one-time, non-blocking hint when bash was used to
 *   grep/find source, steering future calls to codescout. bash still runs.
 *
 * Source of truth: codescout repo contrib/pi/codescout-mode.ts, symlinked to
 * ~/.pi/agent/extensions/codescout-mode.ts.
 */
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { isBashToolResult } from "@earendil-works/pi-coding-agent";

// Must match the directTools list in contrib/pi/mcp.json.
const CODESCOUT_HOT_SET = [
	"symbols",
	"symbol_at",
	"tree",
	"semantic_search",
	"references",
	"read_file",
	"read_markdown",
	"edit_code",
	"edit_file",
	"edit_markdown",
];

// Pi built-ins codescout supersedes and we drop from the active set.
const DROP_BUILTINS = ["edit"];

// bash invocations that should have used codescout (source search):
// ripgrep, ag, recursive grep, or `find <path> -name`.
const SOURCE_SEARCH = /(^|\s|\|)(rg|ag)\b|(^|\s|\|)grep\s+[^|]*-[a-zA-Z]*r|(^|\s|\|)find\s+\S+\s+-name\b/;

export default function (pi: ExtensionAPI) {
	let nudged = false;

	pi.on("session_start", async (_event, ctx) => {
		const all = pi.getAllTools();
		const has = (name: string) => all.some((t) => t.name === name);

		// Safety guard: only curate when codescout's core tools are present.
		if (!has("edit_code") || !has("symbols")) return;

		const active = new Set(pi.getActiveTools());
		for (const name of DROP_BUILTINS) active.delete(name);
		for (const name of CODESCOUT_HOT_SET) if (has(name)) active.add(name);
		try { await pi.setActiveTools([...active]); } catch (e) { if (ctx.hasUI) ctx.ui.notify(`codescout-mode: setActiveTools failed (${String(e)})`, "warn"); return; }

		if (ctx.hasUI) {
			ctx.ui.notify("codescout-mode: codescout tools active; native `edit` dropped", "info");
		}
	});

	pi.on("tool_result", async (event) => {
		if (nudged) return undefined;
		if (!isBashToolResult(event)) return undefined;
		const command = (event.input as { command?: string }).command ?? "";
		if (!SOURCE_SEARCH.test(command)) return undefined;
		nudged = true;
		return {
			content: [
				...event.content,
				{
					type: "text" as const,
					text:
						"\n[codescout-mode] For source search prefer `grep` / `semantic_search` / `references`; " +
						"for reading code prefer `symbols` / `read_file`. (Shown once per session.)",
				},
			],
		};
	});
}
```

- [ ] **Step 2: Symlink into the Pi extensions dir**

Run: `mkdir -p ~/.pi/agent/extensions && ln -sf "$(pwd)/contrib/pi/codescout-mode.ts" ~/.pi/agent/extensions/codescout-mode.ts && readlink -f ~/.pi/agent/extensions/codescout-mode.ts`
Expected: resolves to `<repo>/contrib/pi/codescout-mode.ts`.

- [ ] **Step 3: Verify the extension loads without error (interactive)**

Launch `pi` in the codescout repo. After the codescout server connects (it spawns lazily on first tool use, so trigger it: ask Pi to "list the symbols in src/main.rs"), run `/reload`.
Expected: no extension load error in the status area; the `codescout-mode` notify toast appears ("codescout tools active; native `edit` dropped"). If an error shows, it names the file + line — fix in `contrib/pi/codescout-mode.ts` (the symlink picks it up on next `/reload`).

- [ ] **Step 4: Verify curation took effect**

In the same pi session, ask: "What tools do you have available? List them." (or use `/mcp tools` for the codescout ones).
Expected: the codescout hot-set is present; Pi's native `edit` is **absent**; `read`, `write`, `bash` remain.

- [ ] **Step 5: Commit**

```bash
git add contrib/pi/codescout-mode.ts
git commit -m "feat(contrib/pi): codescout-mode Pi extension (curate active tools + bash source-search nudge)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 6: Write the global `AGENTS.md` tool-map

**Files:**
- Create: `contrib/pi/AGENTS.md` (repo source of truth)
- Symlink target: `~/.pi/agent/AGENTS.md`

- [ ] **Step 1: Write the AGENTS.md**

Create `contrib/pi/AGENTS.md`:

```markdown
# codescout-aware harness

codescout's tools are the primary path for reading, searching, and editing code.
Use them instead of bash equivalents.

## Reading code
- `symbols` — file/dir symbol overview; add `include_body` for function bodies.
- `read_file` — non-source files or specific line ranges.
- `read_markdown` — markdown (heading-addressed).
- Do NOT `cat`/`sed`/`head` source files via bash.

## Searching
- exact-regex search: codescout `grep` via the `mcp` proxy (dropped from first-class tools — its bare name clashes with Pi's built-in `grep`).
- `semantic_search` — concept-level / natural-language search.
- `references` — who calls / uses a symbol (NOT bash grep).
- Do NOT `rg`/`grep -r`/`find -name` source via bash.

## Editing
- `edit_code` — structural, LSP-aware edits (rename, replace/insert/remove a symbol).
- `edit_file` — text/import edits by exact string match.
- `edit_markdown` — markdown edits by heading.
- `write` — create new files.
- Pi's native `edit` is intentionally disabled in this setup.

## Shell
- `bash` — tests, git, build, and process tasks only.

## Deeper codescout (on demand)
- Trackers/artifacts, project memory, librarian, workspace, indexing, and other
  codescout tools are reachable via the `mcp` proxy tool when needed.
```

- [ ] **Step 2: Symlink into the Pi agent dir (without clobbering an existing real file)**

Run:
```bash
if [ -e ~/.pi/agent/AGENTS.md ] && [ ! -L ~/.pi/agent/AGENTS.md ]; then
  mv ~/.pi/agent/AGENTS.md ~/.pi/agent/AGENTS.md.bak
  echo "Backed up existing AGENTS.md -> AGENTS.md.bak"
fi
ln -sf "$(pwd)/contrib/pi/AGENTS.md" ~/.pi/agent/AGENTS.md
readlink -f ~/.pi/agent/AGENTS.md
```
Expected: resolves to `<repo>/contrib/pi/AGENTS.md`; any pre-existing real file is backed up.

- [ ] **Step 3: Verify Pi loads it**

Launch `pi` in any directory, ask: "What are your project instructions?"
Expected: Pi paraphrases the codescout tool-map (confirms `~/.pi/agent/AGENTS.md` is loaded globally).

- [ ] **Step 4: Commit**

```bash
git add contrib/pi/AGENTS.md
git commit -m "feat(contrib/pi): global AGENTS.md tool-map steering Pi to codescout

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 7: Reproducible installer, README, and end-to-end verification

**Files:**
- Create: `contrib/pi/install.sh`
- Create: `contrib/pi/README.md`

- [ ] **Step 1: Write the installer**

Create `contrib/pi/install.sh`:

```bash
#!/usr/bin/env bash
# Symlink the codescout↔Pi integration files into the Pi agent dir.
set -euo pipefail

SRC="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
AGENT_DIR="${PI_CODING_AGENT_DIR:-$HOME/.pi/agent}"

mkdir -p "$AGENT_DIR/extensions"

ln -sf "$SRC/mcp.json" "$AGENT_DIR/mcp.json"
ln -sf "$SRC/codescout-mode.ts" "$AGENT_DIR/extensions/codescout-mode.ts"

if [ -e "$AGENT_DIR/AGENTS.md" ] && [ ! -L "$AGENT_DIR/AGENTS.md" ]; then
  mv "$AGENT_DIR/AGENTS.md" "$AGENT_DIR/AGENTS.md.bak"
  echo "Backed up existing AGENTS.md -> AGENTS.md.bak"
fi
ln -sf "$SRC/AGENTS.md" "$AGENT_DIR/AGENTS.md"

echo "Installed codescout<->Pi integration into $AGENT_DIR"
echo "Next: launch pi in a repo, run '/mcp reconnect codescout' once to warm the cache."
```

- [ ] **Step 2: Make it executable and verify it's idempotent**

Run: `chmod +x contrib/pi/install.sh && bash contrib/pi/install.sh && bash contrib/pi/install.sh`
Expected: prints the install message both times with no error (idempotent); the three symlinks resolve into `contrib/pi/`.

- [ ] **Step 3: Write the README**

Create `contrib/pi/README.md` (replace `<PINNED>` with the version recorded in Task 3 Step 3):

```markdown
# codescout <-> Pi integration

Makes codescout's code-intelligence tools the primary read/search/edit path
inside the Pi coding agent (pi.dev). Design rationale:
`docs/superpowers/specs/2026-06-19-codescout-pi-integration-design.md`.

## How it works
pi-mcp-adapter connects codescout (codescout start, stdio, lazy) and promotes a
hot-set of codescout tools to first-class Pi tools. The codescout-mode extension
drops Pi's native edit and activates the codescout hot-set on every session
(guarded so it no-ops when codescout isn't loaded), and appends a one-time hint
when bash is used to search source. AGENTS.md documents the tool-map for the model.

## Prerequisites
- Node >= 23.6 (node -v).
- codescout on PATH: command -v codescout resolves to target/release/codescout.

## Install

    npm install -g --ignore-scripts @earendil-works/pi-coding-agent
    pi install npm:pi-mcp-adapter@<PINNED>
    bash contrib/pi/install.sh

Then launch pi in a repo and run once: /mcp reconnect codescout (warms the
directTools cache).

## Files
- mcp.json -> ~/.pi/agent/mcp.json (codescout server + directTools hot-set)
- codescout-mode.ts -> ~/.pi/agent/extensions/ (curation + bash nudge)
- AGENTS.md -> ~/.pi/agent/AGENTS.md (tool-map guidance)
- install.sh (idempotent symlink installer)

## Contingency: grep name collision
Pi has an optional built-in grep. If /mcp tools reports a clash, either drop grep
from directTools in mcp.json (use it via the mcp proxy) or add a settings block
with toolPrefix "cs" and update the hot-set names in codescout-mode.ts.
```

- [ ] **Step 4: End-to-end behavioral verification (dogfood in the codescout repo)**

Launch `pi` in the codescout repo (auth set via env or `/login`). Confirm each:
1. `/mcp` → codescout shows **connected**; hot-set tools present.
2. Prompt "show the symbols in `src/main.rs`" → Pi calls `symbols` (not `cat`/`read`).
3. Prompt "who calls `librarian_enabled_at_runtime`?" → Pi calls `references` (not bash grep).
4. Prompt "rename a local variable in <small fn>" → Pi calls `edit_code`.
5. Tool list shows **no** native `edit`; `read`/`write`/`bash` present.
6. Prompt "run `rg TODO src/`" → bash runs AND the `[codescout-mode]` hint appears once in the result.
7. Run `bash -c 'echo "checks ok"'` style task → bash still works for non-search shell.

Expected: all seven hold. Note any failures with the exact prompt + observed tool.

- [ ] **Step 5: Verify risk #1 — codescout activates the right project from inside Pi**

In the same pi session, via the proxy: ask Pi to "call codescout's `workspace` tool with action=status" (or invoke `mcp` proxy → `workspace`/`status`).
Expected: `project_root` is the codescout repo path (proves the Pi-spawned `codescout start` adopted Pi's cwd). If it reports a different/empty project, the adapter did not spawn codescout in Pi's cwd — fix by adding a `cwd` to the codescout entry in `mcp.json` (or launch pi from the repo root) and re-verify. Do NOT switch to `--project .`.

- [ ] **Step 6: Commit**

```bash
git add contrib/pi/install.sh contrib/pi/README.md
git commit -m "feat(contrib/pi): installer + README for the codescout<->Pi integration

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Notes & deferred (out of scope for this plan)

- **No native CLI wrapping** of codescout's librarian/artifact tools (those reach Pi via the proxy). A future add could `pi.registerTool` + `pi.exec("codescout", ["artifact", ...])` since that surface has a CLI.
- **bash steering is a one-time `tool_result` hint, not a block** — Pi's `tool_call` return only controls blocking, so a non-blocking nudge must come from `tool_result`. Hardening into a selective block is a future option.
- **Global `AGENTS.md`** applies codescout guidance in every Pi session (including non-code dirs, where the extension no-ops). Acceptable for a daily-driver; scope to project `.pi/` later if it becomes noise.
- All `contrib/pi/` work stays on `experiments`; promotion to `master` follows the Standard Ship Sequence if desired.
```