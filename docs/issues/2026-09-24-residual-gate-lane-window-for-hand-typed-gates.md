---
id: b8f756b54ac934a5
kind: bug
status: open
title: 'RESIDUAL: Close the cli_doc/lean-lane race for sessions that run the four gate commands by hand (per-session CARGO_TARGET_DIR is only applied by scripts/gate.sh)'
tags:
- cluster/transient-shared-state-lies-to-readers
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/2026-09-14-the-gate-ordering-guarantee-is-false-under-concurrency.md
- docs/issues/archive/2026-08-30-shared-target-dir-feature-clobber-reds-the-cli-tests.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/2026-09-14-the-gate-ordering-guarantee-is-false-under-concurrency.md`, `docs/issues/archive/2026-08-30-shared-target-dir-feature-clobber-reds-the-cli-tests.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Close the cli_doc/lean-lane race for sessions that run the four gate commands by hand (per-session CARGO_TARGET_DIR is only applied by scripts/gate.sh).

## Parent caveat, verbatim

`docs/issues/2026-09-14-the-gate-ordering-guarantee-is-false-under-concurrency.md` (status `mitigated`):

> The race itself is NOT closed -- only its legibility, and its arming for whoever runs scripts/gate.sh. cli_doc now names the cause instead of reading as the reader's own feature-gating regression, and the gate lanes build in a per-session CARGO_TARGET_DIR keyed on CLAUDE_CODE_SESSION_ID. WHAT REMAINS IS ADOPTION, NOT A DECISION -- the earlier form of this field named direction 1 as an unmade operator call, and a scoped variant of it shipped in 58b6bafc; four sessions had adopted it by 2026-09-15, 61 G across four isolated trees. A session that types the four commands by hand still shares target/ and can still replace target/debug/codescout inside another lane's run phase, so the script is a mechanism for whoever runs it and a policy for everyone else. RETRACTED 2026-09-15: this field previously named tests/cross_process_write_lock.rs and tests/librarian/mcp_integration.rs as carrying the same by-path exposure. Neither does -- mcp_integration is #[ignore]d against a binary the 2026-05-16 dissolution deleted, and cross_process_write_lock declares no required-features so it runs in both lanes and passes against a lean binary (measured, 5.04s). cli_doc is the only detector this corpus can have, which is why it must never be made to skip. Measured cost of the isolated lanes, first cold run: 13 G and 3m10s, against 358 G free and a 113 G shared tree.

`docs/issues/archive/2026-08-30-shared-target-dir-feature-clobber-reds-the-cli-tests.md` (status `fixed`):

> No regression test: nothing fails if the gate order is reverted, so this can silently regress via a CLAUDE.md edit. And the fix closes the TERMINAL state only, not the window — during the lean lane the binary is still librarian-less (measured: ~54s), so two sessions gating concurrently still collide and nothing detects that. Not archived for the first reason: the documented archive trigger requires a regression test. RESIDUAL CONFIRMED 2026-09-02, 3 days after closure: a session running the full four-command gate in the documented order got 10 of 11 cli_artifact failures with this file's exact `error: unrecognized subcommand 'artifact'` signature, because a peer's lean lane landed inside the window. Re-running both lanes alone immediately after gave exit 0 and 11/11, isolating it to concurrency rather than to ordering. This is the confirmation published rather than absorbed — the window half of this bug is LIVE and unmitigated, and the terminal-state fix cannot reach it. It also cost the observer a near-miss worth naming: the failure reads as a falsification of CLAUDE.md's "following the gate cannot arm the trap" claim, and a draft saying so got as far as being written before the isolating re-run; the archive already predicted the case in this very field.

## Fix

Not started. The parent's § Fix and § Resume hold the design context; read them before acting, and re-check the caveat against HEAD first — it was written at the parent's closing and may have been overtaken since.

## References

- `docs/issues/2026-09-14-the-gate-ordering-guarantee-is-false-under-concurrency.md` — parent
- `docs/issues/archive/2026-08-30-shared-target-dir-feature-clobber-reds-the-cli-tests.md` — parent
