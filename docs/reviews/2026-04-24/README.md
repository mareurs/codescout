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
| 6 | Git | `d618b9d` | 3 minors + Q1 doc | Q2 ceiling_dirs, I5 Repository cache (perf) |
| 7 | Dashboard | `d465bd6` | S2, S3, S4, I3, Q1(esc) + spec compliance | S1 (auth), I1, I2 (SRI), P1 (cache), Q1 (sanitize tighten), minors |
| 8 | Prompts + MCP resources | `976a5e1` | S3 SAFETY, I1 scope-doc, I2 lib routing | S1 repo-prompt trust, S2 prefs gating, I3 hoist, minors |
| 9 | Cross-cutting | `b0da313` | S9-1 denylist expand, I9-3 canon, I9-2 cfg | S9-3, C9-2, I9-1, I9-2 sig, minors (S9-2, C9-1 already done) |

**All 9 phases shipped.** Remaining work lives in
[review-residuals.md](review-residuals.md).

### Resuming work in a new session

All 9 phases have landed on `review/2026-04-24`. Next session picks up with:

1. Checkout the branch: `git checkout review/2026-04-24`
2. Skim this README + [review-residuals.md](review-residuals.md) for context.
3. Decide priority from the residuals buckets:
   - **Needs user decision before code:** phase-7 S1 (auth), phase-8 S1
     (repo-prompt trust), phase-8 S2 (preferences gating), C9-2
     (containment redesign).
   - **Ready to land once someone profiles:** phase-6 I5 + phase-7 P1
     (Repository cache).
   - **Windows-specific, needs a Windows dev pass:** S9-3, I9-1, phase-5 S3.
   - **Refactors / polish:** phase-8 I3 hoist, `path_security.rs` split,
     various minors.
4. Implementation loop (same as prior phases):
   `cargo fmt` + `cargo clippy --lib -- -D warnings` + `cargo test --lib`
   before each commit; one focused commit per residual item.
5. **When residuals are either landed or closed-as-won't-fix**:
   run the Standard Ship Sequence — cherry-pick each of the 9 phase
   commits from `review/2026-04-24` onto `master`, push, then rebase
   `experiments` on the new `master`.
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
