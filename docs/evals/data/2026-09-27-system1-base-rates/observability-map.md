# Observability map — System 1 base-rate measurement (Task 13)

Spec A1.6 requires this map before any Codex-judge call: for every outcome and every
causal-chain link, a label (measurable now / needs adjudication / unobservable) plus the
coverage number from the events database that justifies it, or the reason none exists.

**Provenance of this run.** Built 2026-09-27 from a scratch, codescout-project-only corpus
(corpus id scratch-2026-09-27-codescout), frozen via archive.freeze from the three Claude
Code profiles' codescout transcript directories (the main, sdd and kat profiles) plus this
repo's own usage database. It does NOT include the contrast project (MRV-poc). Task 12
freezes the real corpus (codescout plus the contrast project) and this map's coverage() /
render_map() functions are re-run against it then, per Task 13's Step 5 — the labels and
table shapes below are expected to carry over; the raw counts below are this scratch run's
own and are expected to change.

- Retained window (everything since the oldest surviving transcript, spec A1.2): 2026-08-26T00:00:00Z to 2026-09-27T10:06:17Z.
- Decision window (last 7 days; the only window the mistakes go/no-go rule reads, spec A1.2): 2026-09-20T10:06:17Z to 2026-09-27T10:06:17Z.
- Corpus totals: 138 transcript sessions seen, 119 kept, 19 excluded; 736 subagent transcripts; 71059 usage-database rows folded in.
- Exclusions recomputed at coverage-build time via transcripts.exclusions(transcripts.sessions(corpus_dir), join.SPEC_EXCLUDED_SIDS) agreed exactly with the events database's own persisted sessions_kept / sessions_excluded counters (R57's mismatch check did not raise).

## mistakes (operator-caught addressable misses; this outcome decides go/no-go)

| link | label | coverage | basis |
|---|---|---|---|
| opportunity | measurable now | 47401 | decision points = assistant_text turns |
| signal/request | needs adjudication | 2660 | prompt + interrupt rows are the candidate population; whether each is a correction is judged |
| delivery/action | needs adjudication | n/a | deliveries before the decision; their relevance is judged |
| observed use | needs adjudication | n/a | next_action_aligned (Amendment 2(c)) |
| checked outcome | needs adjudication | n/a | the Codex judge, plus the operator's 25-item spot-check |

## context (Outcome 2)

| link | label | coverage | basis |
|---|---|---|---|
| opportunity | measurable now | 47401 | assistant_text turns |
| signal/request | measurable now | 114551 | tool_use requests |
| delivery/action | measurable now | 7875 | deliveries by source, with join coverage |
| observed use | needs adjudication | n/a | evidence_used |
| checked outcome | needs adjudication | n/a | the judge, plus the spot-check |

## transfer (Outcome 3)

| link | label | coverage | basis |
|---|---|---|---|
| opportunity | needs adjudication | n/a | lesson applicability is judged |
| signal/request | needs adjudication | n/a | the lesson inventory is Task 7, not yet built; state that reason |
| delivery/action | measurable now | 4095 | operator-rule (OP-N) and get_guide deliveries |
| observed use | needs adjudication | n/a | applied / missed |
| checked outcome | needs adjudication | n/a | the judge, plus the spot-check |
| rediscovery (extra row) | needs adjudication | n/a | always: it needs semantic matching (A1.4) |

## background-worker (A1.5)

| link | label | coverage | basis |
|---|---|---|---|
| opportunity | measurable now | 114552 | tool calls by A1.5 task family |
| signal/request | measurable now | 1200 | Agent tool_uses plus delegation turns |
| delivery/action | measurable now | 112220 | subagent turns (agent_path set) |
| observed use | needs adjudication | n/a | did the parent use the result |
| checked outcome | unobservable | n/a | there is no counterfactual; A1.5 measures opportunity size, not delegability or saving |

## Coverage appendix (Amendment 1 A1.6 / R57 required breakdowns)

### tool_events by join method

Overall: exact 85, heuristic 59618, none 26201, not_codescout 28648 (total 114552).

