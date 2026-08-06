# Project Memory Index

## architecture-snow-lion

- [outputguard-cross-cutting-law](architecture-snow-lion/outputguard-cross-cutting-law.md) — every variable-output tool must flow through OutputGuard; bypass is a defect
- [tool-registration-rule-of-three](architecture-snow-lion/tool-registration-rule-of-three.md) — flat collections favored over registry abstractions until 3rd entry earns extraction
- [agentic-surface-as-moat](architecture-snow-lion/agentic-surface-as-moat.md) — LLM-facing surface is the moat; weight surface changes heavier than backend
- [tracker-as-augmented-artifact](architecture-snow-lion/tracker-as-augmented-artifact.md) — some docs are stateful artifacts with prompt+params; check before editing
- [cross-cutting-side-effects-at-the-chokepoint](architecture-snow-lion/cross-cutting-side-effects-at-the-chokepoint.md) — side-effects live at the operation's chokepoint, gated; audit entry points with references(), not the call site in front of you
- [platform-law-leaks-at-call-sites](architecture-snow-lion/platform-law-leaks-at-call-sites.md) — any "eliminate subprocess X" law (replace with a library binding OR env-derivation; platform builders, libgit2, go-env) leaks at out-of-file sibling call sites (Drop impls, hot + cold paths); grep the whole tree, convert all siblings in one pass

- [codescout-observability-three-layers](architecture-snow-lion/codescout-observability-three-layers.md) — three composable observability layers (usage.db→analyze-usage, llm-proxy/Langfuse+JSONL→claude-traces, Arize→arize-logs); route to the owner, don't build a fourth; claude-traces is the Headroom trial's analysis surface
- [repair-and-continue-input-law](architecture-snow-lion/repair-and-continue-input-law.md) — deterministic input mistakes are repaired + noted, not errored (saves the retry LLM call); RecoverableError only for missing/ambiguous; writes never auto-guessed
## docs-lotus-frog

- [experimental-docs-lifecycle](docs-lotus-frog/experimental-docs-lifecycle.md) — new-subsystem pages go straight into the main manual with a uniform unreleased callout; staging-then-move retired at 0/62 compliance; callout comes off at release, not at merge
- [release-notes-soul](docs-lotus-frog/release-notes-soul.md) — three-act structure (compression → retrieval → evals) and the *codescout-grades-codescout* soul line for release notes

## common

- [dont-fabricate-commit-rationale](common/dont-fabricate-commit-rationale.md) — never invent the "why" in commit messages; state only what changed when the rationale isn't documented
