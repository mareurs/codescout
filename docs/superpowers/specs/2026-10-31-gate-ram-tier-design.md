# Gate RAM tier — a compressed in-memory slot pool for `scripts/gate.sh`, opt-in per machine

Date: 2026-10-31
Status: draft, not yet approved
Related: `docs/issues/2026-10-30-the-gate-wall-time-is-recorded-nowhere-and-dev-debug-info-makes-huge-test-binaries.md` (debug-info measurement), `docs/issues/2026-10-31-a-shell-and-cargo-resolve-different-sccache-binaries-so-shell-sccache-commands-reach-the-wrong-server.md` (found while measuring)

## Intent

The gate's build pool (`~/.cache/codescout-gate`, leased by `scripts/slot-pool.sh`) uses a lot of disk on this machine. On 2026-10-31 it held 77G in three slots, and `/home` was 96–97% full. The operator wants the gate to build in RAM on this machine. Free RAM is limited, so the RAM use must have a hard cap and careful eviction.

Other contributors keep today's disk pool. The RAM tier is switched on only by a local, untracked configuration. Without that configuration, the scripts behave exactly as they do today.

## Goals

- Up to **2** gate slots live in RAM, on a compressed zram filesystem, with **at most 20G of RAM** in total.
- A lease takes a free RAM slot first. It uses the disk pool only when both RAM slots are held, or when an admission check fails.
- RAM slots stay warm between runs, because a rebuild after an eviction costs about 4.4 min (measured below). They are evicted under pressure and after an idle period.
- The RAM tier is prevented by sizing from making a gate red that would otherwise be green, and when it does happen anyway, the gate names the RAM tier as the cause. When the tier cannot be used safely at lease time, the lease goes to disk and says why. There is no automatic retry of a failed lane on disk (operator decision, 2026-10-31, builder-check Q1).

## Non-goals

- No change to `Cargo.toml`. The `debug = 1` dev profile stays for plain `cargo` builds (the 2026-10-30 issue left that decision to the operator).
- No change to `cargo rb` or `target/release`. `gate.sh` already explains why the release build stays in the repo's `target/`.
- No change to the mutation-probe pool (`scripts/mutation-probe.sh` calls `slot_lease` and `slot_tend` directly, with its own pool `${ROOT}.worktrees`, not through `lease_gate_target`).
- No change to sccache placement or size. The measured cache for one gate config is 611 MiB, so the default 10 GiB cap is not the constraint.
- No change to CI.

## Anti-goals (a failure even if it technically works)

- A red gate caused by the RAM tier that reads like a code failure (for example `No space left on device` or an I/O error in the middle of a lane) **with nothing that names the tier**. Such a failure is made unlikely by the pool admission rule (§2), and the gate's output must make it recognisable when it happens (§7).
- Gate data written to the root filesystem because the zram mount is missing and the mount-point directory is a plain directory.
- Memory pressure that makes earlyoom kill user applications. This happened before: SYS-010 in `~/.cargo/config.toml`, 2026-09-30.
- Any behaviour change for a machine that has no local configuration.

## Measurements (2026-10-31, this machine)

64 cores, 125G RAM. `MemAvailable` was 31–57G across the samples. Swap was 100% full (32G swapfile plus 16G zram swap on `zram0`). Builds used `CARGO_BUILD_JOBS=16`, on a shared machine with other sessions running, so the times are noisy. Trees were built cold in a scratch target dir with the gate's three cargo commands, with the test lanes as `--no-run` (tests write their temp dirs outside `target/`).

