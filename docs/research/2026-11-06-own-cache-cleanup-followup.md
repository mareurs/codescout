---
id: '4dcc14505cbc396a'
kind: report
status: done
title: Own closed experiment cache cleanup follow-up
---

# Own closed experiment cache cleanup follow-up

Operator requested cleanup limited to this effort-steering work. Removed20Cargo cache directories (.fingerprint/deps/build/incremental) from5targets: public-profile-reference-v2, public-profile-cold-v1, two closed hint-authority grader targets, and original Vtarget. Current gate slot and new fixworktree/campaign/runtime/model inputs preserved. No images, containers, volumes, source, logs, receipts, costs, top-level product binaries or cache tags deleted.

## Measured preservation

Removed paths' allocated du sum21486379008bytes (~20.01GiB); not exclusive physical reclaimed space. Before deletion: no process/container references, existing Cargo locks held where present; live118input hashes and4exact requiredimage IDs checked. All52683non-target regular-file/link records identical before/after; zero preservation deltas; all118protectedinputs/images independently checked again. Cache removal job@bg_000000f2 exited0. Do not refill old caches as a cleanup check; their historical warm-cache timing no longer reproducible without restoration.

Rootdf available observed52383207424bytes before removal,67176591360after; later57.86GiB. Concurrent live graders/other jobs and Btrfs sharing/backups prevent exclusive attribution. Dataset inventory total56072634368allocated bytes before cleanup is directory allocation, not physical unique disk usage. Current external private grader and compact gate left running. New qualification was still waiting at initial inspection.

## Evidence

Archive docs/research/data/2026-11-06-own-cache-cleanup-followup-evidence.tar.gz SHA256b442d400cd366c81f17edbb767bf5985d952c27964c5b0f757fead5f21f7d17a,3210924bytes,10actualpayloadfilesreopened/hashverified. Includes complete52683record manifest, inventory, exactremovalpaths/receipts/script, protectedhashes and spaceobservations. Summary docs/research/data/2026-11-06-own-cache-cleanup-followup-preservation.json. Residentroot /home/marius/.cache/codescout-resident/codescout/disk-cleanup-followup-20261106.

## Final supplement: completed compact gate cache

During this cleanup, compact gate@bg_000000ed finishedexit0; Pythonlight/FMT/CLIPPY/LEAN/DEFAULT all0. Saved its complete output before deleting onlyslot-0/debug under the existing exclusive slot-0.lock inode268 after no referencing processes were observed. Removed additional5649510400du-allocatedbytes. Total for this follow-up27135889408bytes (~25.27GiB allocated), not exclusive physical space freed. All118protectedinput hashes checked again afterward; source/results remain intact.

Final rootavailable53717123072bytes (~50.03GiB); concurrent writers continue, so no causal before/after free-space claim. Original four-arm controller@bg_cc exited0 and results.json now exists. No own live grader container was found at the final Docker check. New public qualifier@bg_ee exited1 before any public command: expectedimageHEADmatched, but native startup added `.codescout/private-memories/` to .gitignore and the clean-source guard rejected it. Exactcontainer removed/independentlyabsent. FiledMAIN bug1720ba9339aa9ad6 immediately. No automatic retry/new paid run.

Supplement archive docs/research/data/2026-11-06-own-cache-cleanup-followup-closure.tar.gz SHA2569cefd8fd24f34e86e8d611c33a4d5c11d0762ab8ef4465897466d090e45193a2,252856bytes,4payloadfilesreopened/hashverified: gateoutput, gatecachecleanupscript/receipt and finalclosure. Original10payloadarchive remains unchanged. No cache regeneration performed as cleanup validation.

## Remaining candidates and protections

Old native qualification directories contain repeated Jina weights; globalcanonicalmodelcache remains. No model deletion in this pass. Kept old top-level compiled products and all research data. Registered actor/grader images, SAB template volumes, active gatecache, active fixworktree and backup snapshots remain protected. Largest transient ownDockergrader layers should be removed by the existing grader lifecycle after results capture; do not force-remove running work.
