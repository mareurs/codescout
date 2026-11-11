# Setup A/B fresh pool: batch-1 screen corrected offline (2026-11-07)

Script: job tmp corrections_batch1.py (not git). Output: setup-ab-data/fresh-screen/live-screen/corrections-batch1.json. rows.jsonl is NOT edited. It re-grades the 14 saved grade.log files with the grader's own expected_sets and grade.verdict.

Recorded 9/14 pass. Tolerant parser (template 3) for nushell: 12/14. Tolerant parser and P2P hazard tests removed: 13/14.
- nushell-19172 opus and nushell-19283 sonnet: parser artifacts (now pass).
- nushell-18863 both models: fail only because of two P2P hazard tests (`last_result::mixed_str_length_result_does_replace_last`, `commands::table::collapse_expand::table_expand_top_level_error_list_still_throws`); with them removed both pass.
- Only RA-22992 Sonnet still fails (a real F2P failure, `regression_22820`).
- 18976, 23115, 23586: pass for both models in every variant.

Meaning: the fresh pool is saturated for bare sonnet-5-5 and opus-5-5 at medium effort. Only RA-22992 discriminates (Sonnet fails, Opus passes). A v2 screen on the same tasks adds a second rep, not new difficulty. The lever is harder tasks (larger multi-file PRs, other languages, lower effort), not more of the same.
