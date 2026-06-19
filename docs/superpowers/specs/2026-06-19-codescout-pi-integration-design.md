# Design — codescout ↔ Pi daily-driver integration

- **Date:** 2026-06-19
- **Status:** approved (brainstorm) → pending implementation plan
- **Author:** Marius + Claude
- **Scope:** Wire codescout's code-intelligence tools into the Pi coding agent
  (`pi.dev`, `earendil-works/pi`) as a personal daily-driver, with
  companion-style guardrails realized the Pi-idiomatic way.

## 1. Goal & non-goals

**Goal.** A solid personal setup that makes Pi behave like a codescout-aware
harness: codescout's read/search/edit tools become the primary code path inside
Pi, Pi's redundant code-edit primitive is removed, bash stays usable for
tests/git/build, and an `AGENTS.md` makes the substitution legible to the model.

**Non-goals (YAGNI).**

- No published/shareable `pi-codescout` npm package.
- No hand-rolled MCP client (we compose `pi-mcp-adapter`, not reimplement it).
- No port of codescout-companion's *hooks* — they are Claude-Code-specific
  (`PreToolUse` shell hooks) and have no meaning in Pi. We re-express the intent
  via Pi's own extension API.
- No wrapping of codescout's librarian/artifact tools as native Pi CLI tools in
  v1 (they come "for free" via the proxy; a `pi.exec` + `codescout artifact`
  wrapper is a clean later add — see §9).

## 2. Background — why this is even possible (and not against Pi's grain)

Pi deliberately ships **no MCP in core** because registering an MCP server
normally injects every tool's schema into context permanently (Pi cites
Playwright MCP = 21 tools / 13.7k tokens). codescout is a heavyweight server by
that metric (~26 tools, large schemas), so a naive connection is Pi's worst
case.

`pi-mcp-adapter` resolves this: it registers a **single ~200-token proxy tool**
that lazily discovers and calls MCP tools on demand, and can **promote** a
chosen subset to first-class "direct" tools. This is Pi's own
primitives-+-progressive-disclosure philosophy applied to MCP — so the adapter
is the blessed bridge, not a hack against the grain.

**Key constraint that shapes everything:** codescout's *code-intelligence* core
(`symbols`, `semantic_search`, `references`, `grep`, `edit_code`, …) is
**MCP-only** — there is no `codescout symbols` CLI subcommand. Verified in
`codescout` repo `src/main.rs:20-196`: the CLI exposes `start`, `index`,
`dashboard`, `migrate-memories`, `doctor`, `audit-doc-refs`, and the
`artifact*` librarian verbs — but none of the code-intel tools. Therefore the
code-intel surface can reach Pi **only** over MCP (the adapter); the librarian
surface additionally has a CLI.

## 3. Grounded Pi API facts (verified against the clone at `../pi`, depth-1)

The implementation plan may rely on these without re-verifying:

- Extension factory + tool registration: `pi.registerTool(definition)` —
  `packages/coding-agent/src/core/extensions/loader.ts:192`
  (`extension.tools.set(tool.name, …)`); documented in
  `packages/coding-agent/docs/extensions.md` §"ExtensionAPI Methods" (L1283) and
  §"Custom Tools" (L1727).
- Event hooks: `pi.on(event, handler)` — `docs/extensions.md` L1279.
- `tool_call` hook carries `{ block?: boolean; reason?: string }` —
  `packages/agent/docs/hooks.md:42`; documented at `docs/extensions.md` §"tool_call" (L700).
- Active-tool curation: `pi.getActiveTools() / pi.getAllTools() /
  pi.setActiveTools(names)` — `docs/extensions.md` L1549.
- Overriding built-in tools is supported — `docs/extensions.md` §"Overriding
  Built-in Tools" (L1891).
- `pi.exec(command, args, options?)` (run external commands from an extension) —
  `docs/extensions.md` L1540. (Relevant only to the deferred librarian-CLI add.)