| Config | Tree after clippy / lean / default | Final tree | Cold time | Per-4KiB-page ratio (deflate-1, zram-like) | RAM in zram (estimate) |
|---|---|---|---|---|---|
| Today (`debug = 1`, incremental on) | 2.1G / 7.5G / 20.0G | 20.0G (`deps` 13.8G, `incremental` 5.3G, `build` 0.6G) | 383 s | `deps` 2.59x, `incremental` 2.33x, `build` 2.48x | ≈7.8G |
| Variant (`CARGO_PROFILE_DEV_DEBUG=line-tables-only`, `CARGO_INCREMENTAL=0`) | 1.2G / 3.8G / 10.0G | 10.0G (`deps` 9.2G, `build` 0.6G) | 338 s | `deps` 2.97x, `build` 2.48x | ≈3.4G |

Rebuild after an eviction, variant config, isolated sccache with a 40G cap:

| Run | clippy | lean | default | Total | sccache |
|---|---|---|---|---|---|
| Fill (empty cache) | 129 s | 75 s | 114 s | 318 s | 1149 misses (851 Rust, 284 C/C++, 14 asm) |
| Warm | 112 s | 58 s | 96 s | **266 s** | 99.83% hits; 285 non-cacheable calls (211 for crate type) |

Other facts:

- Live slots were 24–27G against a cold floor of 20G. Most of the difference was `incremental` (9.8G in slot-0 against 5.3G cold).
- In slot-0, `deps` is 9.8G of our own crates and test binaries and 3.4G of third-party crates. The 45 files in `tests/*.rs` each link a test binary of about 250–350M.
- The real gate takes 968–1065 s (2026-10-30 issue), and running the tests is most of it.
- Lock-file mtimes show 5 concurrent slots on 2026-10-31 and 6 on 2026-10-30. Two slots were held while these measurements ran.
- The per-page ratios come from a Python estimate that compresses each 4 KiB page alone with deflate level 1. A real zram device with zstd was not measured. Same-filled pages were 0.2–0.5%.

## Constraints

- **The lock is the only proof that a slot is idle** (`scripts/slot-pool.sh`, header). Every removal in the RAM tier must happen under that slot's `flock`, the same as on disk.
- **Lock files are never unlinked** (`scripts/slot-pool.sh`, header). On the RAM filesystem, the only removal is the unmount at shutdown, when no process can hold a lock.
- **zram pages cannot be reclaimed by the kernel.** A full RAM tier holds its RAM until files are deleted. Deleted files give RAM back only through discard. zram-generator mounts with `discard` by default (`man zram-generator.conf`, `options=`).
- **Hitting the zram RAM cap fails writes with I/O errors.** Filling the filesystem fails writes with `ENOSPC`. So the logical size (`zram-size`) must fill before the RAM cap (`zram-resident-limit`) is reached.
- Any build setting must be set where `gate.sh` and `with-slot.sh` both get it, which is `lease_gate_target`. Otherwise the two would rebuild over each other in one slot.
- `/sys/block/zramN/mm_stat` is world-readable. Its fields are `orig_data_size compr_data_size mem_used_total mem_limit mem_used_max same_pages pages_compacted huge_pages huge_pages_since`. `mem_limit` itself is root-only, but `mm_stat` field 4 reports it.

## Approaches

| | Approach | Trade-off | Outcome |
|---|---|---|---|
| A | zram filesystem as a RAM tier inside `slot-pool.sh`, with variant build settings in that tier | Compressed (about 3x), hard RAM cap, clean `ENOSPC` before the cap. Costs a one-time root setup and new lease logic plus tests | **Chosen by the operator** |
| B | plain tmpfs tier with `size=` | Simplest: one `fstab` line. No compression, so two variant slots fill 20G at their cold size. tmpfs pages can only go to swap, which is full | Rejected |
| C | no RAM tier; apply the variant to every gate lease on disk | About 30G for three slots instead of 77G, no root, no RAM. Does not give the RAM tier the operator asked for | Rejected as the whole answer. A carries C's settings inside the RAM tier |

Other ideas considered and dropped: (1) a bigger sccache cap, because the measured cache is 611 MiB and the warm rebuild saved only 16%; (2) a zstd snapshot of an evicted slot restored at lease time, because it adds disk writes and a staleness rule, and the operator chose "rebuild from sccache"; (3) merging the 45 integration tests into one binary, because it changes the repo for everyone and is a separate decision.

