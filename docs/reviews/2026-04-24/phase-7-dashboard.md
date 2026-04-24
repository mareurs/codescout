# Phase 7 — Dashboard

**Date:** 2026-04-24
**Scope:** `src/dashboard/`, `src/dashboard/api/`, `src/dashboard/static/`
**Reviewer:** superpowers:code-reviewer + buddy:security-ibex
**Status:** open

---

## Cross-check answers (Phase 1, 5, 6)

- **Phase 1 S1 (weak auth-token fallback):** **Refute — different surface, but worse.** Dashboard has its own server (`src/dashboard/mod.rs:14`, `src/dashboard/routes.rs:20`); zero matches for `auth|token|bearer|Authorization` in `src/dashboard/`. **No auth at all.** Mitigated only by default `--host 127.0.0.1` (`src/main.rs:68`) and CORS pinned to bound port (`routes.rs:27-32`). → S1-DASH below.
- **Phase 1 S4 (verbose anyhow leaked):** **Confirm, broader surface.** Multiple endpoints stringify internal errors verbatim into JSON: `lsp.rs:32, 47`, `usage.rs:29, 44`, `memories.rs:28, 39, 60, 67, 87`. → S2-DASH.
- **Phase 5 S1/S2/C1/C2 (embed/library):** Dashboard exposes embeddings DB read-only via `/api/index`, `/api/drift`; libraries via `/api/libraries`. No write paths. `/api/libraries` returns absolute `path` (`libraries.rs:17`) → I1-DASH.
- **Phase 6 (uncached `Repository::discover` at `dashboard/api/project.rs:45`):** **Confirm.** Re-discovers on every poll (`POLL_INTERVAL` in `dashboard.js:4`). → P1-DASH.

---

## Security (Ibex)

### S1-DASH — MEDIUM — Dashboard exposes unauthenticated read+write+delete API; safety hinges entirely on host flag
- **Location:** `src/dashboard/routes.rs:34-66` (router, no auth layer); `src/main.rs:67-68` (`--host` user-overridable).
- **Evidence:** `build_router` registers `GET/POST/DELETE /api/memories/...`, `/api/project`, `/api/config`, `/api/index`, `/api/drift`, `/api/usage`, `/api/lsp`, `/api/errors`, `/api/libraries` with no auth middleware — only `CorsLayer`. CLI permits `--host 0.0.0.0`.
- **Exploit:** User runs `codescout dashboard --host 0.0.0.0` on coffee-shop Wi-Fi → anyone on LAN can `curl -X DELETE http://victim:8099/api/memories/architecture` to wipe memories, `POST` arbitrary content, `GET /api/project|/api/libraries` to scrape paths/config. CORS doesn't protect non-browser clients (curl ignores it).
- **Fix:** (a) Hard-bind 127.0.0.1 and refuse non-loopback unless `--token` provided + enforced via `RequireAuthorizationLayer::bearer`, OR (b) generate random token on startup, print URL with token, enforce in middleware. Add startup warning when host non-loopback. Reuse the MCP HTTP transport's bearer machinery (Phase 1 S1).
- **Confidence:** high.

### S2-DASH — LOW — Internal error chains leaked via JSON
- **Location:** `lsp.rs:32, 47`; `usage.rs:29, 44`; `memories.rs:28, 39, 60, 67, 87`.
- **Evidence:** `Json(json!({ "error": e.to_string() }))`. `MemoryStore::open` errors include absolute paths; index/usage paths surface DB filenames + SQLite errors verbatim.
- **Exploit:** Combined with S1-DASH, unauthenticated reader gets `~/projects/foo/.codescout/memories/...` paths and SQLite version/schema for chained recon.
- **Fix:** Log details via `tracing::warn!`; return generic `{"error": "internal"}`. Add `into_safe_response()` helper for consistency.
- **Confidence:** high.

### S3-DASH — LOW — Drift `threshold` parameter unbounded
- **Location:** `src/dashboard/api/index.rs:54-72`.
- **Evidence:** `threshold: Option<f32>` consumed verbatim into `query_drift_report`. Negative, NaN, `INFINITY` flow through.
- **Fix:** Clamp to `0.0..=1.0` or `is_finite()` check.
- **Confidence:** medium.

### S4-DASH — LOW (debug-only) — `serve_index` reads from CWD with relative path
- **Location:** `src/dashboard/routes.rs:80-91, 93-99, 101-107`.
- **Evidence:** Debug-build reads `"src/dashboard/static/index.html"` relative to CWD. Production embeds via `include_str!`. Already cfg-gated.
- **Fix:** Anchor to `CARGO_MANIFEST_DIR` for less surprise. Optional.
- **Confidence:** high (genuinely debug-only).