- Reference enforcement examples to model the extension on:
  `packages/coding-agent/examples/extensions/permission-gate.ts` (24-line
  `(pi) => pi.on("tool_call", …)` blocker), plus `tool-override.ts`,
  `protected-paths.ts`, `confirm-destructive.ts`, `tools.ts`, `dynamic-tools.ts`.
- Lifecycle events available for safe timing of `setActiveTools`:
  `session_start`, `resources_discover` — `docs/extensions.md` §"Events".
- Pi install is npm-based; local node toolchain confirmed: `node v26.2.0`,
  `npm 11.16.0` (Pi/Gondolin need node ≥ 23.6 — satisfied). `pi` and `bun` not
  yet installed.

## 4. Architecture overview

Three artifacts plus an install, all global under `~/.pi/agent/` so codescout is
available in every repo; the extension is defensive so it no-ops where codescout
isn't applicable.

```
Pi agent loop
  → model calls `symbols` (directTool)
      → pi-mcp-adapter → codescout (stdio `codescout start`) → result → render
  → model calls `bash rg foo src/`
      → codescout-mode `tool_call` hook injects a nudge (no block) → bash runs
  → model calls `artifact find` (rare)
      → single `mcp` proxy tool → adapter discovers/forwards → result
```

codescout runs as a **separate process** from Claude Code's existing server, so
the process-global active-project slot is isolated (per project CLAUDE.md). The
existing `~/.cargo/bin/codescout` symlink → `target/release/codescout` is reused.

## 5. Components

### 5.1 Install & layout

