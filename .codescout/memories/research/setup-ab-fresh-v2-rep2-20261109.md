# Setup A/B pool v2: rep-2 repeat of the two discriminating tasks (2026-11-09)

Registered sub-pool `multilang-pool-fresh-v2-rep2.json` (commit cb329e492, local then pushed by another session with the stack), 2 tasks (RA-22992, nushell-18863) x 2 models x 2 reps = 8 runs, bare arm, medium effort, account ~/.claude, cost $2.09 of cap 25. Data: setup-ab-data/fresh-screen-v2/live-screen-rep2. Tasks were chosen after the rep-0 results (disclose).

Result: 8 of 8 pass. RA-22992 Sonnet passes both reps (11 and 12 turns), Opus passes both; nushell-18863 Sonnet passes both reps, Opus both. So the two Sonnet failures in rep 0 (5 and 7 turns, early stop) did not repeat: over 12 pass/fail outcomes per cell pair, bare Sonnet fails 2 of 6 and bare Opus 0 of 6 on these two tasks. Reading: the pool v2 is almost saturated for the bare arm; Sonnet's rep-0 failures were an unstable early stop, not a hard task. No evidence yet of a task where bare fails reliably.

Follow-up work done the same day: arm profile v2 in arms.py (8a44c4eca), four bug files, spec for codescout product changes (docs/superpowers/specs/2026-11-09-codescout-changes-from-the-cs-usage-audit.md). See research/setup-ab-audit-cs-usage-20261109 and research/setup-ab-fresh-onboarding-20261109.
