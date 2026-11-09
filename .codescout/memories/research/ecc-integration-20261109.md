# ECC integration assessment — 2026-11-09

## Scope and provenance

User requested exploring https://github.com/affaan-m/ecc for integration with codescout and this setup, then explicitly requested cloning it under ~/work/claude. Cloned to /home/marius/work/claude/ecc. Observed HEAD d2ef2680594f9a17526a6e1a2e18b3d242c5d17b (2026-11-06); .codex-plugin/plugin.json version 2.2.3. git status --short was empty after inspection. No ECC installer, npm install, hooks or configuration changes were run. This is a static integration assessment, not a successful runtime-integration experiment.

## Actual host setup

Read selected settings/config keys only. ~/.codex/config.toml enables codescout-companion, buddy, superpowers, frontend-design-director and artifact/browser plugins; mcp_servers names are codescout, researcher, node_repl. ~/.claude, ~/.claude-sdd and ~/.claude-kat all enable codescout-companion, buddy and superpowers, plus other profile-specific plugins. Direct existence checks found ~/.Codex, ~/.Codex-sdd, ~/.Codex-kat, ~/.codex-sdd and ~/.codex-kat absent on this host. Do not create those directories just to conform to stale personal setup prose. Recheck actual profiles before a future installation; this observation does not authorize installing ECC.

## Findings verified in the checkout

ECC has a native Codex manifest (.codex-plugin/plugin.json), pointing skills at ./skills/, MCP config at ./.mcp.json and hooks at ./hooks/codex-hooks.json. The latter declares a SessionStart bootstrap, not the full Claude hooks/hooks.json event surface. README documents selective skills/modules and a minimal no-hooks installation, but no install dry-run or runtime validation has been performed.

agents/silent-failure-hunter.md, agents/pr-test-analyzer.md and agents/rust-reviewer.md are concrete specialist-review candidates. Their native Read/Grep/Glob/Bash tool declarations and diagnostic recipes need adaptation to codescout navigation, edits and shell tools. The serving Claude companion hooks/hooks.json has native-tool guards and an MCP principal stamp; do not assume original ECC agents can use codescout when their tool allowlists do not grant it.

agents/spec-miner.md extracts Requirement/Invariant assertions, enforcement locations and test anchors, but assumes OpenSpec paths and native tools. A useful adaptation would obtain evidence through symbols/references/call_graph and create existing librarian-managed spec artifacts, distinguishing observed behavior from intended requirements and naming deferred exploration.

skills/continuous-learning-v2/SKILL.md describes hook observations and separate ecc-homunculus project/global instinct storage. skills/unified-memory/SKILL.md describes a separate ECC Memory Vault under .ecc/memory and ~/.ecc/memory, with optional CLI/MCP prerequisites. These overlap codescout's common project memory and trackers. Prefer borrowing evidence/proposal workflows and keeping durable facts in codescout, rather than creating a second authoritative knowledge store or automatically promoting repeated behavior into policy.

skills/eval-harness and skills/agent-eval each contained only SKILL.md in the inspected checkout. They provide protocols/templates; do not describe them as an installed executable evaluator. Existing codescout eval-design memory has stronger local controls for arm symmetry, contamination and measurement validity. Any ECC review adaptation should preserve actual applied mutation checks required by this project's instructions, rather than replacing them with coverage arguments or a generic percentage threshold.

## Recommendation (proposal, not approved implementation)

Treat ECC as an upstream library of selected domain skills/review lenses. Keep codescout as IDE/memory/document infrastructure, companion as tool/workspace guardrails, and existing superpowers/buddy workflows in place. Pilot one adapted spec-miner workflow and one specialist reviewer in a small optional plugin in the shared claude-plugins source repository, pinned to an upstream commit. Adapt tools, output artifacts, model policy and actual mutation verification explicitly. Evaluate against the current setup on the same task/commit/model with isolated state; record observed results, false positives, tool refusals and available time/token/cost measurements. Only expand after demonstrated value. Full ECC install is an alternative experiment for an isolated environment, not the recommended default on these profiles.
