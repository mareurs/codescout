#!/usr/bin/env bash
#
# worktree-remove.sh — remove a linked git worktree safely: find and classify the
# processes that hold it, stop the ones that are stoppable (only with --stop), remove the
# worktree, and verify that nothing is left behind.
#
# Usage:
#   scripts/worktree-remove.sh [--stop] [--wait SECS] <worktree path>
#   scripts/worktree-remove.sh -h | --help      print the usage and exit 0; removes nothing
#
# Output:
#   stdout  one row per holder, `PID<TAB>KIND<TAB>CMDLINE`, ordered mux first, then the
#           other stoppable kinds, then `child`, then `other`. KIND is one of
#           mux | rust-analyzer | cargo | rustc | sccache | build-output | child | other
#   stderr  the occupancy warning, refusals, `no holders under <path>`, one line per signal
#           a stop sends (`worktree-remove: signalled <pid> <kind> with SIGTERM`, or
#           `worktree-remove: could not signal <pid> <kind> (already gone, or not permitted)`),
#           the holders that survive a stop (rows in the same format as stdout), and what git
#           or the verification found. A blocker is listed there as `  <pid>  <cmdline>`, with
#           `(the parent of this script)` or `(an ancestor of this script)` after an ancestor
#           that is not of kind `other`
#   exit    0 the worktree is removed and verified gone
#           2 refused (usage, unregistered path, main checkout, caller's cwd inside, no /proc,
#             or the first process scan timed out: nothing was changed)
#           3 holders exist and --stop was not given
#           4 --stop, but a blocker exists, in the scan before the stop or in the first scan
#             of the stop loop (nothing was signalled)
#           5 --stop, but a holder survived the stop, or a rescan timed out so the holders
#             could not be established (nothing was removed)
#           6 git refused to remove the worktree and still lists it (untracked files, a lock):
#             never forced
#           7 the directory is still there: git removed the worktree (it may have exited
#             non-zero after deregistering it) but something wrote to the path again; its
#             contents are listed, never deleted
#
# WHY THIS EXISTS
#   A worktree that served `symbols` on a Rust file keeps a live
#   `codescout mux ... rust-analyzer` process and its `cargo check` child. After
#   `git worktree remove` deregisters the worktree, `cargo check` writes `target/` again,
#   so git fails with "Directory not empty" and a directory is left behind (bug
#   3dde1548c3e5c310). Nothing stops those processes when a session activates another
#   project. This script lists them, so the operator can stop them first.
#   Design: docs/superpowers/specs/2026-11-12-worktree-removal-helper-design.md
#
# WHY IT CLASSIFIES, AND WHY `other` IS A BLOCKER
#   A mux is keyed by workspace and shared by every session working in that checkout;
#   this script cannot see which session is a client. So stopping holders is never
#   implicit: it needs `--stop` and prints an occupancy warning
#   (docs/conventions/reaching-a-peer-session.md, Step 4: ask the occupant first).
#   Only the processes a worktree's own rust tooling leaves behind, and what they start, are
#   stoppable. Any other holder (an editor, a shell, a test run) is a blocker: with `--stop`
#   the script prints it and exits 4 without signalling anything, unless it descends from a
#   stoppable holder (then it is a `child`, below). So is any ancestor of this script (its
#   parent and the parent's ancestors: they are what runs the script) that holds the worktree,
#   whatever its kind, a `cargo` too: the script prints it with `(the parent of this script)`
#   or `(an ancestor of this script)` and exits 4 before any signal. The script never signals
#   its own pid, its subshells or an ancestor.
#   A `child` is a process that would be `other` by its command line but descends from a
#   holder of a stoppable kind: a cold build's C compiler chain (`make`, `cc`, `cc1`, `as`,
#   `sh configure`) runs under the `cargo` that runs a build script, and ends with it. It is
#   stoppable. The test walks the `PPid:` of its /proc status upward (see HOW A PROCESS IS
#   MATCHED). A real `other` holder, one with no such ancestor (an editor, a shell, a test
#   run), still blocks `--stop` with exit 4 before any signal. Never a `child`: any ancestor
#   of this script (its parent and the parent's ancestors), and a `codescout` that is not a
#   mux (a peer session's MCP server), wherever it runs.
#
# HOW IT SCANS
#   Every scan runs in a child under `timeout -k 1 $WORKTREE_REMOVE_SCAN_TIMEOUT` (default
#   10 s): reading another process's /proc/<pid>/cmdline waits for that process's memory-map
#   lock, so one scan can block for as long as that process holds it. A scan that times out
#   or fails means the holders are UNKNOWN, never "none": the script never goes on to
#   `git worktree remove` after one.
#
# HOW IT STOPS
#   SIGTERM only, never SIGKILL to a holder (the scan child runs under `timeout -k 1`, which
#   may SIGKILL the script's own scan). The script prints one stderr line for each signal it
#   sends, `signalled` or `could not signal` (see Output). The mux holders first (a graceful
#   mux exit kills the language server's process group), then, after a pause or until the mux
#   is gone, the other stoppable holders (a `child` too: see WHY IT CLASSIFIES). The pause is
#   up to ten rounds of a 0.1 s sleep plus one scan:
#   about one second on a small /proc, up to about 5 s on a busy real /proc (a scan takes
#   about 0.4 s there), and it is not counted in --wait. A blocker (an `other` row, or an
#   ancestor of this script) found by the first scan of the stop loop ends the run with exit 4
#   before any signal, as a blocker found before it does. A pid is signalled only if the
#   latest scan still lists it as a stoppable holder (a pid can be recycled during the
#   pause). Liveness is a RESCAN of the holders, never `kill -0` (a zombie answers it). The
#   rescan runs about every 0.5 s for up to --wait
#   seconds in all (default 30) and signals what is new, at most three times, because a
#   dying `cargo` can start children. A holder still listed afterwards is a survivor:
#   exit 5, nothing removed. A signal that fails because the process is already gone is
#   not an error; if the rescan still lists it, it is a survivor like any other.
#
# HOW IT REMOVES
#   `git -C <main checkout> worktree remove <path>` without --force. When git fails, the
#   script asks `git worktree list` whether the worktree is still registered. Still listed:
#   exit 6, with git's message and `git status --porcelain -uall` (first 50 lines, run only
#   when <path>/.git exists; ignored files never block git and are not listed). No longer
#   listed: git deleted the work tree and then failed (the bug: `rmdir target/` loses to a
#   live `cargo check`), so the script goes on as after a success. Then `git worktree
#   prune`, then a check that the path is gone (exit 7 when it is not: the contents are
#   listed, never deleted; exit 0 when it is, even if git exited non-zero). Git deletes the
#   worktree's ignored files with it (`target/`, `.superpowers/`, `.codescout/`); copy out
#   what you keep first. The branch is not deleted.
#
# HOW A PROCESS IS MATCHED
#   Holder   its cwd is the worktree or below it, OR an element of its command line
#            holds the worktree path as a whole path component: preceded by the start of
#            the element or by a character that is not part of a path, followed by `/` or
#            the end of the element. `/p/wt`, `/p/wt/Cargo.toml`, `--cwd=/p/wt` match;
#            `/p/wt-other`, `/p/wt2/x`, `/x/p/wt` do not.
#   Kind     the basename of argv[0] from /proc/<pid>/cmdline, never `comm` (which is
#            cut at 15 characters and settable by the process). `codescout` is `mux`
#            only when argv[1] is exactly `mux`; any other codescout (an MCP server) is
#            `other`, wherever its binary lives. `rust-analyzer*`, `cargo`, `rustc` and
#            `sccache` are themselves. Any other argv[0] under `<worktree>/target/` is
#            `build-output`. An `other` row becomes `child` when an ancestor of it is a holder of
#            one of those kinds (the kinds are the ones by argv[0], never a `child` result). Two
#            things never become `child` and never count as that ancestor: this script's own
#            ancestry (its parent and the parent's ancestors, found by the same walk, which is
#            also what makes an ancestor holder a blocker), and a
#            `codescout` whatever its arguments (it is never promoted). The walk reads
#            `PPid:` from `<proc>/<pid>/status`, also through processes that are not holders,
#            and ends at pid 1 or 0, at a repeated pid, at a missing or unreadable `status`,
#            at a `PPid:` that is not a number, and after 64 steps. Then the row stays `other`.
#
# WHAT THIS DOES NOT AND CANNOT DO
#   * It cannot see peers. A listed mux may have a live client in another session; a
#     green run says nothing about that.
#   * It sees only processes whose /proc entry this user can read (cwd and cmdline).
#     A holder owned by another user is invisible, so no row is not proof of no holder.
#   * A process that holds the worktree only through an open file descriptor, with a cwd
#     elsewhere and no path in its command line, is invisible.
#   * The path is matched in its canonical spelling. A command line that spells the
#     worktree through a symlink is not matched.
#   * It needs Linux /proc, like scripts/stale-servers.sh; without it the script refuses.
#   * A long-lived process that a stoppable holder started (for example `cargo run`) is a
#     `child` and is signalled with the holder: the script cannot tell a compiler from a
#     program the build started.
#   * A descendant that was re-parented to the nearest subreaper or to pid 1 after its
#     ancestor exited has no stoppable ancestor in the rescan, so it is `other`: it is never
#     signalled for that reason, and if it survives, the run ends in exit 5.
#   * The script's own ancestry is known only as far as `PPid:` can be read (`<proc>/<pid>/
#     status`, the walk of HOW A PROCESS IS MATCHED). An ancestor above a process whose
#     `status` is unreadable is not seen, so a holder there is not marked as an ancestor.
#   * Between the last scan and `git worktree remove`, a new process can start in the
#     worktree; that may show up as exit 6 or 7, or not at all (a late process that writes
#     nothing lets git succeed, and exit 0 then leaves a process whose cwd is deleted). It
#     is never signalled.
#
# TEST SEAM
#   WORKTREE_REMOVE_SCAN_TIMEOUT (whole seconds, default 10) bounds each scan.
#   WORKTREE_REMOVE_PROC (default /proc) points the scan at a fixture tree:
#   `<dir>/<pid>/cwd` (a symlink) and `<dir>/<pid>/cmdline` (NUL-separated arguments), and
#   optionally `<dir>/<pid>/status` (a `PPid:` line, for the `child` kind), so
#   tests/worktree-remove.sh needs no real process.

