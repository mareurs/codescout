# Setup A/B fresh pool v1: gold/empty replay through the real grader (2026-11-06/07)

Data: /home/marius/work/claude/setup-ab-data/fresh-screen/replay/ (replay.log, <task>.<gold1|gold2|empty>.json, grades/, SUSPEND-NOTE.txt). Driver: job tmp replay_gold.py (not git).

Results (gold patch twice, empty patch once, one docker job at a time):
- nushell-19283: gold pass, pass; empty fail (2 not passing = its 2 F2P).
- nushell-19172: gold pass, pass; empty fail (6 = its 6 F2P). The empty run overlapped an 11 h machine suspend (22:17 to 09:15); verdict readable, timing void.
- nushell-18863: gold1 FAIL (not_passing 1, missing 1), gold2 pass; empty fail. The gold1 failure is a grader artifact: a result line glued to child-process output at its START (`should_not_overwrite_lasttest last_result::bare_external_with_inherited_stdout_does_not_capture ... ok`), which the strict parser (needs a line starting with `test`) loses.
- RA-22992: gold pass, pass; empty fail (1).
- No ENOSPC/Bus error/Killed signature in any replay log.

Parser finding: the four v1 nushell records (19283, 19172, 18976, 18863) carry the STRICT log parser (template 1/2, hash 6348ec74); only nushell-19052 carries the tolerant one (template 3, hash 9c8b1ecc). Run on all nine nushell replay logs, the tolerant parser recovers exactly the glued tests (always the same two: `last_result::zero_budget_does_not_force_external_capture` and `last_result::bare_external_with_inherited_stdout_does_not_capture`), with 0 status changes and 0 losses. So the residual risk is two known tests, not "1 in 10,000". Rust-analyzer logs have no glue.

Consequence: with the strict parser a correct patch fails a nushell task in about 1 run in 8 (1 of 8 gold replays). Pool v2 must re-derive the nushell records with the tolerant parser before any screen. Batch-1 corrections file must list 18863 (artifact + hazard test `mixed_str_length_result_does_replace_last`) and the two artifacts already known (19172 opus, 19283 sonnet).

Open: P2P hazard tests (reader verdict 'hazard') exist in 18863 (2), 19172 (1), 19052 (2), and are not excluded anywhere yet. Operator decision pending.
