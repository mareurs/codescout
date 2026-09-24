# scripts/slot-pool.sh — the leased pool of build trees behind scripts/gate.sh,
# scripts/with-slot.sh and scripts/mutation-probe.sh. SOURCE it; it runs nothing itself.
#
# A run takes the lowest-numbered free slot and holds a `flock` on it until it exits, so
# a tree is reused by whichever run comes next rather than kept per session: keying on
# the session id left 323G in 17 trees on 2026-09-24 (bug 1da62c896d649aa6). The lock
# sits on an fd every child inherits, deliberately WITHOUT `flock -o`: with `-o`,
# SIGKILLing the leasing script frees the slot while its cargo is still writing into it;
# without it, a daemon started mid-run (sccache, measured) pins one slot, which costs disk
# and never correctness (bug-fix-session-log:F-173).
#
# A pool of long-lived trees needs two more bounds, and `slot_tend` applies both on every
# lease (bug b085022bc2f05c36). Cargo never garbage-collects a target dir, so one slot
# grows toward the longest-lived tree on the machine: 104G measured beside a 17G slot on
# 2026-09-24. A slot over CODESCOUT_SLOT_CEILING_MB is therefore emptied, and that one
# run builds cold. And the high-water mark never fell, so free slots numbered KEEP or
# higher are removed by the next run to come along.
#
# THE LOCK IS THE ONLY PROOF A TREE IS IDLE, which is why the pruning lives here and not
# in a cron job or a warning. `flock -n` succeeding is the one observation that no leased
# run is inside a slot, so every removal happens under that lock. A tree with no lock
# file (a session that typed its own CARGO_TARGET_DIR, bug 294ba0ae7ed8c7b1) cannot be
# proven idle, and nothing here touches it.
#
# LOCK FILES ARE NEVER UNLINKED. A run that opened `slot-N.lock` just before the unlink
# could lock the orphaned inode while the next run creates and locks a new file, and two
# runs would each hold "slot-N". An empty file per slot number ever reached is the cost.

[ "${BASH_SOURCE[0]}" = "$0" ] && { echo "slot-pool.sh: source this file, do not run it" >&2; exit 2; }

# Chosen above the largest per-session tree measured (33G), which lived a whole session
# of ordinary use, and below the 104G a long-lived tree reached. Three kept slots then cap
# the gate pool near 144G.
SLOT_CEILING_MB_DEFAULT=49152

# slot_lease <pool> <prefix> <who> — take the lowest free slot and hold it on an fd every
# child inherits. Sets SLOT, SLOT_FD and SLOT_DIR. Returns 2, holding nothing, when flock
# cannot run.
slot_lease() {
    local pool="$1" prefix="$2" who="$3" rc
    mkdir -p "$pool" || return 2
    SLOT=0
    while :; do
        exec {SLOT_FD}>"$pool/$prefix$SLOT.lock" || return 2
        flock -n "$SLOT_FD"; rc=$?
        [ "$rc" -eq 0 ] && break
        exec {SLOT_FD}>&-
        # Exit 1 means held; anything else (flock missing, say) would loop forever.
        [ "$rc" -eq 1 ] || { echo "$who: flock failed with exit $rc" >&2; return 2; }
        SLOT=$((SLOT + 1))
    done
    SLOT_DIR="$pool/$prefix$SLOT"
}

# slot_tend <who> <pool> <prefix> <keep-var> <keep-default> <tree> <remove-fn>
# Call it holding a lease. Empties <tree> when it has outgrown CODESCOUT_SLOT_CEILING_MB,
# then hands every FREE slot numbered KEEP or higher to <remove-fn>, under that slot's
# lock. KEEP is read from the variable named <keep-var>. Returns 2, having removed
# nothing, when either number is not a whole number: a typo such as `48G` would otherwise
# switch the bound off without a word.
slot_tend() {
    local who="$1" pool="$2" prefix="$3" keep_var="$4" keep="${!4:-$5}" tree="$6" remove="$7"
    local ceiling="${CODESCOUT_SLOT_CEILING_MB:-$SLOT_CEILING_MB_DEFAULT}" size lock n fd
    case "$ceiling" in ''|*[!0-9]*)
        echo "$who: CODESCOUT_SLOT_CEILING_MB must be a whole number of MiB, got '$ceiling'" >&2
        return 2 ;;
    esac
    case "$keep" in ''|*[!0-9]*)
        echo "$who: $keep_var must be a whole number of slots, got '$keep'" >&2
        return 2 ;;
    esac

    size=$(du -sm "$tree" 2>/dev/null | cut -f1)
    if [ "${size:-0}" -gt "$ceiling" ]; then
        echo "$who: $tree is ${size}M, over the ${ceiling}M ceiling (CODESCOUT_SLOT_CEILING_MB); emptying it, so this run builds cold" >&2
        rm -rf -- "$tree"
    fi

    for lock in "$pool/$prefix"[0-9]*.lock; do
        n="${lock##*/"$prefix"}"; n="${n%.lock}"
        [ "$n" -ge "$keep" ] 2>/dev/null || continue
        # Lock files outlive their slots, so without this every run would re-report every
        # slot ever reclaimed.
        [ -e "$pool/$prefix$n" ] || continue
        exec {fd}>"$lock" || continue
        # Our own slot fails here too: flock conflicts across open file descriptions,
        # even within one process.
        if flock -n "$fd"; then
            echo "$who: reclaimed free slot $prefix$n; slots from $keep up are kept only while in use ($keep_var)" >&2
            "$remove" "$pool/$prefix$n"
        fi
        exec {fd}>&-
    done
}

slot_remove_dir() { rm -rf -- "$1"; }

# lease_gate_target <who> — lease a cargo target dir from the pool gate.sh and
# with-slot.sh share, tend it, and export CARGO_TARGET_DIR. Sets GATE_POOL.
lease_gate_target() {
    GATE_POOL="${CODESCOUT_GATE_POOL:-$HOME/.cache/codescout-gate}"
    slot_lease "$GATE_POOL" slot- "$1" || return 2
    # Three is the most concurrent gate runs observed on this machine.
    slot_tend "$1" "$GATE_POOL" slot- CODESCOUT_GATE_POOL_KEEP 3 "$SLOT_DIR" slot_remove_dir || return 2
    export CARGO_TARGET_DIR="$SLOT_DIR"
}
