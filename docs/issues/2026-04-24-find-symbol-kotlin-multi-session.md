---
status: mitigated
opened: 2026-04-24
closed:
severity: medium
owner: marius
related: ["BUG-048"]
tags: ["find_symbol", "kotlin-lsp", "multi-session", "joinset", "detect_fatal_stderr"]
---

# BUG: `find_symbol` hung ~90 s when kotlin-lsp hit "Multiple editing sessions"

## Summary

`find_symbol` against a Kotlin file could hang ~90 s when kotlin-lsp emitted "Multiple editing sessions" error (another editor or agent holding the kotlin-lsp workspace lock). Mitigated 2026-04-24: per-language 8 s hard budget in `find_symbol` JoinSet plus `detect_fatal_stderr` fast-fail on every init attempt. First call still pays up to ~8 s before falling back; subsequent calls fast-fail.

## Symptom (Effect)

`find_symbol("<symbol>", path="<some-kotlin-file>")` waited ~90 s before returning. kotlin-lsp stderr contained "Multiple editing sessions" repeatedly.

## Reproduction

Open the project in another agent or IDE that holds the kotlin-lsp workspace lock. From this session, call `find_symbol` with a `path` pointing at a Kotlin file.

## Environment

- Date observed: 2026-04-24
- Tool: `find_symbol`
- LSP backend: kotlin-lsp
- Triggering condition: another editor / agent holds the kotlin-lsp workspace lock

## Root cause

kotlin-lsp emits the "Multiple editing sessions" error to stderr but does not fail-fast on RPC; the JoinSet in `src/tools/symbol/find_symbol.rs` waited for any per-language LSP attempt to time out (default budget significantly larger than 8 s).

## Evidence

kotlin-lsp stderr capture from the hang. Tests `detect_fatal_stderr_flags_kotlin_multi_session` and `detect_fatal_stderr_ignores_benign_lines` exercise the regression.

## Hypotheses tried

1. **Hypothesis:** Per-language hard budget on the JoinSet would cap any single language's contribution to the join. **Verdict:** Confirmed — adopted as part of the fix. **Evidence link:** see Fix.
2. **Hypothesis:** Detect the fatal stderr signature on init and fail-fast. **Verdict:** Confirmed — adopted as the second half of the fix. **Evidence link:** see Fix.

## Fix

Applied 2026-04-24:

- Per-language 8 s hard budget in `src/tools/symbol/find_symbol.rs` JoinSet.
- `detect_fatal_stderr` in `src/lsp/client.rs` fast-fails kotlin-lsp's multi-session error on every init attempt.

## Tests added

- `detect_fatal_stderr_flags_kotlin_multi_session`
- `detect_fatal_stderr_ignores_benign_lines`

## Workarounds

Pin `path=` to a non-Kotlin file to skip kotlin-lsp entirely. Or close the other editor / agent holding the kotlin-lsp workspace lock first.

## Resume

If the 8 s budget proves too generous in practice (i.e. routine warm calls bump up against it), profile typical kotlin-lsp first-call latency on a small fixture and tighten. Concrete next action: open `src/tools/symbol/find_symbol.rs`, locate the JoinSet wiring, instrument with `tracing::debug!` for per-language latency over a representative session.

## References

- Originally tracked as **BUG-049** in `docs/TODO-tool-misbehaviors.md` (deprecated 2026-05-09; superseded by per-file system).
- Related: BUG-048 (60 s cold-start hang); broader kotlin-lsp issues tracked in `docs/issues/2026-03-24-kotlin-lsp-concurrent-instances.md`.
