# scripts/slot-pool.sh — the leased pool of build trees behind scripts/gate.sh,
# scripts/with-slot.sh and scripts/mutation-probe.sh. SOURCE it; it runs nothing itself.
# The opt-in RAM tier below is set up per machine as docs/conventions/gate-ram-tier.md says.
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
# A fourth bound is the same cause for the workspace crates themselves: `slot_evict_foreign_members`
# stamps the slot with the checkout that last leased it and, when the lessee changes, removes
# the fingerprints of every workspace crate (and its build-script units), because cargo
# trusts a fingerprint by mtime and a checkout older than the slot's last build is handed
# the other checkout's compiled crate (bug 2026-10-16, E0432 on a module the source declares).
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

# slot_lease <pool> <prefix> <who> [slot] [max] — take the lowest free slot, or exactly <slot>,
# and hold it on an fd every child inherits. Sets SLOT, SLOT_FD and SLOT_DIR. Returns 2,
# holding nothing, when flock cannot run, or when a named slot is held, absent or malformed.
# With <max> (a whole number) and no <slot> the search is bounded: it tries 0 … max-1 only,
# opens no lock file for a slot numbered <max> or higher, and returns 1, holding nothing, when
# every one of them is held. <max> is ignored for a named slot; the caller range-checks that.
slot_lease() {
    local pool="$1" prefix="$2" who="$3" only="${4:-}" max="${5:-}" rc
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
        # Checked before the open: the bound must also keep the lock file from being created.
        [ -z "$max" ] || [ "$SLOT" -lt "$max" ] || return 1
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

# ---------------------------------------------------------------------------------------
# The RAM tier: an OPT-IN, machine-local pool of up to RAM_SLOTS build trees on a zram
# filesystem, tried before the disk pool. Design: docs/superpowers/specs/2026-10-31-gate-ram-tier-design.md.
# Nothing here runs, prints or changes an exit code unless this machine's config names a pool
# (${XDG_CONFIG_HOME:-$HOME/.config}/codescout/gate-pool.env, or CODESCOUT_GATE_RAM_POOL in the
# environment). The functions below are the admission half; every refusal sets RAM_SKIP to one
# line, "<who>: RAM tier skipped: <cause> (<knob>)", and the caller falls back to the disk pool.
#
# THE CONFIG FILE IS PARSED, NEVER SOURCED. It lives in the operator's home and holds paths, so
# sourcing it would run whatever a line says (`CODESCOUT_GATE_RAM_SLOTS=$(...)`). Each line is
# matched against a fixed shape and only the seven CODESCOUT_GATE_RAM_* keys are read; any other
# line is ignored. An environment value of the same name wins over the file's.
#
# TEST SEAMS (the suite sets them; CI has no zram device): `findmnt` is resolved through PATH;
# CODESCOUT_GATE_RAM_MEMINFO (default /proc/meminfo) and CODESCOUT_GATE_RAM_MMSTAT (default
# /sys/block/<dev>/mm_stat) replace the two kernel files.
# ---------------------------------------------------------------------------------------

# ram_tier_load_config <who> — read the config. Returns 0, and sets RAM_POOL RAM_SLOTS
# RAM_GROWTH_MB RAM_COLD_MB RAM_MEM_FLOOR_MB RAM_IDLE_MIN RAM_CARGO_ENV, when a pool is named
# and every value is valid. Returns 1 otherwise: with RAM_SKIP set when something is wrong
# (a line that is not KEY=VALUE, a value that is not a whole number, a relative pool path),
# and RAM_SKIP empty when the tier is simply not configured. A numeric value is read as
# decimal (leading zeros are not octal) and capped at 12 digits so later arithmetic cannot wrap.
ram_tier_load_config() {
    local who="$1" file="${XDG_CONFIG_HOME:-$HOME/.config}/codescout/gate-pool.env"
    local shape='^CODESCOUT_GATE_RAM_[A-Z_]+=[^[:space:]]*$'
    local -A fromfile=() value=() default=([POOL]="" [SLOTS]=2 [GROWTH_MB]=4096 [COLD_MB]=10240
        [MEM_FLOOR_MB]=12288 [IDLE_MIN]=60 [CARGO_ENV]=1)
    local line key name var n=0
    RAM_SKIP=""
    if [ -r "$file" ]; then
        while IFS= read -r line || [ -n "$line" ]; do
            n=$((n + 1))
            case "$line" in
                ''|'#'*) continue ;;
                CODESCOUT_GATE_RAM_*) ;;
                *) continue ;;
            esac
            if [[ "$line" =~ $shape ]]; then
                fromfile[${line%%=*}]="${line#*=}"
            else
                key="${line%%[[:space:]=]*}"
                RAM_SKIP="$who: RAM tier skipped: $file line $n is not KEY=VALUE without spaces ($key)"
                return 1
            fi
        done < "$file"
    fi
    for name in POOL SLOTS GROWTH_MB COLD_MB MEM_FLOOR_MB IDLE_MIN CARGO_ENV; do
        var="CODESCOUT_GATE_RAM_$name"
        value[$name]="${default[$name]}"
        [ -z "${fromfile[$var]+set}" ] || value[$name]="${fromfile[$var]}"
        [ -z "${!var:-}" ] || value[$name]="${!var}"
    done
    [ -n "${value[POOL]}" ] || return 1
    case "${value[POOL]}" in
        /*) ;;
        *) RAM_SKIP="$who: RAM tier skipped: CODESCOUT_GATE_RAM_POOL must be an absolute path, got '${value[POOL]}'"
           return 1 ;;
    esac
    for name in SLOTS GROWTH_MB COLD_MB MEM_FLOOR_MB IDLE_MIN CARGO_ENV; do
        if ! [[ "${value[$name]}" =~ ^[0-9]{1,12}$ ]]; then
            RAM_SKIP="$who: RAM tier skipped: CODESCOUT_GATE_RAM_$name must be a whole number (1 to 12 digits), got '${value[$name]}'"
            return 1
        fi
    done
    RAM_POOL="${value[POOL]}"
    RAM_SLOTS=$((10#${value[SLOTS]})); RAM_GROWTH_MB=$((10#${value[GROWTH_MB]}))
    RAM_COLD_MB=$((10#${value[COLD_MB]})); RAM_MEM_FLOOR_MB=$((10#${value[MEM_FLOOR_MB]}))
    RAM_IDLE_MIN=$((10#${value[IDLE_MIN]})); RAM_CARGO_ENV=$((10#${value[CARGO_ENV]}))
}

# ram_mmstat_field <n> — print field <n> (1-based) of the first line of $RAM_MMSTAT.
ram_mmstat_field() { awk -v n="$1" 'NR == 1 { print $n; exit }' "$RAM_MMSTAT" 2>/dev/null; }

# ram_tier_check_mount <who> — $RAM_POOL must sit on a /dev/zram* device whose mm_stat field 4
# (mem_limit, bytes) is above 0. A zram with no limit has no budget to check: the pool rule would
# pass forever while the device grew into RAM. Sets RAM_DEV and RAM_MMSTAT. Returns 1 with RAM_SKIP.
ram_tier_check_mount() {
    local who="$1" src dev cap
    src="$(findmnt -no SOURCE -T "$RAM_POOL" 2>/dev/null | head -n 1)"
    dev="${src#/dev/}"; dev="${dev%%[!a-z0-9]*}"      # "/dev/zram1[/sub]" -> "zram1"
    if [ "${src#/dev/}" = "$src" ] || ! [[ "$dev" =~ ^zram[0-9]+$ ]]; then
        RAM_SKIP="$who: RAM tier skipped: $RAM_POOL is not a zram mount (mount source: ${src:-none}; CODESCOUT_GATE_RAM_POOL)"
        return 1
    fi
    RAM_DEV="$dev"
    RAM_MMSTAT="${CODESCOUT_GATE_RAM_MMSTAT:-/sys/block/$RAM_DEV/mm_stat}"
    cap="$(ram_mmstat_field 4)"
    if ! [[ "$cap" =~ ^[0-9]+$ ]]; then
        RAM_SKIP="$who: RAM tier skipped: cannot read mm_stat of $RAM_DEV from $RAM_MMSTAT (CODESCOUT_GATE_RAM_POOL)"
        return 1
    fi
    if [ "$cap" -le 0 ]; then
        RAM_SKIP="$who: RAM tier skipped: $RAM_DEV has no RAM cap (mm_stat mem_limit is 0), so a budget cannot be checked; give the device a mem_limit (CODESCOUT_GATE_RAM_POOL)"
        return 1
    fi
}

# ram_mem_available_mb — print MemAvailable in whole MiB (rounded down); nothing when unreadable.
ram_mem_available_mb() {
    awk '/^MemAvailable:/ { print int($2 / 1024); exit }' "${CODESCOUT_GATE_RAM_MEMINFO:-/proc/meminfo}" 2>/dev/null
}

# ram_tier_check_floor <who> — MemAvailable must be at least RAM_MEM_FLOOR_MB, so the RAM tier
# never starts a build on a machine that is already short of memory. Returns 1 with RAM_SKIP.
ram_tier_check_floor() {
    local who="$1" avail
    avail="$(ram_mem_available_mb)"
    if ! [[ "$avail" =~ ^[0-9]+$ ]]; then
        RAM_SKIP="$who: RAM tier skipped: cannot read MemAvailable from ${CODESCOUT_GATE_RAM_MEMINFO:-/proc/meminfo}"
        return 1
    fi
    if [ "$avail" -lt "$RAM_MEM_FLOOR_MB" ]; then
        RAM_SKIP="$who: RAM tier skipped: MemAvailable ${avail}M is below the ${RAM_MEM_FLOOR_MB}M floor (CODESCOUT_GATE_RAM_MEM_FLOOR_MB)"
        return 1
    fi
}

# ram_tier_admit <who> — the mount, then the floor. Call it after ram_tier_load_config succeeded.
ram_tier_admit() { ram_tier_check_mount "$1" && ram_tier_check_floor "$1"; }

# THE POOL BUDGET. The RAM slots share one filesystem and one RAM cap, and two of them can build at
# the same time, so a lease is admitted against what EVERY held RAM slot still has to write, not
# against its own tree alone (spec 2 and 4.d). For each held slot i, this lease included:
#     need_i = max(GROWTH, COLD - size_i)      (size_i is `du -sm`, which rounds UP)
# and the lease fits when both budgets cover the sum:
#     logical: df free MiB                       >= sum
#     RAM:     (mm_stat field 4 - field 3) in MiB >= sum * 10 / 22      (the revisit ratio 2.2)
# "Held" means `flock -n` on the other slot's lock fails. The caller takes its own lock BEFORE
# asking, so two leases that start together each count the other one. An idle slot is not counted:
# ram_pool_make_room may empty it, and a slot it leaves alone is idle and can be taken.

# ram_slot_need_mb <tree> — print max(RAM_GROWTH_MB, RAM_COLD_MB - size of <tree> in MiB); a tree
# that is missing has size 0.
ram_slot_need_mb() {
    local size need
    size="$(du -sm "$1" 2>/dev/null | cut -f1)"
    [[ "$size" =~ ^[0-9]+$ ]] || size=0
    need=$((RAM_COLD_MB - size))
    [ "$need" -ge "$RAM_GROWTH_MB" ] || need="$RAM_GROWTH_MB"
    echo "$need"
}

# ram_pool_fits <who> — call it holding the lease (SLOT and SLOT_DIR set by lease_ram_target, after
# the lessee evictions, which change the own slot's size). Returns 0 when both budgets cover the sum
# of need over the own slot and every other RAM slot whose lock is held; 1 when one of them does
# not, with RAM_SKIP naming both budgets and both sums; 2, with RAM_SKIP, when df or mm_stat cannot
# be read, so the caller can tell "short" (empty slots) from "unknown" (empty nothing).
ram_pool_fits() {
    local who="$1" n lock fd rc sum held=1 free used cap left ram_need
    sum="$(ram_slot_need_mb "$SLOT_DIR")"
    for ((n = 0; n < RAM_SLOTS; n++)); do
        [ "$n" -ne "$SLOT" ] || continue
        lock="$RAM_POOL/slot-$n.lock"
        [ -e "$lock" ] || continue          # never create a lock file to look at it
        exec {fd}>"$lock" || continue
        if flock -n "$fd"; then rc=0; else rc=$?; fi
        exec {fd}>&-
        [ "$rc" -eq 1 ] || continue         # taken (idle) or not testable: not counted
        sum=$((sum + $(ram_slot_need_mb "$RAM_POOL/slot-$n"))); held=$((held + 1))
    done
    free="$(df -Pm -- "$RAM_POOL" 2>/dev/null | awk 'NR == 2 { print $4; exit }')"
    used="$(ram_mmstat_field 3)"; cap="$(ram_mmstat_field 4)"
    if ! [[ "$free" =~ ^[0-9]{1,15}$ ]]; then
        RAM_SKIP="$who: RAM tier skipped: cannot read the free space of $RAM_POOL from df (CODESCOUT_GATE_RAM_POOL)"
        return 2
    fi
    if ! [[ "$used" =~ ^[0-9]{1,15}$ && "$cap" =~ ^[0-9]{1,15}$ ]]; then
        RAM_SKIP="$who: RAM tier skipped: cannot read mem_used_total and mem_limit from $RAM_MMSTAT (CODESCOUT_GATE_RAM_POOL)"
        return 2
    fi
    left=$(((cap - used) / 1048576))
    ram_need=$(((sum * 10 + 21) / 22))
    if [ "$free" -ge "$sum" ] && [ $((left * 22)) -ge $((sum * 10)) ]; then
        RAM_SKIP=""
        return 0
    fi
    RAM_SKIP="$who: RAM tier skipped: the pool budget does not cover the $held held RAM slot(s): logical (df) ${free}M free, ${sum}M needed; RAM (mm_stat) ${left}M left, ${ram_need}M needed (CODESCOUT_GATE_RAM_GROWTH_MB, CODESCOUT_GATE_RAM_COLD_MB)"
    return 1
}

# ram_pool_make_room <who> — empty idle RAM slots until ram_pool_fits holds, the slot leased longest
# ago first (the oldest mtime of <slot>/.codescout-lessee, none counting as the oldest, ties by slot
# number). Idle means `flock -n` on the slot's lock succeeds, and the lock is held across the
# `rm -rf`; the lock file stays. The own slot is never a candidate (its lock is ours, and it is
# skipped by number as well): the lease would only trade a warm slot for a cold build. Returns 0 when
# the pool fits, 1 when no idle slot is left or the budgets cannot be read; RAM_SKIP is then the last
# ram_pool_fits verdict.
ram_pool_make_room() {
    local who="$1" m n dir fd rc
    while read -r m n; do
        [ -n "$n" ] || continue
        dir="$RAM_POOL/slot-$n"
        exec {fd}>"$dir.lock" || continue
        if flock -n "$fd"; then
            echo "$who: emptying idle RAM slot-$n (last leased $(date -d "@$m" '+%F %T' 2>/dev/null || echo unknown)) to make room in the pool" >&2
            rm -rf -- "$dir"
            exec {fd}>&-
            rc=0; ram_pool_fits "$who" || rc=$?
            [ "$rc" -ne 0 ] || return 0
            [ "$rc" -eq 1 ] || return 1
        else
            exec {fd}>&-
        fi
    done < <(
        for ((n = 0; n < RAM_SLOTS; n++)); do
            [ "$n" -ne "$SLOT" ] || continue
            [ -e "$RAM_POOL/slot-$n.lock" ] && [ -e "$RAM_POOL/slot-$n" ] || continue
            m="$(stat -c %Y -- "$RAM_POOL/slot-$n/.codescout-lessee" 2>/dev/null)" || m=0
            echo "${m:-0} $n"
        done | sort -k1,1n -k2,2n
    )
    return 1
}

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

# slot_evict_foreign_members <who> <tree> <checkout> <cargo-home>
# Call it holding a lease. When <tree> was last leased for a different checkout than
# <checkout> (or for none on record), removes the cargo fingerprint of every compiled unit
# of a WORKSPACE crate, so cargo compiles the lessee's own crates again, then records
# <checkout> as the lessee. With an empty <checkout> (not a git tree, or git refused) it
# removes the record instead and evicts nothing: nobody can be named as the lessee, and a
# stale record would let this lease write units its previous owner then trusts.
#
# WHY. Cargo hashes a workspace member by its path RELATIVE to the workspace root, so two
# checkouts of one repository name the same unit `codescout-embed-<hash>` and share one
# fingerprint, and it calls that fingerprint fresh when no source file in the CURRENT
# checkout is newer than the dep-info the OTHER checkout's build wrote. A checkout whose
# files are older than the slot's last build is therefore handed the other checkout's
# compiled crate, whatever its source says. Measured 2026-10-17 with two toy workspaces in
# one target dir, tree B older than A's build and its member `emb` gaining `pub mod ceiling`:
# `cargo test --no-run` printed `Compiling emb` and then `E0432 unresolved import
# emb::ceiling`, because only one unit of `emb` was dirty (its lib-test unit was new) and
# the binary linked the other checkout's stale lib unit. That is the 2026-10-16 failure in
# a gate slot, line for line. A tree whose members are all skipped instead prints
# `Finished` and runs the other checkout's code. `slot_evict_foreign_build_scripts` closes
# the same hole for the `env!`-baked path of a compiled build script only.
#
# WHAT IS EVICTED, in two passes over each `<profile>/.fingerprint` directory (`debug`,
# `release`, and the same under a `--target <triple>` directory):
#   1. every unit whose `<profile>/deps/<crate>-<hash>.d` lists, as its FIRST source, a path
#      that is not under <cargo-home>. A workspace member lists relative paths, a path
#      dependency outside the workspace lists absolute ones, and a registry or git
#      dependency lists absolute ones under <cargo-home>, which is kept because it is meant to
#      be shared and recompiling C would cost minutes. The fingerprint directory is
#      `.fingerprint/<package>-<hash>`, the same hash.
#   2. the build-script units, compile and run, of every package pass 1 evicted. They carry
#      no `deps/*.d` (the run unit's fingerprint records only package-relative paths), so
#      the package NAME is the only link to them, and the matching `build/<package>-<hash>`
#      directories go too, because the run unit's OUT_DIR holds files such as
#      `include_str!`-ed copies of the other checkout's sources. Measured 2026-10-17: a
#      build.rs copying a file of its own tree into OUT_DIR printed the other tree's marker
#      after pass 1 alone.
# Removing a fingerprint is what makes cargo call the unit dirty, and a dirty unit dirties
# everything that depends on it.
#
# THE OWNER FIELD. Cargo records no checkout, so the lessee is stamped in
# `<tree>/.codescout-lessee`, AFTER the eviction: a run interrupted part-way leaves the old
# stamp, and the next lease evicts again. Same checkout, same stamp, nothing evicted: an
# edit there bumps an mtime, so cargo's own rule is sound inside one checkout. A tree with no
# stamp (every slot that predates this) is treated as foreign once. The stamp is compared
# whole, never as a prefix: a worktree nested under the stamped checkout is another lessee.
#
# LIMIT. The lessee is the git toplevel of the CWD of the leasing command, which is what
# `with-slot.sh` and `gate.sh` build in the usual case. A command that builds another tree
# (`--manifest-path ../other/Cargo.toml`, or a `cd` inside `bash -c`) is stamped as the CWD's
# checkout, and the tree it built stays unprotected until the next lease from that CWD.
#
# COST. One lease that changes lessee reads the dep-info of every unit and removes the
# workspace ones, with no per-unit `find` or fork: measured on a copy of a real slot, see
# the issue record for this change. The next build compiles the workspace crates, not the
# registry dependencies.
slot_evict_foreign_members() {
    local who="$1" tree="$2" checkout="$3" cargo_home="$4" stamp owner="" n=0
    local fpdir prof dep line src base fp name
    local -A foreign pkgs
    stamp="$tree/.codescout-lessee"
    [ -n "$checkout" ] || { rm -f -- "$stamp"; return 0; }
    [ -r "$stamp" ] && IFS= read -r owner < "$stamp"
    if [ "$owner" != "$checkout" ]; then
        # One find, pruned at each `.fingerprint`; depth 3 reaches `<triple>/debug`.
        while IFS= read -r fpdir; do
            prof="${fpdir%/.fingerprint}"
            foreign=(); pkgs=()
            for dep in "$prof"/deps/*.d; do
                [ -r "$dep" ] || continue
                line=""; IFS= read -r line < "$dep" || [ -n "$line" ] || continue
                case "$line" in *".d: "*) ;; *) continue ;; esac
                # The pattern below is a prefix test, so the first source decides: no need to cut it.
                src="${line#*.d: }"
                [ -n "$src" ] || continue
                # An empty <cargo-home> would make the pattern `/*`, which matches every path.
                if [ -n "$cargo_home" ]; then
                    case "$src" in "$cargo_home"/*) continue ;; esac
                fi
                base="${dep##*/}"; base="${base%.d}"; foreign["${base##*-}"]=1
            done
            for fp in "$fpdir"/*/; do
                fp="${fp%/}"; name="${fp##*/}"
                [ -n "${foreign["${name##*-}"]:-}" ] || continue
                pkgs["${name%-*}"]=1
                rm -rf -- "$fp"; n=$((n + 1))
            done
            for fp in "$fpdir"/*/; do
                fp="${fp%/}"; name="${fp##*/}"
                [ -n "${pkgs["${name%-*}"]:-}" ] || continue
                compgen -G "$fp/*build-script-*" >/dev/null || continue
                rm -rf -- "$fp" "$prof/build/$name"; n=$((n + 1))
            done
        done < <(find "$tree" -maxdepth 3 -type d -name .fingerprint -prune -print 2>/dev/null)
        [ "$n" -eq 0 ] || echo "$who: evicted $n compiled units of workspace crates from $tree: it was last leased for ${owner:-no recorded checkout}, not for $checkout" >&2
    fi
    mkdir -p "$tree" 2>/dev/null && printf '%s\n' "$checkout" > "$stamp"
}

