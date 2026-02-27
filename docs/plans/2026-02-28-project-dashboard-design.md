# Project Dashboard — Design

> **ORA-6** | Unified web dashboard for project health, tool stats, and configuration management.

## Overview

A standalone CLI subcommand (`cargo run -- dashboard`) that launches a lightweight axum HTTP server, serving a single-page web application. The dashboard reads the same project files as the MCP server (`.code-explorer/project.toml`, `embeddings.db`, `usage.db`, memories, libraries) and presents them in a browser-based UI with charts and management features.

**Key decision:** The dashboard is a **separate process**, not embedded in the MCP server. This avoids overhead during normal MCP usage (99%+ of runtime) while providing full observability on demand.

---

## Delivery Model

### CLI subcommand

```
cargo run -- dashboard --project . [--host 127.0.0.1] [--port 8099] [--open]
```

- Adds a `Dashboard` variant to the `Commands` enum in `main.rs`
- Spins up an axum HTTP server on `127.0.0.1:8099` (configurable)
- `--open` (default: true) auto-launches the browser via the `open` crate
- Ctrl+C shuts it down cleanly
- Does NOT start MCP protocol, LSP servers, or tool machinery
- Read-only observer of project state (files/DBs written by the MCP server)

### Data access pattern

The dashboard imports and calls the same library functions the MCP tools use:

| Data | Function | Source file |
|------|----------|-------------|
| Project config | `ProjectConfig::load()` | `config/mod.rs` |
| Language detection | `ast::detect_language()` | `ast/mod.rs` |
| Index stats | `embed::index::get_stats()` | `embed/index.rs` |
| Drift scores | `embed::index::find_drifted()` | `embed/index.rs` |
| Tool usage | `usage::db::query_stats()` | `usage/db.rs` |
| Recent errors | `usage::db::recent_errors()` | `usage/db.rs` |
| Memories | `memory::MemoryStore` | `memory/mod.rs` |
| Libraries | `library::LibraryRegistry` | `library/mod.rs` |

No data duplication — the dashboard module is a thin HTTP layer over existing library code.

---

## Architecture

### Module layout

```
src/dashboard/
    mod.rs           — pub async fn serve(root, host, port, open) → Result<()>
    routes.rs        — axum Router: static file serving + JSON API routes
    api/
        mod.rs       — re-exports
        project.rs   — GET /api/project
        config.rs    — GET /api/config
        index.rs     — GET /api/index
        usage.rs     — GET /api/usage?window=30d
        errors.rs    — GET /api/errors?limit=20
        memories.rs  — GET/POST/DELETE /api/memories
        libraries.rs — GET /api/libraries

src/dashboard/static/       — embedded in binary via include_str!
    index.html              — SPA shell
    dashboard.js            — page navigation, polling, chart rendering
    dashboard.css           — styling (light/dark theme)
```

### Dependencies

New crates (behind `dashboard` feature, default: on):

| Crate | Version | Purpose |
|-------|---------|---------|
| `axum` | 0.8 | HTTP framework (Tokio-native) |
| `tower-http` | 0.6 | Static file serving, CORS |
| `open` | 5 | Browser auto-launch |

### Feature flag

```toml
[features]
default = ["remote-embed", "dashboard"]
dashboard = ["dep:axum", "dep:tower-http", "dep:open"]
```

Users who want a minimal MCP-only binary can `--no-default-features`.

---

## API Endpoints

All endpoints return JSON. Prefix: none (served from root).

### Phase 1 (no dependencies)

| Method | Path | Response |
|--------|------|----------|
| GET | `/api/health` | `{"status": "ok"}` |
| GET | `/api/project` | `{"name", "root", "languages", "git_branch", "git_dirty"}` |
| GET | `/api/config` | Full `ProjectConfig` as JSON |
| GET | `/api/index` | `{"file_count", "chunk_count", "last_indexed", "staleness"}` |
| GET | `/api/drift?threshold=0.1` | `{"files": [{"path", "avg_drift", "max_drift"}]}` |

### Phase 2 (depends on Tool Usage Monitor / ORA-7)

| Method | Path | Response |
|--------|------|----------|
| GET | `/api/usage?window=30d` | Same shape as `get_usage_stats` tool output |
| GET | `/api/errors?limit=20` | `{"errors": [{"tool", "timestamp", "message"}]}` |

