# Gate RAM Tier Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an opt-in, machine-local RAM tier to the gate's slot pool: up to 2 slots on a zram filesystem, tried before the disk pool, with a shared pool budget, LRU eviction and an idle evictor.

**Architecture:** All tier logic lives in `scripts/slot-pool.sh`, behind `lease_gate_target`, so `gate.sh` and `with-slot.sh` get it together. Without a local config, nothing new runs and nothing new prints. A separate `scripts/gate-ram-evict.sh` empties idle RAM slots and is run by a machine-local systemd user timer.

**Tech Stack:** Bash 5, util-linux (`flock`, `findmnt`), coreutils (`df`, `du`, `stat`), zram via zram-generator 1.2.1.

**Spec:** `docs/superpowers/specs/2026-10-31-gate-ram-tier-design.md` (commit `6ef568eae`). Read it before any task.

## Global Constraints

- Config file: `${XDG_CONFIG_HOME:-$HOME/.config}/codescout/gate-pool.env`. An environment variable of the same name overrides the file value.
- Keys and defaults: `CODESCOUT_GATE_RAM_POOL` (unset = no tier), `CODESCOUT_GATE_RAM_SLOTS=2`, `CODESCOUT_GATE_RAM_GROWTH_MB=4096`, `CODESCOUT_GATE_RAM_COLD_MB=10240`, `CODESCOUT_GATE_RAM_MEM_FLOOR_MB=12288`, `CODESCOUT_GATE_RAM_IDLE_MIN=60`, `CODESCOUT_GATE_RAM_CARGO_ENV=1`.
- Pool rule: `need_i = max(GROWTH, COLD − size_i)` over every held RAM slot, this lease included. Admit when `df` free MB ≥ Σ need_i AND (`mm_stat` field 4 − field 3) in MiB ≥ Σ need_i × 10 / 22.
- The RAM tier never makes an unnamed lease return 2. A malformed value, a missing mount, a low floor or a short budget each skip to disk with one message naming the cause and the knob. The one exception is an explicitly named `--slot ram-N`, which refuses with 2 rather than fall back (next line but one).
- Without a configured `CODESCOUT_GATE_RAM_POOL`, `slot-pool.sh`, `gate.sh` and `with-slot.sh` behave exactly as today, and print nothing new.
- Every removal happens under the slot's `flock`. Lock files are never unlinked.
- RAM slots are `<RAM_POOL>/slot-N` with `<RAM_POOL>/slot-N.lock`, N in `0 … RAM_SLOTS-1`. `with-slot.sh --slot ram-N` names one; a named slot never falls back to the other tier.
- In a RAM slot, export `CARGO_PROFILE_DEV_DEBUG=line-tables-only` and `CARGO_INCREMENTAL=0` when `RAM_CARGO_ENV=1`, unless the caller already set either one.
- The disk ceiling `CODESCOUT_SLOT_CEILING_MB` does not apply to RAM slots.
- The last-lease time of a slot is the mtime of `<slot>/.codescout-lessee`. A slot with no stamp counts as the oldest.
- Test seams (documented as such in the code): `findmnt` and `df` are resolved through `PATH`; `CODESCOUT_GATE_RAM_MEMINFO` (default `/proc/meminfo`) and `CODESCOUT_GATE_RAM_MMSTAT` (default `/sys/block/<dev>/mm_stat`) override the two kernel files. CI (`gate-slot-tests`, `ubuntu-latest`) has no zram device.
- Every test case that expects the RAM tier to be used sets `CODESCOUT_GATE_RAM_MEMINFO` to a fixture with `MemAvailable` far above the floor. The real `/proc/meminfo` of a CI runner or of a busy machine may be below 12G, which would turn those cases into flakes.
- Shared checkout: commit by pathspec with a `Session-Id:` trailer, per `docs/conventions/shared-checkout-commit-sequence.md`. Each task's code and its tests land in one commit.

## Review Focus