## Decision (ADR)

**Decision:** `lease_gate_target` gets an optional RAM tier: up to 2 slots on a zram-backed filesystem, tried before the disk pool, enabled only by a machine-local configuration file, with `line-tables-only` debug info and incremental compilation off inside that tier.

**Context:** `/home` is 96–97% full and the pool is 77G. RAM for this is capped at 20G and swap is already full. The variant config makes a slot about 3.4G of RAM, so two warm slots fit with room to grow. A rebuild costs about 4.4 min, so slots should stay warm.

**Alternatives considered:** B (tmpfs: no compression, no room to grow) and C (disk only: no RAM tier), above.

**Consequences:**
- now easier: gate runs on this machine stop writing about 20G per slot to `/home`. Two concurrent runs build in RAM. The disk pool sees only the third and later concurrent runs.
- now harder: one more failure mode (the RAM filesystem fills or the cap is hit). A root-owned piece of machine setup. A debugger on a RAM-tier binary loses local variables (line tables remain). `slot-pool.sh` gains a tier concept and its tests grow.

**Change scenarios absorbed:**
- The RAM budget changes: only the local config and the zram-generator file change.
- Another machine wants the tier: it adds the same two local files. No repo change.
- The repo later adopts `line-tables-only` in `Cargo.toml`: the RAM tier's build env becomes redundant and can be dropped without touching the lease logic.

**Revisit-when:**
- `gate.sh` reports a RAM-tier slot whose RAM per logical byte is worse than 1/2.2 (the cap could be hit before the filesystem fills).
- Concurrent gate runs regularly exceed 2, so most runs land on disk anyway.
- `MemAvailable` regularly falls below the floor, so the tier is skipped most of the time.

**Confidence:** medium. The size and time numbers are measured. The zram per-page ratio is an estimate, discard freeing zram RAM is documented but not measured here, and the full test lanes were not run under the variant settings.

## Design

### 1. Machine layer (this machine only, not in the repo)

A zram-generator drop-in, for example `/etc/systemd/zram-generator.conf.d/codescout-gate.conf`:

```ini
[zram1]
compression-algorithm = zstd
# logical size: fills (ENOSPC) before the RAM cap at the worst measured ratio, 2.33x
zram-size = 45056
# RAM cap, MiB
zram-resident-limit = 20480
fs-type = ext4
mount-point = /var/cache/codescout-gate-ram
options = discard,noatime
```

`zram0` stays the swap device. The mounted filesystem's root directory is owned by root, so the setup must also create a pool directory owned by the operator after each mount, for example with a oneshot unit ordered after the mount unit that runs `tune2fs -m 0 <device>` (ext4 reserves 5% for root by default, which `df` hides from the operator) and `install -d -o marius -g marius /var/cache/codescout-gate-ram/pool`. The contents are lost at every unmount and reboot. That is accepted: the next lease builds cold.

The repo should carry this example as documentation (not as an installed file), in `docs/conventions/` next to the gate docs, so another machine can copy it.

### 2. Local opt-in configuration

A file outside every checkout: `${XDG_CONFIG_HOME:-$HOME/.config}/codescout/gate-pool.env`, read by `scripts/slot-pool.sh`. A file is chosen over environment variables alone because the gate runs from many shells (four Claude Code profiles, Codex sessions, native shells), and a file reaches all of them. An environment variable of the same name overrides the file value, the same way the existing `CODESCOUT_GATE_POOL` works.

