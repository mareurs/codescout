---
kind: convention
status: active
title: Gate RAM tier — how to set it up on one machine
owners: []
tags:
  - testing
  - ci
  - gate
  - shared-target
  - zram
---

# Gate RAM tier — how to set it up on one machine

The RAM tier is an opt-in pool of build trees on a zram filesystem. `scripts/gate.sh` tries it before the disk pool. Design and measurements: [`docs/superpowers/specs/2026-10-31-gate-ram-tier-design.md`](../superpowers/specs/2026-10-31-gate-ram-tier-design.md). Code: `scripts/slot-pool.sh` and `scripts/gate-ram-evict.sh`.

**None of it is installed by the repo.** The repo holds the scripts and this document. The zram drop-in, the pool unit, the config file and the timer exist only on a machine where an operator creates them. A machine without `gate-pool.env` behaves as it did before the tier existed, and prints nothing about it.

The examples below use the user name `<user>` and the checkout path `<checkout>`. Replace them.

## 1. The zram device (root)

Create `/etc/systemd/zram-generator.conf.d/codescout-gate.conf`:

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

`zram0` stays the swap device. `zram1` is the gate device.

The gate needs these settings:

- `zram-resident-limit` must be above 0. The gate refuses a zram device with no RAM cap, because it cannot check a budget against no cap.
- `options` must include `discard`. Without it, a deleted file never frees its zram RAM. The pool then looks full to the RAM budget, and the budget keeps refusing leases.
- A freed block can reach `mm_stat` only after the next journal commit. Run `sync`, then read `mm_stat` again, before you decide that discard does not work.

The contents of the device are lost at every unmount and reboot. The next lease builds cold.

## 2. The pool directory (root)

The root directory of the new filesystem belongs to root. A oneshot unit must make the pool directory for the operator after each mount. It must also set the ext4 root reserve to 0. By default ext4 reserves 5% for root, and `df` hides that reserve from the operator.

Create `/etc/systemd/system/codescout-gate-ram-pool.service`:

```ini
[Unit]
Description=Pool directory for the codescout gate RAM tier
Requires=var-cache-codescout\x2dgate\x2dram.mount
After=var-cache-codescout\x2dgate\x2dram.mount

[Service]
Type=oneshot
RemainAfterExit=yes
ExecStart=/usr/bin/tune2fs -m 0 /dev/zram1
ExecStart=/usr/bin/install -d -o <user> -g <user> /var/cache/codescout-gate-ram/pool

[Install]
WantedBy=multi-user.target
```

`var-cache-codescout\x2dgate\x2dram.mount` is the output of `systemd-escape -p --suffix=mount /var/cache/codescout-gate-ram`. Then run `systemctl daemon-reload` and `systemctl enable codescout-gate-ram-pool.service`.

## 3. The config file (operator)

Create `${XDG_CONFIG_HOME:-$HOME/.config}/codescout/gate-pool.env`. The values below are the defaults. Only `CODESCOUT_GATE_RAM_POOL` has no default.

```
CODESCOUT_GATE_RAM_POOL=/var/cache/codescout-gate-ram/pool
CODESCOUT_GATE_RAM_SLOTS=2
CODESCOUT_GATE_RAM_GROWTH_MB=4096
CODESCOUT_GATE_RAM_COLD_MB=10240
CODESCOUT_GATE_RAM_MEM_FLOOR_MB=12288
CODESCOUT_GATE_RAM_IDLE_MIN=60
CODESCOUT_GATE_RAM_CARGO_ENV=1
```

| Key | Meaning |
|---|---|
| `CODESCOUT_GATE_RAM_POOL` | Absolute path of the RAM pool. Unset means no RAM tier. |
| `CODESCOUT_GATE_RAM_SLOTS` | Number of RAM slots. `0` means no RAM slots: every lease uses the disk pool, and the lease prints no RAM-tier message. |
| `CODESCOUT_GATE_RAM_GROWTH_MB` | Growth that the budget assumes for a run in a warm slot. |
| `CODESCOUT_GATE_RAM_COLD_MB` | Logical size of a cold slot. The budget assumes a small slot still needs `COLD` minus its size. |
| `CODESCOUT_GATE_RAM_MEM_FLOOR_MB` | `MemAvailable` below this value skips the RAM tier at lease time. The evictor uses the same value as its pressure limit. |
| `CODESCOUT_GATE_RAM_IDLE_MIN` | Idle minutes after which the evictor empties a slot. |
| `CODESCOUT_GATE_RAM_CARGO_ENV` | `1` exports `CARGO_PROFILE_DEV_DEBUG=line-tables-only` and `CARGO_INCREMENTAL=0` in a RAM lease. A variable that the caller already set is left alone. `0` turns this off. Any other whole number also counts as off, and the tier does not warn about it. |

### File format

`ram_tier_load_config` parses the file. It never sources the file.

