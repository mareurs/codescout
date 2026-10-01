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
# lease (bug 503fa887ab0ec144). Cargo never garbage-collects a target dir, so one slot
# grows toward the longest-lived tree on the machine: 104G measured beside a 17G slot on
# 2026-09-24. A slot over CODESCOUT_SLOT_CEILING_MB is therefore emptied, and that one
# run builds cold. And the high-water mark never fell, so free slots numbered KEEP or
# higher are removed by the next run to come along.
#
# A third bound is by cause, not by size or count: `slot_evict_foreign_build_scripts` removes
# a compiled build script whose recorded CARGO_MANIFEST_DIR is another checkout's, because
# cargo shares one script across checkouts and a 4 MB stale one is invisible to a size rule
# (bug f1162428494d0ad1).
#
# THE LOCK IS THE ONLY PROOF A TREE IS IDLE, which is why the pruning lives here and not
# in a cron job or a warning. `flock -n` succeeding is the one observation that no leased
# run is inside a slot, so every removal happens under that lock. A tree with no lock
# file (a session that typed its own CARGO_TARGET_DIR, bug 097aa5ca2222a91d) cannot be
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

# slot_lease <pool> <prefix> <who> [slot] — take the lowest free slot, or exactly <slot>,
# and hold it on an fd every child inherits. Sets SLOT, SLOT_FD and SLOT_DIR. Returns 2,
# holding nothing, when flock cannot run, or when a named slot is held, absent or malformed.
slot_lease() {
    local pool="$1" prefix="$2" who="$3" only="${4:-}" rc
    mkdir -p "$pool" || return 2
    SLOT=0
    if [ -n "$only" ]; then
        # A named slot is for repairing or inspecting THAT tree, so it is never swapped for
        # another: bug f1162428494d0ad1 cleaned the wrong slot because the lease picked
        # whichever was free. Held, absent and malformed are three refusals, each naming itself.
        case "$only" in *[!0-9]*) echo "$who: --slot wants a whole slot number, got '$only'" >&2; return 2 ;; esac
        [ -d "$pool/$prefix$only" ] || { echo "$who: no slot $prefix$only in $pool" >&2; return 2; }
        SLOT="$only"
        exec {SLOT_FD}>"$pool/$prefix$SLOT.lock" || return 2
        flock -n "$SLOT_FD"; rc=$?
        if [ "$rc" -ne 0 ]; then
            if [ "$rc" -eq 1 ]; then echo "$who: $prefix$only is held by another run; nothing was run" >&2
            else echo "$who: flock failed with exit $rc" >&2; fi
            return 2
        fi
        SLOT_DIR="$pool/$prefix$SLOT"
        return 0
    fi
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

# slot_evict_foreign_build_scripts <who> <tree> <checkout> <cargo-home>
# Call it holding a lease. Removes each compiled build script in <tree> whose recorded
# CARGO_MANIFEST_DIR is neither inside <checkout> nor inside <cargo-home>, so cargo
# compiles that script again for the lessee. Nothing is removed when <checkout> is empty:
# the pattern `/*` then matches every absolute path. That is relied on, not guarded: a
# `return` ahead of it would refuse exactly what the pattern already skips, so no test could
# tell it from its absence. The direct call in tests/gate-slot.sh case L pins the behaviour.
#
# WHY THIS IS NOT SIZE OR COUNT. Cargo keeps no checkout path in a path package's hash, so
# checkouts share one compiled build script per feature set, and it calls that script fresh
# when the lessee's build.rs is no newer than the compiled copy. It never reads the
# `# env-dep:CARGO_MANIFEST_DIR=` line rustc wrote into the script's dep-info, which is
# where an `env!`-baked path lives. Measured with two throwaway crates in one target dir:
# the second printed the first's marker (bug f1162428494d0ad1). That line is the only owner
# field a slot carries, and nothing compared it with the lessee until this.
#
# TWO EXEMPTIONS, BOTH NEEDED. A path under <cargo-home> is a registry or git dependency,
# which is meant to be shared (libsqlite3-sys, measured in all three slots), and removing it
# would recompile C on every lease. A path under <checkout> is the lessee's own, workspace
# members included. The test is `<dir>/` against `<checkout>/*`, with the slash, because a
# bare prefix would call `<checkout>.worktrees/x` the lessee's own: that sibling shape is
# exactly what the pool held when this was written.
#
# SCOPE. It catches the `env!` form only, which is the one cross-checkout hazard observed.
# A script that bakes a path some other way records nothing here, and stays shared.
slot_evict_foreign_build_scripts() {
    local who="$1" tree="$2" checkout="$3" cargo_home="$4" dep line manifest
    # The whole directory holding the dep-info is removed, so `-path` confines the search to
    # a directory cargo named build/<pkg>-<hash>; the same file name under deps/ would
    # otherwise take deps/ with it. Depth 5 reaches a `--target <triple>` layout.
    while IFS= read -r dep; do
        line="$(grep -m1 '^# env-dep:CARGO_MANIFEST_DIR=' "$dep" 2>/dev/null)" || continue
        manifest="${line#*CARGO_MANIFEST_DIR=}"
        case "$manifest/" in "$checkout"/*) continue ;; esac
        # An empty <cargo-home> would make the pattern `/*`, which matches every path.
        if [ -n "$cargo_home" ]; then
            case "$manifest/" in "$cargo_home"/*) continue ;; esac
        fi
        echo "$who: evicting ${dep%/*}: its build script was compiled for $manifest, not for $checkout" >&2
        rm -rf -- "${dep%/*}"
        done < <(find "$tree" -maxdepth 5 -path '*/build/*' -name 'build_script_build-*.d' 2>/dev/null)
}

# lease_gate_target <who> [slot] — lease a cargo target dir from the pool gate.sh and
# with-slot.sh share (the lowest free one, or exactly <slot>), tend it, evict build scripts
# compiled for another checkout, and export CARGO_TARGET_DIR. Sets GATE_POOL.
lease_gate_target() {
    GATE_POOL="${CODESCOUT_GATE_POOL:-$HOME/.cache/codescout-gate}"
    slot_lease "$GATE_POOL" slot- "$1" "${2:-}" || return 2
    # Three is the most concurrent gate runs observed on this machine.
    slot_tend "$1" "$GATE_POOL" slot- CODESCOUT_GATE_POOL_KEEP 3 "$SLOT_DIR" slot_remove_dir || return 2
    # The lessee is whichever checkout the command will build, which is the cwd's, not the
    # one this script lives in. Outside a git tree there is none, and nothing is evicted.
    slot_evict_foreign_build_scripts "$1" "$SLOT_DIR" \
        "$(git rev-parse --show-toplevel 2>/dev/null)" "${CARGO_HOME:-$HOME/.cargo}"
    export CARGO_TARGET_DIR="$SLOT_DIR"
}