### Q1 — Stored XSS via memory topic name (chain depends on S1-DASH)
- **Location:** `src/dashboard/static/dashboard.js:316` (`data-topic="' + esc(t) + '"'`); `dashboard.js:47-52` (`esc` strips `& < >` only, not `"` or `'`).
- **Evidence:** `sanitize_topic` (`src/memory/mod.rs:131-146`) keeps `Component::Normal` segments only — `"` is legal Linux filename char. Topic `foo" onclick="alert(1)` survives sanitization, persists to disk as `.md`, echoes via `/api/memories`, breaks out of `data-topic` attribute → stored XSS.
- **Reachability:** requires write access to `/api/memories/{topic}` — gated by S1-DASH. If S1-DASH is unfixed and `--host 0.0.0.0`, real stored XSS. If localhost-only, attacker needs RCE first → meaningless.
- **Fix:** (a) Escape `"` and `'` in `esc()` (`.replace(/"/g,'&quot;').replace(/'/g,'&#39;')`), OR (b) build DOM via `createElement` + `textContent`, OR (c) tighten `sanitize_topic` to strict `[A-Za-z0-9._ -]+`. (c) is cleanest.
- **Severity:** LOW–MEDIUM if S1-DASH unfixed; LOW defense-in-depth otherwise.

### Cleared (checked, not flagged)
- Path traversal via `Path(topic)`: defended by `sanitize_topic`.
- CORS: pinned to localhost+bound port (not `*`); tests `cors_rejects_external_origin` + `cors_rejects_wrong_port` enforce.
- CSRF on memory mutations: blocked by CORS pinning + `application/json` preflight requirement.
- XSS in static HTML: no inline event handlers, no inline scripts beyond loader.
- SQL injection: parameterised via `embed_index` / `usage::db` helpers.

---

## Critical (non-security)
None.

---

## Important (non-security)

### P1-DASH — `/api/project` re-discovers git repo on every poll
- **Location:** `src/dashboard/api/project.rs:7-26, 44-56`.
- **Evidence:** `git_info` → `Repository::discover` per call. Dashboard JS polls overview every `POLL_INTERVAL` and on tab switch.
- **Fix:** Cache `git_branch` + dirty status with 1-5s TTL or file-watcher invalidation. (Cross-confirms Phase 6 finding.)

### I1-DASH — `/api/libraries` and `/api/project` leak absolute filesystem paths
- **Location:** `libraries.rs:17` (`e.path.display()`); `project.rs:21` (`root.display()`).
- **Evidence:** With S1-DASH unfixed + `--host 0.0.0.0`, LAN sees project owner's home directory layout.
- **Fix:** Strip to basename / display-relative path for non-loopback context, or omit entirely.

### I2-DASH — Chart.js loaded from CDN with no SRI hash
- **Location:** `index.html:8` `<script src="https://cdn.jsdelivr.net/npm/chart.js@4">`.
- **Evidence:** No `integrity=`, no `crossorigin=`. CDN compromise → arbitrary JS in dashboard origin → reads/writes all unauthenticated APIs.
- **Fix:** Bundle Chart.js into `static/` (treated like embedded CSS/JS) OR add `integrity="sha384-..." crossorigin="anonymous"`.

### I3-DASH — No request body size limit, no per-request timeout
- **Location:** `routes.rs:20-67` — `CorsLayer` only.
- **Evidence:** Axum default `DefaultBodyLimit` is 2 MiB per-extractor, easy to overlook. `POST /api/memories/{topic}` accepts arbitrarily-large bodies if not pinned.
- **Fix:** `tower_http::limit::RequestBodyLimitLayer` (1 MiB on memories) + `tower_http::timeout::TimeoutLayer` (30s).

---

## Minor (grouped)

- No `X-Content-Type-Options: nosniff`, no CSP. `default-src 'self' https://cdn.jsdelivr.net; script-src 'self' https://cdn.jsdelivr.net` would blunt XSS slipping past `esc`.
- `dashboard.js:171` and similar concat HTML via `innerHTML` even where DOM construction would be safer. Codebase-wide pattern is a footgun (Q1).
- `lsp.rs:40`, `usage.rs:37` — `unwrap_or_default()` on `serde_json::to_value(stats)` silently produces `null` on serialization fail. Add `tracing::error!`.
- `/api/health` no payload schema in tests (`routes.rs:69-71`). Note only.
- `config.rs:10` `Json(serde_json::to_value(config).unwrap_or_default())` — entire project config dumped to anyone hitting `/api/config`. Today no secrets; future additions (LLM keys) would auto-leak. Consider explicit `PublicConfig` projection.

---

## Open questions

1. **Intended deployment model.** Strictly localhost / single-user, or `--host 0.0.0.0` for team dashboards / remote dev? Determines S1-DASH severity. If "loopback only, period," then S1-DASH drops to LOW and right fix is removing `--host` knob (or `--insecure-network` opt-in flag with printed token).
2. **Tighten `sanitize_topic` to `[A-Za-z0-9._ -]+`?** Closes Q1's XSS chain at source, removes class of fragile-escaping bugs from frontend.
3. **CLAUDE.md compliance:** `POST /api/memories/{topic}` returns `(StatusCode::OK, Json(json!({"status": "ok"})))` (`memories.rs:64`). Spec says writes return `json!("ok")`. `{"status":"ok"}` is structure-echo for zero info gain. Not security; CLAUDE.md.