# lease_ram_target <who> [n] — lease a RAM slot from $RAM_POOL (the caller has loaded the config
# and admitted the tier). With no <n> it takes the lowest free of the RAM_SLOTS slots, and never
# creates a slot or a lock file numbered RAM_SLOTS or higher; with <n> it takes exactly slot-<n>
# (the caller has range-checked it). Returns 0 with SLOT SLOT_FD SLOT_DIR set, GATE_POOL=$RAM_POOL
# and GATE_TIER=ram; returns 1, holding nothing, otherwise. The slot's directory is created here:
# the evictor empties a RAM slot and leaves only its lock file, so a free slot may have none.
# It does not tend: the disk ceiling and the high-water bound are disk rules.
lease_ram_target() {
    local who="$1" n="${2:-}"
    if [ -n "$n" ]; then slot_lease "$RAM_POOL" slot- "$who" "$n" || return 1
    else slot_lease "$RAM_POOL" slot- "$who" "" "$RAM_SLOTS" || return 1
    fi
    mkdir -p "$SLOT_DIR" || { exec {SLOT_FD}>&-; return 1; }
    GATE_POOL="$RAM_POOL"; GATE_TIER=ram
}

# lease_gate_target <who> [slot] — lease a cargo target dir and export CARGO_TARGET_DIR. Sets
# GATE_POOL and GATE_TIER (ram|disk). <slot> is:
#   (none)   the lowest free RAM slot when the opt-in RAM tier is configured and admitted,
#            else the lowest free slot of the disk pool gate.sh and with-slot.sh share. A tier
#            that is configured but refused says why on stderr and the disk lease goes ahead; an
#            unconfigured one says nothing, and RAM_SLOTS=0 is configured with no slots to lease.
#   N        exactly disk slot-N.
#   ram-N    exactly RAM slot-N. Returns 2, running nothing, when the tier is not configured or
#            not admitted, N is outside 0 … RAM_SLOTS-1, or the slot is held or absent: a named
#            slot never falls back to the other tier.
# A disk lease is tended; both tiers evict build scripts and workspace crates compiled for
# another checkout. An UNNAMED RAM lease is then admitted against the pool budget (ram_pool_fits), idle
# RAM slots being emptied oldest lease first to make room; when it still does not fit, it releases the
# RAM slot and goes to disk, saying why. A named ram-N slot is never budgeted, never empties a
# neighbour and is never refused for budget. Whether the RAM tier is configured is read off the RETURN CODES of
# ram_tier_load_config and ram_tier_admit, never off the RAM_* globals: after a refusal they
# keep stale values.
lease_gate_target() {
    local who="$1" slot="${2:-}" n disk_pool rc
    GATE_POOL="${CODESCOUT_GATE_POOL:-$HOME/.cache/codescout-gate}"
    disk_pool="$GATE_POOL"
    GATE_TIER=disk
    RAM_SKIP=""
    case "$slot" in
        ram-*)
            n="${slot#ram-}"
            case "$n" in
                ''|*[!0-9]*) echo "$who: --slot wants ram-<whole number>, got '$slot'; nothing was run" >&2; return 2 ;;
            esac
            if ! ram_tier_load_config "$who"; then
                [ -z "$RAM_SKIP" ] || echo "$RAM_SKIP" >&2
                echo "$who: --slot $slot names the RAM tier, which is not configured or not usable here, and a named slot never falls back to the disk pool; nothing was run (CODESCOUT_GATE_RAM_POOL)" >&2
                return 2
            fi
            if ! ram_tier_admit "$who"; then
                echo "$RAM_SKIP" >&2
                echo "$who: --slot $slot names the RAM tier, which is not admitted now, and a named slot never falls back to the disk pool; nothing was run" >&2
                return 2
            fi
            # More than 9 digits is out of range whatever RAM_SLOTS is, and keeps the arithmetic safe.
            if [ "${#n}" -gt 9 ] || [ "$((10#$n))" -ge "$RAM_SLOTS" ]; then
                echo "$who: --slot $slot is outside the RAM tier's slots (CODESCOUT_GATE_RAM_SLOTS=$RAM_SLOTS, numbered from 0); nothing was run" >&2
                return 2
            fi
            lease_ram_target "$who" "$((10#$n))" || {
                echo "$who: --slot $slot was not leased from the RAM tier ($RAM_POOL); nothing was run" >&2
                return 2
            }
            ;;
        '')
            # RAM_SLOTS=0 is a valid config with no slots to lease: straight to disk, saying nothing.
            if ram_tier_load_config "$who" && [ "$RAM_SLOTS" -gt 0 ]; then
                if ram_tier_admit "$who"; then lease_ram_target "$who" || true; fi
            fi
            [ -z "$RAM_SKIP" ] || echo "$RAM_SKIP" >&2
            ;;
    esac
    # A RAM lease is admitted against the pool AFTER the lessee evictions, which shrink the own slot
    # and so change its need. When the pool cannot fit it, even after emptying every idle RAM slot,
    # the RAM lock is released and the disk lease below runs in full, as if the tier did not exist.
    # A NAMED slot (--slot ram-N) is the operator's explicit choice, often to repair or inspect that
    # very tree: it is never budgeted, never empties a neighbour and is never refused for budget.
    if [ "$GATE_TIER" = ram ]; then
        lease_evict_lessee "$who"
        if [ -z "$slot" ]; then
            rc=0; ram_pool_fits "$who" || rc=$?
            if [ "$rc" -eq 1 ]; then rc=0; ram_pool_make_room "$who" || rc=1; fi
            if [ "$rc" -ne 0 ]; then
                echo "$RAM_SKIP" >&2
                exec {SLOT_FD}>&-
                unset SLOT SLOT_DIR SLOT_FD
                GATE_POOL="$disk_pool"; GATE_TIER=disk
            fi
        fi
    fi
    if [ "$GATE_TIER" != ram ]; then
        slot_lease "$GATE_POOL" slot- "$who" "$slot" || return 2
        # Three is the most concurrent gate runs observed on this machine.
        slot_tend "$who" "$GATE_POOL" slot- CODESCOUT_GATE_POOL_KEEP 3 "$SLOT_DIR" slot_remove_dir || return 2
        lease_evict_lessee "$who"
    fi
    # Build settings of a KEPT RAM lease (spec 5): they halve a slot and drop `incremental`, the main
    # source of slot growth. They come AFTER the pool block, so a RAM lease given up above never carries
    # them into the disk run, and lease_ram_target stays free of exports. A variable the caller already
    # set, even to an empty value, is left alone; each is decided on its own.
    if [ "$GATE_TIER" = ram ] && [ "$RAM_CARGO_ENV" -eq 1 ]; then
        [ -n "${CARGO_PROFILE_DEV_DEBUG+x}" ] || export CARGO_PROFILE_DEV_DEBUG=line-tables-only
        [ -n "${CARGO_INCREMENTAL+x}" ] || export CARGO_INCREMENTAL=0
    fi
    export CARGO_TARGET_DIR="$SLOT_DIR"
}

