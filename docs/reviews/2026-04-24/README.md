# Codebase Review — 2026-04-24

Whole-state quality + security audit of codescout, run in 9 phases. Each phase = one `superpowers:code-reviewer` dispatch with `buddy:security-ibex` loaded for security findings.

## Phases

| # | Phase | Scope | Status | File |
|---|-------|-------|--------|------|
| 1 | Core / server / agent | `lib.rs`, `main.rs`, `server.rs`, `workspace.rs`, `agent/`, `config/` | done | [phase-1-core-server-agent.md](phase-1-core-server-agent.md) |
| 2 | Tools layer | `tools/` (29 tools, ex `tools/symbol/`) | done | [phase-2-tools.md](phase-2-tools.md) |
| 3 | LSP integration | `lsp/`, `lsp/mux/`, `lsp/servers/` | done | [phase-3-lsp.md](phase-3-lsp.md) |
| 4 | AST + symbols | `ast/`, `symbol/`, `tools/symbol/` | done | [phase-4-ast-symbols.md](phase-4-ast-symbols.md) |
| 5 | Embeddings + memory + library | `embed/`, `memory/`, `library/` | done | [phase-5-embed-memory-library.md](phase-5-embed-memory-library.md) |
| 6 | Git | `git/` (gh CLI tools covered in Phase 2) | done | [phase-6-git.md](phase-6-git.md) |
| 7 | Dashboard | `dashboard/` (web UI) | done | [phase-7-dashboard.md](phase-7-dashboard.md) |
| 8 | Prompts + MCP resources | `prompts/`, `mcp_resources/` | done | [phase-8-prompts-mcp-resources.md](phase-8-prompts-mcp-resources.md) |
| 9 | Cross-cutting | `platform/`, `util/`, `usage/`, `logging.rs`, `hardware.rs` | done | [phase-9-cross-cutting.md](phase-9-cross-cutting.md) |

---

## Fix Status (as of 2026-04-24)

All nine phase audits are complete; the tracker below shows **fix progress**,
not review progress. Fixes landed on branch `review/2026-04-24` as one commit
per phase. Items that were deferred are captured in
[review-residuals.md](review-residuals.md) with per-item unblock checklists.

| # | Phase | Fix Commit | Landed | Deferred |
|---|-------|------------|--------|----------|
| 1 | Core / server / agent | `8773432` | 14/14 | — |
| 2 | Tools layer | `4038036` | 10/15 | F4, C5, I5 + 3 minors |
| 3 | LSP integration | `50509fb` | 9/15 | S1, S2, C2, I3, I1, I4, I6 + 7 minors |
| 4 | AST + symbols | `2682548` | 9/9 + 4 minors | S2, M4, M5, M6, M9 + open Q1–Q5 |
| 5 | Embeddings + memory + library | `8b041d4` | 10/10 | S3, phase-5 minors, Q1–Q4 |
| 6 | Git | pending | 3 minors + Q1 doc | Q2 ceiling_dirs, I5 cache (perf) |
| 7 | Dashboard | pending | S2, S3, S4, I3, Q1(esc) + 2 minors | S1 (auth), I1, I2 (SRI), P1 (cache), Q1 (sanitize tighten) |
| 8 | Prompts + MCP resources | pending | S3 SAFETY, I1 scope-doc, I2 lib routing | S1 repo-prompt trust, S2 prefs gating, I3 hoist, minors |
| 9 | Cross-cutting | — | 0/? | not started |

### Resuming work in a new session

1. Checkout the branch: `git checkout review/2026-04-24`
2. Skim this README + [review-residuals.md](review-residuals.md) for context.
3. Pick the next phase (4 unless priorities shifted).
4. Read the phase file, propose a green/yellow split (safe fixes vs deferred),
   get user approval, then implement.
5. Pattern per phase: cargo fmt + `cargo clippy --lib -- -D warnings` +
   `cargo test --lib` before committing.
6. One commit per phase with detailed body enumerating landed + deferred
   items. Co-author line: `Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>`.
7. After all phases: Standard Ship Sequence (cherry-pick each phase commit
   to master, push, rebase experiments).

### Open cross-cutting decisions

These surfaced during phase 2/3 and should be resolved before phase 4 starts
depending on scope:

- **F4** read-path containment: does `read_file("/home/user/.aws/credentials")`
  count as legitimate cross-project nav, or is this a leak to close? Affects
  how phase 3 S2 is ultimately fixed too.
- **C4 companion**: `insert_code`-via-`edit_file.append` is the legitimate
  "add new symbol" path per phase 2 telemetry. Phase 4 will likely have
  opinions about whether `insert_code` should be the blessed entry point
  for net-new definitions.
## Triage convention

Per phase file: findings grouped Security (Ibex S#) / Critical (C#) / Important (I#) / Minor / Questions. Each finding has Location, Evidence, Fix, Confidence.

After all 9 phases done: consolidate into a fix-priority backlog at the workspace level.