set -uo pipefail
export LC_ALL=C
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE

PROC="${WORKTREE_REMOVE_PROC:-/proc}"
SCAN_TIMEOUT="${WORKTREE_REMOVE_SCAN_TIMEOUT:-10}"
SELF=$(realpath -- "${BASH_SOURCE[0]}")
STOP=0
WAIT_SECS=30

die() {
    echo "worktree-remove: $*" >&2
    exit 2
}

usage() {
    echo "usage: worktree-remove.sh [--stop] [--wait SECS] <worktree path>   (-h, --help: this text, exit 0)" >&2
}

# resolve_worktree PATH: print the canonical path of a registered linked worktree, or
# print the reason on stderr and return 2. The three refusals the design names: the main
# checkout, an unregistered path, and a path that contains the caller's current directory.
resolve_worktree() {
    local arg=${1:-} canon porcelain line p here first=1 is_main=0 found=0
    if [ -z "$arg" ]; then
        echo "worktree-remove: no worktree path given" >&2
        return 2
    fi
    canon=$(realpath -m -- "$arg" 2>/dev/null) || {
        echo "worktree-remove: cannot resolve [$arg]" >&2
        return 2
    }
    # The repository is the one the path belongs to; only a path that no longer exists
    # falls back to the repository of the caller's current directory.
    if [ -d "$canon" ]; then
        porcelain=$(git -C "$canon" worktree list --porcelain 2>/dev/null)
    else
        porcelain=$(git worktree list --porcelain 2>/dev/null)
    fi
    while IFS= read -r line; do
        case $line in
            "worktree "*)
                p=$(realpath -m -- "${line#worktree }" 2>/dev/null) || continue
                if [ "$p" = "$canon" ]; then
                    found=1
                    is_main=$first
                fi
                first=0
                ;;
        esac
    done <<<"$porcelain"
    if [ "$found" -ne 1 ]; then
        echo "worktree-remove: $canon is not a registered worktree of any git repository" >&2
        return 2
    fi
    if [ "$is_main" -eq 1 ]; then
        echo "worktree-remove: $canon is the main checkout, not a linked worktree" >&2
        return 2
    fi
    here=$(pwd -P)
    if [ "$here" = "$canon" ] || [[ $here == "$canon"/* ]]; then
        echo "worktree-remove: the current directory ($here) is inside $canon; run this from elsewhere" >&2
        return 2
    fi
    printf '%s\n' "$canon"
}

# mentions_path TEXT PATH: succeed when TEXT holds PATH as a whole path component.
mentions_path() {
    local text=$1 p=$2 head pre pos end pc nc i=0 len=${#2} n=${#1}
    while :; do
        head=${text:i}
        [[ $head == *"$p"* ]] || return 1
        pre=${head%%"$p"*}
        pos=$((i + ${#pre}))
        end=$((pos + len))
        pc=""
        nc=""
        [ "$pos" -gt 0 ] && pc=${text:pos-1:1}
        [ "$end" -lt "$n" ] && nc=${text:end:1}
        if { [ -z "$pc" ] || [[ $pc != [[:alnum:]._/+~%@-] ]]; } && { [ -z "$nc" ] || [ "$nc" = / ]; }; then
            return 0
        fi
        i=$((pos + 1))
    done
}

# kind_of_argv ARGV0 ARGV1 PATH: print the kind of a process from its command line.
kind_of_argv() {
    local argv0=${1:-} argv1=${2:-} wt=$3 base
    base=${argv0##*/}
    case $base in
        codescout)
            if [ "$argv1" = mux ]; then echo mux; else echo other; fi
            return 0
            ;;
        rust-analyzer*) echo rust-analyzer; return 0 ;;
        cargo) echo cargo; return 0 ;;
        rustc) echo rustc; return 0 ;;
        sccache) echo sccache; return 0 ;;
    esac
    case $argv0 in
        "$wt"/target/*) echo build-output; return 0 ;;
    esac
    echo other
}

# kind_rank KIND: the order rows are printed in, mux first and blockers last.
kind_rank() {
    case $1 in
        mux) echo 1 ;;
        rust-analyzer) echo 2 ;;
        cargo) echo 3 ;;
        rustc) echo 4 ;;
        sccache) echo 5 ;;
        build-output) echo 6 ;;
        child) echo 7 ;;
        *) echo 8 ;;
    esac
}

# ppid_of PID: sets UP to the parent pid in <proc>/PID/status, one `sed`. Returns 1 when there is
# no usable parent: no readable status, a `PPid:` that is not a number or has 11 or more digits,
# or pid 0 or 1. (`sed`, not `awk`: a `$` followed by a digit is rewritten by some harnesses.)
ppid_of() {
    UP=$(sed -n 's/^PPid:[[:space:]]*//p' "$PROC/$1/status" 2>/dev/null)
    case $UP in '' | *[!0-9]* | ???????????*) return 1 ;; esac
    UP=$((10#$UP))
    [ "$UP" -gt 1 ]
}

# ancestry_of START sets ANCESTRY to a space-padded list of pids: START, then each ancestor found
# through ppid_of (" 4100 4090 "). The walk follows `PPid:` upward and ends at pid 1 or 0, at a
# repeated pid, at a missing or unreadable `status`, at a `PPid:` that is not a number or has
# 11 or more digits, and after 64 steps. An empty or non-numeric START gives " ". It returns 0.
ancestry_of() {
    local cur=$1 steps=0 seen
    case $cur in '' | *[!0-9]*)
        ANCESTRY=" "
        return 0
        ;;
    esac
    seen=" $cur "
    while [ "$steps" -lt 64 ] && ppid_of "$cur"; do
        steps=$((steps + 1))
        case $seen in *" $UP "*) break ;; esac
        seen+="$UP "
        cur=$UP
    done
    ANCESTRY=$seen
    return 0
}

