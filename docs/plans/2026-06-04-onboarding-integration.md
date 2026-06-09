---
id: b5bc613afc68e6d3
kind: plan
status: draft
title: Onboarding Integration — manual workflow → codescout capability
owners:
- marius
tags:
- onboarding
- language-detection
- companion-plugin
- language-patterns
- memory
topic: null
time_scope: null
---

# Plan — Onboarding Integration (manual workflow → codescout capability)

> Captures the full integration vision surfaced while onboarding a polyglot client
> repo (hermes-agent: Python core + TS/JS subprojects) by hand. The Rust correctness
> half is already filed as a bug; this plan records the whole picture and the
> companion-skill half so the work isn't lost.

## Problem
A manual onboarding pass exposed that codescout's onboarding produces low-fidelity —
sometimes wrong — project context:

- Per-project `languages` + `primary_language` are derived from the root **manifest
  type**, not file content. A Python repo with a root `package.json` (kept for tooling)
  is labeled `javascript` and Python vanishes. → **issue `e7454c40`**
  (`docs/issues/2026-06-03-project-languages-from-manifest-not-files.md`).
- `language-patterns` memory content is **hardcoded-generic** per language
  (`src/prompts/builders.rs:8-131`), not grounded in the project's real lint/type/test
  conventions — so it must be rewritten by hand to be useful (the generic Python block
  literally said "pyright / pyproject-over-setup.py", neither of which this project uses).
- No human-facing **`ONBOARDING.md`** is ever produced; the orientation doc we wrote was
  100% manual plus the `ShareOnboardingGuide` harness tool.

Net effect: onboarding's persisted memories + system-prompt draft mislead every
downstream agent session on affected repos.

## Goal
Turn the manual workflow (proven this session on `hermes-agent` + `hermes_cli`) into a
repeatable system capability, split by determinism:

- **deterministic correctness → codescout (Rust) core**
- **LLM-quality synthesis (grounded patterns, prose doc) → a codescout-companion skill**

## Constraints / facts (scouted 2026-06-03/04 — codescout `d059f70`, v0.14.0)
- Onboarding lives in `src/tools/onboarding.rs`: `perform_full_onboarding` (638-1030)
  file-walks languages for the MAIN project (accurate), writes `onboarding` +
  `language-patterns` memories (`PROGRAMMATIC` const at :201), builds a version-gated
  system-prompt draft (`build_system_prompt_draft`, :900), and writes per-subproject
  memories (:838-844). Per-project languages come from `DiscoveredProject.languages` (:1184).
- `DiscoveredProject.languages` is set by **manifest type** in `src/workspace.rs:29-216`
  `discover_projects` (one manifest per dir, hardcoded lang list). ← root cause of `e7454c40`.
- `primary_language`: `src/mcp_resources/project_hints.rs:36-55` and
  `src/mcp_resources/project_summary.rs:63-98` — both manifest-first.
- The accurate file-walk detector already exists but is **unused** by discovery/hints:
  `src/dashboard/api/project.rs:28-42` `detect_languages` (+ `src/ast/mod.rs:61`).
- `language_patterns(lang)` is a hardcoded `&'static str` per language, sourced from
  `docs/research/claude-language-patterns.md` (`src/prompts/builders.rs:8-131`); assembled
  by `build_language_patterns_memory` (:135).
- **No `ONBOARDING.md` generation anywhere in `src/`.**
- Companion plugin: `claude-plugins/codescout-companion` (v1.11.7) — skills
  `explore-project`, `reconnaissance`; command `/dashboard`; hooks (session-start
  injection, IL guards, auto-reindex). `explore-project/SKILL.md` is the model
  (brief → confirm → bootstrapped subagent → present).
- `ShareOnboardingGuide` harness tool uploads `ONBOARDING.md` → shareable link.

## Workstreams