# ram_gib <bytes> — print <bytes> in GiB with one decimal and a G, or ? when it is not a whole number (an
# unreadable mm_stat or df field). The tier's figures are report only: they never decide anything.
ram_gib() {
    if [[ "$1" =~ ^[0-9]{1,15}$ ]]; then awk -v n="$1" 'BEGIN { printf "%.1fG", n / 1073741824 }'
    else printf '?'
    fi
}

# ram_tier_figures — print the zram figures of the leased RAM tier, for gate.sh: mm_stat fields 1
# (orig_data_size, the logical bytes), 3 (mem_used_total, the RAM bytes) and 4 (mem_limit, the cap), and
# the free space of $GATE_POOL from `df -Pm`. A figure that cannot be read prints as ?; it never fails.
ram_tier_figures() {
    local free
    free="$(df -Pm -- "$GATE_POOL" 2>/dev/null | awk 'NR == 2 { print $4; exit }')"
    if [[ "$free" =~ ^[0-9]{1,15}$ ]]; then free=$((free * 1048576)); else free=""; fi
    printf '%s logical, %s RAM of %s cap, %s free' \
        "$(ram_gib "$(ram_mmstat_field 1)")" "$(ram_gib "$(ram_mmstat_field 3)")" \
        "$(ram_gib "$(ram_mmstat_field 4)")" "$(ram_gib "$free")"
}

# ram_tier_report — print `gate.sh: tier RAM (<dev>): <figures>`. Call it only after a RAM lease
# (GATE_TIER=ram, with RAM_DEV, RAM_MMSTAT and GATE_POOL set by it). Always returns 0.
ram_tier_report() {
    printf 'gate.sh: tier RAM (%s): %s\n' "$RAM_DEV" "$(ram_tier_figures)"
}

# lease_evict_lessee <who> — call it holding a lease. The lessee is whichever checkout the command
# will build, which is the cwd's, not the one this script lives in. Outside a git tree there is
# none, and nothing is evicted.
lease_evict_lessee() {
    slot_evict_foreign_build_scripts "$1" "$SLOT_DIR" \
        "$(git rev-parse --show-toplevel 2>/dev/null)" "${CARGO_HOME:-$HOME/.cargo}"
    slot_evict_foreign_members "$1" "$SLOT_DIR" \
        "$(git rev-parse --show-toplevel 2>/dev/null)" "${CARGO_HOME:-$HOME/.cargo}"
}