| Key | Meaning | Proposed default |
|---|---|---|
| `CODESCOUT_GATE_RAM_POOL` | directory of the RAM pool. Unset means no RAM tier | unset |
| `CODESCOUT_GATE_RAM_SLOTS` | number of RAM slots | 2 |
| `CODESCOUT_GATE_RAM_GROWTH_MB` | remaining growth assumed for a run in a warm RAM slot | 4096 |
| `CODESCOUT_GATE_RAM_COLD_MB` | logical size of a cold slot, so the remaining growth of a run in a small or empty slot | 10240 (measured 10.0G) |
| `CODESCOUT_GATE_RAM_MEM_FLOOR_MB` | `MemAvailable` below which the RAM tier is skipped | 12288 |
| `CODESCOUT_GATE_RAM_IDLE_MIN` | idle minutes after which the evictor empties a RAM slot | 60 (operator, 2026-10-31) |
| `CODESCOUT_GATE_RAM_CARGO_ENV` | whether the tier exports the variant build settings | 1 |

The values must be validated as whole numbers. Unlike `slot_tend`, which returns 2 and stops the gate on a value such as `48G`, a malformed RAM-tier value makes the lease **skip the RAM tier** and use the disk pool, with a message that names the key and the bad value. The disk path keeps every existing bound, so nothing is switched off without a word, and a typo in one machine-local file cannot block every session on the machine. (Builder-check Q3, settled by the designer.)

**The budget belongs to the pool, not to each slot (operator decision, 2026-10-31).** No RAM slot has a ceiling of its own. The RAM slots share one budget, which is the zram filesystem (44G logical, about 43G usable once the ext4 root reserve is set to 0, §1) and its 20G RAM cap. So a small slot leaves room for a big one, and the RAM goes where the builds need it.

**Pool admission rule (replaces a per-slot sizing rule from builder-check Q2).** Two RAM slots can build at the same time, so one lease must count the run in the other slot too. For each held RAM slot *i*, this lease included, the remaining growth is

    need_i = max(GROWTH, COLD − size_i)

A warm slot needs only `GROWTH` (4G). A small or empty slot needs the rest of a cold build (up to 10G). The lease is admitted when both budgets cover the sum of the needs:

- logical: the filesystem's free space (`df`) ≥ Σ need_i;
- RAM: `mm_stat` field 4 (the cap) − field 3 (`mem_used_total`) ≥ Σ need_i ÷ 2.2, the revisit ratio.

Each lease takes its slot's lock **before** it computes the rule, so two leases that start together each count the other one. The other run's `size_i` is read with `du` while that run writes, so it is an estimate, and the formula then estimates what that run still has to write.

The residual risk is a warm run that grows by more than `GROWTH`, for example after a `Cargo.lock` change rebuilds many third-party crates under new hashes. The growth of a warm variant slot per run is not measured; see Testing. At the worst measured page ratio, 2.33x, the full 43G of usable logical space needs about 18.5G of RAM, which is under the 20G cap. So the filesystem fills (with `ENOSPC`) before the cap is reached, as §1 requires.

### 3. Lease order (`lease_gate_target`)

1. A preset `CARGO_TARGET_DIR` is honoured untouched, as today (in `gate.sh` and `with-slot.sh`).
2. If the RAM tier is configured and admitted (§4), try RAM slots `0 … RAM_SLOTS-1` with `flock -n`. The first free one wins. The search is bounded: it never creates a RAM slot numbered `RAM_SLOTS` or higher.
3. If a RAM slot is won: run the existing lessee evictions (`slot_evict_foreign_build_scripts`, `slot_evict_foreign_members`), apply the pool admission rule with LRU eviction (§4.d), export the tier's build env (§5), and export `CARGO_TARGET_DIR`. The disk ceiling `CODESCOUT_SLOT_CEILING_MB` does not apply to RAM slots.
4. Otherwise, lease from the disk pool exactly as today.
5. Print one line that names the tier and, for RAM, why it was chosen or skipped (for example `gate.sh: RAM tier skipped: MemAvailable 9.8G is below the 12G floor (CODESCOUT_GATE_RAM_MEM_FLOOR_MB)`).

`with-slot.sh --slot N` must still name exactly one tree. It needs a way to name a RAM slot too (for example `--slot ram-0`). The exact spelling is left to the builder, but a named slot must never fall back to the other tier.

