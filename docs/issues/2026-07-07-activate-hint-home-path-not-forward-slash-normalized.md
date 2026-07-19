---
status: fixed
opened: 2026-07-07
closed: 2026-07-07
severity: low
owner: marius
related: [2026-07-07-windows-glob-overview-path-separator-test-mismatch.md]
tags: [windows, cross-platform, path-separator, hint, config]
kind: bug
---

# BUG: `workspace(activate)` hint's "remember to workspace(activate, path=...)" suggestion uses native separators while CWD uses forward slashes

## Summary
`build_activation_response`'s `SwitchAway` hint text mixes two path
renderings in the same string: `project_root_str` (forward-slash normalized,
via `to_forward_slash(&p.root)`) for the `CWD:` segment, and `home_str` (raw
`.display().to_string()`, native separators on Windows) for the "remember to
`workspace(action='activate', path="...")`" suggestion. On Windows this
produces a hint like:
```
Browsing .tmp... (read-only). CWD: //?/C:/Users/.../foo — remember to
workspace(action='activate', path="\\?\C:\Users\...\bar") when done.
```
— one path forward-slash, the other backslash, in the same sentence. Not
just a test-portability issue: this is a genuine inconsistency in the
production hint text served to agents.

## Symptom (Effect)
Test failure surfaced it:
```
thread 'tools::config::tests::activate_hint_shows_switched_when_away_from_home' panicked at src\tools\config\tests.rs:392:5:
should contain home path: Browsing .tmpPmCq9v (read-only). CWD: //?/C:/Users/MAILINCA.BRN.002/AppData/Local/Temp/.tmpPmCq9v — remember to workspace(action='activate', path="\\?\C:\Users\MAILINCA.BRN.002\AppData\Local\Temp\.tmpIS680h") when done.
```

## Reproduction
1. Commit prior to this fix: any commit in the `experiments` forward-slash
   normalization series (e.g. `1a3b6fc2`) plus our local rebuild.
2. `cargo test --lib -- tools::config::tests::activate_hint_shows_switched_when_away_from_home`
   on Windows.

## Environment
- OS: Windows, PowerShell 7
- codescout v0.15.0, `experiments` branch

## Root cause
[src/tools/config/mod.rs:602-621](../../src/tools/config/mod.rs#L602-L621) —
both `SwitchAway` arms build `home_str` via
`home_root.as_ref().map(|p| p.display().to_string())`, while
`project_root_str` (used for `CWD:` in the same format string) is computed
earlier via `to_forward_slash(&p.root)`
([src/tools/config/mod.rs:530](../../src/tools/config/mod.rs#L530)). The
`home_str` construction was never updated when the forward-slash
normalization series landed — a narrower miss of the same series that broke
`try_build_runtime` and the `ads_colon` doctor check.

## Evidence
```
$ grep -n "home_str" src/tools/config/mod.rs
602:            let home_str = home_root.as_ref().map(|p| p.display().to_string())...
612:            let home_str = home_root.as_ref().map(|p| p.display().to_string())...
530:                to_forward_slash(&p.root),   // project_root_str, same function
```

## Fix
Changed both `home_str` constructions to `home_root.as_ref().map(|p| to_forward_slash(p))`,
matching `project_root_str`'s normalization. `src/tools/config/mod.rs` — see
commit in this session.

## Tests added
Fixed the 3 existing tests that caught this shape drift
(`activate_hint_shows_switched_when_away_from_home`,
`activate_hint_shows_returned_when_back_home`,
`activate_includes_cwd_hint`) — no new test needed since these already
assert on the hint's exact path content; they were just asserting against
the pre-normalization (native-separator) form and needed updating to expect
the normalized form.

## Workarounds
N/A — fixed.

## Resume
N/A — fixed, `cargo test --lib`: 2864 passed, 0 failed, 10 ignored.

## References
- [src/tools/config/mod.rs:591-622](../../src/tools/config/mod.rs#L591-L622) — the fixed hint-building code
- [src/tools/config/tests.rs](../../src/tools/config/tests.rs) — the 3 tests that caught this
- [docs/issues/2026-07-07-windows-glob-overview-path-separator-test-mismatch.md](2026-07-07-windows-glob-overview-path-separator-test-mismatch.md) — sibling issue from the same normalization series
