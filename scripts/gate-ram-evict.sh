#!/usr/bin/env bash
# Empty the idle RAM slots of the gate's opt-in RAM tier. Takes no arguments; meant for a
# machine-local timer (a systemd user timer every 10 minutes is the expected shape; the timer
# is not in the repo). Design: docs/superpowers/specs/2026-10-31-gate-ram-tier-design.md, section 6.
#
# WHY IT EXISTS. A lease already empties idle RAM slots to make room (ram_pool_make_room), but
# only when a gate runs. RAM that nobody leases stays pinned by the zram device for as long as
# the machine is up, so something has to run when no gate does.
#
# A slot is emptied when `flock -n` takes its lock (the ONLY proof it is idle, see the header of
# scripts/slot-pool.sh) AND one of:
#   - its last lease is at least CODESCOUT_GATE_RAM_IDLE_MIN minutes old. The last-lease time is
#     the mtime of <slot>/.codescout-lessee, which every lease rewrites. Lock-file mtimes are NOT
#     usable: every scan reopens the lock files.
#   - it has no stamp (a slot nobody has leased since the stamp was lost counts as the oldest);
#   - MemAvailable is under CODESCOUT_GATE_RAM_MEM_FLOOR_MB. Low memory is a reason to evict, so
#     this script never calls ram_tier_check_floor, which refuses instead.
# The removal happens while the lock is held. The slot DIRECTORY goes; the lock file stays (lock
# files are never unlinked, and never created here). Only slot-0 ... slot-(RAM_SLOTS-1) are touched,
# and only a real directory: a symlink is never followed.
#
# stdout: one line per emptied slot, naming it and the reason. Nothing else is printed there.
# Exit 0: the tier is not configured, nothing is due, or slots were emptied.
# Exit 2: the config is invalid, the pool is not a zram mount with a RAM cap, or an argument was given
#         (stderr says which). Exit 1: a slot could not be emptied (stderr says which).

set -u

if [ $# -ne 0 ]; then
    echo "usage: gate-ram-evict.sh   (takes no arguments; configure it as scripts/slot-pool.sh does)" >&2
    exit 2
fi

. "$(dirname "${BASH_SOURCE[0]}")/slot-pool.sh" || exit 2

who="gate-ram-evict.sh"

# The return code says whether a config exists; RAM_SKIP only says whether it is wrong.
if ! ram_tier_load_config "$who"; then
    [ -n "${RAM_SKIP:-}" ] || exit 0
    echo "$RAM_SKIP" >&2
    exit 2
fi
[ "$RAM_SLOTS" -gt 0 ] || exit 0
# Nothing is removed from a path that is not a capped zram mount.
if ! ram_tier_check_mount "$who"; then
    echo "$RAM_SKIP" >&2
    exit 2
fi
command -v flock >/dev/null 2>&1 || { echo "$who: flock not found; no slot can be proven idle" >&2; exit 2; }

now=$(date +%s)
avail=$(ram_mem_available_mb)
pressure=0
if [[ "$avail" =~ ^[0-9]+$ ]]; then
    [ "$avail" -ge "$RAM_MEM_FLOOR_MB" ] || pressure=1
else
    echo "$who: cannot read MemAvailable from ${CODESCOUT_GATE_RAM_MEMINFO:-/proc/meminfo}; the memory-pressure rule is skipped" >&2
fi

rc=0
for ((n = 0; n < RAM_SLOTS; n++)); do
    dir="$RAM_POOL/slot-$n"
    lock="$dir.lock"
    [ -e "$lock" ] || continue              # never create a lock file; no lock, no proof of idleness
    exec {fd}>"$lock" || continue
    if flock -n "$fd"; then
        # Everything below is decided under the lock, so a lease cannot change the slot meanwhile.
        if [ -d "$dir" ] && [ ! -L "$dir" ]; then
            stamp="$dir/.codescout-lessee"
            reason=""
            m=$(stat -c %Y -- "$stamp" 2>/dev/null) || m=""
            if ! [[ "$m" =~ ^[0-9]+$ ]]; then
                reason="no stamp"
            else
                age=$(((now - m) / 60))
                if [ "$age" -ge "$RAM_IDLE_MIN" ]; then
                    reason="idle for $age min"
                fi
            fi
            if [ -z "$reason" ] && [ "$pressure" -eq 1 ]; then
                reason="memory pressure: MemAvailable ${avail}M is below the ${RAM_MEM_FLOOR_MB}M floor"
            fi
            if [ -n "$reason" ]; then
                if slot_remove_dir "$dir"; then
                    echo "$who: emptied $dir ($reason)"
                else
                    echo "$who: could not empty $dir ($reason)" >&2
                    rc=1
                fi
            fi
        fi
    fi
    exec {fd}>&-
done
exit "$rc"