# list_holders PATH: print one `PID<TAB>KIND<TAB>CMDLINE` row per holder of the worktree.
# An entry whose cmdline cannot be read, or that has neither a cwd nor a path in its
# command line, is skipped. The script's own pid and its subshell are never holders. Its
# parent is exempt from the command-line test only (its command line names the worktree
# because it launched this script), never from the cwd test: a launcher that sits inside
# the worktree and starts the script from elsewhere holds the worktree like any other.
list_holders() {
    local wt=$1 d pid cwd a hit kind cmd argv_exempt rec parent=$PPID
    local self=" $$ $BASHPID ${WR_SCAN_SELF-} "
    local -a argv rows=() hpids=() child_pids=()
    local -A cwds=() hkind=() hcmd=() is_codescout=()
    local cur steps seen anc_done=0
    # Run as a bounded scan (scan_holders), the process above this one is `timeout`, not a
    # holder, and the parent that matters is the main script's, passed in WR_SCAN_PARENT.
    if [ -n "${WR_SCAN_PARENT-}" ]; then
        self+="$PPID "
        parent=$WR_SCAN_PARENT
    fi
    # On the real /proc one `find` reads every cwd link at once: a `readlink` per process is a
    # fork per process, seconds on a busy machine, and the stop loop rescans. The kernel
    # reports a canonical path, which is what `readlink -f` yields. A process that has no entry
    # (another user's, or started since) has no readable cwd, as before. A fixture tree
    # (WORKTREE_REMOVE_PROC) may hold non-canonical symlinks, so it keeps `readlink -f`.
    if [ "$PROC" = /proc ]; then
        while IFS= read -r -d '' rec; do
            cwds["${rec%%$'\t'*}"]=${rec#*$'\t'}
        done < <(find "$PROC" -mindepth 2 -maxdepth 2 -name cwd -printf '%h\t%l\0' 2>/dev/null)
    fi
    for d in "$PROC"/[0-9]*; do
        [ -d "$d" ] || continue
        pid=${d##*/}
        case $pid in *[!0-9]*) continue ;; esac
        case $self in *" $pid "*) continue ;; esac
        argv_exempt=0
        [ "$pid" = "$parent" ] && argv_exempt=1
        [ -r "$d/cmdline" ] || continue
        if [ "$PROC" = /proc ]; then
            cwd=${cwds["$d"]-}
        else
            cwd=$(readlink -f -- "$d/cwd" 2>/dev/null) || cwd=""
        fi
        argv=()
        while IFS= read -r -d '' a || [ -n "$a" ]; do
            argv+=("$a")
        done 2>/dev/null <"$d/cmdline"
        hit=0
        if [ -n "$cwd" ] && { [ "$cwd" = "$wt" ] || [[ $cwd == "$wt"/* ]]; }; then
            hit=1
        elif [ "$argv_exempt" -eq 0 ]; then
            for a in "${argv[@]+"${argv[@]}"}"; do
                if mentions_path "$a" "$wt"; then
                    hit=1
                    break
                fi
            done
        fi
        [ "$hit" -eq 1 ] || continue
        if [ "${#argv[@]}" -gt 0 ]; then
            kind=$(kind_of_argv "${argv[0]}" "${argv[1]-}" "$wt")
            cmd="${argv[*]}"
            [ "${argv[0]##*/}" = codescout ] && is_codescout[$pid]=1
        else
            kind=other
            cmd="(empty command line)"
        fi
        # Every control character (tab, newline, CR, ESC, ...) becomes a space: the row is
        # one line of a table an operator decides on, and a command line must not be able to
        # move the cursor, recolour or overwrite it.
        cmd=${cmd//[[:cntrl:]]/ }
        hpids+=("$pid")
        hkind[$pid]=$kind
        hcmd[$pid]=$cmd
    done
    # A row of kind `other` whose ancestor is a holder of a stoppable kind is a `child`: a
    # cold build's C compiler chain (make, cc, cc1, as, sh configure) runs under a cargo that
    # is itself a holder, and ends with it. The test uses the kinds of the first pass (by
    # argv[0]), so a `child` never makes another row a child. Two things are never made a child:
    # a `codescout` whatever its arguments (one that is not a mux is a peer session's MCP
    # server), and any ancestor of this script (its parent and the parent's ancestors: they
    # are what runs the script), which also never counts as the stoppable ancestor. The walk
    # follows `PPid:` upward, one `sed` per step (ancestry_of, ppid_of), over processes that are
    # not holders too, and ends at pid 1 or 0, at a repeated pid, at a missing or unreadable
    # `status`, at a `PPid:` that is not a number, and after 64 steps: then the row stays `other`.
    for pid in "${hpids[@]+"${hpids[@]}"}"; do
        [ "${hkind[$pid]}" = other ] || continue
        [ -z "${is_codescout[$pid]-}" ] || continue
        if [ "$anc_done" -eq 0 ]; then
            ancestry_of "$parent"
            anc_done=1
        fi
        case $ANCESTRY in *" $pid "*) continue ;; esac
        cur=$pid
        seen=" $pid "
        steps=0
        while [ "$steps" -lt 64 ] && ppid_of "$cur"; do
            steps=$((steps + 1))
            case $seen in *" $UP "*) break ;; esac
            seen+="$UP "
            case $ANCESTRY in *" $UP "*) break ;; esac
            if [ -n "${hkind[$UP]-}" ] && [ "${hkind[$UP]}" != other ]; then
                child_pids+=("$pid")
                break
            fi
            cur=$UP
        done
    done
    for pid in "${child_pids[@]+"${child_pids[@]}"}"; do
        hkind[$pid]=child
    done
    for pid in "${hpids[@]+"${hpids[@]}"}"; do
        rows+=("$(kind_rank "${hkind[$pid]}")"$'\t'"$pid"$'\t'"${hkind[$pid]}"$'\t'"${hcmd[$pid]}")
    done
    if [ "${#rows[@]}" -gt 0 ]; then
        printf '%s\n' "${rows[@]}" | sort -t$'\t' -k1,1n -k2,2n | cut -f2-
    fi
}

