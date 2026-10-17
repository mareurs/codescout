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

# lease_gate_target <who> [slot] — lease a cargo target dir from the pool gate.sh and
# with-slot.sh share (the lowest free one, or exactly <slot>), tend it, evict build scripts
# and workspace crates compiled for another checkout, and export CARGO_TARGET_DIR. Sets GATE_POOL.
lease_gate_target() {
    GATE_POOL="${CODESCOUT_GATE_POOL:-$HOME/.cache/codescout-gate}"
    slot_lease "$GATE_POOL" slot- "$1" "${2:-}" || return 2
    # Three is the most concurrent gate runs observed on this machine.
    slot_tend "$1" "$GATE_POOL" slot- CODESCOUT_GATE_POOL_KEEP 3 "$SLOT_DIR" slot_remove_dir || return 2
    # The lessee is whichever checkout the command will build, which is the cwd's, not the
    # one this script lives in. Outside a git tree there is none, and nothing is evicted.
    slot_evict_foreign_build_scripts "$1" "$SLOT_DIR" \
        "$(git rev-parse --show-toplevel 2>/dev/null)" "${CARGO_HOME:-$HOME/.cargo}"
    slot_evict_foreign_members "$1" "$SLOT_DIR" \
        "$(git rev-parse --show-toplevel 2>/dev/null)" "${CARGO_HOME:-$HOME/.cargo}"
    export CARGO_TARGET_DIR="$SLOT_DIR"
}
