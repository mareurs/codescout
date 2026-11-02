# Run storage cleanup 2026-11-01 — complete

Operator authorized reclaiming disk while retaining useful old-run evidence. Record root /home/marius/.cache/codescout-resident/codescout/run-storage-cleanup-20261101 (obtained with scripts/resident-scratch.sh; helper creates the empty directory). plan.json, failed cargo-clean actions/logs, successful pool-clean-actions.json, before/after df, result.json retained. No tracked repository source changes or model calls.

Measured resident archive allocation ~6.9GiB; latest in-repo blind challenger ~139MiB. Retained ALL resident archives, source snapshots, model outputs, charges, controls, seals and worktrees. Kept Docker entirely unchanged because contemporaneous sab images and other evaluation work are present; unused alone does not establish stale. Kept slot-0 warm cache and all lock files.

Cleaned ONLY /home/marius/.cache/codescout-gate/slot-1/debug and slot-2/debug. Acquired each existing slot-N.lock nonblocking EXCLUSIVE continuously through deletion and verification; verified inode unchanged, no lock unlinked. Verified real Cargo .rustc_info.json, debug/.fingerprint, debug/build, debug/deps; exact checked paths, no root symlinks. Used existing scripts/slot-pool.sh slot_remove_dir under held lease. Initial cargo clean attempts both exited101 because legacy pool lacks valid CACHEDIR.TAG; no deletion in those attempts; logs preserved. Did not fabricate marker or disable Cargo guard.

Allocated-size reduction across exact two cache roots =87,200,849,920 bytes (~81.2GiB), NOT a measurement of exclusive physical disk reclaimed (sharing/concurrent activity matter). df immediately before available171,039,059,968; after240,330,420,224 bytes, observed increase69,291,360,256 (~64.5GiB), includes concurrent machine activity. Final df-h /home 224G free,88% used (initial broad inspection166G free92%). No causal claim that all df delta belongs exclusively to cleanup.

Both recorded resident seal.json anchors unchanged. Latest N closure plus nested prior Q closure passed AFTER cleanup: verified-closed-negative-challenger,4870files/0literal links, commit441dc53ae545f936e5bba0ca7e3d8f17a639e000, costs unchanged24.195173699999999820/40. No source/test/build run needed for external cache deletion; don't rebuild emptied slots just to verify disk cleanup. Shared pool slots1/2 now cold and regenerate on future leases. Do not repeat cleanup from stale available-lock observations; reacquire actual existing lock for any future action.



## Ownership and re-check — 2026-11-02

Ownership: no session claimed this file. At the operator's request, session `1a35461f-23c2-41e6-ba35-052481f7dd96` (Claude, `.claude-stef`) took it over on 2026-11-02. That session did not run the cleanup. It checked the record against the disk.

Verified on 2026-11-02:
- The byte counts above match `result.json` and `df-before.txt` and `df-after.txt` in the record directory: reduction 87,200,849,920, available before 171,039,059,968, after 240,330,420,224, delta 69,291,360,256.
- The files named above exist in the record directory.
- The in-repo blind challenger is 139 MiB (`du -sm`). Commit `441dc53ae545f936e5bba0ca7e3d8f17a639e000` exists (`git cat-file -t` says `commit`).

Stale on 2026-11-02:
- Slot state: `slot-2/debug` does not exist, and `slot-1/debug` exists again at 22,171,684 kB (about 21.1 GiB). Slot 1 is not cold any more.
- Free space on `/home` is now 270,029,086,720 bytes (87% used).
- "166G free 92%" is not in the record files. `df-before.txt` says 171,039,059,968 bytes, about 159 GiB.

Not re-checked: the 6.9 GiB archive size and the two seal anchors.