# main_checkout_of PATH: print the main checkout of the repository PATH belongs to, the first
# entry of `git worktree list --porcelain`. Git commands that remove a worktree run from
# there, never from inside the worktree being removed.
main_checkout_of() {
    local wt=$1 porcelain
    if [ -d "$wt" ]; then
        porcelain=$(git -C "$wt" worktree list --porcelain 2>/dev/null)
    else
        porcelain=$(git worktree list --porcelain 2>/dev/null)
    fi
    porcelain=${porcelain%%$'\n'*}
    [ "${porcelain#worktree }" != "$porcelain" ] || return 1
    printf '%s\n' "${porcelain#worktree }"
}

# set_now_ms: set NOW_MS to the current time in milliseconds.
set_now_ms() {
    local t=${EPOCHREALTIME/./}
    NOW_MS=$((t / 1000))
}

# scan_holders PATH: run list_holders in a child under `timeout`, so a scan that blocks (a read of
# a /proc/<pid>/cmdline waits on the target's mmap lock) cannot hang this script. Sets HOLDERS to
# the rows and returns 0; on a timeout or any failure sets HOLDERS to "" and returns 1, which
# means the holders are UNKNOWN, never "none". The child is this script run in its internal scan
# mode; it is told which pids are this script and its parent so it does not list them.
scan_holders() {
    local rc
    HOLDERS=$(WR_SCAN_SELF="$$ $BASHPID" WR_SCAN_PARENT="$PPID" \
        timeout -k 1 "$SCAN_TIMEOUT" bash "$SELF" --internal-scan "$1")
    rc=$?
    [ "$rc" -eq 0 ] && return 0
    HOLDERS=""
    return 1
}