Heuristic time-source split: heuristic_via_called_at 51857, the rest of the heuristic joins 7761.

First exact-joined tool event timestamp in this corpus: 2026-09-27T04:58:01.005Z. Exact joins
are a small share of the total (85 of 114552) in this run — the usage database's own
retention window is much shorter than the transcripts' retained window, so most tool events
outside its coverage fall back to heuristic or none rather than an exact join.

By UTC day (method: count; a day with no row for a method had zero of that method):

| day | exact | heuristic | none | not_codescout |
|---|---|---|---|---|
| 2026-08-03 | 0 | 0 | 130 | 6 |
| 2026-08-04 | 0 | 0 | 276 | 23 |
| 2026-08-05 | 0 | 0 | 18 | 1 |
| 2026-08-23 | 0 | 0 | 460 | 47 |
| 2026-08-24 | 0 | 0 | 835 | 63 |
| 2026-08-25 | 0 | 0 | 2930 | 134 |
| 2026-08-26 | 0 | 0 | 6664 | 298 |
| 2026-08-27 | 0 | 0 | 6908 | 735 |
| 2026-08-28 | 0 | 9 | 1766 | 652 |
| 2026-08-31 | 0 | 2803 | 9 | 1446 |
| 2026-09-01 | 0 | 6217 | 39 | 2504 |
| 2026-09-02 | 0 | 11311 | 122 | 4177 |
| 2026-09-03 | 0 | 6194 | 64 | 2241 |
| 2026-09-04 | 0 | 431 | 15 | 341 |
| 2026-09-07 | 0 | 465 | 0 | 401 |
| 2026-09-08 | 0 | 1329 | 6 | 1555 |
| 2026-09-09 | 0 | 4706 | 346 | 1446 |
| 2026-09-10 | 0 | 3805 | 13 | 1745 |
| 2026-09-11 | 0 | 2893 | 34 | 1145 |
| 2026-09-12 | 0 | 1661 | 25 | 643 |
| 2026-09-13 | 0 | 2342 | 30 | 831 |
| 2026-09-14 | 0 | 3561 | 59 | 1877 |
| 2026-09-15 | 0 | 837 | 148 | 1211 |
| 2026-09-16 | 0 | 1431 | 65 | 1734 |
| 2026-09-17 | 0 | 1003 | 210 | 788 |
| 2026-09-18 | 0 | 206 | 0 | 379 |
| 2026-09-19 | 0 | 286 | 1170 | 378 |
| 2026-09-20 | 0 | 231 | 660 | 367 |
| 2026-09-21 | 0 | 186 | 1021 | 170 |
| 2026-09-22 | 0 | 99 | 130 | 162 |
| 2026-09-23 | 0 | 362 | 221 | 109 |
| 2026-09-24 | 0 | 4796 | 1049 | 511 |
| 2026-09-25 | 0 | 1046 | 224 | 227 |
| 2026-09-26 | 0 | 788 | 108 | 206 |
| 2026-09-27 | 85 | 620 | 446 | 95 |

### deliveries

By source: transcript_hook 3712, usage_deliveries_json 68, usage_output_json 4095 (total 7875).

hook_success_only: 318. hook_success_twins_dropped: 3839. deliveries_unmapped_session: 11151.

### sessions per project, by window

This corpus contains a single project (the codescout project directory, present under all
three profiles, so its kept sessions are unioned across profiles into one row): retained 118,
decision 24. events_meta's persisted sessions_kept is 119 — the one-session gap is a kept
session none of whose turns' timestamps fall inside either window, so it is counted as kept
overall but in neither window's membership check.

### exclusions by reason

duplicate-prefix-of: 10. excluded-by-spec: 2. sdk-cli: 7. Total 19, matching events_meta's
persisted sessions_excluded.

### divergent-duplicate unowned uuids

None in this corpus. Its 19 exclusions are all duplicate-prefix-of (10) or the other two
reasons above; no divergent-duplicate-of collapse occurred here, so Amendment 3's documented
cost (uuids in a diverged tail that no kept transcript owns) does not apply to this run. Task
12's real-corpus rerun may find a nonzero count.
