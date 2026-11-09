---
id: c1b7f1b0077ea052
kind: research-note
status: archived
title: Docker cache cleanup — 2026-11-01 19:37 UTC
tags:
- effort-steering
- migrated-memory
- historical
topic: effort-steering
interpretation: Historical snapshot; consult the experiment index for current state.
migrated_on: 2026-11-09
original_memory_topic: research/effort-steering/docker-cache-cleanup-20261101
original_sha256: e3e19b3f2f5f8477af9b7655e6516565494329a676ab4de541374cfcb235e264
---

# Docker cache cleanup — 2026-11-01 19:37 UTC

User asked whether cleanup can proceed, then continued interrupted read-only turn. Completed SAFE SUBSET only: `docker buildx prune --builder default --reserved-space 40gb --force` (no --all). Exit0, Docker reported Total18.53GB, default cache inventory186.3→167.8GB; remaining reclaimable41.08GB. Kept eduplanner-builder untouched. No image/container/volume-delete API or raw snapshot deletion requested. Full logs/plan/before+after inventories/calibration/result under /home/marius/.cache/codescout-resident/codescout/docker-cache-cleanup-20261101.

Physical disk result is NOT18.53GB measured freed: immediate df available241,490,489,344→241,384,194,048 bytes (delta -106,295,296); later df-h225GiB88%used. No positive physical free-space gain observed in that interval. Shared data/delayed GC/concurrent activity possible, no cause isolated. Do not claim18.53GB physical reclaimed.

SAB still ACTIVE: initially no SAB running container at instant first ps; afterward c23d5f9bce0e sab-grade-h4ckf0r0day__obscura-447-empty-57134719 appeared, then fd0873c24d84 sab-grade-h4ckf0r0day__obscura-447-gold-3173e2a9 was running at final check. Initial absence did NOT prove job complete. Preserve SAB images including future reserved arms. Concurrently three sab-test-export-2426917 imageIDs09bdfd185c32,243cb975a59f,a799c5169e3c and created container074ebee3c986 disappeared; origin not attributed. Own command only cache-prune. Initial strict invariant 'all before image/container IDs unchanged' failed; observed differences retained honestly, not hidden as own intentional cleanup.

Instrument friction: installed docker buildx du JSON humanizes sizes/times unlike latest documentation example. Read-only du `until=2h` returned same185records as unfiltered, including1h; inuse=false/shared=false each returned0 despite raw booleans173reclaimable/47private; private without '=value' rejected CLI. Thus filtered zero is NOT evidence of no old/reclaimable cache. Captured calibration outputs and abandoned proposed48h/inuse/shared predicate before any prune. Used documented standard builder-cache prune instead; do not reuse uncalibrated selectors or claim age-limited deletion. Official Docker buildx prune/du docs read for actual supported flags and protection of nonreclaimable records.

Own latest challenger+nestedpriorpopulation closure PASSED after cleanup (4870regular/0links, sealedcommit441dc53ae...). Raw evidence/source/charges/registration untouched, no paidcalls. Current docHEAD d50ba381d remains prior roadmapcommit (no repositorysourcechanges in cleanup).

Peer1025374 remains Claude2.1.314, outside skill's measured2.1.312 socket protocol; no message sent or owner response obtained. Do not infer owner approval from process cwd or lack of containers. Future image cleanup requires positive job/dependency status, not unused count or guessed snapshot-size attribution. Current safe cache cleanup completed; large SAB image deletion deferred while actual evaluations run.



## Ownership and re-check — 2026-11-02

Ownership: no session claimed this file. At the operator's request, session `1a35461f-23c2-41e6-ba35-052481f7dd96` (Claude, `.claude-stef`) took it over on 2026-11-02. That session did not run the prune. It checked the record against the disk and Docker. The text above is kept as written.

Verified on 2026-11-02:
- `result.json`, `prune-exit.json` and the `df-*.txt` files in the record directory match the figures above: Docker reported 18.53GB, the `df` delta was -106,295,296 bytes, the prune exited 0.
- `result.json` lists the three `sab-test-export-2426917` images and the container `074ebee3c986` as missing, as written above. That is the record, not a live check.
- Commit `d50ba381d` exists.

Stale on 2026-11-02:
- "SAB still ACTIVE": `docker ps` lists no `sab-*` container now. Many `sab-*` images remain (`docker images --filter reference='sab*'`).
- Build cache, from `docker system df`: 163 records, 167.9GB, 62.97GB reclaimable. The text above gives 41.08GB reclaimable after the prune.
- The note about peer 1025374: no process 1025374 exists now (`/proc/1025374` is missing).
- `docHEAD d50ba381d` was true at the time. The branch has moved on since.
- Free space on `/home` is now 270,029,086,720 bytes (87% used).

Not re-checked: the closure result and the `sab-grade` container names.
