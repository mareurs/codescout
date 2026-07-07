---
id: '88398b061917d789'
kind: bug
status: open
title: 'BUG: Windows path-separator audit missed .display() sites without chained .to_string()'
owners: []
tags:
- windows
- cross-platform
- test-portability
- follow-up
- audit-methodology
topic: null
time_scope: null
opened: '2026-07-07'
owner: marius
related:
- docs/issues/2026-07-07-windows-glob-overview-path-separator-test-mismatch.md
severity: low
---


# BUG: Windows path-separator survey scoped to `.display().to_string()` — broader `.display()` patterns not swept

## Summary

The 2026-07-07 Windows path-separator audit (two subagent forks) searched specifically for the regex `\.display\(\)\.to_string\(\)` across the codebase and found 54 sites in 33 files, of which ~23 were confirmed RISKY and all were fixed (see References). While fixing `src/server.rs`'s `post_process`, one instance of the *same defect* was found that the survey's own grep pattern couldn't see: `format!("{}/", p.display())` — `.display()` feeding directly into `format!`/`write!` without an explicit `.to_string()` call. This site turned out to be the single highest-leverage fix in the whole pass (the shared root-prefix-stripping mechanism behind every non-`run_command` tool response), and was only found by chance while investigating why `mv.rs`'s severity claim needed correction, not by systematic search.

## Symptom (Effect)

None currently known — this is a scope gap in the audit methodology, not an observed failure. The one instance found (`post_process`) has been fixed. There may be others: any `format!("...{}...", path.display())`, `write!(f, "{}", path.display())`, or `println!`/`tracing::info!` with `path.display()` interpolated directly, where the resulting string later crosses a comparison/persistence boundary.

## Reproduction

Not yet reproducible — no known failing case beyond the already-fixed `post_process`. Best lead: `grep(pattern="\\.display\\(\\)", glob="*.rs")` returned 237 matches in 89 files (superset of the 54-site `.display().to_string()` subset already audited) — the remaining ~183 matches across the delta of files have not been individually triaged for the same RISKY/SAFE distinction the two forks applied.

## Environment

Windows only, same as the parent bug class.

## Root cause

Audit methodology gap: grepping for the exact call-chain `.display().to_string()` misses any site where `.display()`'s `Display` impl is consumed via `format!`/`write!` interpolation, string concatenation, or similar, without an explicit `.to_string()` in between. `Path::display()` doesn't need `.to_string()` to be dangerous — it needs to be consumed anywhere a byte-exact comparison or persisted string eventually happens.

## Evidence

- `src/server.rs:454-459` (now fixed, commit `62457959`) — `let root_prefix = ... .map(|p| format!("{}/", p.display())) ...` — no `.to_string()` in the chain, invisible to both audit forks' grep pattern.

## Hypotheses tried

N/A — not yet investigated systematically; the one known instance was found incidentally.

## Fix

Not implemented as a systematic sweep. The one instance found (`post_process`) is fixed. A full second-pass audit using a broader pattern (e.g. `\.display\(\)` without requiring `.to_string()`, then manually excluding pure `println!`/`tracing::*!`/error-message-only consumers) is the natural next step, deferred pending user prioritization.

## Tests added

N/A — no fix implemented here; see the `post_process` fix's own commit for its verification.

## Workarounds

None needed for now — no known live failure beyond the fixed instance.

## Resume

Run `grep(pattern="\\.display\\(\\)", glob="*.rs", mode="files")` (already run once this session — 237 matches, 89 files) and diff against the 33 files already covered by the `.display().to_string()` audit to get the delta file list. Triage the delta the same way the two forks did (RISKY: JSON output / compared / persisted; SAFE: pure error/hint/log text). Given the base rate observed (1 RISKY find in the one delta site checked so far, `post_process`, out of curiosity rather than exhaustive search), expect a low but nonzero hit rate in the remaining ~183 matches.

## References

- `docs/issues/2026-07-07-windows-glob-overview-path-separator-test-mismatch.md` — the original bug this whole audit traces back to.
- `docs/issues/2026-07-07-list-overview-remaining-display-path-separator-sites.md` — the first-identified follow-up (now also fixed as part of this pass, via `to_forward_slash` in `list_overview.rs`... note: verify this file's status is updated separately, it predates this survey).
- `docs/trackers/bug-fix-session-log.md` (`2dd9d90bc83f9f49`) — F-27, the session-log entry for the broader audit's provenance.

