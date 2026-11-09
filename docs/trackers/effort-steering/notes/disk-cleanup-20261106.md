---
id: ae9b1719e06dc453
kind: research-note
status: archived
title: Safe disk cleanup completed — 2026-11-06
tags:
- effort-steering
- migrated-memory
- historical
topic: effort-steering
interpretation: Historical snapshot; consult the experiment index for current state.
migrated_on: 2026-11-09
original_memory_topic: research/effort-steering/disk-cleanup-20261106
original_sha256: d0a898b7cc1041d13be3fa62dcf7c607d4da6d478858e167084895a8bffa301b
---

# Safe disk cleanup completed — 2026-11-06

Operator explicitly requested reclaiming disk without losing important information; autonomous effort-steering campaign remains protected. No product source changes, model reruns, image deletion or existing-container deletion.

## Removed, measured and verified

Evidence root /home/marius/.cache/codescout-resident/codescout/disk-cleanup-20261106.
1. Removed only codescout-gate/slot-0/debug under the EXISTING slot-0.lock exclusive lease continuously through delete+verification. Lock inode268 retained, no lock unlinked. du allocation26512560128bytes.
2. Removed24 known Cargo debug cache directories (.fingerprint/build/deps/incremental) from six CLOSED project copies across complete-workflow-calibration-20261105, complete-workflow-cold-recovery-20261105, complete-workflow-action-budget-20261105 and xhigh-patch-public-diagnostic-20261105. Actual non-target records141162 (regular-file SHA/size and literal links) identical before/after,11 retained top-level product-binary/cache-tag files unchanged,4 historical archived evidence SHA values reverified. No process or Docker container referenced old roots at both admission checks. du allocated-size sum37439565824bytes.
3. Removed exactly75 actually EMPTY anonymous Docker volumes via non-force volume rm. Initially inspected85 anonymous volumes readonly using existing pinned Python image, all entries examined; before each removal revalidated metadata/CreatedAt, no container references and empty filesystem. All75 names independently absent afterward. Kept all10 nonempty anonymous volumes: they contain PostgreSQL data, despite dangling/reclaimable status. Kept every named volume and all images, including SAB templates/future registered cases.

The allocation sum63952125952bytes (~59.56GiB) is NOT measured exclusive physical space recovered. Do not claim59.6GiB freed on disk. Btrfs compression/sharing, backups and concurrent writers matter. Final closure df available66350931968bytes (~61.79GiB),97% used, root device1999323004928bytes. /tmp separate32GiB tmpfs,25017774080bytes available at closure. Earlier initial broad df≈60GiB, intervening pre-cleanup≈47.8GiB; no exclusive causal df delta asserted.

## Snapshot retention actually observed

btrfs subvolume list confirms separate gate and main target subvolumes, two home snapshots2794/2795 and root snapshots3085/3086. Both home snapshots STILL contain action-budget/workflow-0-low/project/target/debug/deps after live-tree deletion. Actual btrfs fi du over that one experiment root in each reports Total16.31GiB/Exclusive0.00B/Set shared12.60GiB. These columns are not unique physical bytes that deleting a snapshot would free. Backups were not deleted or made writable. New MAIN issue482d36eb64f47818 documents disposable build caches being retained in home backups; no future/cache policy changed during active experiment.

SAB template/model volumes were deliberately kept: no confirmed dependency release. One dependency question sent via codescout-companion:reaching-peer-sessions to measured Claude2.1.312 SIDfbe38184-5a8d-4bc3-89dd-fdd3024c909a PID3944213; no reply/release inferred. Peer2038604 is2.1.317 outside measured socket protocol and was not contacted through that protocol. No broadcast or permission laundering. Do not infer unused means unreserved.

## Active experiment integrity / source facts

All79 repair-admission-bound proof files SHA matched after build-cache cleanup, all four exact actor/grader image IDs independently present. External controller @bg_000000cc remains existing job; no duplicate launched and no paid arm altered. At observed later checkpoint NUlow closed/reconciled and RAxhigh active; later two arms pending; private outcomes embargo unchanged. Active external-matched-repairs, runtime snapshots/models/LSP, original /tmp launch/grade scripts and current worktrees/source untouched. Old benchmark outcomes/costs/archives unchanged. Warm build-cache performance of discarded old target trees intentionally lost; do not rerun them to verify cleanup or interpret historical archives using current changed cache state.

Preflight originally rejected absent .rustc_info.json before deleting old targets. All six real Cargo caches use valid CACHEDIR.TAG signature and .fingerprint/build/deps markers; corrected classifier uses those actual markers. Original failed script/log/decision preserved, no fabricated marker. Initial Docker ps --size inventory failed during native container teardown (rw layer missing); empty stdout was NOT treated as empty container population; subsequent no-size inventories used.

New MAIN issue0054f6967e09530f records observed parallel-background reply/record handle mismatch; cf/d0 later command labels were opposite their initial concurrent replies. Cleanup operations launched sequentially and record commands/PIDs checked; no guessed handle used for deletion. Production fix deferred.

## Durable closure and remaining scope

MAIN docs/research/data/2026-11-06-disk-cleanup-evidence.tar.gz SHA f52c8d98bba780e1a6885049004f24a25ed94c5f443b2acc4fbb1eaf968ba131,8974214bytes,39 actual regular payload files reopened/hashverified. Matching disk-cleanup-preservation.json includes exact results, original inventory/failures,141162-record before manifest, scripts/preview/removal logs, volume metadata/probes and protection checks. No application/DB contents or credentials copied into it, only volume names/metadata and classification.

Task safely removed known disposable data; physical disk still97% used. Further major reclaim needs resolving SAB reservations and/or an independently reviewed backup/cache-retention policy. Do not delete persistent DB volumes, reserved images or backup snapshots just to make df improve. Existing current campaign should finish first; keep all important records and avoid regenerating cleared caches.