# scan_failed: the message for a scan that timed out or failed once signals may have been sent.
scan_failed() {
    echo "worktree-remove: the holders could not be established: a process scan timed out or failed (limit ${SCAN_TIMEOUT}s). Nothing was removed; SIGTERM may already have been sent." >&2
}

# blockers_of ROWS: print one line per blocker among ROWS (`PID<TAB>KIND<TAB>CMDLINE`): a holder
# of kind `other`, this script's parent whatever its kind, and any other holder that is an
# ancestor of this script whatever its kind (the kind comes from kind_of_argv like any row; an
# ancestor is a blocker because its pid is in ANCESTRY, which main sets). Print nothing when
# there is none.
blockers_of() {
    local pid kind cmd
    while IFS=$'\t' read -r pid kind cmd; do
        if [ "$kind" = other ]; then
            printf '  %s  %s\n' "$pid" "$cmd"
        elif [ "$pid" = "$PPID" ]; then
            printf '  %s  %s  (the parent of this script)\n' "$pid" "$cmd"
        else
            case $ANCESTRY in
                *" $pid "*) printf '  %s  %s  (an ancestor of this script)\n' "$pid" "$cmd" ;;
            esac
        fi
    done <<<"$1"
}

# refuse_blockers BLOCKERS: the exit-4 message; nothing was signalled.
refuse_blockers() {
    {
        echo "worktree-remove: --stop refused; these holders are not stoppable (nothing was signalled):"
        printf '%s\n' "$1"
    } >&2
}