### WS1 — Rust: correct language attribution  [→ issue `e7454c40`, FOUNDATION]
File-dominance `languages` + `primary_language` instead of manifest-type. Everything
downstream consumes this, so it lands first. Full root cause, fix sketch, and test plan
are in the bug. In short: in `discover_projects` post-pass, set each
`DiscoveredProject.languages = merge(file_dominance_scan(root), manifest_langs)` (manifest
as fallback so manifest-only dirs still resolve), and repoint `primary_language` to the
dominant language. Reuse the bounded walk from `src/dashboard/api/project.rs:28-42`.

### WS2 — Companion skill: `/onboard` (the LLM-quality layer)
New `skills/onboard-project/SKILL.md` in codescout-companion, modeled on
`explore-project`. Flow:
1. `onboarding()` → baseline memories + system-prompt draft (post-WS1: correct languages).
2. **Ground `language-patterns`**: read the project's lint/type/test config (ruff/ty/
   eslint/tsconfig), CONTRIBUTING/AGENTS; rewrite each language section with the real
   conventions; `memory(action="write", topic="language-patterns", project_id=...)`.
3. **Generate `ONBOARDING.md`**: synthesize the orientation doc from README/AGENTS/
   CONTRIBUTING + structure; write to repo root.
4. **Correct the `onboarding` memory** with real structure.
5. Optional: `ShareOnboardingGuide`.
6. **Per-subproject loop** over workspace projects (repeat 2 + 4).
Rationale: grounded patterns + prose need an LLM; they cannot be pure Rust.

### WS3 — (decision) `ONBOARDING.md` ownership
Does the Rust core emit a baseline `ONBOARDING.md` draft during onboarding, or is it
skill-only? **Recommendation: skill-only** — writing into the user's repo from the Rust
core is intrusive, and the LLM prose is the actual value. Decide before building WS2 step 3.

### WS4 — (optional) better baseline `language_patterns`
Improve `docs/research/claude-language-patterns.md` → `builders.rs::language_patterns`.
Low priority; WS2 is the real grounding, and per the language-patterns design these are
only meant as a deterministic baseline.

## Sequencing
WS1 → WS2 (WS2 depends on WS1 for correct languages). WS3 decision gates the WS2 step-3
boundary. WS4 is independent / optional.

## Testing
- **WS1:** fixture dir with `package.json` (+scripts) AND `pyproject.toml` AND mostly
  `.py` files → assert `primary_language == "python"` and `languages` lists `python`
  first. Update manifest-only fixtures in `src/workspace.rs` / `project_hints.rs` /
  `project_summary.rs` tests. (See bug.)
- **WS2:** agent-driven skill (no unit tests). Validate by re-running on `hermes-agent`
  and diffing the produced memories + `ONBOARDING.md` against the hand-authored versions
  from this session (2026-06-03/04).

## Risks / open questions
- **Perf:** the file-dominance scan runs on every `Agent::new`/activate — must stay
  bounded (WS1).
- **In-repo writes:** is `ONBOARDING.md` opt-in? what's the overwrite policy? (WS3)
- **Versioning drift:** companion skill vs. codescout `ONBOARDING_VERSION`
  (`src/tools/onboarding.rs:27`).
- **Clobbering:** keep the grounded `language-patterns` from being overwritten by a
  later `onboarding(force=true)` that re-writes the generic baseline.

## References
- Issue: `docs/issues/2026-06-03-project-languages-from-manifest-not-files.md` (`e7454c40`)
- `src/tools/onboarding.rs` (`perform_full_onboarding`, :201, :838-844, :900, :1184, ONBOARDING_VERSION :27)
- `src/prompts/builders.rs:8-131` (`language_patterns`), :135 (`build_language_patterns_memory`)
- `src/workspace.rs:29-216` (`discover_projects`)
- `src/mcp_resources/project_hints.rs:36-55`; `src/mcp_resources/project_summary.rs:63-98`
- `src/dashboard/api/project.rs:28-42` (`detect_languages`); `src/ast/mod.rs:61` (`detect_language`)
- `claude-plugins/codescout-companion/skills/explore-project/SKILL.md` (skill model)
- `ShareOnboardingGuide` harness tool
- Session 2026-06-03/04: `hermes-agent` + `hermes_cli` onboarding — memories + `ONBOARDING.md` produced by hand