### Phase 3 (management features)

| Method | Path | Response |
|--------|------|----------|
| GET | `/api/memories` | `{"topics": ["architecture", "conventions", ...]}` |
| GET | `/api/memories/:topic` | `{"topic", "content"}` |
| POST | `/api/memories/:topic` | Body: `{"content": "..."}` → 200 OK |
| DELETE | `/api/memories/:topic` | 200 OK |
| GET | `/api/libraries` | `{"libraries": [{"name", "path", "language", "indexed"}]}` |

---

## Frontend Design

### Stack

- **Vanilla JS** — no framework (same approach as Serena)
- **Chart.js** from CDN (`cdn.jsdelivr.net`) — bar/pie charts for tool stats
- **Custom CSS** — CSS variables for light/dark theme, responsive cards
- **No jQuery** — modern `fetch()` + `document.querySelector()`

### Pages

**Overview** (default landing):
- Project card: name, root, languages, git branch/dirty status
- Config card: embedding model, chunk size, security mode, ignored paths
- Index card: file/chunk counts, last indexed, staleness indicator (green/yellow/red dot)
- Drift summary: top 5 files by drift score (if threshold exceeded)

**Tool Stats** (Phase 2):
- Window selector: `1h` / `24h` / `7d` / `30d` radio buttons
- Summary bar: total calls, error rate %, overflow rate %
- Bar chart: calls per tool (sorted desc)
- Table: per-tool rows with calls, errors, error%, overflows, overflow%, p50, p99
- Recent errors list: last 20 errors with timestamp, tool, message

**Memories** (Phase 3):
- Topic list (left sidebar)
- Content viewer (right panel, rendered as preformatted text)
- Edit button → textarea with save/cancel
- Delete button with confirmation
- Create new memory button

### Behavior

- Auto-polls `/api/*` every 5 seconds
- "Last refreshed: Xs ago" indicator
- Light/dark theme toggle (persisted in `localStorage`)
- Graceful degradation: if `usage.db` doesn't exist, Tool Stats page shows a helpful message

### Static file embedding

```rust
#[cfg(not(debug_assertions))]
const INDEX_HTML: &str = include_str!("static/index.html");

#[cfg(debug_assertions)]
fn index_html() -> String {
    std::fs::read_to_string("src/dashboard/static/index.html")
        .unwrap_or_else(|_| "Dashboard static files not found".into())
}
```

Release builds embed everything in the binary. Debug builds read from filesystem for fast iteration.

---

## Error Handling

| Condition | Behavior |
|-----------|----------|
| No `usage.db` | Phase 2 APIs return `{"available": false, "reason": "..."}`. Frontend shows message. |
| No `embeddings.db` | Index API returns `{"available": false, "reason": "No index. Run code-explorer index."}` |
| No `project.toml` | Config API returns defaults with `"custom": false` |
| Port in use | Try port+1 through port+10, then error with clear message |
| Malformed DB | Return 500 with error message. Frontend shows error banner. |

---

## Phasing

### Phase 1 — Project overview (no external dependencies)

- Dashboard CLI subcommand + axum server scaffold
- Static file serving (embedded HTML/CSS/JS)
- API: `/api/health`, `/api/project`, `/api/config`, `/api/index`, `/api/drift`
- Frontend: Overview page with all Phase 1 cards
- Light/dark theme, auto-refresh

### Phase 2 — Tool statistics (depends on ORA-7)

- API: `/api/usage`, `/api/errors`
- Frontend: Tool Stats page with charts and error log
- `recent_errors()` query in `usage/db.rs`

### Phase 3 — Management features

- API: `/api/memories` CRUD, `/api/libraries`
- Frontend: Memories page with viewer/editor
- Libraries section on Overview page

---

## What This Unblocks

- Human-readable project health checks without running MCP tools
- Visual tool usage analysis (which tools are noisy, slow, or failing)
- Memory management without CLI/MCP
- Demo-friendly: `cargo run -- dashboard` is a one-command showcase

---

*Created: 2026-02-28*
*Depends on: ORA-7 (Tool Usage Monitor) for Phase 2*