# send_term PID KIND: SIGTERM, never to this script, its subshell, its parent or any other
# ancestor of this script (ANCESTRY, set by main). It prints one
# stderr line for the signal: `signalled <pid> <kind> with SIGTERM`, or `could not signal <pid>
# <kind> (already gone, or not permitted)`. A failed signal is not an error; the rescan decides
# whether the holder is still there.
send_term() {
    local pid=$1 kind=$2
    case $pid in "$$" | "$BASHPID" | "$PPID") return 0 ;; esac
    case $ANCESTRY in *" $pid "*) return 0 ;; esac
    if kill -TERM "$pid" 2>/dev/null; then
        echo "worktree-remove: signalled $pid $kind with SIGTERM" >&2
    else
        echo "worktree-remove: could not signal $pid $kind (already gone, or not permitted)" >&2
    fi
    return 0
}

# stop_holders PATH WAIT: SIGTERM the stoppable holders of PATH, the mux first, and rescan until
# none is left. Return 0 when none remains, 5 when something does (the survivors are printed
# on stderr) or when a scan fails, 4 when the first scan holds a blocker (nothing signalled).
# Liveness is the rescan through scan_holders, never `kill -0`.
#   * The first scan of the loop may show a blocker that appeared after `main`'s scan: an
#     `other` row, or an ancestor of this script. Then nothing is signalled and the return is 4,
#     as `main`'s own refusal. A blocker that appears in a later round is a survivor.
#   * A pid is signalled once. A holder that appears in a later round (a dying `cargo` can
#     start children) is signalled in that round; at most three rounds signal anything.
#   * A holder of kind `other` is never signalled; one that appears later is a survivor.
#   * A pid is signalled only if the most recent scan lists it as a stoppable holder: the
#     others are checked against the scan taken after the mux pause, because a pid can be
#     recycled inside that pause.
#   * The wait is WAIT seconds in all, polled about every 0.5 s.
stop_holders() {
    local wt=$1 wait_s=$2 rows pid kind cmd round=0 signalled=" " deadline i found fresh blockers
    local -a mux_pids other_pids
    local -A kind_of=()
    set_now_ms
    deadline=$((NOW_MS + wait_s * 1000))
    while :; do
        scan_holders "$wt" || { scan_failed; return 5; }
        rows=$HOLDERS
        [ -n "$rows" ] || return 0
        if [ "$round" -eq 0 ]; then
            blockers=$(blockers_of "$rows")
            if [ -n "$blockers" ]; then
                refuse_blockers "$blockers"
                return 4
            fi
        fi
        mux_pids=()
        other_pids=()
        kind_of=()
        while IFS=$'\t' read -r pid kind cmd; do
            case $signalled in *" $pid "*) continue ;; esac
            case $pid in "$$" | "$BASHPID" | "$PPID") continue ;; esac
            case $ANCESTRY in *" $pid "*) continue ;; esac
            kind_of[$pid]=$kind
            case $kind in
                other) ;;
                mux) mux_pids+=("$pid") ;;
                *) other_pids+=("$pid") ;;
            esac
        done <<<"$rows"
        if [ $((${#mux_pids[@]} + ${#other_pids[@]})) -gt 0 ] && [ "$round" -lt 3 ]; then
            round=$((round + 1))
            for pid in "${mux_pids[@]+"${mux_pids[@]}"}"; do
                signalled+="$pid "
                send_term "$pid" "${kind_of[$pid]}"
            done
            if [ "${#mux_pids[@]}" -gt 0 ]; then
                # Give a graceful mux exit time to take its language server with it: up to ten
                # rounds of a 0.1 s sleep plus one scan. A scan on a busy real /proc takes about
                # 0.4 s, so this can last about 5 s, and it is not counted in the --wait deadline.
                for i in 1 2 3 4 5 6 7 8 9 10; do
                    sleep 0.1
                    scan_holders "$wt" || { scan_failed; return 5; }
                    rows=$HOLDERS
                    found=0
                    for pid in "${mux_pids[@]}"; do
                        case $'\n'"$rows" in *$'\n'"$pid"$'\t'*) found=1 ;; esac
                    done
                    [ "$found" -eq 1 ] || break
                done
            fi
            # `rows` is the latest scan. Signal only what it still lists as stoppable.
            fresh=" "
            while IFS=$'\t' read -r pid kind cmd; do
                [ -n "$pid" ] && [ "$kind" != other ] || continue
                fresh+="$pid "
                kind_of[$pid]=$kind
            done <<<"$rows"
            for pid in "${other_pids[@]+"${other_pids[@]}"}"; do
                case $fresh in
                    *" $pid "*)
                        signalled+="$pid "
                        send_term "$pid" "${kind_of[$pid]}"
                        ;;
                esac
            done
        fi
        set_now_ms
        [ "$NOW_MS" -lt "$deadline" ] || break
        sleep 0.5
    done
    # The last scan may predate the last signals: look once more before calling anything a survivor.
    scan_holders "$wt" || { scan_failed; return 5; }
    rows=$HOLDERS
    [ -n "$rows" ] || return 0
    {
        echo "worktree-remove: these holders are still there after SIGTERM and ${wait_s}s (nothing was removed; SIGKILL is never used):"
        printf '%s\n' "$rows"
    } >&2
    return 5
}

# worktree_listed ROOT PATH: succeed when `git worktree list` of ROOT still names PATH (whole-path
# match on the `worktree <path>` lines, each canonicalised as resolve_worktree does). A failing
# `git worktree list` counts as listed: when git cannot say, assume the registration is still
# there.
worktree_listed() {
    local root=$1 wt=$2 porcelain line p
    porcelain=$(git -C "$root" worktree list --porcelain 2>/dev/null) || return 0
    while IFS= read -r line; do
        case $line in
            "worktree "*)
                p=$(realpath -m -- "${line#worktree }" 2>/dev/null) || continue
                [ "$p" = "$wt" ] && return 0
                ;;
        esac
    done <<<"$porcelain"
    return 1
}

