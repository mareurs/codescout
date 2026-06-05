---
status: fixed
opened: 2026-05-31
closed: 2026-05-31
severity: medium
owner: marius
related: []
tags: [prompts, server-instructions, 2kb-cap, concurrent-work]
kind: bug
---

# BUG: Phase-5 commit pushed the server_instructions slice over the 2200-byte cap and left the snapshot stale

## Summary
Commit `b13c8c66` ("Phase 5 polish — when-to-pin guidance") added a ~351-byte
paragraph to the always-loaded `server_instructions` slice without re-checking
the 2200-byte cap or re-blessing the snapshot fixture. The branch
`feat/per-request-workspace-pinning` was left red on two prompt invariants.
Realized impact contained (gate caught it on a feature branch, never on
master); **potential** impact high — had it shipped, Claude Code would
truncate the slice mid-content in `initialize.instructions`, degrading
subagent dispatch silently across sessions.

## Symptom (Effect)
`cargo test --lib prompts` on `b13c8c66` (before any further edit) fails two tests:

```
---- prompts::redesign_invariants::source_md_under_cap stdout ----
server instructions are 2272 chars; cap is 2200. Cut content or move it to get_guide.

---- prompts::tests::prompt_surfaces_server_instructions_snapshot stdout ----
prompt surface drift in `server_instructions.md`
  expected: 1921 bytes
  actual:   2272 bytes
```

## Reproduction
```
git checkout b13c8c66        # the Phase-5 polish commit
cargo test --lib prompts     # source_md_under_cap + snapshot both FAIL
```
Branch: `feat/per-request-workspace-pinning`. The over-cap slice is the
`## Workspace gate` section of `src/prompts/source.md` (the when-to-pin
paragraph).

## Environment
codescout v0.14.0, Linux, branch `feat/per-request-workspace-pinning` at
`b13c8c66`. Build-time slice extraction (`build.rs` + `extract_surface`);
cap enforced at test time.

## Root cause
`b13c8c66` appended a 5-line "when-to-pin" paragraph to the
`server_instructions` surface of `src/prompts/source.md`, growing the rendered
slice from 1921 → 2272 bytes. `prompts::redesign_invariants::source_md_under_cap`
(`src/prompts/mod.rs`) caps `build_server_instructions(None)` at 2200; the commit
neither ran `cargo test --lib prompt` nor re-blessed the
`tests/fixtures/prompt_surfaces/server_instructions.md` snapshot (still 1921).
Fix-then-forget, but applied to a compiled invariant rather than a tracker.

## Evidence
HEAD slice measured directly from the committed file:
```
$ git show HEAD:src/prompts/source.md | awk '/@surface server_instructions/{f=1;next} /@end/{if(f)f=0} f' | wc -c
2272
```
Session-start scout had measured 1921 B at the prior HEAD (`4b814627`); HEAD
advanced twice during the prompt-refresh session, surfacing the breach only on
re-scout. Full investigation: F-8 / W-5 in
`docs/trackers/prompt-guide-refactor-session-log.md`.

## Hypotheses tried
N/A — not a mystery. Cause identified directly from `git show` + `git log`
(concurrent commit grew the slice). See Root cause.

## Fix
Per `src/prompts/README.md` rule 8 ("don't raise the cap — move content to
`get_guide`"), the verbose per-call-pin mechanism was relocated from the slice
into `get_guide("workspace-state")` (new `## Per-call workspace pinning`
section in `src/prompts/guides/workspace-state.md`), leaving a 2-line directive
+ pointer in the slice. Slice 2272 → 2130 B (70 under cap); snapshot re-blessed.

Fix commit: **`66bfd45c`** on `feat/per-request-workspace-pinning` (NOT yet on
master — feature-branch SHA; cite the master-side SHA here once it ships per
CLAUDE.md § "After cherry-pick"). Changes in `src/prompts/source.md`,
`src/prompts/guides/workspace-state.md`, `tests/fixtures/prompt_surfaces/server_instructions.md`.

## Tests added
None new — the **pre-existing** `source_md_under_cap` and
`prompt_surfaces_server_instructions_snapshot` invariants are exactly what
caught this; their value is that they fired. Post-fix the full suite passes
(2690 passed, 0 failed). The durable hardening is documentation: CLAUDE.md
§ "Prompt Surface Consistency" → "Verify the slice before committing
(shared-branch hazard)", added in `92eb2a3c`.

## Workarounds
N/A — fixed.

## Resume
N/A — fixed in `66bfd45c`. If this recurs, the lesson (CLAUDE.md "Verify the
slice before committing") plus the gate are the guard: run
`cargo test --lib prompt` before committing any prompt-surface edit, and on a
shared branch re-measure the slice on current HEAD.

## References
- `docs/trackers/prompt-guide-refactor-session-log.md` — F-8 (the gap), W-5
  (the catch), W-3 / F-4 (the first firing of the same gate, 2026-05-28).
- `src/prompts/README.md` rule 8 — the cap-remediation discipline.
- `docs/architecture/mcp-channel-caps.md` — why ~2 KB truncation happens.
- Fix commit `66bfd45c`; lesson commit `92eb2a3c`.