### 4. Admission checks for the RAM tier

a. **Configured:** `CODESCOUT_GATE_RAM_POOL` is set. Otherwise skip silently, which is the behaviour for every other contributor.
b. **Really a zram mount:** the pool's filesystem is a mount point and its source is a `/dev/zram*` device (for example `findmnt -no SOURCE -T <pool>`). Otherwise skip with a message. This prevents gate data from going to the root filesystem when the mount is missing.
c. **Memory floor:** `MemAvailable` from `/proc/meminfo` is at least `CODESCOUT_GATE_RAM_MEM_FLOOR_MB`. Otherwise skip with a message.
d. **Pool budget, checked after the slot is leased** (the rule in §2): if the logical or the RAM budget does not cover Σ need_i, empty idle RAM slots under their lock, **least recently leased first**, and compute the rule again after each one. "Idle" means `flock -n` succeeds. "Least recently leased" means the oldest `.codescout-lessee` stamp (§6). A slot with no stamp counts as the oldest. If the budget still does not fit after every idle slot is empty (for example, the other slot is held and large), release the RAM lease and go to disk with a message that names both budgets. The lease never empties its own slot to make room, because a cold build there would need more than the warm slot it removes.
e. *(Merged into d: the RAM budget is now part of the pool rule.)*

### 5. Build settings inside the RAM tier

When `CODESCOUT_GATE_RAM_CARGO_ENV=1` and a RAM slot is leased, export `CARGO_PROFILE_DEV_DEBUG=line-tables-only` and `CARGO_INCREMENTAL=0`, unless the caller already set either one. These halve a slot (20.0G → 10.0G) and remove `incremental`, the main source of slot growth. They apply only inside the RAM tier, so plain `cargo` builds and the disk pool keep `debug = 1`. Because a slot's tier never changes, every lease of a slot builds it with the same settings.

### 6. Eviction

- **On every lease** (no daemon needed): the pool admission rule with LRU eviction of idle RAM slots (§4.d), and the existing lessee evictions.
- **Idle and pressure eviction** (needs something that runs when no gate does): a committed script, for example `scripts/gate-ram-evict.sh`, that uses the functions in `slot-pool.sh`. For each RAM slot that `flock -n` proves idle, it empties the slot when the slot's last lease is older than `CODESCOUT_GATE_RAM_IDLE_MIN`, or when `MemAvailable` is below the floor. The last-lease time is the mtime of the slot's `.codescout-lessee` stamp, which `slot_evict_foreign_members` rewrites on every lease. Lock-file mtimes are not usable: every `slot_lease` scan reopens them. A machine-local systemd user timer (for example every 10 min) runs the script. The timer is not in the repo.

A daemon started inside a run (sccache, measured in F-173) inherits the lock fd and pins its slot until it exits. sccache exits after its idle timeout, so a RAM slot can be pinned for that long after a run. This costs RAM for a while, never correctness.

### 7. Reporting

At the end of a run, `gate.sh` prints the tier with the tree size. For a RAM slot it also prints the zram figures from `mm_stat`: logical data, RAM used and the RAM cap, for example `gate.sh: tier RAM (zram1) — 10.1G logical, 3.5G RAM of 20.0G cap`. This is how the revisit trigger (a ratio worse than 1/2.2) becomes visible.

When any lane exits non-zero in a RAM slot, the gate also adds one line to its closing summary, next to `GATE EXITS`. The line names the RAM tier, prints the `mm_stat` figures and the filesystem's free space, and says that a `No space left on device` or I/O error in the lane output means the tier, not the code. It is printed on every red run in a RAM slot, because the gate does not read lane output to decide.

### 8. Failure modes