1. **The operator's real config leaking into the suite.** Once this machine has `~/.config/codescout/gate-pool.env` or an exported `CODESCOUT_GATE_RAM_*`, the existing cases A–R must still see the disk pool. `run_gate` and `run_with` must unset `XDG_CONFIG_HOME` and every `CODESCOUT_GATE_RAM_*` variable (Task 1).
2. **The Python light lane's fixture.** `tests/test_python_runner.py:136` replaces `slot-pool.sh` with an empty file and presets `CARGO_TARGET_DIR`. Any new call in `gate.sh` must be guarded on `GATE_TIER`, which only the real `lease_gate_target` sets, or the light lane reds with `command not found` (Task 5).
3. **A zram device with no RAM cap** (`mm_stat` field 4 = 0). The budget check would pass forever. The tier must be skipped with a message (Task 1).
4. **A config file that is not plain `KEY=VALUE`.** A value such as `$(touch x)` must never run, and a non-RAM key in the file (for example `CODESCOUT_GATE_POOL=/x`) must be ignored. The file is parsed, never sourced (Task 1).
5. **A RAM slot whose directory was emptied by the evictor while its lock file stays.** The next lease must recreate it and build, and its missing stamp must make it the first LRU candidate (Tasks 3 and 6).

---

### Task 1: Config loading and admission checks

**Files:**
- Modify: `scripts/slot-pool.sh` (new functions after `slot_remove_dir`, and a header paragraph naming the RAM tier and the spec)
- Test: `tests/gate-slot.sh` (helpers `run_gate`, `run_with`; new case S before the summary at the end)

**Interfaces:**
- Produces:
  - `ram_tier_load_config <who>` → returns 0 when the tier is configured and every value is a whole number; returns 1 otherwise. Sets globals `RAM_POOL RAM_SLOTS RAM_GROWTH_MB RAM_COLD_MB RAM_MEM_FLOOR_MB RAM_IDLE_MIN RAM_CARGO_ENV`. Sets `RAM_SKIP` to a one-line reason on an invalid value; leaves it empty when the tier is simply not configured.
  - `ram_tier_check_mount <who>` → returns 0 or 1 (with `RAM_SKIP`): the mount source is `/dev/zram*` and `mm_stat` field 4 is > 0. Sets `RAM_DEV` (for example `zram1`) and `RAM_MMSTAT`.
  - `ram_mem_available_mb` → prints `MemAvailable` from `${CODESCOUT_GATE_RAM_MEMINFO:-/proc/meminfo}` in MiB.
  - `ram_tier_check_floor <who>` → returns 0 or 1 (with `RAM_SKIP`): `ram_mem_available_mb` ≥ `RAM_MEM_FLOOR_MB`.
  - `ram_tier_admit <who>` → `ram_tier_check_mount && ram_tier_check_floor`.
  - Helper `ram_mmstat_field <n>` → prints field n of `$RAM_MMSTAT`.

- [ ] **Step 1: Make the suite hermetic.** In `run_gate` and `run_with`, add `-u XDG_CONFIG_HOME` to the `env` call and unset every `CODESCOUT_GATE_RAM_*` variable present in the suite's environment (for example by building `-u` arguments from `compgen -v CODESCOUT_GATE_RAM_`). Run `bash tests/gate-slot.sh`. Expected: the existing summary line with `0 failed`.