- Write plain `KEY=VALUE` lines. Do not write `export`, indentation, spaces around `=`, or quotes.
- Use the exact key names from the table.
- The parser ignores blank lines and lines that start with `#`.
- The parser ignores every line that does not start with `CODESCOUT_GATE_RAM_`. This includes a line with `export` or with indentation. It ignores them silently, so such a key has no effect.
- A line that starts with `CODESCOUT_GATE_RAM_` must match `^CODESCOUT_GATE_RAM_[A-Z_]+=[^[:space:]]*$`. Otherwise the whole RAM tier is skipped, with a message that names the file and the line.
- A misspelled key that matches that shape (uppercase letters and underscores only) is also ignored silently. For example, `CODESCOUT_GATE_RAM_SLOT=3` does nothing.
- A quote is part of the value. A quoted number is not a whole number, and a quoted path is not absolute. Both skip the RAM tier, with a message.
- A number must be a whole number of 1 to 12 digits. A value such as `48G` skips the RAM tier, with a message that names the key and the value. The gate then uses the disk pool.
- A non-empty environment variable of the same name overrides the value in the file.

Every skip writes one line to stderr in the form `<who>: RAM tier skipped: <cause> (<knob>)`.

## 4. The evictor timer (operator)

A lease empties idle RAM slots only when a gate runs. Something else must run when no gate does. `scripts/gate-ram-evict.sh` is that script. It takes no arguments. Point the timer at your main checkout, not at a worktree that you may delete.

Create `~/.config/systemd/user/codescout-gate-ram-evict.service`:

```ini
[Unit]
Description=Empty idle slots of the codescout gate RAM tier

[Service]
Type=oneshot
ExecStart=<checkout>/scripts/gate-ram-evict.sh
```

Create `~/.config/systemd/user/codescout-gate-ram-evict.timer`:

```ini
[Unit]
Description=Run the codescout gate RAM evictor every 10 minutes

[Timer]
OnBootSec=10min
OnUnitActiveSec=10min

[Install]
WantedBy=timers.target
```

Then run `systemctl --user daemon-reload` and `systemctl --user enable --now codescout-gate-ram-evict.timer`.

### What the evictor does

The evictor checks each slot from `slot-0` to `slot-<SLOTS-1>`. It empties a slot only when both of these are true:

- `flock -n` takes the slot lock. A slot that a run holds is never emptied. The lock is the only proof that the slot is idle.
- The slot is due, for one of these reasons:
  - The `.codescout-lessee` stamp is `CODESCOUT_GATE_RAM_IDLE_MIN` minutes old or older.
  - The slot has no stamp. A slot with no stamp counts as the oldest.
  - `MemAvailable` is below `CODESCOUT_GATE_RAM_MEM_FLOOR_MB`. Then every unheld slot is due, whatever its idle age.

The evictor removes the slot directory under the lock. It leaves the lock file. It skips a slot that has no lock file. It prints one line to stdout for each emptied slot.

If it cannot read `MemAvailable`, it prints a note to stderr on each run. An unreadable `MemAvailable` does not count as pressure.

### Exit codes

| Code | Meaning |
|---|---|
| 0 | Nothing is due, or the tier is not configured. |
| 1 | A removal failed. |
| 2 | An argument was given, `flock` is missing, the config is invalid, or the pool is not a zram mount. |

Exit 2 for "not a zram mount" makes the timer unit go failed on every tick while the mount is down.

`CODESCOUT_GATE_RAM_SLOTS=0` makes the evictor exit 0 before it checks the mount. A bad pool with zero slots is therefore not reported.

### Two limits of the idle rule

1. The idle clock runs from the start of the last lease, not from its end. The stamp (`.codescout-lessee`) is written when the lease starts. A run that lasts longer than `CODESCOUT_GATE_RAM_IDLE_MIN` is due as soon as it releases its slot.
2. A lease from outside a git tree leaves no stamp. `slot_evict_foreign_members` removes the stamp when the lessee is empty. That slot counts as due at the next timer tick after it becomes idle.

## 5. Naming a RAM slot

`scripts/with-slot.sh --slot ram-N` names exactly one RAM slot. A named slot never falls back to the disk pool, and the budget rule does not apply to it. The command stops with exit 2 when the tier is not configured or admitted, `N` is out of range, or the slot is held.

## 6. Go-live checks

Do these four checks on the machine before you rely on the tier.

1. **A green gate in a RAM slot.** Run `./scripts/gate.sh`. The output must contain a line that starts with `gate.sh: tier RAM (zram1)`, and `GATE EXITS` must show all lanes at 0. The design measurements ran the test lanes as `--no-run` only.
2. **Discard frees RAM.** Read `cat /sys/block/zram1/mm_stat` and note field 3 (`mem_used_total`). Delete an idle slot, for example `rm -rf /var/cache/codescout-gate-ram/pool/slot-0`. Run `sync`. Read `mm_stat` again. Field 3 must fall. The fall can lag until the next journal commit.
3. **The real ratio.** With a full slot, divide field 1 (`orig_data_size`) by field 3. The result must stay above 2.2. A lower ratio means the RAM cap can fill before the filesystem does.
4. **Growth of a warm slot.** Run the gate several times, and once after a `Cargo.lock` change. Measure the slot with `du -sm`. The growth per run must stay under `CODESCOUT_GATE_RAM_GROWTH_MB` (4096).
