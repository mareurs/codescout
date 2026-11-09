---
id: '9664b022f3d04aa9'
kind: research-note
status: archived
title: RAM clarification — 2026-11-07
tags:
- effort-steering
- migrated-memory
- historical
topic: effort-steering
interpretation: Historical snapshot; consult the experiment index for current state.
migrated_on: 2026-11-09
original_memory_topic: research/effort-steering/baseline-first-oracle-ram-clarification-20261107
original_sha256: 37debe1e27ff8d84afe1f7e9c2a9b3127a7f557838b2bd1e0e973b15acdc524c
---

# RAM clarification — 2026-11-07

At 2026-11-07T22:46:21+02:00 a fresh `free -h` measured 125 GiB physical RAM and about 45 GiB MemAvailable; swap 47 GiB essentially full. Prior wording implying insufficient installed RAM overstated the evidence. Resource stops are evaluator admission/host-headroom policy: offline_single_job.main requires 28 GiB MemAvailable for three samples 10 s apart, continuous host floor 24 GiB, while native container hard limit stays 8 GiB / 4 CPU. These are our experimental configuration thresholds, not measured memory demand. Prior original xhigh interruption at roughly 22.375 GiB host available and recorded zero cgroup OOM kills is not evidence that RAM was exhausted or xhigh tests failed. Prior wait-only attempts performed no native compilation. Current fresh availability exceeds registered admission threshold; no extra RAM is demonstrated necessary. This clarification does not launch a run, change guards, invalidate previous valid low cells, or claim xhigh test outcomes. Both paid authors remain frozen; remaining comparisons are offline and need no author/model rerun. Old resource-blocked status was a timestamped headroom condition, not permanent capacity diagnosis.
