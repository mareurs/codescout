---
id: '5cbaa87d92772a51'
kind: bug
status: fixed
title: 'RESIDUAL: Make CI invoke the plugin repo''s tests/run-all.sh runner so skill-colocated tests actually run; add a test for the self-identification half'
tags:
- cluster/addressing-without-an-escape-hatch
closed: 2026-10-07
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-09-02-greedy-name-regex-reads-a-former-session-name-as-the-current-one.md
severity: low
unverified: No CI run was opened after the test and the duplicate-line removal landed, so green in CI is not observed; the local suite passes (11 of 11 for this skill).
---

## Summary

Remaining work split out of `docs/issues/archive/2026-09-02-greedy-name-regex-reads-a-former-session-name-as-the-current-one.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Make CI invoke the plugin repo's tests/run-all.sh runner so skill-colocated tests actually run; add a test for the self-identification half.

## Parent caveat, verbatim

`docs/issues/archive/2026-09-02-greedy-name-regex-reads-a-former-session-name-as-the-current-one.md` (status `fixed`):

> The regression test covers the name read only. The self-identification half of the same commit is verified by hand, not by test — see the sibling bug file. The test is newly reachable: `tests/run-all.sh` globbed hooks only until 2026-09-03, so a skill-colocated test was discovered by nothing; the glob was widened in the same commit and has not yet run in CI, which invokes two named targets rather than the runner.

## Fix

**Status stays `open`: the CI half is fixed, the self-identification test is not.**

See `## Partial fix (2026-10-06)` below; this section is intentionally not a closure. The parent's design context is unchanged.

## Partial fix (2026-10-06)

The parent caveat's claim that the widened glob "has not yet run in CI, which invokes two named targets rather than the runner" was overtaken on 2026-09-10 and 2026-09-13 by two commits in the sibling repo, `claude-plugins` (branch `main`), both verified on 2026-10-06 against claude-plugins HEAD:

- **SHA:** `claude-plugins:502365e` (branch: `main`; also in `feat/effort-steering-adapter`, `feat/effort-steering-core`, `fix/buddy-codex-summon`) — "ci: run the full hook + skill suite, and root-cause 5 of the 16". Adds the `full-suite` job ("full hook + skill suite (linux)", `ubuntu-latest`) to `.github/workflows/cross-platform-hooks.yml`, whose `Full suite` step is `run: bash tests/run-all.sh`; also touches `tests/run-all.sh` (adds the `CS_TEST_SKIP` deny-list: space-separated suite basenames, and the run FAILS if a name matches no discovered suite) and the plugin repo's `docs/issues/2026-08-05-test-run-all-pre-existing-failures-under-fresh-wsl.md`.
- **patch-id:** `2f6476cdb53fab43250685fd76bc28141cb8147e`
- **SHA:** `claude-plugins:8a95728` (branch: `main`; same other branches) — "fix(tests): make 4 of 5 ambient-config hook-test suites hermetic". Makes four suites independent of ambient `~/.claude*` config (`tests/test-pre-tool-guard.sh`, `test-rendezvous-isolation.sh`, `test-session-start.sh`, `test-worktree-activate.sh`) and shrinks the `CS_TEST_SKIP` list in the workflow to the one remaining suite.
- **patch-id:** `cdd20df75ab5e2090b9b7eca431b125b8f926cc5`

At the bytes: `.github/workflows/cross-platform-hooks.yml` lines ~65-116 hold the `full-suite` job (checkout, Python 3.13 + PyYAML, Node 24, then `bash tests/run-all.sh` with `CS_TEST_SKIP`); `tests/run-all.sh` builds its suite list from `test-*.sh`, `codescout-companion/hooks/*.test.sh`, `codescout-companion/skills/*/*.test.sh` (the skill glob that reaches `reaching-peer-sessions.test.sh`), hook and pi-extension `*.test.mjs`, and `effort-steering/**/*.test.mjs`. Linux only, by the job's own comment. I read the workflow and the runner; I did NOT open a CI run, so "it runs and is green in CI" is unverified here.

Note: `CS_TEST_SKIP` lists `pre-tool-guard.test.sh` twice (workflow lines ~114-115). Harmless (the runner matches by basename), worth deleting the duplicate.

**What remains: DONE 2026-10-07, see the Update below.** It was: a test for the self-identification half of `bb14719` (the walk that terminates on socket presence rather than on `comm == claude`). `codescout-companion/skills/reaching-peer-sessions/reaching-peer-sessions.test.sh` (header, lines ~16-26) says it cannot be tested without editing the skill: `SKILL.md`'s walk reads `/proc` and `/run/user/<uid>/cc-socks` by hardcoded absolute path, and a source-text assertion would be a proxy, not the behaviour. It was verified by hand on 2026-09-03 only. Making it testable is a change to a shared skill in the plugin repo (parameterise the two roots, then drive it against a fixture tree); size M. The `claude-plugins` repo is out of scope for this sweep.

**Update 2026-10-07: the self-identification test is written and merged.**

- **SHA:** `claude-plugins:ffe67e5` (branch `main` of `claude-plugins`, via the merge `231cd81`)
- **patch-id:** `67f3c4dfaeb358aa59a4881ae0cd049472b55964`

Step 1 of `SKILL.md` now reads its two roots from `CS_PEERS_PROC` and `CS_PEERS_SOCKS`, which default to the real `/proc` and `/run/user/<uid>/cc-socks`, so the skill's behaviour is unchanged. The test file gains cases 8 to 11. They run the block extracted from `SKILL.md` against a fixture tree with real unix sockets: a version-pinned socket owner is found and marked `<-- you`, an orphan prints the loud warning and marks no row, the walk climbs past socket-less ancestors, and a control runs the old `comm`-keyed rule on the same fixture and must fail it. The same commit deletes the duplicate `pre-tool-guard.test.sh` from `CS_TEST_SKIP`, which the note above asked for.

Re-run 2026-10-06 in the fork's worktree: `reaching-peer-sessions.test.sh` 11 passed, 0 failed. The fork also mutation-checked it against the real skill (restoring the `comm` rule fails cases 8 and 10; silencing the warning fails case 9). The merge into `main` was checked on 2026-10-07: the patch-id appears among the last 30 commits of `main`.

Not verified: a CI run. The only workflow change is the removed duplicate line, and nobody opened a run, so "green in CI" is not observed.

## Resume

Status stays `open` (partial). Next action: in `/home/marius/work/claude/claude-plugins`, make the two roots in `codescout-companion/skills/reaching-peer-sessions/SKILL.md`'s self-identification walk overridable (env var or argument defaulting to `/proc` and `/run/user/$UID/cc-socks`), then add a fixture-driven case to `reaching-peer-sessions.test.sh` asserting the walk stops at the socket-owning ancestor whose `comm` is not `claude` (the old `comm`-based walk must fail it). Delete the duplicate `pre-tool-guard.test.sh` in `CS_TEST_SKIP` while there. Not filed as separate items.

## References

- `docs/issues/archive/2026-09-02-greedy-name-regex-reads-a-former-session-name-as-the-current-one.md` — parent
