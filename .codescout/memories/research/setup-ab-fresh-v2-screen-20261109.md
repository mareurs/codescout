# Setup A/B fresh pool v2 bare-arm screen (2026-11-08/09)

Registered screen on `multilang-pool-fresh-v2.json` (commit a88165d54, now on origin/experiments, pushed by another session on its own user's instruction, not by this session's operator). Data: setup-ab-data/fresh-screen-v2/live-screen (not git). 4 tasks x sonnet-5-5 and opus-5-5 x 1 rep = 8 runs, bare arm, medium effort, account ~/.claude token, cost $2.957 of cap 25. Pool: RA-22992, nushell 19052, 19283, 18863 (tolerant parser, P2P hazard tests removed, gold x2 + empty replay all sound, prescan 0 hits).

Results (pass): RA-22992 S fail / O pass; nushell-19052 S pass / O pass; nushell-19283 S pass / O pass; nushell-18863 S fail / O pass. 6/8.
Audit: all integrity ok, 0 codescout calls, no OOM, no cap hit, no ENOSPC/Bus error/Killed signature in any grade.log. Both failures are failed F2P tests, not artifacts: RA-22992 Sonnet `regression_22820` (5 turns), nushell-18863 Sonnet `error_values_do_not_clobber_last_*` x2 (7 turns, 6 native calls).

Reading: Opus passes 4/4, Sonnet 2/4 at medium effort with grep/read/bash only. The two Sonnet failures are short runs (5 and 7 turns), so the discriminating effect is partly effort/turns, not only difficulty. n=1 per cell: calibration data, not a comparison. Where tools could help: tasks where a bare Sonnet stops early.