| Failure | What happens | Mitigation |
|---|---|---|
| zram mount missing (boot failure, config removed) | admission b fails | disk pool, with a message |
| Filesystem fills during a lane | cargo fails with `No space left on device`, and the gate is red | the pool admission rule (§2, §4.d) counts the remaining growth of every run in flight, so this needs a warm run that grows by more than `GROWTH`. When any lane fails in a RAM slot, the gate's failure summary names the tier and prints the `mm_stat` figures, so a reader sees that the red may be environmental. No automatic retry (Goals) |
| RAM cap reached before the filesystem fills (ratio worse than 2.2x) | I/O errors in a lane | `zram-size` chosen from the worst measured ratio. `mm_stat` printed every run. Revisit trigger |
| Machine under memory pressure | zram pages cannot be reclaimed | floor check at lease time. The evictor empties idle slots under pressure |
| Reboot | RAM slots are gone | the next lease builds cold (about 5–6 min) |
| A daemon pins a RAM slot | that slot stays held | the second RAM slot or the disk pool serves the run. The daemon's exit frees it |

### 9. What does not change

Without `gate-pool.env` (and without the environment variables), `slot-pool.sh`, `gate.sh` and `with-slot.sh` must behave exactly as they do today. A test must pin this.

## Testing

`tests/gate-slot.sh` already drives the real `gate.sh` with a stub `cargo` that records the `CARGO_TARGET_DIR` each lane saw, and with `HOME` and `CODESCOUT_GATE_POOL` in a temp dir. The RAM tier can be tested the same way, with a temp directory standing in for the RAM pool. The cases need:

- no configuration → identical to today (the stub sees the disk pool);
- configuration present, RAM pool is not a zram mount → disk pool and a message (the zram check must be injectable for tests, for example through an overridable `findmnt` on `PATH`);
- both RAM slots held → disk pool;
- `MemAvailable` below the floor → disk pool (the `/proc/meminfo` path must be injectable);
- pool budget too small, one RAM slot idle → that slot is emptied, the lease stays in RAM;
- pool budget too small, two RAM slots idle besides this one (with `RAM_SLOTS=3` in the test) → the least recently leased one is emptied first, and the other stays when that is enough;
- pool budget too small, other RAM slot held → disk pool, and the held slot is untouched;
- the need of a held slot is counted: an empty held slot needs `COLD`, a warm one needs `GROWTH`, and the logical and RAM budgets are each checked (the `df` and `mm_stat` readings must be injectable);
- the lease never empties its own slot;
- the RAM tier exports the variant env, and a preset value is honoured;
- the evictor empties an idle slot older than the limit and leaves a held or recent one;
- `with-slot.sh` naming a RAM slot never falls back to disk;
- a lane that exits non-zero in a RAM slot makes the summary print the RAM-tier line, and a red run on disk does not print it.

Before the tier is used for real, these must be verified on this machine:

1. one full `./scripts/gate.sh` in a RAM slot with the variant settings, green, because the measurements above ran the test lanes as `--no-run` only;
2. that deleting a slot lowers `mm_stat` `mem_used_total` (discard really frees zram RAM);
3. the real zram ratio of a full slot (`mm_stat` field 1 / field 3) against the 2.2x revisit line;
4. how much a warm variant slot grows per run, over several gate runs and one `Cargo.lock` change, against the 4G `GROWTH` allowance.

## Rollout

1. Repo changes behind the opt-in, with tests. Gate green.
2. Machine setup: the zram-generator drop-in, the pool-directory unit, `gate-pool.env`. Verify the three points under Testing.
3. The evictor timer (machine-local systemd user timer, every 10 min, idle limit 60 min).

## Open questions

None open. Settled by the operator on 2026-10-31: the idle limit is 60 min, and the evictor timer is kept (§6, Rollout step 3).

## Left to the builder

- The exact naming of RAM slots for `with-slot.sh --slot`.
- How `slot_lease` gets its bounded search (a new parameter or a separate loop).
- The mechanism that creates the operator-owned pool directory after each mount.
- The wording of the tier messages, within the rule that every skip names its reason and the knob that controls it.