# remove_worktree ROOT PATH: `git worktree remove` without --force, `git worktree prune`, then check
# that PATH is gone. Return 0 when it is, 6 when git refuses and still lists the worktree, 7 when
# the directory is still there. It never deletes anything itself.
#
# When `git worktree remove` fails, the bug this script exists for looks like this: git deletes
# the work tree, `.git` file included, then cannot `rmdir target/` (a live `cargo check` rewrites
# it) and exits non-zero after it deregistered the worktree. So a failure is told apart by
# `git worktree list`: still listed is a refusal (exit 6); no longer listed is the removal having
# happened, and the path is verified like after a success.
remove_worktree() {
    local root=$1 wt=$2 out failed=0 st n
    if ! out=$(git -C "$root" worktree remove "$wt" 2>&1); then
        if worktree_listed "$root" "$wt"; then
            {
                echo "worktree-remove: git refused to remove $wt; nothing was forced:"
                printf '%s\n' "$out" | sed 's/^/  /'
                # Ignored files never block `git worktree remove`: they are not listed.
                if [ -e "$wt/.git" ]; then
                    echo "  git -C $wt status --porcelain -uall:"
                    st=$(git -C "$wt" status --porcelain -uall 2>&1)
                    if [ -z "$st" ]; then
                        echo "    (nothing listed)"
                    else
                        n=$(printf '%s\n' "$st" | wc -l)
                        printf '%s\n' "$st" | head -n 50 | sed 's/^/    /'
                        [ "$n" -le 50 ] || echo "    ... and $((n - 50)) more"
                    fi
                else
                    echo "  ($wt/.git does not exist: git status skipped)"
                fi
            } >&2
            return 6
        fi
        failed=1
        {
            echo "worktree-remove: git exited non-zero, but it no longer lists $wt (it deleted the work tree and then failed, often on a directory a live process rewrote). Its message:"
            printf '%s\n' "$out" | sed 's/^/  /'
        } >&2
    fi
    git -C "$root" worktree prune >/dev/null || true
    if [ -e "$wt" ] || [ -L "$wt" ]; then
        {
            echo "worktree-remove: git removed the worktree but $wt is still there (something wrote to it afterwards). Its contents, which this script does not delete:"
            ls -A -- "$wt" 2>&1 | sed 's/^/  /'
        } >&2
        return 7
    fi
    if [ "$failed" -eq 1 ]; then
        echo "worktree-remove: removed $wt (git reported a failure after removing it; the path is gone and unregistered)" >&2
    else
        echo "worktree-remove: removed $wt" >&2
    fi
    return 0
}