- [ ] **Step 2: Write case S, failing.** Add `$WORK/ramstub/findmnt` (prints the contents of `$WORK/findmnt-source`) and a fixture writer for `$WORK/home/.config/codescout/gate-pool.env`. Since Task 2 is what leases, case S observes admission through `lease_gate_target` via `run_with … sh -c 'echo "$CARGO_TARGET_DIR"'`, and the message on stderr. Assertions:
  - `S1` no config file → target is `$POOL/slot-0`, and `grep -c 'RAM tier' out` is `0`;
  - `S2` config names `$WORK/ram`, `findmnt-source` is `/dev/nvme0n1p2` → target is the disk slot, and out contains `RAM tier skipped` and `not a zram mount`;
  - `S3` `CODESCOUT_GATE_RAM_MEM_FLOOR_MB=12G` in the file → disk slot, exit status 0, out names `CODESCOUT_GATE_RAM_MEM_FLOOR_MB` and `12G`;
  - `S4` zram mount, `CODESCOUT_GATE_RAM_MEMINFO` fixture with `MemAvailable: 4194304 kB` and floor 12288 → disk slot, out names `MemAvailable` and `CODESCOUT_GATE_RAM_MEM_FLOOR_MB`;
  - `S5` zram mount, `mm_stat` fixture `0 0 0 0 0 0 0 0 0` → disk slot, out contains `no RAM cap`;
  - `S6` file line `CODESCOUT_GATE_RAM_SLOTS=$(touch $WORK/pwned)` → `$WORK/pwned` does not exist, and out names the key;
  - `S7` file line `CODESCOUT_GATE_POOL=$WORK/elsewhere` → target is still under `$POOL`;
  - `S8` an exported `CODESCOUT_GATE_RAM_POOL` in the suite's own shell does not reach a `run_gate` call (target under `$POOL`).

  Run `bash tests/gate-slot.sh`. Expected: S2–S5 FAIL (no message), others may pass.

- [ ] **Step 3: Implement the Task 1 functions in `scripts/slot-pool.sh`.** Parse the file line by line: skip blanks and `#` lines; ignore lines that do not start with `CODESCOUT_GATE_RAM_`; a line that starts with it but does not match `^CODESCOUT_GATE_RAM_[A-Z_]+=[^[:space:]]*$` makes the config invalid, with `RAM_SKIP` naming the key. Never `source` or `eval` the file. An environment value overrides the file. Admission order: mount source from `findmnt -no SOURCE -T "$RAM_POOL"` must match `/dev/zram[0-9]*`; `mm_stat` field 4 must be > 0; `MemAvailable` (kB ÷ 1024) must be ≥ the floor. Each `RAM_SKIP` text names the knob, for example `RAM tier skipped: MemAvailable 4096M is below the 12288M floor (CODESCOUT_GATE_RAM_MEM_FLOOR_MB)`. Wire both into `lease_gate_target` only as far as printing `RAM_SKIP` to stderr and continuing to the disk lease (the RAM lease comes in Task 2).

- [ ] **Step 4: Run** `bash tests/gate-slot.sh`. Expected: every S assertion PASS and the summary shows `0 failed`.

- [ ] **Step 5: Commit** `scripts/slot-pool.sh tests/gate-slot.sh`: `feat(gate): load an opt-in RAM-tier config and admit only a capped zram mount`.

### Task 2: Bounded RAM lease in `lease_gate_target`

**Files:**
- Modify: `scripts/slot-pool.sh` (`slot_lease`, `lease_gate_target`; new `lease_ram_target`)
- Modify: `scripts/with-slot.sh` (the usage text only: document `--slot ram-N` and that a named slot never falls back to the other tier)
- Test: `tests/gate-slot.sh` (new case T)

**Interfaces:**
- Consumes: `ram_tier_load_config`, `ram_tier_admit` (Task 1).
- Produces:
  - `slot_lease <pool> <prefix> <who> [slot] [max]` — new optional 5th argument. With `max` set and no `slot`, it tries `0 … max-1` only and returns **1**, holding nothing, when all are held. The 3- and 4-argument calls (`mutation-probe.sh:171`, `lease_gate_target`) are unchanged.
  - `lease_ram_target <who> [n]` → 0 with `SLOT SLOT_FD SLOT_DIR` set, `GATE_POOL=$RAM_POOL`, `GATE_TIER=ram`; 1 otherwise.
  - `lease_gate_target <who> [slot]`: `slot` of the form `ram-N` leases exactly RAM slot N (refusing, return 2, when held, absent from the range, or the tier is not admitted); a numeric `slot` leases the disk slot as today; no `slot` tries RAM first, then disk. Sets `GATE_TIER=ram|disk` whenever it leases.

