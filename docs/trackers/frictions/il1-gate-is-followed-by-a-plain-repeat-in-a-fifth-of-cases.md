---
kind: tracker
status: active
title: "Evidence — the il1 source-read gate is followed by a plain repeat in a fifth of cases"
tags:
  - frictions
  - read_file
  - il1
---

# Evidence — the `il1` source-read gate is followed by a plain repeat in a fifth of cases

Ledger entry: FR-9 in [index.md](index.md). Window: 2026-10-12 to 2026-11-10 UTC. Corpus: `.codescout/usage.db`. Source: the `readfile` miner (full population, not a sample); the family total was re-counted by the lead (354).

## Scale

`err_family='il1_read_overlaps_symbol'`: 354 rows at the lead's count (352 at the miner's). That is 35% of the 1,017 `read_file` errors. By file type: 233 `.rs`, 55 `.py`, 32 `.sh`, 7 `.ts`, 25 with an empty input. Spikes: 2026-10-16 (37), 2026-10-17 (34), 2026-11-07 (44), 2026-11-09 (23).

The agent wanted a line range inside a named symbol. The hint says: "Use symbols(name=..., include_body=true)… Pass force=true".

## First follow-up call (327 rows with an input)

| Next call | Rows |
|---|---|
| `symbols` success | 96 |
| `read_file` with `force=true`, success | 85 |
| `read_file` plain, failed again | **75** (43 on the same file, 41 on other files; the two counts overlap by the miner's grouping) |
| `grep` | 41 |
| `run_command` | 33 (28 success, 5 error) |
| `read_file` plain, success | 16 |
| other | 6 |

181 of 327 (55%) got the content in one call. `force=true` works as an escape hatch: all 85 forced retries succeeded. Of 52 `run_command` follow-ups, 4 used `sed`, `cat`, `head`, `tail` or `awk` on the file.

## Why this is filed as low

The gate is healthy by the project's own test: the median calls-to-recovery is 1. The 75 repeats are the part that did not learn from the text. They are 23% of the follow-ups.

## Related

`docs/trackers/friction-deep-dive-2026-10.md` (the `il1` diagnosis).

## Idea — checked 2026-11-12, already in the code

The idea was to name the overlapping symbol in the refusal text. The refusal already does that: a gated read on 2026-11-12 returned `source range overlaps named symbol(s): 'tests', 'tests/verify_does_not_count_files_the_indexer_cannot_represent_as_missing', ... and 1 more`, with a hint that names `force=true` and the line count it would read. `git log -S'overlaps named symbol' -- src` shows the gate and its symbol list from `9692161ce` (2026-05-05), and `c4a7b1ddc` (2026-08-16) ordered the hint by requested extent. So all 354 refusals in the 2026-10-12 to 2026-11-10 window were issued with the names in the text, and naming cannot explain the 75 plain repeats.

What this does not settle: why an agent repeats a plain read after seeing the names. That needs the intent behind the repeats (see Not verified). No change made.

## Not verified

- Whether the 75 repeats are overlapping ranges or a second gated read.
- The intent behind the repeats.