main() {
    local path="" wt root holders count blockers rc
    # Internal: a bounded scan, run by scan_holders as `timeout ... bash <this file> --internal-scan PATH`.
    if [ "${1:-}" = --internal-scan ]; then
        list_holders "${2:?}"
        exit $?
    fi
    while [ $# -gt 0 ]; do
        case $1 in
            --stop) STOP=1 ;;
            --wait)
                [ $# -ge 2 ] || { usage; die "--wait needs a number of seconds"; }
                WAIT_SECS=$2
                shift
                ;;
            --wait=*) WAIT_SECS=${1#--wait=} ;;
            -h | --help) usage; exit 0 ;;
            --)
                shift
                break
                ;;
            -*) usage; die "unknown option: $1" ;;
            *)
                [ -z "$path" ] || { usage; die "only one worktree path may be given"; }
                path=$1
                ;;
        esac
        shift
    done
    if [ $# -gt 0 ]; then
        [ -z "$path" ] && [ $# -eq 1 ] || { usage; die "only one worktree path may be given"; }
        path=$1
    fi
    [[ $WAIT_SECS =~ ^(0|[1-9][0-9]*)$ ]] || { usage; die "--wait needs a whole number of seconds without leading zeros (bash would read 08 as octal), got [$WAIT_SECS]"; }
    [ -n "$path" ] || { usage; die "no worktree path given"; }
    [ -d "$PROC" ] || die "cannot scan processes: [$PROC] is not a directory; this script needs Linux /proc"
    [[ $SCAN_TIMEOUT =~ ^[1-9][0-9]*$ ]] || die "WORKTREE_REMOVE_SCAN_TIMEOUT needs a whole number of seconds above 0, got [$SCAN_TIMEOUT]"
    # The ancestry of this script, computed once here: blockers_of, send_term and stop_holders
    # read it, and an ANCESTRY in the inherited environment must not stand in for it.
    ancestry_of "$PPID"

    wt=$(resolve_worktree "$path") || exit 2
    root=$(main_checkout_of "$wt") || die "cannot find the main checkout of $wt"

    scan_holders "$wt" || die "the process scan timed out or failed (limit ${SCAN_TIMEOUT}s); nothing was changed"
    holders=$HOLDERS
    if [ -z "$holders" ]; then
        echo "no holders under $wt" >&2
        remove_worktree "$root" "$wt"
        exit $?
    fi

    printf '%s\n' "$holders"
    count=$(printf '%s\n' "$holders" | wc -l)
    {
        echo "worktree-remove: $count process(es) hold $wt."
        echo "  Each may have an occupant: a codescout mux is shared by every session working in this"
        echo "  checkout, and stopping it ends their language server. Ask the occupant before"
        echo "  stopping anything."
    } >&2

    blockers=$(blockers_of "$holders")

    if [ "$STOP" -eq 1 ]; then
        if [ -n "$blockers" ]; then
            refuse_blockers "$blockers"
            exit 4
        fi
        stop_holders "$wt" "$WAIT_SECS"
        rc=$?
        [ "$rc" -eq 0 ] || exit "$rc"
        remove_worktree "$root" "$wt"
        exit $?
    fi
    exit 3
}

# Sourced, the file only defines its functions.
if [ "${BASH_SOURCE[0]}" = "$0" ]; then
    main "$@"
fi