- [ ] **Step 1: Write case T, failing.** With a valid config, a zram `findmnt` stub, a `mm_stat` fixture with a 20 GiB cap, and a `df` stub (`$WORK/ramstub/df`) that models a real filesystem: it prints, as `Available` in `df -Pm` format, the value of `$WORK/df-cap` minus `du -sm` of the RAM pool (default cap 1000000):
  - `T1` a gate run's lanes all see `$RAM/slot-0`;
  - `T2` with `slot-0.lock` held (`hold_lock`), the run sees `$RAM/slot-1`;
  - `T3` with both RAM locks held, the run sees `$POOL/slot-0`, and no `$RAM/slot-2` directory or lock appears;
  - `T4` after creating the directory `$RAM/slot-1` (a named lease keeps `slot_lease`'s existing refusal of an absent slot), `run_with … bash "$WITH" --slot ram-1 …` sees `$RAM/slot-1`; with `slot-1.lock` held it exits 2 and runs nothing;
  - `T5` `--slot ram-1` with no config exits 2 and names the RAM tier;
  - `T6` the lessee eviction still runs in a RAM slot: `$RAM/slot-0/.codescout-lessee` holds the fake checkout's path after T1;
  - `T7` `CODESCOUT_GATE_RAM_SLOTS=0` means no RAM slots: the run sees the disk slot, says nothing about a RAM tier, and exits 0.

- [ ] **Step 2: Run** `bash tests/gate-slot.sh`. Expected: T1–T6 FAIL.

- [ ] **Step 3: Implement** the 5th argument of `slot_lease`, `lease_ram_target`, and the dispatch in `lease_gate_target`. Rely on the return codes of `ram_tier_load_config` and `ram_tier_admit`, never on the `RAM_*` globals after a failure (they keep stale values). Print `RAM_SKIP` as is: it already starts with `<who>: `. `RAM_SLOTS=0` is a valid config and means no RAM slots. The RAM path runs `slot_evict_foreign_build_scripts` and `slot_evict_foreign_members` exactly as the disk path does, skips `slot_tend`, and exports `CARGO_TARGET_DIR`. `mkdir -p "$SLOT_DIR"` happens in the lease, because an emptied RAM slot keeps only its lock file.

- [ ] **Step 4: Run** `bash tests/gate-slot.sh`. Expected: all cases PASS, `0 failed`. Also run `bash tests/mutation-probe.sh`. Expected: its own summary with no failures.

- [ ] **Step 5: Commit** `scripts/slot-pool.sh tests/gate-slot.sh`: `feat(gate): lease a RAM slot before the disk pool, bounded to CODESCOUT_GATE_RAM_SLOTS`.

### Task 3: Pool budget and LRU eviction

**Files:**
- Modify: `scripts/slot-pool.sh` (new `ram_slot_need_mb`, `ram_pool_fits`, `ram_pool_make_room`; called from `lease_ram_target`)
- Test: `tests/gate-slot.sh` (new case U)

**Interfaces:**
- Consumes: `lease_ram_target` (Task 2), `ram_mmstat_field` (Task 1).
- Produces:
  - `ram_slot_need_mb <tree>` → prints `max(RAM_GROWTH_MB, RAM_COLD_MB − du -sm <tree>)`; a missing tree has size 0.
  - `ram_pool_fits <who>` → 0 when both budgets cover Σ need over the own slot and every other RAM slot whose lock `flock -n` cannot take; 1 otherwise, with `RAM_SKIP` naming both budgets and both sums.
  - `ram_pool_make_room <who>` → empties idle RAM slots (lock taken with `flock -n` and held during `rm -rf`), oldest `.codescout-lessee` mtime first, no-stamp first of all, never the own slot, re-checking `ram_pool_fits` after each; returns 0 when it fits, 1 when no idle slot is left.

- [ ] **Step 1: Write case U, failing.** Use `CODESCOUT_GATE_RAM_COLD_MB=8 CODESCOUT_GATE_RAM_GROWTH_MB=2`, `fill` (2 MiB per call) for slot contents, and the Task 2 `df` stub, so free space is `df-cap` minus what the pool holds and emptying a slot frees its size. `du -sm` rounds up, so a 2 MiB `fill` may read as 3: in each sub-case, set `df-cap` to the measured `du -sm` of the pool plus the free value stated below, and read the slot sizes the need formula sees from `du -sm` too. The values below assume exactly 2 MiB per `fill`:
  - `U1` own slot empty, `df-cap` 7 → need 8 > 7 free, no other slot → run goes to `$POOL`, out names both budgets;
  - `U2` own slot empty, `df-cap` 9 → run stays in `$RAM/slot-0`;
  - `U3` with `CODESCOUT_GATE_RAM_SLOTS=3`: idle `slot-1` (2 MiB, stamp touched 2 h ago), idle `slot-2` (2 MiB, stamp touched 1 h ago), `df-cap` 11 → free 7 < 8, emptying `slot-1` gives 9 → `slot-1` is gone, `slot-2` is kept, run in `slot-0`;
  - `U4` same shape, but `slot-2` has no stamp → `slot-2` is emptied and `slot-1` (2 h stamp) is kept;
  - `U5` held `slot-1` with 6 MiB, own `slot-0` with 4 MiB, `df-cap` 13 → free 3; need = max(2, 8−4) + max(2, 8−6) = 6 > 3 → run goes to disk, and both slots keep their contents;
  - `U6` only the RAM budget is short: `mm_stat` fixture with cap − used = 1 MiB, `df-cap` 1000000 → disk, out names `mm_stat`.

- [ ] **Step 2: Run** `bash tests/gate-slot.sh`. Expected: U1–U6 FAIL.

- [ ] **Step 3: Implement** the three functions and call `ram_pool_fits || ram_pool_make_room` in `lease_ram_target` after the lessee evictions. On failure, release the RAM lock (`exec {SLOT_FD}>&-`), unset `SLOT SLOT_DIR SLOT_FD`, and return 1 so `lease_gate_target` leases from disk. The RAM ratio uses integer math: `ram_left_mib * 22 >= sum_need_mb * 10`.

- [ ] **Step 4: Run** `bash tests/gate-slot.sh`. Expected: `0 failed`.

- [ ] **Step 5: Commit** `scripts/slot-pool.sh tests/gate-slot.sh`: `feat(gate): admit a RAM lease against the shared pool budget and evict idle slots LRU first`.

### Task 4: Build settings inside the RAM tier

**Files:**
- Modify: `scripts/slot-pool.sh` (`lease_gate_target`)
- Test: `tests/gate-slot.sh` (new case V)

**Interfaces:**
- Consumes: `GATE_TIER` (Task 2), `RAM_CARGO_ENV` (Task 1).
- Produces: in a RAM lease with `RAM_CARGO_ENV=1`, exported `CARGO_PROFILE_DEV_DEBUG=line-tables-only` and `CARGO_INCREMENTAL=0`, each only when unset.

- [ ] **Step 1: Write case V, failing**, using `run_with … sh -c 'echo "${CARGO_PROFILE_DEV_DEBUG:-<unset>} ${CARGO_INCREMENTAL:-<unset>}" > "$1"'`:
  - `V1` RAM lease → `line-tables-only 0`;
  - `V2` RAM lease with preset `CARGO_INCREMENTAL=1` → `line-tables-only 1`;
  - `V3` RAM lease with `CODESCOUT_GATE_RAM_CARGO_ENV=0` → `<unset> <unset>`;
  - `V4` disk lease (both RAM locks held) → `<unset> <unset>`.

- [ ] **Step 2: Run** `bash tests/gate-slot.sh`. Expected: V1 and V2 FAIL.

- [ ] **Step 3: Implement** the two conditional exports in `lease_gate_target`'s RAM branch.

- [ ] **Step 4: Run** `bash tests/gate-slot.sh`. Expected: `0 failed`.

- [ ] **Step 5: Commit** `scripts/slot-pool.sh tests/gate-slot.sh`: `feat(gate): build RAM-tier slots with line-tables-only debug info and no incremental`.

### Task 5: Tier reporting in `gate.sh`

**Files:**
- Modify: `scripts/slot-pool.sh` (new `ram_tier_report`), `scripts/gate.sh` (after the `CARGO_TARGET_DIR` echo near line 112, after the pool-total line near line 180, after `GATE EXITS` near line 183)
- Test: `tests/gate-slot.sh` (new case W)

**Interfaces:**
- Consumes: `GATE_TIER`, `RAM_MMSTAT`, `RAM_DEV` (Tasks 1–2).
- Produces: `ram_tier_report` → prints `gate.sh: tier RAM (<dev>): <field1 in G>G logical, <field3 in G>G RAM of <field4 in G>G cap, <df free>G free`.

- [ ] **Step 1: Write case W, failing.** Make the stub `cargo` exit with `${GATE_TEST_EXIT:-0}` (a one-line change to the heredoc), then:
  - `W1` a RAM run prints one `gate.sh: tier RAM (` line near the top and one `ram_tier_report` line after the tree size;
  - `W2` a RAM run with `GATE_TEST_EXIT=1` prints a line after `GATE EXITS` that contains `RAM tier` and `No space left on device`;
  - `W3` a disk run with `GATE_TEST_EXIT=1` and no config prints no line containing `RAM tier`;
  - `W4` `python3 ./scripts/python-tests.py --lane light` passes (the empty-fixture `slot-pool.sh` path; this is the lane the gate itself runs).

- [ ] **Step 2: Run** `bash tests/gate-slot.sh`. Expected: W1 and W2 FAIL.

- [ ] **Step 3: Implement** `ram_tier_report` and three `gate.sh` lines, each guarded by `[ "${GATE_TIER:-}" = ram ]`, so the fixture `slot-pool.sh` never reaches an undefined function. The red-run line is printed when any of `PYTHON_LIGHT FMT CLIPPY LEAN DEFAULT` is non-zero.

- [ ] **Step 4: Run** `bash tests/gate-slot.sh` and `python3 ./scripts/python-tests.py --lane light`. Expected: `0 failed` and a passing pytest summary.

- [ ] **Step 5: Commit** `scripts/slot-pool.sh scripts/gate.sh tests/gate-slot.sh`: `feat(gate): print the RAM tier and its zram figures, and name the tier on a red run`.

### Task 6: Idle and pressure evictor

**Files:**
- Create: `scripts/gate-ram-evict.sh` (executable)
- Test: `tests/gate-slot.sh` (new case X)

**Interfaces:**
- Consumes: `ram_tier_load_config`, `ram_tier_check_mount`, `ram_mem_available_mb` (Task 1), `slot_remove_dir`. It does not call `ram_tier_check_floor`, because low memory is a reason to evict, not to stop.
- Produces: `scripts/gate-ram-evict.sh` — no arguments. Exit 0 when the tier is not configured or nothing is due. Exit 2 with a message on an invalid config or a pool that is not a zram mount. For each `slot-N` (N < `RAM_SLOTS`) whose directory exists and whose lock `flock -n` takes: empty it when its stamp is older than `RAM_IDLE_MIN` minutes, when it has no stamp, or when `MemAvailable` is below the floor. One line per emptied slot.

- [ ] **Step 1: Write case X, failing:**
  - `X1` no config → exit 0, no output;
  - `X2` idle slot with a stamp touched 61 min ago → emptied; lock file still exists;
  - `X3` idle slot with a stamp touched 10 min ago → kept;
  - `X4` held slot with a stamp touched 2 h ago → kept;
  - `X5` idle slot with a 10 min stamp and `MemAvailable` below the floor → emptied;
  - `X6` after X2, a gate run leases that RAM slot and its lanes see it (Review Focus 5).

- [ ] **Step 2: Run** `bash tests/gate-slot.sh`. Expected: X1–X6 FAIL (the script does not exist).

- [ ] **Step 3: Implement** `scripts/gate-ram-evict.sh`, sourcing `slot-pool.sh` the same way `with-slot.sh` does. Age in minutes is `( $(date +%s) − $(stat -c %Y <stamp>) ) / 60`.

- [ ] **Step 4: Run** `bash tests/gate-slot.sh`. Expected: `0 failed`.

- [ ] **Step 5: Commit** `scripts/gate-ram-evict.sh tests/gate-slot.sh`: `feat(gate): empty idle or pressured RAM slots from a timer-driven evictor`.

### Task 7: Machine-setup document and full gate

**Files:**
- Create: `docs/conventions/gate-ram-tier.md`
- Modify: `docs/conventions/gate-ordering.md` (one pointer line), `scripts/slot-pool.sh` header (one line pointing at the new doc)

- [ ] **Step 1: Write `docs/conventions/gate-ram-tier.md`** with: the zram-generator drop-in from spec §1 verbatim; a oneshot unit `codescout-gate-ram-pool.service` (`After=` and `Requires=` the mount unit of `/var/cache/codescout-gate-ram`, running `tune2fs -m 0 /dev/zram1` and `install -d -o <user> -g <user> /var/cache/codescout-gate-ram/pool`); an example `gate-pool.env` naming `/var/cache/codescout-gate-ram/pool` and the defaults; a systemd user timer `codescout-gate-ram-evict.timer` (`OnBootSec=10min`, `OnUnitActiveSec=10min`) with its service running `<checkout>/scripts/gate-ram-evict.sh`; and the four go-live checks from the spec's Testing section. State that none of it is installed by the repo. State two limits of the idle rule: the idle clock runs from the start of the last lease (the mtime of `.codescout-lessee`), not from its end; and a lease from outside a git tree leaves no stamp (`slot_evict_foreign_members` removes it), so that slot counts as due at the next timer tick once it is idle. State the config file format as `ram_tier_load_config` parses it: plain `KEY=VALUE` lines, no `export`, no indentation, no quotes, exact key names. A line that does not start with `CODESCOUT_GATE_RAM_` is ignored silently, which includes an `export` or indented line; a misspelled key that does start with it is also ignored silently. State that `CODESCOUT_GATE_RAM_SLOTS=0` means no RAM slots, so every lease uses disk.

- [ ] **Step 2: Run** `bash tests/gate-slot.sh`. Expected: `0 failed`.

- [ ] **Step 3: Run** `./scripts/gate.sh`. Expected: `GATE EXITS -> FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0` and `PYTHON_LIGHT=0`, and no line containing `RAM tier` (this machine has no config yet). If a lane is red in files this plan did not touch, record it and do not fix it here.

- [ ] **Step 4: Commit** the three doc paths: `docs(conventions): how to set up the gate RAM tier on one machine`.

### Task 8: Rollout on this machine (operator-gated)

Every step that uses `sudo` needs the operator's explicit approval in the session at that moment. Do not run them from a subagent.

- [ ] **Step 1:** With approval, install the drop-in and the oneshot unit from `docs/conventions/gate-ram-tier.md`, then `sudo systemctl daemon-reload` and start the zram1 setup unit and the oneshot unit. Check: `findmnt -no SOURCE,FSTYPE,OPTIONS /var/cache/codescout-gate-ram` shows `/dev/zram1 ext4` with `discard`, and `awk '{print $4}' /sys/block/zram1/mm_stat` prints `21474836480`.
- [ ] **Step 2:** Write `~/.config/codescout/gate-pool.env`. Run `scripts/with-slot.sh sh -c 'echo $CARGO_TARGET_DIR'`. Expected: `/var/cache/codescout-gate-ram/pool/slot-0`.
- [ ] **Step 3: Go-live check 1.** Run `./scripts/gate.sh`. Expected: all lanes exit 0 in the RAM slot, with the `tier RAM` lines.
- [ ] **Step 4: Go-live checks 2 and 3.** Record `mm_stat` fields 1 and 3 after the run (ratio against 2.2), then `scripts/with-slot.sh --slot ram-0 sh -c 'rm -rf "$CARGO_TARGET_DIR"/*'` and confirm that field 3 falls.
- [ ] **Step 5: Go-live check 4.** Over the next several gate runs and one `Cargo.lock` change, record the slot size after each run against the 4096 MiB `GROWTH`.
- [ ] **Step 6:** Install and start the user timer. Check `systemctl --user list-timers codescout-gate-ram-evict.timer`.
- [ ] **Step 7:** Write the four results into the spec under a new `## Go-live results (YYYY-MM-DD)` section and commit it.
