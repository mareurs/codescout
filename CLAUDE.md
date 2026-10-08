# codescout

Rust MCP server giving LLMs IDE-grade code intelligence — symbol-level navigation, semantic search, git integration. Inspired by [Serena](https://github.com/oraios/serena).

You are a proficient Rust developer. You follow all known good/scalable patterns. You are honest and recognize your limits and your mistakes, you own them. If you are not sure, you always ask me for feedback.

## What belongs in this file

A line earns residency here only if a session that never opens another doc would act wrongly without it. Measurements, incident history, derivations and attributions belong in `docs/conventions/<topic>.md` or the tracker that holds them; leave a one-line pointer under the same heading, because scripts and other docs cite this file's sections by name. Tracker promotion paths end in `docs/conventions/`, not here.

`claude_md_stays_within_its_byte_budget` (`src/prompts/mod.rs`) caps this file. The budget only moves down, and when it reds the fix is to move text out, not to raise the number.

## Development Commands

**Run `./scripts/gate.sh`: Python tests, then `./scripts/fmt-mine.sh`, `cargo clippy --workspace --all-targets --features local-embed -- -D warnings`, `cargo test --workspace --no-default-features`, `cargo test --workspace` in a `target/` leased for that run from a pool, before completing any task.** For a targeted `cargo test`, use `scripts/with-slot.sh cargo test …`. Why those four in that order → [`docs/conventions/gate-ordering.md`](docs/conventions/gate-ordering.md); what they do not cover is printed by `gate.sh` on every run. Live-MCP release build: `./scripts/rb.sh`, then `/mcp` (more → memory `development-commands`).

## Testing Discipline — what a green suite is evidence for

The laws for reading a green or red result → [`docs/conventions/testing-discipline.md`](docs/conventions/testing-discipline.md); their derivations → [`docs/conventions/what-green-is-evidence-for.md`](docs/conventions/what-green-is-evidence-for.md). Run mutation tests with `./scripts/mutation-probe.sh` ([`docs/PROBES.md`](docs/PROBES.md)).

## Bug Tracking

Every bug noticed during work gets its own file in `docs/issues/`, copied from [`docs/issues/_TEMPLATE.md`](docs/issues/_TEMPLATE.md), which holds the procedure.

## Session Intelligence Trackers

Every tracker prefix, with its file, append call and promotion path → [`docs/TAXONOMY.md`](docs/TAXONOMY.md). Conventions (entry ids, archiving, params writes) → `get_guide("tracker-conventions")` and `get_guide("librarian")`.

## Git Workflow

Branch policy, the release and ship sequence, citing a fix (SHA + patch-id), and the rules for committing and pushing on this shared checkout → [`docs/RELEASE.md`](docs/RELEASE.md); the step-by-step commit sequence → [`docs/conventions/shared-checkout-commit-sequence.md`](docs/conventions/shared-checkout-commit-sequence.md). Commit style → memory `conventions`; cross-repo `<repo>:<sha>` citations → memory `gotchas`.

## Reaching a Peer Session — address by scope, not by the list you were handed

Before any peer count or peer routing is load-bearing, run `/codescout-companion:reaching-peer-sessions`. The rules and measurements → [`docs/conventions/reaching-a-peer-session.md`](docs/conventions/reaching-a-peer-session.md).

## Observer Blindness — when care is the wrong instrument

The rule, its three-part remedy and the measurements → [`docs/conventions/observer-blindness-long-form.md`](docs/conventions/observer-blindness-long-form.md). Classes are recorded as `OB-N` in [`docs/trackers/observer-blindness.md`](docs/trackers/observer-blindness.md).

## Parsers Over a Namespace — owe an escape and a disambiguator

The rule and its corollary for recorded history → [`docs/conventions/parsers-over-a-namespace.md`](docs/conventions/parsers-over-a-namespace.md).

## Design Principles

Conventions and architecture → memories `conventions` and `architecture`. Before adding or modifying a tool, read [`docs/PROGRESSIVE_DISCOVERABILITY.md`](docs/PROGRESSIVE_DISCOVERABILITY.md), and for one that can return a negative result, [`docs/adrs/2026-08-27-negative-results-name-their-scope.md`](docs/adrs/2026-08-27-negative-results-name-their-scope.md). Errors → `get_guide("error-handling")`; test isolation → [`docs/conventions/test-env-isolation.md`](docs/conventions/test-env-isolation.md).

## Prompt Surface Consistency

The prompt surfaces (the `src/prompts/` ones and `.codescout/system-prompt.md`), when to bump `ONBOARDING_VERSION`, the slice cap and the style guide → [`src/prompts/README.md`](src/prompts/README.md).

## Companion Plugin: codescout-companion

`codescout-companion` (`../claude-plugins/codescout-companion/`) is always active here, and its hooks print `[cs-hint]` advisories. Hook inventory and cross-repo flow → [`docs/architecture/companion-plugin.md`](docs/architecture/companion-plugin.md).

## Language-Specific LSP Issues

See codescout memory `gotchas` (LSP section) for Kotlin multi-instance conflicts,
cold start behavior, circuit breaker, and LSP mux details.

## Docs

- [`docs/PROBES.md`](docs/PROBES.md) — every measurement instrument and its blind spot; check it before answering with a number.
- [`docs/conventions/cross-machine-catalog-resume.md`](docs/conventions/cross-machine-catalog-resume.md) — run after pulling onto a machine that has not been building codescout.
- `docs/manual/src/architecture.md` — components, tech stack, design principles.
- `docs/ROADMAP.md` — status overview.
- `CONTRIBUTING.md` — contributor setup and PR checklist.
