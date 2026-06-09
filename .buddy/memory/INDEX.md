# Project Memory Index

## architecture-snow-lion

- [outputguard-cross-cutting-law](architecture-snow-lion/outputguard-cross-cutting-law.md) — every variable-output tool must flow through OutputGuard; bypass is a defect
- [tool-registration-rule-of-three](architecture-snow-lion/tool-registration-rule-of-three.md) — flat collections favored over registry abstractions until 3rd entry earns extraction
- [agentic-surface-as-moat](architecture-snow-lion/agentic-surface-as-moat.md) — LLM-facing surface is the moat; weight surface changes heavier than backend
- [tracker-as-augmented-artifact](architecture-snow-lion/tracker-as-augmented-artifact.md) — some docs are stateful artifacts with prompt+params; check before editing
- [cross-cutting-side-effects-at-the-chokepoint](architecture-snow-lion/cross-cutting-side-effects-at-the-chokepoint.md) — side-effects live at the operation's chokepoint, gated; audit entry points with references(), not the call site in front of you

## docs-lotus-frog

- [experimental-docs-lifecycle](docs-lotus-frog/experimental-docs-lifecycle.md) — experimental doc creation, removal, and graduation checklist for this repo
- [release-notes-soul](docs-lotus-frog/release-notes-soul.md) — three-act structure (compression → retrieval → evals) and the *codescout-grades-codescout* soul line for release notes

## common

- [dont-fabricate-commit-rationale](common/dont-fabricate-commit-rationale.md) — never invent the "why" in commit messages; state only what changed when the rationale isn't documented
