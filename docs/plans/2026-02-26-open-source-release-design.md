# Open Source Release Design

*Date: 2026-02-26*
*Repo: mareurs/code-explorer*

## Context

code-explorer is a Rust MCP server giving LLMs IDE-grade code intelligence — LSP
navigation, semantic search, git integration, persistent memory. All 29 tools are
implemented, 131+ tests passing. Time to publish on GitHub.

## Decisions

| Decision | Choice | Rationale |
|---|---|---|
| Installation | `cargo install` only | Growth-stage, add prebuilt binaries later |
| CI | fmt + clippy + test (3 feature combos) + MSRV | Feature flags need matrix coverage |
| PR process | CI must pass + 1 reviewer (maintainer) | Light review, PR template for structure |
| README style | Problem-centric | Most visitors won't know what MCP servers are |
| Contributor bar | Low, Claude Code PRs welcomed | Grow community first, tighten later |
| Skills | Deferred | Write after real contributors reveal friction |
| License | MIT | Already declared in Cargo.toml |

## Deliverables

### Layer 1: Foundation

- **README.md** — problem statement, quick start, 29-tool table by category, architecture
  overview, configuration basics, companion plugin reference, feature matrix vs competitors,
  link to CONTRIBUTING.md
- **LICENSE** — MIT with copyright holder
- **.gitignore** — remove `/serena-as-reference/`, add `/docs/observations.md`, keep
  `/docs/research/` private
- **Delete** `serena-as-reference/` directory

### Layer 2: CI

- **`.github/workflows/ci.yml`** — single workflow, triggers on push to main + all PRs
- Jobs:
  - `fmt`: `cargo fmt --check`
  - `clippy`: `cargo clippy -- -D warnings`
  - `test-default`: `cargo test` (default features = remote-embed)
  - `test-local-embed`: `cargo test --features local-embed --no-default-features`
  - `test-no-features`: `cargo test --no-default-features`
  - `msrv`: build with minimum supported Rust version (verify actual MSRV)
- Linux only, no cross-platform matrix

### Layer 3: Contributor Experience

- **CONTRIBUTING.md** — getting started, making changes, what to contribute, Claude Code
  PRs explicitly welcomed
- **`.github/pull_request_template.md`** — what / why / testing sections
- **`.github/ISSUE_TEMPLATE/bug_report.md`** — what happened, expected, reproduce steps
- **`.github/ISSUE_TEMPLATE/feature_request.md`** — what and why

### Layer 4: Docs Polish

- **ARCHITECTURE.md** — verify current with 29-tool final state
- **ROADMAP.md** — update "What's Built", add "What's Next" section
- **CLAUDE.md** — no changes needed

## Explicitly Not Included

- Prebuilt binaries / cargo-dist
- Cross-platform CI (macOS, Windows)
- README badges
- Screenshots (no visual UI)
- Contributor skills (deferred until real friction is observed)
- Conventional commits enforcement
- Signed commits
- Code coverage reporting
- Security audit workflow
