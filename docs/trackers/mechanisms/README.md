---
id: e271ef8f0c0f9b3b
kind: tracker
status: active
title: Mechanisms — correcting what Opus review finds
owners:
- marius
tags:
- mechanisms
- subagent-driven-development
- review-mining
---

# Mechanisms — correcting what Opus review finds

## Purpose

In `superpowers:subagent-driven-development` runs, Opus reviews the implementer's work and finds many defects. This folder holds experiments, observations, mechanisms and scripts that aim to stop those defects before the review. Idea from the user, 2026-11-11. Bug-style pointer: `docs/issues/2026-11-11-opus-review-finds-many-defects-the-implementer-should-not-ship.md` (severity high).

## Contents

| File | What it holds |
|---|---|
| `opus-review-mining-wave-1.md` | Observations from the first mining wave: method, counts, limits |
| `candidates.md` | Ledger `MX-N` of candidate mechanisms, each with a status and a measured effect when one exists |
| `tools/extract_reviews.py` | Finds Opus review subagents in the transcripts of all four profiles |
| `tools/make_shards.py` | Stratified sample of reviews, split into shard files for miners |
| `tools/miner-brief.md` | Prompt for a haiku miner: findings, category, mechanical, implementer-preventable |
| `tools/verify_findings.py` | Checks that each quote exists literally in its source review |
| `tools/aggregate.py` | Verifies all shards, drops unverified findings, counts |

## Method (reproducible)

All steps are read-only. The corpus holds transcript text, so keep it outside the repo (scratchpad).

1. `python3 -I docs/trackers/mechanisms/tools/extract_reviews.py <corpus_dir>`
2. `python3 -I docs/trackers/mechanisms/tools/make_shards.py <corpus_dir> <shard_dir> --shards 8 --per-shard 10 --seed 1 [--exclude <earlier manifest.json>]`
3. Dispatch one haiku per shard with `tools/miner-brief.md`; each writes `<out_dir>/shard-N.json`.
4. `python3 -I docs/trackers/mechanisms/tools/aggregate.py <out_dir> <shard_dir> <result.json>`

Use `--exclude` with the manifest of an earlier wave, so a new wave draws different reviews. Wave 1 used seed 1 and excluded the pilot manifest.

## Status — 2026-11-11

- Wave 1 done: 80 of 671 reviews (12%). See `opus-review-mining-wave-1.md`.
- Nothing is promoted into the implementer prompt or a hook yet. Candidates are in `candidates.md`.
- Next: check the `mechanical: yes` labels by hand on a sample, then try the best candidate on a real run and measure.

## Open questions

- Do the defect classes repeat across tasks, or only inside one task? Wave 1 counts per review, not per implementer prompt.
- A re-review lists defects of the first review again. Wave 1 flags re-reviews but does not de-duplicate.
- The corpus covers only this project's transcripts. Other projects may differ.