| Artifact | Location | Purpose |
|---|---|---|
| Pi | `npm i -g --ignore-scripts @earendil-works/pi-coding-agent` | the `pi` binary (Pi's own supply-chain-safe install) |
| pi-mcp-adapter | `pi install npm:pi-mcp-adapter` (pin exact version) | proxy bridge to MCP servers |
| codescout server entry | `~/.pi/agent/mcp.json` | how the adapter spawns codescout |
| `codescout-mode.ts` | `~/.pi/agent/extensions/codescout-mode.ts` | curate-&-substitute extension |
| `AGENTS.md` | `~/.pi/agent/AGENTS.md` | task→tool map read every session |

### 5.2 `~/.pi/agent/mcp.json`

```json
{
  "servers": {
    "codescout": {
      "command": "codescout",
      "args": ["start"],
      "lifecycle": "lazy",
      "directTools": [
        "symbols", "symbol_at", "tree",
        "grep", "semantic_search", "references",
        "read_file", "read_markdown",
        "edit_code", "edit_file", "edit_markdown"
      ]
    }
  }
}
```

The hot-set becomes first-class Pi tools. All other codescout tools
(`artifact*`, `librarian`, `memory`, `workspace`, `run_command`, `index`,
`onboarding`, `peer`, `get_guide`, …) remain behind the single `mcp` proxy tool
— reachable on demand at zero standing token cost. `lifecycle: lazy` defers the
codescout spawn until the first code tool call.

> The exact `args` are the subject of open risk #1 (§7) — `["start"]` may need to
> become `["start", "."]` or a wrapper if codescout does not auto-activate Pi's
> launch cwd. Resolved empirically at setup.

### 5.3 `codescout-mode` extension (Approach A — curate & substitute)

A single small `(pi) => { … }` factory. Three guarded behaviors:

1. **Curate.** In a `session_start` / `resources_discover` handler (i.e. *after*
   the adapter's directTools are registered), call `pi.setActiveTools([...])` to
   **drop Pi's native `edit`** while keeping `bash`, `write`, and Pi's native
   `read`, plus the codescout hot-set. Net effect: the only code-edit tools are
   `edit_code`/`edit_file`/`edit_markdown`; the natural read/search path is
   `symbols`/`read_file`/`grep`/`semantic_search`/`references`.
2. **Nudge `bash`.** A `tool_call` handler that, when `bash` runs
   `rg`/`grep`/`find`/`fd`/`cat` against source, returns a `reason` pointing to
   the codescout equivalent **without** `block` — bash stays fully functional for
   tests/git/build; only source-searching is steered.
3. **Safety no-op.** If the codescout directTools are **not** registered (adapter
   didn't load, non-code dir), the extension does nothing — it must never strip
   `edit` and leave the session with no edit tool.

**Deliberate keeps** (the safe reading of "replace built-ins"): native `read`
stays (codescout cannot read images/dirs/glob); `bash` stays (load-bearing).
This replaces the *code* read/search/edit path, it does not remove Pi's
primitives.

### 5.4 `~/.pi/agent/AGENTS.md`

Compact task→tool map (mirrors codescout's Iron Laws):

- **Read code:** `symbols` (overview) · `symbols include_body` (bodies) ·
  `read_file` (ranges/non-source) · `read_markdown` (markdown). Don't `cat` source.
- **Search:** `grep` (regex) · `semantic_search` (concept) · `references`
  (callers). Don't `rg`/`grep` source via bash.
- **Edit:** `edit_code` (LSP/structural) · `edit_file` (text) · `edit_markdown`
  (markdown) · `write` (new files).
- **Shell:** `bash` for tests/git/build only.
- **Deeper codescout** (artifacts/memory/librarian/workspace): via the `mcp`
  proxy tool on demand.

## 6. Data flow & error handling

- **Direct tool call:** model → adapter → codescout stdio → result. Adapter
  filters content to text+image. Tool errors surface as tool results (Pi feeds
  validation/tool errors back to the model rather than crashing).
- **Proxy call:** model → `mcp` proxy → adapter discovers/forwards.
- **Hook errors:** the `tool_call` handler is written to fail-open (never throw);
  a bug in the nudge must not break tool dispatch.
- **Adapter absent / codescout down:** extension no-ops; Pi keeps its native
  tools so the session stays usable.

## 7. Open risks (verified at setup, not assumed)

1. **🔴 codescout active-project resolution inside the Pi-spawned server.**
   `codescout start` with no project may start unactivated, and the adapter will
   not call `workspace(activate)`. Must confirm codescout adopts Pi's launch cwd;
   if not, fix via `args: ["start", "."]` or a thin wrapper that activates cwd.
   This is the highest-impact failure mode (silently wrong/empty project) and
   gets an explicit verification check.
2. **Load-order vs directTools timing.** `setActiveTools` must run after the
   adapter registers tools — hook `session_start`/`resources_discover`, plus the
   §5.3 no-op guard.
3. **`edit`-drop footgun.** Mitigated by the safety no-op (only curate when
   codescout edit tools are confirmed present).
4. **Supply chain.** Install Pi with `--ignore-scripts`; pin `pi-mcp-adapter` to
   an exact version.
5. **codescout binary freshness.** Pi uses `~/.cargo/bin/codescout` →
   `target/release/codescout`; a stale/missing symlink loads old code. Verify the
   symlink before first run.

## 8. Verification plan (definition of done)

Dogfood inside the codescout repo. Launch `pi`, then confirm:

1. `/mcp` panel shows codescout **connected** with the hot-set present.
2. "Show the symbols in `src/main.rs`" → Pi calls `symbols`, not `cat`.
3. "Find references to <symbol>" → Pi calls `references`, not bash `grep`.
4. A structural edit → Pi calls `edit_code`.
5. Pi's native `edit` is **absent** from the active tool list; `read`/`write`/`bash` present.
6. `bash` still runs the test suite; the bash source-grep **nudge** fires on `rg src/`.
7. **Risk #1 check:** from inside Pi, confirm codescout reports the codescout
   repo as the active project (e.g. via a proxy `workspace status` call).

## 9. Deferred / future (explicitly out of v1)

- Wrap codescout's librarian/artifact CLI (`codescout artifact …`, `doctor`,
  `audit-doc-refs`) as native Pi tools via `pi.registerTool` + `pi.exec` — the
  Pi-native path for the surface that *does* have a CLI.
- Harden the bash nudge into a selective `block` once false-positive patterns are
  understood.
- Package the config + extension as a shareable `pi-codescout` package.
